use std::time::Duration;

use clap::Parser;
use synoplayer::api::auth::AuthApi;
use synoplayer::api::client::SynoClient;
use synoplayer::api::song::SongApi;
use synoplayer::api::stream::StreamApi;
use synoplayer::cache::manager::CacheManager;
use synoplayer::cli;
use synoplayer::config::model::AppConfig;
use synoplayer::credentials::store::CredentialStore;
use synoplayer::player::engine::AudioEngine;
use synoplayer::player::state::TrackInfo;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = cli::Cli::parse();
    let config = AppConfig::load()?;

    match cli.command {
        // --- Config ---
        cli::Commands::Config { action } => match action {
            cli::ConfigAction::Show => {
                println!("{}", toml::to_string_pretty(&config)?);
            }
            cli::ConfigAction::SetServer { host } => {
                let mut config = config;
                config.server.host = host;
                config.save()?;
                println!("Server host updated.");
            }
            cli::ConfigAction::SetPort { port } => {
                let mut config = config;
                config.server.port = port;
                config.save()?;
                println!("Server port updated.");
            }
        },

        // --- Login ---
        cli::Commands::Login { no_save } => {
            let save = !no_save;
            let username = prompt("Username: ")?;
            let password = prompt_password("Password: ")?;

            let mut client = SynoClient::new(&config.base_url());
            let mut auth = AuthApi::new(&mut client);

            auth.discover().await?;
            auth.login(&username, &password).await?;

            let sid = auth.client.sid().unwrap_or("").to_string();
            save_session(&sid)?;

            if save {
                let store = CredentialStore::from_config(&config.auth.credential_store);
                store.save(&username, &password)?;
                println!("Logged in. Credentials saved.");
            } else {
                println!("Logged in (session only, credentials not saved).");
            }
        }

        // --- Logout ---
        cli::Commands::Logout => {
            let mut client = SynoClient::new(&config.base_url());
            if let Some(sid) = load_session() {
                client.set_sid(sid);
                let mut auth = AuthApi::new(&mut client);
                auth.logout().await?;
            }
            clear_session()?;
            println!("Logged out.");
        }

        // --- Credentials ---
        cli::Commands::Credentials { action } => match action {
            cli::CredentialAction::Clear => {
                let store = CredentialStore::from_config(&config.auth.credential_store);
                store.clear()?;
                println!("Credentials cleared.");
            }
        },

        // --- Songs ---
        cli::Commands::Songs {
            album,
            artist,
            genre,
            limit,
        } => {
            let client = connect(&config).await?;
            let api = SongApi::new(&client);

            let mut params: Vec<(&str, &str)> = vec![];
            let album_ref = album.as_deref().unwrap_or("");
            let artist_ref = artist.as_deref().unwrap_or("");
            let genre_ref = genre.as_deref().unwrap_or("");
            if !album_ref.is_empty() {
                params.push(("album", album_ref));
            }
            if !artist_ref.is_empty() {
                params.push(("artist", artist_ref));
            }
            if !genre_ref.is_empty() {
                params.push(("genre", genre_ref));
            }

            let data = api.list(0, limit).await?;
            println!("Songs ({}/{}):", data.songs.len(), data.total);
            for song in &data.songs {
                print_song(song);
            }
        }

        // --- Search ---
        cli::Commands::Search { keyword } => {
            let client = connect(&config).await?;
            let api = synoplayer::api::search::SearchApi::new(&client);
            let data = api.search(&keyword, 0, 50).await?;

            if !data.songs.is_empty() {
                println!("Songs ({}):", data.songs.len());
                for song in &data.songs {
                    print_song(song);
                }
            }
            if !data.albums.is_empty() {
                println!("\nAlbums ({}):", data.albums.len());
                for album in &data.albums {
                    println!("  {} - {} ({})", album.name, album.artist, album.year);
                }
            }
            if !data.artists.is_empty() {
                println!("\nArtists ({}):", data.artists.len());
                for artist in &data.artists {
                    println!("  {}", artist.name);
                }
            }
            if data.songs.is_empty() && data.albums.is_empty() && data.artists.is_empty() {
                println!("Nothing found for '{keyword}'.");
            }
        }

        // --- Play ---
        cli::Commands::Play { target } => {
            let client = connect(&config).await?;

            // Try target as song ID first, then search by name
            let song = if target.starts_with("music_") {
                let api = SongApi::new(&client);
                api.get_info(&target).await?
            } else {
                let api = SongApi::new(&client);
                let data = api.search(&target, 0, 1).await?;
                data.songs.into_iter().next().ok_or_else(|| {
                    synoplayer::error::SynoError::Player(format!("No song found: {target}"))
                })?
            };

            let stream_api = StreamApi::new(&client);
            let url = stream_api.stream_url(&song.id)?;

            let track = track_from_song(&song);
            let engine = AudioEngine::new();

            println!("Playing: {} - {}", track.artist, track.title);
            println!(
                "Album: {} | Duration: {}",
                track.album,
                format_duration(track.duration)
            );

            engine.play_url(&url, track)?;

            // Wait for playback to finish
            loop {
                if engine.check_finished() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
            println!("Playback finished.");
        }

        // --- Now ---
        cli::Commands::Now => {
            eprintln!("No active player session. Use `synoplayer play <song>` to start.");
        }

        // --- Queue ---
        cli::Commands::Queue => {
            eprintln!("No active player session.");
        }

        // --- Volume ---
        cli::Commands::Volume { level } => {
            eprintln!("Volume control requires active player session. (Would set to {level}%)");
        }

        // --- Pause/Resume/Stop/Next/Prev ---
        cli::Commands::Pause
        | cli::Commands::Resume
        | cli::Commands::Stop
        | cli::Commands::Next
        | cli::Commands::Prev => {
            eprintln!("No active player session. Playback controls work during `synoplayer play`.");
        }

        // --- Albums ---
        cli::Commands::Albums { artist } => {
            let client = connect(&config).await?;
            let api = synoplayer::api::album::AlbumApi::new(&client);
            let data = api.list(0, 50, artist.as_deref(), None).await?;
            println!("Albums ({}/{}):", data.albums.len(), data.total);
            for album in &data.albums {
                if album.artist.is_empty() {
                    println!("  {} ({})", album.name, album.year);
                } else {
                    println!("  {} - {} ({})", album.name, album.artist, album.year);
                }
            }
        }

        // --- Artists ---
        cli::Commands::Artists => {
            let client = connect(&config).await?;
            let api = synoplayer::api::artist::ArtistApi::new(&client);
            let data = api.list(0, 100).await?;
            println!("Artists ({}/{}):", data.artists.len(), data.total);
            for artist in &data.artists {
                println!("  {}", artist.name);
            }
        }

        // --- Genres ---
        cli::Commands::Genres => {
            let client = connect(&config).await?;
            let api = synoplayer::api::genre::GenreApi::new(&client);
            let data = api.list(0, 200).await?;
            println!("Genres ({}/{}):", data.genres.len(), data.total);
            for genre in &data.genres {
                println!("  {}", genre.name);
            }
        }

        // --- Composers ---
        cli::Commands::Composers => {
            let client = connect(&config).await?;
            let api = synoplayer::api::composer::ComposerApi::new(&client);
            let data = api.list(0, 200).await?;
            println!("Composers ({}/{}):", data.composers.len(), data.total);
            for composer in &data.composers {
                println!("  {}", composer.name);
            }
        }

        // --- Folders ---
        cli::Commands::Folders { path } => {
            let client = connect(&config).await?;
            let api = synoplayer::api::folder::FolderApi::new(&client);
            let data = api.list(path.as_deref(), 0, 200).await?;
            println!("Folder ({}/{}):", data.items.len(), data.total);
            for item in &data.items {
                let icon = if item.is_dir { "[D]" } else { "   " };
                println!("  {icon} {} ({})", item.title, item.id);
            }
        }

        // --- Playlists ---
        cli::Commands::Playlists => {
            let client = connect(&config).await?;
            let api = synoplayer::api::playlist::PlaylistApi::new(&client);
            // Fetch all playlists without library filter (API returns both)
            let data = api.list(0, 200, None).await?;

            println!("Playlists ({}):", data.playlists.len());
            for pl in &data.playlists {
                let count = pl.songs_count.unwrap_or(0);
                println!(
                    "  [{}] {} ({} songs, {})",
                    pl.id, pl.name, count, pl.library
                );
            }
        }

        // --- Rate ---
        cli::Commands::Rate { song_id, rating } => {
            if !(0..=5).contains(&rating) {
                eprintln!("Rating must be 0-5.");
                std::process::exit(1);
            }
            let client = connect(&config).await?;
            let api = SongApi::new(&client);
            api.set_rating(&song_id, rating).await?;
            println!("Rating set to {rating} for {song_id}.");
        }

        // --- Favorites ---
        cli::Commands::Favorite { song_id } => {
            let client = connect(&config).await?;
            let api = synoplayer::api::pin::PinApi::new(&client);
            api.pin(&song_id).await?;
            println!("Added to favorites: {song_id}");
        }
        cli::Commands::Unfavorite { song_id } => {
            let client = connect(&config).await?;
            let api = synoplayer::api::pin::PinApi::new(&client);
            api.unpin(&song_id).await?;
            println!("Removed from favorites: {song_id}");
        }
        cli::Commands::Favorites => {
            let client = connect(&config).await?;
            let api = synoplayer::api::pin::PinApi::new(&client);
            let data = api.list().await?;
            println!("Favorites ({}):", data.total);
            for item in &data.items {
                let display_name = if item.name.is_empty() {
                    &item.title
                } else {
                    &item.name
                };
                println!("  [{}] {} ({})", item.id, display_name, item.item_type);
            }
        }

        // --- Lyrics ---
        cli::Commands::Lyrics { song_id } => {
            if let Some(id) = song_id {
                let client = connect(&config).await?;
                let api = synoplayer::api::lyrics::LyricsApi::new(&client);
                let data = api.get(&id).await?;
                if data.lyrics.is_empty() {
                    println!("No lyrics found for {id}.");
                } else {
                    println!("{}", data.lyrics);
                }
            } else {
                eprintln!("Specify song_id. Usage: synoplayer lyrics <song_id>");
            }
        }

        // --- Cache ---
        cli::Commands::Cache { action } => match action {
            cli::CacheAction::Status => {
                let cache = CacheManager::new(config.cache.clone());
                let status = cache.status()?;
                println!(
                    "Cache: {}",
                    if status.enabled {
                        "enabled"
                    } else {
                        "disabled"
                    }
                );
                println!("Path: {}", status.path.display());
                println!("Files: {}", status.file_count);
                println!(
                    "Size: {:.1} MB / {:.0} MB",
                    status.total_size_bytes as f64 / 1_048_576.0,
                    status.max_size_bytes as f64 / 1_048_576.0
                );
            }
            cli::CacheAction::Clear { older: _ } => {
                let cache = CacheManager::new(config.cache.clone());
                cache.clear()?;
                println!("Cache cleared.");
            }
            _ => {
                eprintln!("Not yet implemented.");
            }
        },

        // --- Playlist subcommands ---
        // --- Playlist subcommands ---
        cli::Commands::Playlist { action } => {
            let client = connect(&config).await?;
            let api = synoplayer::api::playlist::PlaylistApi::new(&client);
            match action {
                cli::PlaylistAction::Show { name } => {
                    let pl_id = resolve_playlist_id(&api, &name).await?;
                    let pl = get_playlist_detail(&api, &pl_id).await?;
                    let songs = pl.all_songs();
                    println!("Playlist: {} ({} songs)", pl.name, songs.len());
                    for song in songs {
                        print_song(song);
                    }
                }
                cli::PlaylistAction::Create { name } => {
                    api.create(&name, "personal").await?;
                    println!("Playlist '{name}' created.");
                }
                cli::PlaylistAction::Delete { name } => {
                    let pl_id = resolve_playlist_id(&api, &name).await?;
                    api.delete(&pl_id).await?;
                    println!("Playlist '{name}' deleted.");
                }
                cli::PlaylistAction::Add { playlist, song_id } => {
                    let pl_id = resolve_playlist_id(&api, &playlist).await?;
                    let pl = get_playlist_detail(&api, &pl_id).await?;
                    let mut ids: Vec<&str> = pl.all_songs().iter().map(|s| s.id.as_str()).collect();
                    ids.push(&song_id);
                    api.update_songs(&pl_id, &ids).await?;
                    println!("Added {song_id} to '{playlist}'.");
                }
                cli::PlaylistAction::Remove { playlist, song_id } => {
                    let pl_id = resolve_playlist_id(&api, &playlist).await?;
                    let pl = get_playlist_detail(&api, &pl_id).await?;
                    let ids: Vec<&str> = pl
                        .all_songs()
                        .iter()
                        .map(|s| s.id.as_str())
                        .filter(|id| *id != song_id)
                        .collect();
                    api.update_songs(&pl_id, &ids).await?;
                    println!("Removed {song_id} from '{playlist}'.");
                }
                cli::PlaylistAction::Play { name } => {
                    let pl_id = resolve_playlist_id(&api, &name).await?;
                    let pl = get_playlist_detail(&api, &pl_id).await?;
                    let songs = pl.all_songs();
                    if songs.is_empty() {
                        eprintln!("Playlist '{name}' is empty.");
                    } else {
                        let first = &songs[0];
                        let stream_api = StreamApi::new(&client);
                        let url = stream_api.stream_url(&first.id)?;
                        let track = track_from_song(first);
                        println!("Playing playlist '{name}' ({} songs)", songs.len());
                        println!("  {} - {}", track.artist, track.title);
                        let engine = AudioEngine::new();
                        engine.play_url(&url, track)?;
                        loop {
                            if engine.check_finished() {
                                break;
                            }
                            tokio::time::sleep(Duration::from_millis(500)).await;
                        }
                    }
                }
                cli::PlaylistAction::Import { path, name } => {
                    import_m3u_playlist(&client, &api, &path, name.as_deref()).await?;
                }
            }
        }

        // --- Not yet implemented ---
        _ => {
            eprintln!("Not yet implemented. Run `synoplayer --help` for usage.");
        }
    }

    Ok(())
}

// --- Helpers ---

/// Connect to NAS with auto-login from saved credentials/session.
async fn connect(config: &AppConfig) -> anyhow::Result<SynoClient> {
    let mut client = SynoClient::new(&config.base_url());

    // Try existing session first
    if let Some(sid) = load_session() {
        client.set_sid(sid);
        // Try a quick API info call to validate session
        let mut auth = AuthApi::new(&mut client);
        if auth.discover().await.is_ok() {
            return Ok(client);
        }
        // Session expired, try re-login
        client.clear_sid();
    }

    // Try saved credentials
    let store = CredentialStore::from_config(&config.auth.credential_store);
    if let Some((username, password)) = store.load()? {
        let mut auth = AuthApi::new(&mut client);
        auth.discover().await?;
        auth.login(&username, &password).await?;
        let sid = auth.client.sid().unwrap_or("").to_string();
        save_session(&sid)?;
        return Ok(client);
    }

    anyhow::bail!("Not authenticated. Run `synoplayer login` first.")
}

fn track_from_song(song: &synoplayer::api::types::Song) -> TrackInfo {
    let (title, artist, album, duration) = if let Some(ref add) = song.additional {
        let tag = add.song_tag.as_ref();
        let audio = add.song_audio.as_ref();
        (
            tag.map(|t| t.title.clone())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| song.title.clone()),
            tag.map(|t| t.artist.clone()).unwrap_or_default(),
            tag.map(|t| t.album.clone()).unwrap_or_default(),
            audio
                .map(|a| Duration::from_secs(a.duration as u64))
                .unwrap_or_default(),
        )
    } else {
        (
            song.title.clone(),
            String::new(),
            String::new(),
            Duration::ZERO,
        )
    };

    TrackInfo {
        id: song.id.clone(),
        title,
        artist,
        album,
        duration,
    }
}

fn print_song(song: &synoplayer::api::types::Song) {
    let (title, artist, album, duration, rating) = if let Some(ref add) = song.additional {
        let tag = add.song_tag.as_ref();
        let audio = add.song_audio.as_ref();
        let rat = add.song_rating.as_ref();
        (
            tag.map(|t| t.title.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or(&song.title),
            tag.map(|t| t.artist.as_str()).unwrap_or(""),
            tag.map(|t| t.album.as_str()).unwrap_or(""),
            audio.map(|a| a.duration).unwrap_or(0),
            rat.map(|r| r.rating).unwrap_or(0),
        )
    } else {
        (song.title.as_str(), "", "", 0, 0)
    };

    let stars = "*".repeat(rating as usize);
    let dur = format_duration(Duration::from_secs(duration as u64));
    println!(
        "  [{id}] {artist} - {title} ({album}) [{dur}] {stars}",
        id = song.id
    );
}

fn format_duration(d: Duration) -> String {
    let secs = d.as_secs();
    let m = secs / 60;
    let s = secs % 60;
    format!("{m}:{s:02}")
}

fn prompt(msg: &str) -> anyhow::Result<String> {
    eprint!("{msg}");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

fn prompt_password(msg: &str) -> anyhow::Result<String> {
    prompt(msg)
}

fn session_path() -> std::path::PathBuf {
    AppConfig::session_path()
}

fn save_session(sid: &str) -> anyhow::Result<()> {
    let path = session_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let data = serde_json::json!({
        "sid": sid,
        "created_at": chrono::Utc::now().to_rfc3339(),
    });
    std::fs::write(path, serde_json::to_string(&data)?)?;
    Ok(())
}

fn load_session() -> Option<String> {
    let path = session_path();
    let content = std::fs::read_to_string(path).ok()?;
    let data: serde_json::Value = serde_json::from_str(&content).ok()?;
    data["sid"].as_str().map(|s| s.to_string())
}

fn clear_session() -> anyhow::Result<()> {
    let path = session_path();
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

/// Get playlist detail, handling both `playlist` and `playlists` response formats.
async fn get_playlist_detail(
    api: &synoplayer::api::playlist::PlaylistApi<'_>,
    id: &str,
) -> anyhow::Result<synoplayer::api::types::PlaylistDetail> {
    let data = api.get_info(id).await?;
    data.into_playlist()
        .ok_or_else(|| anyhow::anyhow!("Playlist not found: {id}"))
}

/// Find playlist ID by name or pass through if already an ID.
async fn resolve_playlist_id(
    api: &synoplayer::api::playlist::PlaylistApi<'_>,
    name_or_id: &str,
) -> anyhow::Result<String> {
    // If it looks like a playlist ID, use directly
    if name_or_id.starts_with("playlist_") {
        return Ok(name_or_id.to_string());
    }
    // Search by name across personal and shared
    for lib in &["personal", "shared"] {
        let data = api.list(0, 200, Some(lib)).await?;
        for pl in &data.playlists {
            if pl.name.eq_ignore_ascii_case(name_or_id) {
                return Ok(pl.id.clone());
            }
        }
    }
    anyhow::bail!("Playlist not found: {name_or_id}")
}

/// Import a .m3u playlist file from NAS into Audio Station.
///
/// Reads the .m3u file, resolves song paths to Audio Station IDs via search,
/// and creates a new playlist with the found songs.
async fn import_m3u_playlist(
    client: &SynoClient,
    api: &synoplayer::api::playlist::PlaylistApi<'_>,
    path: &str,
    name: Option<&str>,
) -> anyhow::Result<()> {
    // Determine playlist name from filename if not provided
    let playlist_name = if let Some(n) = name {
        n.to_string()
    } else {
        std::path::Path::new(path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Imported")
            .to_string()
    };

    // Read .m3u file via NAS filesystem
    // The file is on the NAS, not local. We need to use the Folder API or
    // read it via a different mechanism.
    // For now: use the Song search API to find songs by filename from the .m3u

    // Try to read the file locally (if running on the NAS itself)
    let entries = if std::path::Path::new(path).exists() {
        read_m3u_local(path)?
    } else {
        anyhow::bail!(
            "Cannot read '{path}' — file not accessible locally.\n\
             Run this command on the NAS itself, or copy the .m3u file locally."
        );
    };

    if entries.is_empty() {
        anyhow::bail!("No entries found in '{path}'.");
    }

    println!(
        "Found {} entries in .m3u, searching for songs...",
        entries.len()
    );

    // Resolve each entry to a song ID by searching
    let song_api = synoplayer::api::song::SongApi::new(client);
    let mut found_ids: Vec<String> = Vec::new();
    let mut not_found: Vec<String> = Vec::new();

    for entry in &entries {
        // Extract filename without extension for search
        let filename = std::path::Path::new(entry)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(entry);

        match song_api.search(filename, 0, 1).await {
            Ok(data) if !data.songs.is_empty() => {
                found_ids.push(data.songs[0].id.clone());
                println!("  + {filename}");
            }
            _ => {
                not_found.push(entry.clone());
                println!("  - {filename} (not found)");
            }
        }
    }

    if found_ids.is_empty() {
        anyhow::bail!("No matching songs found in Audio Station.");
    }

    // Create playlist
    let id_refs: Vec<&str> = found_ids.iter().map(|s| s.as_str()).collect();
    api.create_with_songs(&playlist_name, "personal", &id_refs)
        .await?;

    println!(
        "\nPlaylist '{}' created with {}/{} songs.",
        playlist_name,
        found_ids.len(),
        entries.len()
    );
    if !not_found.is_empty() {
        println!("{} songs not found in library.", not_found.len());
    }

    Ok(())
}

/// Parse a local .m3u file, returning non-comment, non-empty lines.
fn read_m3u_local(path: &str) -> anyhow::Result<Vec<String>> {
    let content = std::fs::read_to_string(path)?;
    let entries: Vec<String> = content
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.to_string())
        .collect();
    Ok(entries)
}
