use crossterm::event::{KeyCode, KeyEvent};

use crate::api::client::SynoClient;
use crate::api::favorites::FavoritesApi;
use crate::api::folder::FolderApi;
use crate::api::playlist::PlaylistApi;
use crate::api::song::SongApi;
use crate::api::stream::StreamApi;
use crate::api::types::Song;
use crate::cache::manager::CacheManager;
use crate::config::model::CacheConfig;
use crate::ipc::protocol::{IpcData, IpcRequest, IpcResponse, QueueTrack};
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
    pub favorites_playlist: &'a str,
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

        // --- Space: play/stop toggle ---
        KeyCode::Char(' ') => {
            if app.now_playing.is_some() {
                app.stop_playback(ctx.engine);
            } else if let Err(e) = handle_space_play(app, ctx).await {
                app.status = format!("Error: {e}");
            }
        }

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
            if let Err(e) = skip_next(app, ctx).await {
                app.now_playing = None;
                app.status = format!("Error: {e}");
            }
        }
        KeyCode::Char('p') => {
            if let Err(e) = skip_prev(app, ctx).await {
                app.status = format!("Error: {e}");
            }
        }

        // --- Shuffle / Repeat ---
        KeyCode::Char('s') => {
            app.shuffle = !app.shuffle;
            if app.shuffle
                && !app.queue.is_empty()
                && let Some(ref np) = app.now_playing
            {
                let idx = np.queue_index;
                if idx + 1 < app.queue.len() {
                    use rand::seq::SliceRandom;
                    app.queue[idx + 1..].shuffle(&mut rand::thread_rng());
                }
            }
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

        // --- Multi-select (Folders tab) ---
        KeyCode::Insert => {
            if app.active_tab == Tab::Folders
                && let Some(idx) = app.folders.selected()
                && let Some(item) = app.folders.items.get(idx)
                && item.is_directory()
            {
                if app.selected_folders.contains(&idx) {
                    app.selected_folders.remove(&idx);
                } else {
                    app.selected_folders.insert(idx);
                }
                let count = app.selected_folders.len();
                app.status = if count > 0 {
                    format!("{count} folders selected")
                } else {
                    "Selection cleared".to_string()
                };
                app.active_list_next();
            }
        }

        // --- Favorite toggle ---
        KeyCode::Char('f') => {
            if let Err(e) = handle_favorite_toggle(app, ctx).await {
                app.status = format!("Error: {e}");
            }
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

// --- Enter: navigate or play from current position ---

async fn handle_enter(app: &mut App, ctx: &TuiContext<'_>) -> anyhow::Result<()> {
    match app.active_tab {
        Tab::Favorites => {
            // Play all favorites from selected position
            let idx = app.favorites.selected().unwrap_or(0);
            let mut queue = app.favorites.items.clone();
            let start = apply_shuffle(app.shuffle, &mut queue, idx);
            play_and_show_queue(app, ctx, queue, start).await?;
        }
        Tab::Folders => {
            handle_folder_enter(app, ctx).await?;
        }
        Tab::Playlists => {
            if app.playlist_detail.is_some() {
                // Play from selected song in playlist detail
                let (mut queue, idx) = {
                    let detail = app.playlist_detail.as_ref().unwrap();
                    let idx = detail.songs.selected().unwrap_or(0);
                    (detail.songs.items.clone(), idx)
                };
                let start = apply_shuffle(app.shuffle, &mut queue, idx);
                play_and_show_queue(app, ctx, queue, start).await?;
            } else if let Some(pl) = app.playlists.selected_item() {
                // Open playlist detail (don't play yet)
                let id = pl.id.clone();
                let name = pl.name.clone();
                open_playlist(app, ctx, &id, &name).await?;
            }
        }
        Tab::Queue => {}
    }
    Ok(())
}

async fn handle_folder_enter(app: &mut App, ctx: &TuiContext<'_>) -> anyhow::Result<()> {
    let item = match app.folders.selected_item() {
        Some(f) => f.clone(),
        None => return Ok(()),
    };

    if item.is_directory() {
        // Navigate into directory
        app.folder_stack.push((item.id.clone(), item.title.clone()));
        load_folder(app, ctx).await?;
    } else {
        // Play all files from current folder starting at selected
        let songs = collect_folder_songs(app);
        let idx = songs.iter().position(|s| s.id == item.id).unwrap_or(0);
        let mut queue = songs;
        let start = apply_shuffle(app.shuffle, &mut queue, idx);
        play_and_show_queue(app, ctx, queue, start).await?;
    }
    Ok(())
}

// --- Space: play/stop toggle with special actions ---

async fn handle_space_play(app: &mut App, ctx: &TuiContext<'_>) -> anyhow::Result<()> {
    match app.active_tab {
        Tab::Favorites => {
            if app.favorites.items.is_empty() {
                app.status = "No favorites to play".to_string();
                return Ok(());
            }
            let idx = app.favorites.selected().unwrap_or(0);
            let mut queue = app.favorites.items.clone();
            let start = apply_shuffle(app.shuffle, &mut queue, idx);
            play_and_show_queue(app, ctx, queue, start).await?;
        }
        Tab::Folders => {
            handle_folder_space(app, ctx).await?;
        }
        Tab::Playlists => {
            handle_playlist_space(app, ctx).await?;
        }
        Tab::Queue => {}
    }
    Ok(())
}

/// Space on a folder item: play directory contents or file.
/// If folders are multi-selected via Insert, plays all selected directories.
async fn handle_folder_space(app: &mut App, ctx: &TuiContext<'_>) -> anyhow::Result<()> {
    // Multi-select mode: play all selected folders
    if !app.selected_folders.is_empty() {
        let mut indices: Vec<usize> = app.selected_folders.iter().copied().collect();
        indices.sort();
        app.status = format!("Scanning {} folders...", indices.len());
        let mut all_songs = Vec::new();
        for idx in &indices {
            if let Some(item) = app.folders.items.get(*idx)
                && item.is_directory()
            {
                let songs = collect_folder_recursive(ctx.client, &item.id, 10).await?;
                all_songs.extend(songs);
            }
        }
        app.selected_folders.clear();
        if all_songs.is_empty() {
            app.status = "No audio files in selected folders".to_string();
            return Ok(());
        }
        let mut queue = all_songs;
        let start = apply_shuffle(app.shuffle, &mut queue, 0);
        play_and_show_queue(app, ctx, queue, start).await?;
        return Ok(());
    }

    let item = match app.folders.selected_item() {
        Some(f) => f.clone(),
        None => return Ok(()),
    };

    if item.is_directory() {
        // Recursively collect all audio files from directory tree
        app.status = format!("Scanning '{}'...", item.title);
        let songs = collect_folder_recursive(ctx.client, &item.id, 10).await?;
        if songs.is_empty() {
            app.status = format!("No audio files in '{}'", item.title);
            return Ok(());
        }
        let mut queue = songs;
        let start = apply_shuffle(app.shuffle, &mut queue, 0);
        play_and_show_queue(app, ctx, queue, start).await?;
    } else {
        // Same as Enter on a file
        let songs = collect_folder_songs(app);
        let idx = songs.iter().position(|s| s.id == item.id).unwrap_or(0);
        let mut queue = songs;
        let start = apply_shuffle(app.shuffle, &mut queue, idx);
        play_and_show_queue(app, ctx, queue, start).await?;
    }
    Ok(())
}

/// Space on a playlist: load and play immediately.
async fn handle_playlist_space(app: &mut App, ctx: &TuiContext<'_>) -> anyhow::Result<()> {
    if app.playlist_detail.is_some() {
        // Play from selected song in playlist detail
        let (mut queue, idx) = {
            let detail = app.playlist_detail.as_ref().unwrap();
            let idx = detail.songs.selected().unwrap_or(0);
            (detail.songs.items.clone(), idx)
        };
        let start = apply_shuffle(app.shuffle, &mut queue, idx);
        play_and_show_queue(app, ctx, queue, start).await?;
    } else if let Some(pl) = app.playlists.selected_item() {
        // Load playlist and play from first track
        let id = pl.id.clone();
        let name = pl.name.clone();
        app.status = format!("Loading playlist '{name}'...");
        let api = PlaylistApi::new(ctx.client);
        let data = api.get_info(&id).await?;
        if let Some(detail) = data.into_playlist() {
            let songs = detail.all_songs().to_vec();
            if songs.is_empty() {
                app.status = format!("Playlist '{name}' is empty.");
                return Ok(());
            }
            let mut queue = songs;
            let start = apply_shuffle(app.shuffle, &mut queue, 0);
            play_and_show_queue(app, ctx, queue, start).await?;
        } else {
            app.status = format!("Failed to load playlist '{name}'.");
        }
    }
    Ok(())
}

// --- Navigation helpers ---

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

/// Load folder contents based on current folder_stack.
pub async fn load_folder(app: &mut App, ctx: &TuiContext<'_>) -> anyhow::Result<()> {
    let folder_id = app.folder_stack.last().map(|(id, _)| id.as_str());
    let api = FolderApi::new(ctx.client);
    let data = api.list(folder_id, 0, 500).await?;
    let count = data.items.len();
    app.folders = StatefulList::with_items(data.items);
    app.selected_folders.clear();
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

// --- Playback helpers ---

/// Collect all non-directory items from current folder view as Songs.
fn collect_folder_songs(app: &App) -> Vec<Song> {
    app.folders
        .items
        .iter()
        .filter(|f| !f.is_directory())
        .map(|f| f.to_song())
        .collect()
}

/// Recursively collect all audio files from a directory tree.
/// Scans subdirectories up to `max_depth` levels deep.
async fn collect_folder_recursive(
    client: &SynoClient,
    folder_id: &str,
    max_depth: u32,
) -> anyhow::Result<Vec<Song>> {
    let api = FolderApi::new(client);
    let data = api.list(Some(folder_id), 0, 500).await?;

    let mut songs = Vec::new();
    let mut subdirs = Vec::new();

    for item in &data.items {
        if item.is_directory() {
            if max_depth > 0 {
                subdirs.push(item.id.clone());
            }
        } else {
            songs.push(item.to_song());
        }
    }

    for dir_id in subdirs {
        let sub_songs = Box::pin(collect_folder_recursive(client, &dir_id, max_depth - 1)).await?;
        songs.extend(sub_songs);
    }

    Ok(songs)
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

/// Set queue, start playback, and switch to Queue tab.
async fn play_and_show_queue(
    app: &mut App,
    ctx: &TuiContext<'_>,
    queue: Vec<Song>,
    index: usize,
) -> anyhow::Result<()> {
    if queue.is_empty() || index >= queue.len() {
        return Ok(());
    }
    app.queue = queue;
    play_queue_index(app, ctx, index).await?;
    app.active_tab = Tab::Queue;
    Ok(())
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
/// Explicit next: always advances, even in repeat-one mode.
async fn skip_next(
    app: &mut App,
    ctx: &TuiContext<'_>,
) -> anyhow::Result<()> {
    let next_index = match &app.now_playing {
        Some(np) => match app.repeat_mode {
            RepeatMode::All if !app.queue.is_empty() => {
                (np.queue_index + 1) % app.queue.len()
            }
            _ => np.queue_index + 1,
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

/// Explicit prev: always goes back, even in repeat-one mode.
async fn skip_prev(
    app: &mut App,
    ctx: &TuiContext<'_>,
) -> anyhow::Result<()> {
    let prev_index = match &app.now_playing {
        Some(np) => match app.repeat_mode {
            RepeatMode::All if np.queue_index == 0 => {
                app.queue.len() - 1
            }
            _ => {
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

/// Auto-advance: called when track finishes naturally.
/// Respects repeat-one (replays same track).
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

/// Handle an IPC command. Maps IPC requests to the same operations as key presses.
pub async fn handle_ipc(
    app: &mut App,
    ctx: &TuiContext<'_>,
    request: IpcRequest,
) -> IpcResponse {
    match request {
        IpcRequest::Pause => {
            if app.now_playing.is_some() {
                ctx.engine.pause();
                app.status = "Paused (via IPC)".to_string();
                IpcResponse::ok("Paused")
            } else {
                IpcResponse::err("Nothing is playing")
            }
        }
        IpcRequest::Resume => {
            // Resume not fully supported with subprocess model
            app.status = "Resume not supported (subprocess player)".to_string();
            IpcResponse::err("Resume not supported with subprocess player")
        }
        IpcRequest::Stop => {
            app.stop_playback(ctx.engine);
            IpcResponse::ok("Stopped")
        }
        IpcRequest::Next => match skip_next(app, ctx).await {
            Ok(()) => {
                if let Some(ref np) = app.now_playing {
                    IpcResponse::ok(format!(
                        "[{}/{}] {} - {}",
                        np.queue_index + 1,
                        app.queue.len(),
                        np.track.artist,
                        np.track.title,
                    ))
                } else {
                    IpcResponse::ok("Queue finished")
                }
            }
            Err(e) => {
                app.now_playing = None;
                IpcResponse::err(format!("Error: {e}"))
            }
        },
        IpcRequest::Prev => match skip_prev(app, ctx).await {
            Ok(()) => {
                if let Some(ref np) = app.now_playing {
                    IpcResponse::ok(format!(
                        "[{}/{}] {} - {}",
                        np.queue_index + 1,
                        app.queue.len(),
                        np.track.artist,
                        np.track.title,
                    ))
                } else {
                    IpcResponse::ok("At start of queue")
                }
            }
            Err(e) => IpcResponse::err(format!("Error: {e}")),
        },
        IpcRequest::Now => {
            if let Some(ref np) = app.now_playing {
                IpcResponse::ok_with_data(
                    format!("{} - {}", np.track.artist, np.track.title),
                    IpcData::NowPlaying {
                        song_id: np.track.id.clone(),
                        title: np.track.title.clone(),
                        artist: np.track.artist.clone(),
                        album: np.track.album.clone(),
                        position_secs: np.elapsed().as_secs(),
                        duration_secs: np.track.duration.as_secs(),
                        volume: app.volume,
                        shuffle: app.shuffle,
                        repeat: match app.repeat_mode {
                            RepeatMode::Off => "off".to_string(),
                            RepeatMode::One => "one".to_string(),
                            RepeatMode::All => "all".to_string(),
                        },
                        queue_index: np.queue_index,
                        queue_total: app.queue.len(),
                    },
                )
            } else {
                IpcResponse::err("Nothing is playing")
            }
        }
        IpcRequest::Queue => {
            let current_index = app
                .now_playing
                .as_ref()
                .map(|np| np.queue_index)
                .unwrap_or(0);
            let tracks: Vec<QueueTrack> = app
                .queue
                .iter()
                .enumerate()
                .map(|(i, song)| {
                    let track = playback::track_from_song(song);
                    QueueTrack {
                        index: i,
                        title: track.title,
                        artist: track.artist,
                        duration_secs: track.duration.as_secs(),
                    }
                })
                .collect();
            if tracks.is_empty() {
                IpcResponse::err("Queue is empty")
            } else {
                IpcResponse::ok_with_data(
                    format!("{} tracks in queue", tracks.len()),
                    IpcData::QueueList {
                        current_index,
                        tracks,
                    },
                )
            }
        }
        IpcRequest::Volume { level } => {
            app.volume = level.min(100);
            ctx.engine.set_volume(app.volume);
            app.status = format!("Volume: {}%", app.volume);
            IpcResponse::ok(format!("Volume set to {}%", app.volume))
        }
        IpcRequest::Shuffle { mode } => {
            app.shuffle = mode == "on";
            app.status = format!(
                "Shuffle: {}",
                if app.shuffle { "ON" } else { "off" }
            );
            IpcResponse::ok(format!(
                "Shuffle {}",
                if app.shuffle { "ON" } else { "off" }
            ))
        }
        IpcRequest::Repeat { mode } => {
            app.repeat_mode = match mode.as_str() {
                "one" => RepeatMode::One,
                "all" => RepeatMode::All,
                _ => RepeatMode::Off,
            };
            app.status = format!(
                "Repeat: {}",
                match app.repeat_mode {
                    RepeatMode::Off => "off",
                    RepeatMode::One => "one",
                    RepeatMode::All => "all",
                }
            );
            IpcResponse::ok(format!("Repeat: {mode}"))
        }
    }
}

/// Go back to previous track in queue (respects repeat mode).

/// Toggle favorite for the currently playing track.
async fn handle_favorite_toggle(
    app: &mut App,
    ctx: &TuiContext<'_>,
) -> anyhow::Result<()> {
    let song_id = match &app.now_playing {
        Some(np) => np.track.id.clone(),
        None => {
            app.status = "Nothing is playing".to_string();
            return Ok(());
        }
    };

    let fav_api = FavoritesApi::new(
        ctx.client,
        ctx.favorites_playlist,
    );
    let is_fav = app
        .favorites
        .items
        .iter()
        .any(|s| s.id == song_id);

    if is_fav {
        fav_api.remove(&song_id).await?;
        app.favorites.items.retain(|s| s.id != song_id);
        app.status =
            format!("Removed from favorites: {song_id}");
    } else {
        fav_api.add(&song_id).await?;
        let song_api = SongApi::new(ctx.client);
        if let Ok(song) = song_api.get_info(&song_id).await {
            app.favorites.items.push(song);
        }
        app.status =
            format!("Added to favorites: {song_id}");
    }
    Ok(())
}
