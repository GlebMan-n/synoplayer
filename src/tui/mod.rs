pub mod app;
pub mod handler;
pub mod ui;

use std::io;
use std::time::{Duration, Instant};

use crossterm::ExecutableCommand;
use crossterm::event::{self, Event};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::api::client::SynoClient;
use crate::api::folder::FolderApi;
use crate::api::pin::PinApi;
use crate::api::playlist::PlaylistApi;
use crate::api::song::SongApi;
use crate::cache::manager::CacheManager;
use crate::config::model::AppConfig;
use crate::ipc;
use crate::player::engine::AudioEngine;

use app::{App, StatefulList};
use handler::TuiContext;

/// Run the interactive TUI player.
pub async fn run(client: SynoClient, config: AppConfig) -> anyhow::Result<()> {
    let cache = CacheManager::new(config.cache.clone());
    let engine = AudioEngine::new()
        .with_device(&config.player.output_device);

    let mut app = App::new();
    app.volume = config.player.default_volume;
    app.status = "Loading library...".to_string();

    // Load data upfront
    load_data(&client, &mut app).await?;

    let ctx = TuiContext {
        client: &client,
        engine: &engine,
        cache: &cache,
        cache_config: &config.cache,
    };

    // Start IPC server (non-fatal if it fails)
    let ipc_state = ipc::server::try_start();
    let mut ipc_rx = ipc_state.as_ref().map(|(_, _)| ()).and(None);
    let _ipc_guard: Option<ipc::SocketGuard>;
    if let Some((rx, guard)) = ipc_state {
        ipc_rx = Some(rx);
        _ipc_guard = Some(guard);
    } else {
        _ipc_guard = None;
    }

    // Setup terminal
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Panic hook to restore terminal
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic| {
        let _ = terminal::disable_raw_mode();
        let _ = io::stdout().execute(LeaveAlternateScreen);
        original_hook(panic);
    }));

    // Event loop
    let tick_rate = Duration::from_millis(200);
    let mut last_tick = Instant::now();

    while app.running {
        terminal.draw(|f| ui::render(f, &mut app))?;

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) => {
                    // Ignore release events (crossterm sends both press and release on some terminals)
                    if key.kind == crossterm::event::KeyEventKind::Press {
                        handler::handle_key(&mut app, key, &ctx).await;
                    }
                }
                Event::Resize(_, _) => {} // redraw on next loop iteration
                _ => {}
            }
        }

        // Process IPC commands (non-blocking)
        if let Some(ref mut rx) = ipc_rx {
            while let Ok(cmd) = rx.try_recv() {
                let response = handler::handle_ipc(&mut app, &ctx, cmd.request).await;
                let _ = cmd.reply.send(response);
            }
        }

        if last_tick.elapsed() >= tick_rate {
            match app.tick(&engine) {
                Ok(true) => {
                    // Track finished — advance queue
                    match handler::advance_queue(&mut app, &ctx).await {
                        Ok(()) => {}
                        Err(e) => {
                            app.now_playing = None;
                            app.status = format!("Error: {e}");
                        }
                    }
                }
                Err(msg) => {
                    engine.stop();
                    app.now_playing = None;
                    app.status = format!("Player error: {msg}");
                }
                Ok(false) => {} // still playing
            }
            last_tick = Instant::now();
        }
    }

    // Restore terminal
    terminal::disable_raw_mode()?;
    io::stdout().execute(LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    // Remove panic hook
    let _ = std::panic::take_hook();

    Ok(())
}

async fn load_data(client: &SynoClient, app: &mut App) -> anyhow::Result<()> {
    // Load favorites (pinned songs only)
    let pin_api = PinApi::new(client);
    let song_api = SongApi::new(client);
    let pin_data = pin_api.list().await?;
    let mut fav_songs = Vec::new();
    for item in &pin_data.items {
        if item.item_type == "song" {
            if let Ok(song) = song_api.get_info(&item.id).await {
                fav_songs.push(song);
            }
        }
    }
    app.favorites = StatefulList::with_items(fav_songs);

    // Load playlists (both personal and shared)
    let playlist_api = PlaylistApi::new(client);
    let mut all_playlists = Vec::new();
    for lib in &["personal", "shared"] {
        let data = playlist_api.list(0, 200, Some(lib)).await?;
        all_playlists.extend(data.playlists);
    }
    app.playlists = StatefulList::with_items(all_playlists);

    // Load root folders
    let folder_api = FolderApi::new(client);
    let folder_data = folder_api.list(None, 0, 500).await?;
    app.folders = StatefulList::with_items(folder_data.items);

    app.status = format!(
        "Loaded {} favorites, {} playlists, {} folders",
        app.favorites.items.len(),
        app.playlists.items.len(),
        app.folders.items.len()
    );
    Ok(())
}
