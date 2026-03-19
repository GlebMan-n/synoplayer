use crossterm::event::{KeyCode, KeyEvent};

use crate::api::client::SynoClient;
use crate::api::playlist::PlaylistApi;
use crate::api::stream::StreamApi;
use crate::api::types::Song;
use crate::cache::manager::CacheManager;
use crate::config::model::CacheConfig;
use crate::playback;
use crate::player::engine::AudioEngine;

use super::app::{App, PlaylistDetail, StatefulList, Tab};

/// Resources needed by the key handler to perform async operations.
pub struct TuiContext<'a> {
    pub client: &'a SynoClient,
    pub engine: &'a AudioEngine,
    pub cache: &'a CacheManager,
    pub cache_config: &'a CacheConfig,
}

pub async fn handle_key(app: &mut App, key: KeyEvent, ctx: &TuiContext<'_>) {
    match key.code {
        // --- Global ---
        KeyCode::Char('q') => {
            app.stop_playback(ctx.engine);
            app.running = false;
        }
        KeyCode::Tab => app.next_tab(),
        KeyCode::BackTab => app.prev_tab(),
        KeyCode::Char(' ') => app.stop_playback(ctx.engine),

        // --- Navigation ---
        KeyCode::Down | KeyCode::Char('j') => app.active_list_next(),
        KeyCode::Up | KeyCode::Char('k') => app.active_list_previous(),
        KeyCode::PageDown => app.active_list_page_down(10),
        KeyCode::PageUp => app.active_list_page_up(10),

        // --- Actions ---
        KeyCode::Enter => {
            if let Err(e) = handle_enter(app, ctx).await {
                app.status = format!("Error: {e}");
            }
        }
        KeyCode::Esc => handle_escape(app),

        // --- Playback ---
        KeyCode::Char('n') => {
            if let Err(e) = advance_queue(app, ctx).await {
                app.status = format!("Error: {e}");
            }
        }
        KeyCode::Char('p') => {
            if let Err(e) = rewind_queue(app, ctx).await {
                app.status = format!("Error: {e}");
            }
        }

        // --- Volume (visual) ---
        KeyCode::Char('+') | KeyCode::Char('=') => {
            app.volume = (app.volume + 5).min(100);
            app.status = format!("Volume: {}%", app.volume);
        }
        KeyCode::Char('-') => {
            app.volume = app.volume.saturating_sub(5);
            app.status = format!("Volume: {}%", app.volume);
        }

        _ => {}
    }
}

async fn handle_enter(app: &mut App, ctx: &TuiContext<'_>) -> anyhow::Result<()> {
    match app.active_tab {
        Tab::Library => {
            if let Some(idx) = app.songs.selected() {
                let queue: Vec<Song> = app.songs.items.clone();
                play_from_queue(app, ctx, queue, idx).await?;
            }
        }
        Tab::Playlists => {
            if app.playlist_detail.is_some() {
                // Play selected song from playlist detail
                let (queue, idx) = {
                    let detail = app.playlist_detail.as_ref().unwrap();
                    let idx = detail.songs.selected().unwrap_or(0);
                    (detail.songs.items.clone(), idx)
                };
                play_from_queue(app, ctx, queue, idx).await?;
            } else if let Some(pl) = app.playlists.selected_item() {
                // Open playlist detail
                let id = pl.id.clone();
                let name = pl.name.clone();
                open_playlist(app, ctx, &id, &name).await?;
            }
        }
        Tab::Queue => {}
    }
    Ok(())
}

fn handle_escape(app: &mut App) {
    if app.active_tab == Tab::Playlists && app.playlist_detail.is_some() {
        app.playlist_detail = None;
    }
}

async fn open_playlist(
    app: &mut App,
    ctx: &TuiContext<'_>,
    id: &str,
    name: &str,
) -> anyhow::Result<()> {
    app.status = format!("Loading playlist '{name}'...");
    let api = PlaylistApi::new(ctx.client);
    let data = api.get_info(id).await?;
    match data.into_playlist() {
        Some(detail) => {
            let songs = detail.all_songs().to_vec();
            let count = songs.len();
            app.playlist_detail = Some(PlaylistDetail {
                name: name.to_string(),
                songs: StatefulList::with_items(songs),
            });
            app.status = format!("Playlist '{name}' — {count} songs");
        }
        None => {
            app.status = format!("Failed to load playlist '{name}'.");
        }
    }
    Ok(())
}

async fn play_from_queue(
    app: &mut App,
    ctx: &TuiContext<'_>,
    queue: Vec<Song>,
    index: usize,
) -> anyhow::Result<()> {
    if queue.is_empty() || index >= queue.len() {
        return Ok(());
    }
    app.queue = queue;
    play_queue_index(app, ctx, index).await
}

async fn play_queue_index(app: &mut App, ctx: &TuiContext<'_>, index: usize) -> anyhow::Result<()> {
    let song = match app.queue.get(index) {
        Some(s) => s.clone(),
        None => return Ok(()),
    };

    let track = playback::track_from_song(&song);
    app.status = format!("Loading: {} - {}...", track.artist, track.title);

    // Resolve audio source (cache or stream)
    let url = playback::resolve_audio_source(ctx.client, &song, ctx.cache, ctx.cache_config)
        .await
        .or_else(|_| {
            // Fallback to direct stream URL
            let stream_api = StreamApi::new(ctx.client);
            stream_api
                .stream_url(&song.id)
                .map_err(|e| anyhow::anyhow!("{e}"))
        })?;

    // Record history
    let history = crate::history::PlayHistory::new();
    playback::record_history(&history, &track);

    ctx.engine.play_url(&url, track.clone())?;
    app.set_now_playing(track, index);
    app.status = format!("Playing [{}/{}]", index + 1, app.queue.len());
    Ok(())
}

/// Advance to next track in queue.
pub async fn advance_queue(app: &mut App, ctx: &TuiContext<'_>) -> anyhow::Result<()> {
    let next_index = match &app.now_playing {
        Some(np) => np.queue_index + 1,
        None => return Ok(()),
    };

    if next_index < app.queue.len() {
        play_queue_index(app, ctx, next_index).await
    } else {
        app.now_playing = None;
        app.status = "Queue finished.".to_string();
        Ok(())
    }
}

/// Go back to previous track in queue.
async fn rewind_queue(app: &mut App, ctx: &TuiContext<'_>) -> anyhow::Result<()> {
    let prev_index = match &app.now_playing {
        Some(np) if np.queue_index > 0 => np.queue_index - 1,
        _ => return Ok(()),
    };
    play_queue_index(app, ctx, prev_index).await
}
