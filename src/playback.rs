//! Shared playback utilities used by both CLI and TUI composition roots.

use std::time::Duration;

use crate::api::client::SynoClient;
use crate::api::stream::StreamApi;
use crate::api::types::Song;
use crate::cache::manager::{CacheManager, SongMeta};
use crate::cache::storage::CacheStorage;
use crate::config::model::CacheConfig;
use crate::history::{HistoryEntry, PlayHistory};
use crate::player::engine::AudioEngine;
use crate::player::state::TrackInfo;

/// Build TrackInfo from a Song's metadata.
pub fn track_from_song(song: &Song) -> TrackInfo {
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

/// Extract SongMeta from a Song for caching.
pub fn song_meta_from_song(song: &Song) -> SongMeta {
    if let Some(ref add) = song.additional {
        let tag = add.song_tag.as_ref();
        SongMeta {
            title: tag
                .map(|t| t.title.clone())
                .unwrap_or_else(|| song.title.clone()),
            artist: tag.map(|t| t.artist.clone()).unwrap_or_default(),
            album: tag.map(|t| t.album.clone()).unwrap_or_default(),
        }
    } else {
        SongMeta {
            title: song.title.clone(),
            ..Default::default()
        }
    }
}

/// Build a TrackInfo from cache metadata (for offline playback).
pub fn track_from_cache(cache: &CacheManager, song_id: &str) -> TrackInfo {
    if let Some(entry) = cache.get_entry_meta(song_id) {
        TrackInfo {
            id: song_id.to_string(),
            title: if entry.title.is_empty() {
                song_id.to_string()
            } else {
                entry.title
            },
            artist: if entry.artist.is_empty() {
                "Unknown".to_string()
            } else {
                entry.artist
            },
            album: entry.album,
            duration: Duration::ZERO,
        }
    } else {
        TrackInfo {
            id: song_id.to_string(),
            title: song_id.to_string(),
            artist: "Unknown".to_string(),
            album: String::new(),
            duration: Duration::ZERO,
        }
    }
}

/// Resolve the audio source for playback: cache file or stream URL.
pub async fn resolve_audio_source(
    client: &SynoClient,
    song: &Song,
    cache: &CacheManager,
    cache_config: &CacheConfig,
) -> anyhow::Result<String> {
    // Cache hit — play from local file
    if cache.contains(&song.id) {
        let path = cache.file_path(&song.id);
        cache.get(&song.id)?; // touch last_accessed
        tracing::debug!("Cache HIT for {}", song.id);
        return Ok(path.to_string_lossy().to_string());
    }

    let stream_api = StreamApi::new(client);
    let url = stream_api.stream_url(&song.id)?;

    // Cache miss + cache_on_play — download, cache, return local path
    if cache_config.enabled && cache_config.cache_on_play {
        tracing::debug!("Cache MISS for {} — downloading for cache", song.id);
        match client.http().get(&url).send().await {
            Ok(resp) => match resp.bytes().await {
                Ok(data) => {
                    let hash = CacheStorage::hash_content(&data);
                    let meta = song_meta_from_song(song);
                    cache.put_with_meta(&song.id, &data, &hash, &meta)?;
                    let path = cache.file_path(&song.id);
                    return Ok(path.to_string_lossy().to_string());
                }
                Err(e) => tracing::warn!("Download failed for cache: {e}, falling back to stream"),
            },
            Err(e) => tracing::warn!("Request failed for cache: {e}, falling back to stream"),
        }
    }

    // No caching or cache failed — stream directly
    Ok(url)
}

/// Record a track in playback history.
pub fn record_history(history: &PlayHistory, track: &TrackInfo) {
    let entry = HistoryEntry {
        song_id: track.id.clone(),
        title: track.title.clone(),
        artist: track.artist.clone(),
        album: track.album.clone(),
        played_at: chrono::Utc::now().to_rfc3339(),
    };
    if let Err(e) = history.add(entry) {
        tracing::warn!("Failed to record history: {e}");
    }
}

/// Wait for playback subprocess to finish.
/// Returns Err if the player subprocess failed (e.g. bad device).
pub async fn wait_for_playback(
    engine: &AudioEngine,
) -> Result<(), String> {
    loop {
        match engine.check_finished() {
            Ok(true) => return Ok(()),
            Err(msg) => return Err(msg),
            Ok(false) => {}
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

/// Format a duration as M:SS.
pub fn format_duration(d: Duration) -> String {
    let secs = d.as_secs();
    let m = secs / 60;
    let s = secs % 60;
    format!("{m}:{s:02}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::types::{Song, SongAdditional, SongAudio, SongTag};

    #[test]
    fn format_duration_zero() {
        assert_eq!(format_duration(Duration::ZERO), "0:00");
    }

    #[test]
    fn format_duration_minutes_seconds() {
        assert_eq!(format_duration(Duration::from_secs(185)), "3:05");
        assert_eq!(format_duration(Duration::from_secs(60)), "1:00");
        assert_eq!(format_duration(Duration::from_secs(3661)), "61:01");
    }

    #[test]
    fn track_from_song_with_metadata() {
        let song = Song {
            id: "music_1".to_string(),
            title: "fallback".to_string(),
            path: String::new(),
            additional: Some(SongAdditional {
                song_tag: Some(SongTag {
                    title: "Real Title".to_string(),
                    artist: "Artist".to_string(),
                    album: "Album".to_string(),
                    album_artist: String::new(),
                    composer: String::new(),
                    genre: String::new(),
                    year: 2024,
                    track: 1,
                    disc: 1,
                    comment: String::new(),
                }),
                song_audio: Some(SongAudio {
                    duration: 240,
                    bitrate: 320,
                    codec: "mp3".to_string(),
                    container: String::new(),
                    frequency: 44100,
                    channel: 2,
                    lossless: false,
                    filesize: 0,
                }),
                song_rating: None,
            }),
        };
        let track = track_from_song(&song);
        assert_eq!(track.id, "music_1");
        assert_eq!(track.title, "Real Title");
        assert_eq!(track.artist, "Artist");
        assert_eq!(track.album, "Album");
        assert_eq!(track.duration, Duration::from_secs(240));
    }

    #[test]
    fn track_from_song_without_additional() {
        let song = Song {
            id: "music_2".to_string(),
            title: "Basic Song".to_string(),
            path: String::new(),
            additional: None,
        };
        let track = track_from_song(&song);
        assert_eq!(track.title, "Basic Song");
        assert!(track.artist.is_empty());
        assert_eq!(track.duration, Duration::ZERO);
    }

    #[test]
    fn song_meta_extracts_tag_fields() {
        let song = Song {
            id: "music_3".to_string(),
            title: "fallback".to_string(),
            path: String::new(),
            additional: Some(SongAdditional {
                song_tag: Some(SongTag {
                    title: "Tagged".to_string(),
                    artist: "Art".to_string(),
                    album: "Alb".to_string(),
                    album_artist: String::new(),
                    composer: String::new(),
                    genre: String::new(),
                    year: 0,
                    track: 0,
                    disc: 0,
                    comment: String::new(),
                }),
                song_audio: None,
                song_rating: None,
            }),
        };
        let meta = song_meta_from_song(&song);
        assert_eq!(meta.title, "Tagged");
        assert_eq!(meta.artist, "Art");
        assert_eq!(meta.album, "Alb");
    }
}
