use crossterm::event::{KeyCode, KeyEvent};

use crate::api::client::SynoClient;
use crate::api::folder::FolderApi;
use crate::api::playlist::PlaylistApi;
use crate::api::stream::StreamApi;
use crate::api::types::Song;
use crate::cache::manager::CacheManager;
use crate::config::model::CacheConfig;
use crate::playback;
use crate::player::engine::AudioEngine;
use crate::player::queue::RepeatMode;

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
        KeyCode::Esc => {
            if let Err(e) = handle_escape(app, ctx).await {
                app.status = format!("Error: {e}");
            }
        }

        // --- Playback ---
        KeyCode::Char('n') => {
            if let Err(e) = advance_queue(app, ctx).await {
                app.now_playing = None;
                app.status = format!("Error: {e}");
            }
        }
        KeyCode::Char('p') => {
            if let Err(e) = rewind_queue(app, ctx).await {
                app.status = format!("Error: {e}");
            }
        }

        // --- Shuffle / Repeat ---
        KeyCode::Char('s') => {
            app.shuffle = !app.shuffle;
            app.status = format!(
                "Shuffle: {}",
                if app.shuffle { "ON" } else { "off" }
            );
        }
        KeyCode::Char('r') => {
            app.repeat_mode = match app.repeat_mode {
                RepeatMode::Off => RepeatMode::All,
                RepeatMode::All => RepeatMode::One,
                RepeatMode::One => RepeatMode::Off,
            };
            app.status = format!(
                "Repeat: {}",
                match app.repeat_mode {
                    RepeatMode::Off => "off",
                    RepeatMode::One => "one",
                    RepeatMode::All => "all",
                }
            );
        }

        // --- Volume ---
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
                let mut queue: Vec<Song> = app.songs.items.clone();
                let start = apply_shuffle(app.shuffle, &mut queue, idx);
                play_from_queue(app, ctx, queue, start).await?;
            }
        }
        Tab::Folders => {
            handle_folder_enter(app, ctx).await?;
        }
        Tab::Playlists => {
            if app.playlist_detail.is_some() {
                let (mut queue, idx) = {
                    let detail = app.playlist_detail.as_ref().unwrap();
                    let idx = detail.songs.selected().unwrap_or(0);
                    (detail.songs.items.clone(), idx)
                };
                let start = apply_shuffle(app.shuffle, &mut queue, idx);
                play_from_queue(app, ctx, queue, start).await?;
            } else if let Some(pl) = app.playlists.selected_item() {
                let id = pl.id.clone();
                let name = pl.name.clone();
                open_playlist(app, ctx, &id, &name).await?;
            }
        }
        Tab::Queue => {}
    }
    Ok(())
}

async fn handle_escape(app: &mut App, ctx: &TuiContext<'_>) -> anyhow::Result<()> {
    match app.active_tab {
        Tab::Playlists if app.playlist_detail.is_some() => {
            app.playlist_detail = None;
        }
        Tab::Folders if !app.folder_stack.is_empty() => {
            app.folder_stack.pop();
            load_folder(app, ctx).await?;
        }
        _ => {}
    }
    Ok(())
}

async fn handle_folder_enter(app: &mut App, ctx: &TuiContext<'_>) -> anyhow::Result<()> {
    let item = match app.folders.selected_item() {
        Some(f) => f.clone(),
        None => return Ok(()),
    };

    if item.is_dir {
        app.folder_stack.push((item.id.clone(), item.title.clone()));
        load_folder(app, ctx).await?;
    } else {
        // Build queue from all files (non-dir) in current folder
        let songs: Vec<Song> = app
            .folders
            .items
            .iter()
            .filter(|f| !f.is_dir)
            .map(|f| f.to_song())
            .collect();
        let idx = songs.iter().position(|s| s.id == item.id).unwrap_or(0);
        let mut queue = songs;
        let start = apply_shuffle(app.shuffle, &mut queue, idx);
        play_from_queue(app, ctx, queue, start).await?;
    }
    Ok(())
}

/// Load folder contents based on current folder_stack.
pub async fn load_folder(app: &mut App, ctx: &TuiContext<'_>) -> anyhow::Result<()> {
    let folder_id = app.folder_stack.last().map(|(id, _)| id.as_str());
    let api = FolderApi::new(ctx.client);
    let data = api.list(folder_id, 0, 500).await?;
    let count = data.items.len();
    app.folders = StatefulList::with_items(data.items);
    app.status = format!("Loaded {count} items");
    Ok(())
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

/// Apply shuffle to queue if enabled. Returns the starting index.
fn apply_shuffle(shuffle: bool, queue: &mut [Song], selected_idx: usize) -> usize {
    if !shuffle || queue.is_empty() {
        return selected_idx;
    }
    // Move selected track to front
    if selected_idx > 0 && selected_idx < queue.len() {
        queue.swap(0, selected_idx);
    }
    // Shuffle the rest
    if queue.len() > 2 {
        use rand::seq::SliceRandom;
        queue[1..].shuffle(&mut rand::thread_rng());
    }
    0
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

/// Advance to next track in queue (respects repeat mode).
pub async fn advance_queue(app: &mut App, ctx: &TuiContext<'_>) -> anyhow::Result<()> {
    let next_index = match &app.now_playing {
        Some(np) => match app.repeat_mode {
            RepeatMode::One => np.queue_index,
            RepeatMode::All if app.queue.is_empty() => return Ok(()),
            RepeatMode::All => (np.queue_index + 1) % app.queue.len(),
            RepeatMode::Off => np.queue_index + 1,
        },
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

/// Go back to previous track in queue (respects repeat mode).
async fn rewind_queue(app: &mut App, ctx: &TuiContext<'_>) -> anyhow::Result<()> {
    let prev_index = match &app.now_playing {
        Some(np) => match app.repeat_mode {
            RepeatMode::One => np.queue_index,
            RepeatMode::All if app.queue.is_empty() => return Ok(()),
            RepeatMode::All => {
                if np.queue_index == 0 {
                    app.queue.len() - 1
                } else {
                    np.queue_index - 1
                }
            }
            RepeatMode::Off => {
                if np.queue_index > 0 {
                    np.queue_index - 1
                } else {
                    return Ok(());
                }
            }
        },
        _ => return Ok(()),
    };
    play_queue_index(app, ctx, prev_index).await
}
