use std::time::Duration;

use clap::Parser;
use synoplayer::api::auth::AuthApi;
use synoplayer::api::client::SynoClient;
use synoplayer::api::radio::RadioApi;
use synoplayer::api::song::SongApi;
use synoplayer::api::stream::StreamApi;
use synoplayer::cache::manager::CacheManager;
use synoplayer::cache::storage::CacheStorage;
use synoplayer::cli;
use synoplayer::config::model::AppConfig;
use synoplayer::credentials::store::CredentialStore;
use synoplayer::history::PlayHistory;
use synoplayer::playback::{
    format_duration, record_history, resolve_audio_source, song_meta_from_song, track_from_cache,
    track_from_song, wait_for_playback,
};
use synoplayer::player::engine::AudioEngine;
use synoplayer::player::state::TrackInfo;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = cli::Cli::parse();
    let config = AppConfig::load()?;

    // Run TTL cleanup on startup if cache is enabled
    if config.cache.enabled {
        let cache = CacheManager::new(config.cache.clone());
        if let Err(e) = cache.cleanup_expired() {
            tracing::warn!("Cache TTL cleanup failed: {e}");
        }
    }

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
            let cache = CacheManager::new(config.cache.clone());
            let history = PlayHistory::new();

            match connect(&config).await {
                Ok(client) => {
                    // Online mode
                    let song = find_song(&client, &target).await?;
                    let track = track_from_song(&song);
                    let source =
                        resolve_audio_source(&client, &song, &cache, &config.cache).await?;

                    println!("Playing: {} - {}", track.artist, track.title);
                    println!(
                        "Album: {} | Duration: {}",
                        track.album,
                        format_duration(track.duration)
                    );

                    record_history(&history, &track);
                    let engine = AudioEngine::new();
                    engine.play_url(&source, track)?;
                    wait_for_playback(&engine).await;
                    println!("Playback finished.");
                }
                Err(_) => {
                    // Offline mode — try cache
                    if target.starts_with("music_") && cache.contains(&target) {
                        let source = cache.file_path(&target);
                        let track = track_from_cache(&cache, &target);
                        println!(
                            "[offline] Playing from cache: {} - {}",
                            track.artist, track.title
                        );
                        record_history(&history, &track);
                        let engine = AudioEngine::new();
                        engine.play_url(source.to_str().unwrap_or(""), track)?;
                        wait_for_playback(&engine).await;
                        println!("Playback finished.");
                    } else {
                        anyhow::bail!(
                            "Server unreachable and no cached version for '{target}'.\n\
                             Run `synoplayer login` or check server connectivity."
                        );
                    }
                }
            }
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
            // API requires explicit library param — query both and merge
            let mut all_playlists = Vec::new();
            for lib in &["personal", "shared"] {
                let data = api.list(0, 200, Some(lib)).await?;
                all_playlists.extend(data.playlists);
            }

            println!("Playlists ({}):", all_playlists.len());
            for pl in &all_playlists {
                println!(
                    "  [{}] {} ({} songs, {})",
                    pl.id, pl.name, pl.song_count(), pl.library
                );
            }
        }

        // --- Rate ---
        cli::Commands::Rate { song_id, rating } => {
            if !(0..=5).contains(&rating) {
                eprintln!("Rating must be 0-5 (0 to clear).");
                std::process::exit(1);
            }
            let client = connect(&config).await?;
            let api = SongApi::new(&client);
            api.set_rating(&song_id, rating).await?;
            if rating == 0 {
                println!("Rating cleared for {song_id}.");
            } else {
                let stars = "*".repeat(rating as usize);
                println!("Rating set to {stars} ({rating}/5) for {song_id}.");
            }
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
            if data.items.is_empty() {
                println!("No favorites yet. Use `synoplayer favorite <song_id>` to add.");
            } else {
                println!("Favorites ({}):", data.total);
                for item in &data.items {
                    let display_name = if item.name.is_empty() {
                        &item.title
                    } else {
                        &item.name
                    };
                    let type_label = match item.item_type.as_str() {
                        "song" => "Song",
                        "album" => "Album",
                        "artist" => "Artist",
                        "playlist" => "Playlist",
                        other => other,
                    };
                    println!("  [{type_label}] {display_name} ({})", item.id);
                }
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
            cli::CacheAction::Clear { older } => {
                let cache = CacheManager::new(config.cache.clone());
                if let Some(days_str) = older {
                    let days: u32 = days_str.parse().map_err(|_| {
                        anyhow::anyhow!(
                            "Invalid --older value '{days_str}': expected number of days"
                        )
                    })?;
                    let removed = cache.clear_older_than_days(days)?;
                    println!("Removed {removed} entries older than {days} days.");
                } else {
                    cache.clear()?;
                    println!("Cache cleared.");
                }
            }
            cli::CacheAction::List => {
                let cache = CacheManager::new(config.cache.clone());
                let entries = cache.list_entries()?;
                if entries.is_empty() {
                    println!("Cache is empty.");
                } else {
                    println!("Cached tracks ({}):", entries.len());
                    for entry in &entries {
                        let label = if !entry.artist.is_empty() && !entry.title.is_empty() {
                            format!("{} - {}", entry.artist, entry.title)
                        } else {
                            entry.song_id.clone()
                        };
                        println!(
                            "  [{}] {} | {:.1} KB | cached: {}",
                            entry.song_id,
                            label,
                            entry.size_bytes as f64 / 1024.0,
                            entry.cached_at.format("%Y-%m-%d %H:%M"),
                        );
                    }
                }
            }
            cli::CacheAction::Preload { playlist } => {
                let client = connect(&config).await?;
                let playlist_api = synoplayer::api::playlist::PlaylistApi::new(&client);
                let stream_api = StreamApi::new(&client);
                let cache = CacheManager::new(config.cache.clone());

                if !cache.is_enabled() {
                    eprintln!("Cache is disabled. Enable it in config first.");
                    std::process::exit(1);
                }

                let pl_id = resolve_playlist_id(&playlist_api, &playlist).await?;
                let pl = get_playlist_detail(&playlist_api, &pl_id).await?;
                let songs = pl.all_songs();

                if songs.is_empty() {
                    eprintln!("Playlist '{playlist}' is empty.");
                } else {
                    println!(
                        "Preloading playlist '{}' ({} songs)...",
                        pl.name,
                        songs.len()
                    );
                    let mut cached = 0usize;
                    let mut skipped = 0usize;
                    for song in songs {
                        if cache.contains(&song.id) {
                            skipped += 1;
                            continue;
                        }
                        let url = stream_api.stream_url(&song.id)?;
                        match client.http().get(&url).send().await {
                            Ok(resp) => match resp.bytes().await {
                                Ok(data) => {
                                    let hash = CacheStorage::hash_content(&data);
                                    let meta = song_meta_from_song(song);
                                    cache.put_with_meta(&song.id, &data, &hash, &meta)?;
                                    cached += 1;
                                    println!("  + {}", song.title);
                                }
                                Err(e) => eprintln!("  - {} (download error: {e})", song.title),
                            },
                            Err(e) => eprintln!("  - {} (request error: {e})", song.title),
                        }
                    }
                    println!("Done. Cached: {cached}, already cached: {skipped}.");
                }
            }
        },

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
                    for (i, song) in songs.iter().enumerate() {
                        print!("{:3}. ", i + 1);
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
                cli::PlaylistAction::Rename { name, new_name } => {
                    let pl_id = resolve_playlist_id(&api, &name).await?;
                    api.rename(&pl_id, &new_name).await?;
                    println!("Playlist '{name}' renamed to '{new_name}'.");
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
                cli::PlaylistAction::Play {
                    name,
                    from,
                    shuffle,
                    repeat,
                } => {
                    let repeat_mode = match repeat.to_lowercase().as_str() {
                        "one" => synoplayer::player::queue::RepeatMode::One,
                        "all" => synoplayer::player::queue::RepeatMode::All,
                        _ => synoplayer::player::queue::RepeatMode::Off,
                    };
                    let pl_id = resolve_playlist_id(&api, &name).await?;
                    let pl = get_playlist_detail(&api, &pl_id).await?;
                    let songs = pl.all_songs();
                    if songs.is_empty() {
                        eprintln!("Playlist '{name}' is empty.");
                    } else {
                        let mut queue: Vec<_> = songs.to_vec();
                        if shuffle {
                            use rand::seq::SliceRandom;
                            queue.shuffle(&mut rand::thread_rng());
                        }
                        let start = if shuffle {
                            0
                        } else {
                            (from.saturating_sub(1)).min(queue.len() - 1)
                        };
                        let repeat_label = match repeat_mode {
                            synoplayer::player::queue::RepeatMode::One => ", repeat: one",
                            synoplayer::player::queue::RepeatMode::All => ", repeat: all",
                            synoplayer::player::queue::RepeatMode::Off => "",
                        };
                        println!(
                            "Playing playlist '{name}' ({} songs{}{}{})",
                            queue.len(),
                            if shuffle { ", shuffled" } else { "" },
                            if !shuffle && start > 0 {
                                format!(", starting from #{}", start + 1)
                            } else {
                                String::new()
                            },
                            repeat_label,
                        );
                        let stream_api = StreamApi::new(&client);
                        let cache = CacheManager::new(config.cache.clone());
                        let engine = AudioEngine::new();
                        let history = PlayHistory::new();

                        let mut idx = start;
                        loop {
                            let song = &queue[idx];
                            let track = track_from_song(song);
                            let source = resolve_audio_source(&client, song, &cache, &config.cache)
                                .await
                                .unwrap_or_else(|_| {
                                    stream_api.stream_url(&song.id).unwrap_or_default()
                                });
                            println!(
                                "[{}/{}] {} - {} [{}]",
                                idx + 1,
                                queue.len(),
                                track.artist,
                                track.title,
                                format_duration(track.duration)
                            );
                            record_history(&history, &track);
                            engine.play_url(&source, track)?;
                            wait_for_playback(&engine).await;

                            // Advance based on repeat mode
                            match repeat_mode {
                                synoplayer::player::queue::RepeatMode::One => {
                                    // Stay on same track, loop forever
                                }
                                synoplayer::player::queue::RepeatMode::All => {
                                    idx = (idx + 1) % queue.len();
                                }
                                synoplayer::player::queue::RepeatMode::Off => {
                                    idx += 1;
                                    if idx >= queue.len() {
                                        break;
                                    }
                                }
                            }
                        }
                        println!("Playlist finished.");
                    }
                }
                cli::PlaylistAction::Import { path, name } => {
                    import_m3u_playlist(&client, &api, &path, name.as_deref()).await?;
                }
                cli::PlaylistAction::Smart {
                    name,
                    genre,
                    artist,
                    min_rating,
                    year,
                    limit,
                } => {
                    let song_api = synoplayer::api::song::SongApi::new(&client);

                    // Use server-side filtering by artist/genre via list_filtered
                    let data = song_api
                        .list_filtered(0, limit, artist.as_deref(), None, genre.as_deref())
                        .await?;

                    // Apply remaining client-side filters (rating, year)
                    let matching: Vec<&synoplayer::api::types::Song> = data
                        .songs
                        .iter()
                        .filter(|s| check_smart_filter(s, None, None, min_rating, year))
                        .collect();

                    if matching.is_empty() {
                        eprintln!("No songs match the criteria.");
                    } else {
                        let ids: Vec<&str> = matching.iter().map(|s| s.id.as_str()).collect();
                        api.create_with_songs(&name, "personal", &ids).await?;
                        println!("Smart playlist '{name}' created with {} songs.", ids.len());
                    }
                }
            }
        }

        // --- Radio ---
        cli::Commands::Radio { action } => match action {
            cli::RadioAction::List => {
                let client = connect(&config).await?;
                let api = RadioApi::new(&client);
                let data = api.list(0, 200).await?;
                if data.radios.is_empty() {
                    println!("No radio stations configured.");
                } else {
                    println!("Radio stations ({}):", data.radios.len());
                    for station in &data.radios {
                        println!("  [{}] {} — {}", station.id, station.title, station.url);
                    }
                }
            }
            cli::RadioAction::Play { station } => {
                let client = connect(&config).await?;
                let api = RadioApi::new(&client);
                let found = api.find(&station).await?;
                match found {
                    None => {
                        eprintln!("Radio station not found: '{station}'");
                        std::process::exit(1);
                    }
                    Some(s) => {
                        println!("Playing radio: {} ({})", s.title, s.url);
                        let track = TrackInfo {
                            id: s.id.clone(),
                            title: s.title.clone(),
                            artist: "Radio".to_string(),
                            album: String::new(),
                            duration: std::time::Duration::ZERO,
                        };
                        let engine = AudioEngine::new();
                        engine.play_url(&s.url, track)?;
                        loop {
                            if engine.check_finished() {
                                break;
                            }
                            tokio::time::sleep(Duration::from_millis(500)).await;
                        }
                    }
                }
            }
            cli::RadioAction::Add { name, url } => {
                let client = connect(&config).await?;
                let api = RadioApi::new(&client);
                api.add(&name, &url).await?;
                println!("Radio station '{name}' added.");
            }
        },

        // --- Download ---
        cli::Commands::Download { song_id, output } => {
            let client = connect(&config).await?;
            let song = find_song(&client, &song_id).await?;
            let track = track_from_song(&song);
            let stream_api = StreamApi::new(&client);

            println!("Downloading: {} - {} ...", track.artist, track.title);
            let bytes = stream_api.stream_bytes(&song.id).await?;

            let out_path = if let Some(ref p) = output {
                std::path::PathBuf::from(p)
            } else {
                let filename = format!(
                    "{} - {}.mp3",
                    if track.artist.is_empty() {
                        "Unknown"
                    } else {
                        &track.artist
                    },
                    track.title
                );
                std::path::PathBuf::from(&filename)
            };

            std::fs::write(&out_path, &bytes)?;
            println!(
                "Saved to {} ({:.1} MB)",
                out_path.display(),
                bytes.len() as f64 / 1_048_576.0
            );
        }

        // --- History ---
        cli::Commands::History { action } => {
            let history = PlayHistory::new();
            match action {
                Some(cli::HistoryAction::Clear) => {
                    history.clear()?;
                    println!("Playback history cleared.");
                }
                None => {
                    let entries = history.list(50);
                    if entries.is_empty() {
                        println!("No playback history.");
                    } else {
                        println!("Recent history ({} entries):", entries.len());
                        for entry in &entries {
                            println!(
                                "  {} - {} [{}] ({})",
                                entry.artist, entry.title, entry.album, entry.played_at
                            );
                        }
                    }
                }
            }
        }

        // --- TUI ---
        cli::Commands::Tui => {
            let client = connect(&config).await?;
            synoplayer::tui::run(client, config).await?;
        }

        // --- Shuffle / Repeat ---
        cli::Commands::Shuffle { mode } => {
            eprintln!("Shuffle {mode}: no active player session. Start playback first.");
        }
        cli::Commands::Repeat { mode } => {
            eprintln!("Repeat {mode}: no active player session. Start playback first.");
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

/// Find a song by ID or search by name.
async fn find_song(
    client: &SynoClient,
    target: &str,
) -> anyhow::Result<synoplayer::api::types::Song> {
    let api = SongApi::new(client);
    if target.starts_with("music_") {
        Ok(api.get_info(target).await?)
    } else {
        let data = api.search(target, 0, 1).await?;
        data.songs.into_iter().next().ok_or_else(|| {
            synoplayer::error::SynoError::Player(format!("No song found: {target}")).into()
        })
    }
}

// Shared playback helpers are in synoplayer::playback

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

/// Check if a song matches smart playlist filter criteria.
fn check_smart_filter(
    song: &synoplayer::api::types::Song,
    genre: Option<&str>,
    artist: Option<&str>,
    min_rating: Option<i32>,
    year: Option<i32>,
) -> bool {
    let add = match &song.additional {
        Some(a) => a,
        None => {
            return genre.is_none() && artist.is_none() && min_rating.is_none() && year.is_none();
        }
    };

    if let Some(g) = genre {
        let song_genre = add
            .song_tag
            .as_ref()
            .map(|t| t.genre.as_str())
            .unwrap_or("");
        if !song_genre.to_lowercase().contains(&g.to_lowercase()) {
            return false;
        }
    }
    if let Some(a) = artist {
        let song_artist = add
            .song_tag
            .as_ref()
            .map(|t| t.artist.as_str())
            .unwrap_or("");
        if !song_artist.to_lowercase().contains(&a.to_lowercase()) {
            return false;
        }
    }
    if let Some(min) = min_rating {
        let rating = add.song_rating.as_ref().map(|r| r.rating).unwrap_or(0);
        if rating < min {
            return false;
        }
    }
    if let Some(y) = year {
        let song_year = add.song_tag.as_ref().map(|t| t.year).unwrap_or(0);
        if song_year != y {
            return false;
        }
    }
    true
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
