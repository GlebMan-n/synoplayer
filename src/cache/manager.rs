use std::path::PathBuf;

use crate::cache::storage::CacheStorage;
use crate::config::model::CacheConfig;

/// Cache status information.
#[derive(Debug)]
pub struct CacheStatus {
    pub enabled: bool,
    pub file_count: usize,
    pub total_size_bytes: u64,
    pub max_size_bytes: u64,
    pub path: PathBuf,
}

/// Song metadata stored alongside cached audio.
#[derive(Debug, Clone, Default)]
pub struct SongMeta {
    pub title: String,
    pub artist: String,
    pub album: String,
}

/// Metadata for a single cached entry.
#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub song_id: String,
    pub cached_at: chrono::DateTime<chrono::Utc>,
    pub last_accessed: chrono::DateTime<chrono::Utc>,
    pub size_bytes: u64,
    pub title: String,
    pub artist: String,
    pub album: String,
}

/// Manages audio file caching with LRU eviction and TTL.
///
/// Responsible for cache policy (when to evict, what to keep).
/// Delegates actual I/O to CacheStorage (SoC).
pub struct CacheManager {
    config: CacheConfig,
    storage: CacheStorage,
}

impl CacheManager {
    pub fn new(config: CacheConfig) -> Self {
        let storage = CacheStorage::new(config.path.clone());
        Self { config, storage }
    }

    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Store audio data in cache. Triggers LRU eviction if needed.
    pub fn put(&self, song_id: &str, data: &[u8], content_hash: &str) -> crate::error::Result<()> {
        self.put_inner(song_id, data, content_hash, None)
    }

    /// Store audio data with song metadata for offline display.
    pub fn put_with_meta(
        &self,
        song_id: &str,
        data: &[u8],
        content_hash: &str,
        meta: &SongMeta,
    ) -> crate::error::Result<()> {
        self.put_inner(song_id, data, content_hash, Some(meta))
    }

    fn put_inner(
        &self,
        song_id: &str,
        data: &[u8],
        content_hash: &str,
        song_meta: Option<&SongMeta>,
    ) -> crate::error::Result<()> {
        if !self.config.enabled {
            return Ok(());
        }

        // Write data
        self.storage
            .write(song_id, data)
            .map_err(|e| crate::error::SynoError::Cache(e.to_string()))?;

        // Write meta
        let mut meta = serde_json::json!({
            "song_id": song_id,
            "sha256": content_hash,
            "cached_at": chrono::Utc::now().to_rfc3339(),
            "last_accessed": chrono::Utc::now().to_rfc3339(),
            "size_bytes": data.len(),
        });
        if let Some(sm) = song_meta {
            meta["title"] = serde_json::Value::String(sm.title.clone());
            meta["artist"] = serde_json::Value::String(sm.artist.clone());
            meta["album"] = serde_json::Value::String(sm.album.clone());
        }
        let meta_path = self.storage.meta_path(song_id);
        std::fs::write(meta_path, serde_json::to_string(&meta).unwrap_or_default())
            .map_err(|e| crate::error::SynoError::Cache(e.to_string()))?;

        // Evict if over limit
        self.evict_if_needed()?;

        Ok(())
    }

    /// Retrieve audio data from cache. Returns None on miss or integrity failure.
    pub fn get(&self, song_id: &str) -> crate::error::Result<Option<Vec<u8>>> {
        if !self.config.enabled || !self.storage.exists(song_id) {
            return Ok(None);
        }

        let data = match self.storage.read(song_id) {
            Ok(d) => d,
            Err(_) => return Ok(None),
        };

        // Integrity check
        if self.config.verify_integrity {
            let actual_hash = CacheStorage::hash_content(&data);
            let meta_path = self.storage.meta_path(song_id);
            if let Ok(meta_str) = std::fs::read_to_string(&meta_path)
                && let Ok(meta) = serde_json::from_str::<serde_json::Value>(&meta_str)
                && let Some(expected_hash) = meta["sha256"].as_str()
                && actual_hash != expected_hash
            {
                // Corrupted — delete and return None
                let _ = self.storage.delete(song_id);
                return Ok(None);
            }

            // Update last_accessed in meta
            self.touch_meta(song_id);
        }

        Ok(Some(data))
    }

    /// Check if a song is in cache.
    pub fn contains(&self, song_id: &str) -> bool {
        self.config.enabled && self.storage.exists(song_id)
    }

    /// Get the file path for a cached song (for direct playback).
    pub fn file_path(&self, song_id: &str) -> PathBuf {
        self.storage.audio_path(song_id)
    }

    /// Cache status (file count, total size, etc.).
    pub fn status(&self) -> crate::error::Result<CacheStatus> {
        let file_count = self
            .storage
            .file_count()
            .map_err(|e| crate::error::SynoError::Cache(e.to_string()))?;
        let total_size_bytes = self
            .storage
            .total_size()
            .map_err(|e| crate::error::SynoError::Cache(e.to_string()))?;

        Ok(CacheStatus {
            enabled: self.config.enabled,
            file_count,
            total_size_bytes,
            max_size_bytes: self.config.max_size_mb * 1024 * 1024,
            path: self.config.path.clone(),
        })
    }

    /// Remove all cached files.
    pub fn clear(&self) -> crate::error::Result<()> {
        if self.storage.base_path().exists() {
            std::fs::remove_dir_all(self.storage.base_path())
                .map_err(|e| crate::error::SynoError::Cache(e.to_string()))?;
        }
        Ok(())
    }

    /// Remove expired entries (older than ttl_days).
    pub fn cleanup_expired(&self) -> crate::error::Result<()> {
        if self.config.ttl_days > 0 {
            self.clear_older_than_days(self.config.ttl_days)?;
        }
        Ok(())
    }

    /// List all cache entries with metadata.
    pub fn list_entries(&self) -> crate::error::Result<Vec<CacheEntry>> {
        if !self.storage.base_path().exists() {
            return Ok(vec![]);
        }
        let mut entries = Vec::new();
        let read_dir = std::fs::read_dir(self.storage.base_path())
            .map_err(|e| crate::error::SynoError::Cache(e.to_string()))?;
        for entry in read_dir {
            let entry = entry.map_err(|e| crate::error::SynoError::Cache(e.to_string()))?;
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "meta")
                && let Ok(meta_str) = std::fs::read_to_string(&path)
                && let Ok(meta) = serde_json::from_str::<serde_json::Value>(&meta_str)
            {
                let song_id = meta["song_id"].as_str().unwrap_or("").to_string();
                let cached_at = meta["cached_at"]
                    .as_str()
                    .and_then(|s| s.parse::<chrono::DateTime<chrono::Utc>>().ok())
                    .unwrap_or_default();
                let last_accessed = meta["last_accessed"]
                    .as_str()
                    .and_then(|s| s.parse::<chrono::DateTime<chrono::Utc>>().ok())
                    .unwrap_or(cached_at);
                let size_bytes = meta["size_bytes"].as_u64().unwrap_or(0);
                let title = meta["title"].as_str().unwrap_or("").to_string();
                let artist = meta["artist"].as_str().unwrap_or("").to_string();
                let album = meta["album"].as_str().unwrap_or("").to_string();
                if !song_id.is_empty() {
                    entries.push(CacheEntry {
                        song_id,
                        cached_at,
                        last_accessed,
                        size_bytes,
                        title,
                        artist,
                        album,
                    });
                }
            }
        }
        Ok(entries)
    }

    /// Get metadata for a single cached entry (for offline display).
    pub fn get_entry_meta(&self, song_id: &str) -> Option<CacheEntry> {
        let meta_path = self.storage.meta_path(song_id);
        let meta_str = std::fs::read_to_string(meta_path).ok()?;
        let meta: serde_json::Value = serde_json::from_str(&meta_str).ok()?;
        let cached_at = meta["cached_at"]
            .as_str()
            .and_then(|s| s.parse::<chrono::DateTime<chrono::Utc>>().ok())
            .unwrap_or_default();
        Some(CacheEntry {
            song_id: song_id.to_string(),
            cached_at,
            last_accessed: meta["last_accessed"]
                .as_str()
                .and_then(|s| s.parse::<chrono::DateTime<chrono::Utc>>().ok())
                .unwrap_or(cached_at),
            size_bytes: meta["size_bytes"].as_u64().unwrap_or(0),
            title: meta["title"].as_str().unwrap_or("").to_string(),
            artist: meta["artist"].as_str().unwrap_or("").to_string(),
            album: meta["album"].as_str().unwrap_or("").to_string(),
        })
    }

    /// Remove entries cached more than `days` days ago. Returns count removed.
    pub fn clear_older_than_days(&self, days: u32) -> crate::error::Result<usize> {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);
        let entries = self.list_entries()?;
        let mut removed = 0;
        for entry in entries {
            if entry.cached_at < cutoff {
                let _ = self.storage.delete(&entry.song_id);
                removed += 1;
            }
        }
        Ok(removed)
    }

    fn evict_if_needed(&self) -> crate::error::Result<()> {
        let max_bytes = self.config.max_size_mb * 1024 * 1024;
        let current = self
            .storage
            .total_size()
            .map_err(|e| crate::error::SynoError::Cache(e.to_string()))?;

        if current <= max_bytes {
            return Ok(());
        }

        let target_bytes = (max_bytes as f64 * 0.9) as u64;
        let mut entries = self.list_entries()?;
        entries.sort_by_key(|e| e.last_accessed);

        let mut current_size = current;
        for entry in entries {
            if current_size <= target_bytes {
                break;
            }
            let size = entry.size_bytes;
            if self.storage.delete(&entry.song_id).is_ok() {
                current_size = current_size.saturating_sub(size);
            }
        }

        Ok(())
    }

    fn touch_meta(&self, song_id: &str) {
        let meta_path = self.storage.meta_path(song_id);
        if let Ok(meta_str) = std::fs::read_to_string(&meta_path)
            && let Ok(mut meta) = serde_json::from_str::<serde_json::Value>(&meta_str)
        {
            meta["last_accessed"] = serde_json::Value::String(chrono::Utc::now().to_rfc3339());
            let _ = std::fs::write(&meta_path, serde_json::to_string(&meta).unwrap_or_default());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config(dir: &std::path::Path) -> CacheConfig {
        CacheConfig {
            enabled: true,
            path: dir.to_path_buf(),
            max_size_mb: 100,
            ttl_days: 30,
            cache_on_play: true,
            preload_playlist: false,
            transcode_before_cache: false,
            verify_integrity: true,
        }
    }

    #[test]
    fn store_and_retrieve() {
        let dir = tempfile::tempdir().unwrap();
        let cache = CacheManager::new(test_config(dir.path()));
        let data = b"fake audio data";
        let hash = CacheStorage::hash_content(data);
        cache.put("song_1", data, &hash).unwrap();
        assert!(cache.contains("song_1"));
        let retrieved = cache.get("song_1").unwrap().unwrap();
        assert_eq!(retrieved, data);
    }

    #[test]
    fn miss_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let cache = CacheManager::new(test_config(dir.path()));
        assert!(cache.get("nonexistent").unwrap().is_none());
    }

    #[test]
    fn disabled_cache_does_not_store() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = test_config(dir.path());
        config.enabled = false;
        let cache = CacheManager::new(config);
        let hash = CacheStorage::hash_content(b"data");
        cache.put("song_1", b"data", &hash).unwrap();
        assert!(!cache.contains("song_1"));
    }

    #[test]
    fn integrity_check_detects_corruption() {
        let dir = tempfile::tempdir().unwrap();
        let cache = CacheManager::new(test_config(dir.path()));
        let data = b"original data";
        let hash = CacheStorage::hash_content(data);
        cache.put("song_1", data, &hash).unwrap();

        // Corrupt the file
        let file_path = cache.file_path("song_1");
        std::fs::write(&file_path, b"corrupted!").unwrap();

        // get should return None and delete the entry
        assert!(cache.get("song_1").unwrap().is_none());
        assert!(!cache.contains("song_1"));
    }

    #[test]
    fn status_reports_correct_stats() {
        let dir = tempfile::tempdir().unwrap();
        let cache = CacheManager::new(test_config(dir.path()));
        let hash1 = CacheStorage::hash_content(&vec![0u8; 1000]);
        let hash2 = CacheStorage::hash_content(&vec![0u8; 2000]);
        cache.put("s1", &vec![0u8; 1000], &hash1).unwrap();
        cache.put("s2", &vec![0u8; 2000], &hash2).unwrap();

        let status = cache.status().unwrap();
        assert_eq!(status.file_count, 2);
        assert!(status.total_size_bytes >= 3000); // audio + meta files
        assert_eq!(status.max_size_bytes, 100 * 1024 * 1024);
    }

    #[test]
    fn clear_removes_all() {
        let dir = tempfile::tempdir().unwrap();
        let cache = CacheManager::new(test_config(dir.path()));
        let hash = CacheStorage::hash_content(b"data");
        cache.put("s1", b"data", &hash).unwrap();
        cache.put("s2", b"data", &hash).unwrap();
        cache.clear().unwrap();
        assert!(!cache.contains("s1"));
        assert!(!cache.contains("s2"));
    }

    #[test]
    fn lru_eviction_removes_oldest() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = test_config(dir.path());
        config.max_size_mb = 1; // 1 MB
        let cache = CacheManager::new(config);

        // Add files that exceed 1MB total
        let big_data = vec![0u8; 400_000];
        let hash = CacheStorage::hash_content(&big_data);
        cache.put("s1", &big_data, &hash).unwrap();
        cache.put("s2", &big_data, &hash).unwrap();
        cache.put("s3", &big_data, &hash).unwrap(); // total ~1.2MB > 1MB

        // After eviction, at least one of the oldest entries should be gone
        let remaining = [
            cache.contains("s1"),
            cache.contains("s2"),
            cache.contains("s3"),
        ]
        .iter()
        .filter(|&&b| b)
        .count();
        assert!(
            remaining < 3,
            "eviction should have removed at least one entry"
        );
    }

    #[test]
    fn list_entries_returns_all_cached() {
        let dir = tempfile::tempdir().unwrap();
        let cache = CacheManager::new(test_config(dir.path()));
        let hash1 = CacheStorage::hash_content(b"data1");
        let hash2 = CacheStorage::hash_content(b"data2");
        cache.put("song_a", b"data1", &hash1).unwrap();
        cache.put("song_b", b"data2", &hash2).unwrap();

        let entries = cache.list_entries().unwrap();
        assert_eq!(entries.len(), 2);
        let ids: Vec<&str> = entries.iter().map(|e| e.song_id.as_str()).collect();
        assert!(ids.contains(&"song_a"));
        assert!(ids.contains(&"song_b"));
    }

    #[test]
    fn put_with_meta_stores_song_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let cache = CacheManager::new(test_config(dir.path()));
        let data = b"audio data";
        let hash = CacheStorage::hash_content(data);
        let meta = SongMeta {
            title: "Comfortably Numb".to_string(),
            artist: "Pink Floyd".to_string(),
            album: "The Wall".to_string(),
        };
        cache.put_with_meta("song_1", data, &hash, &meta).unwrap();

        let entries = cache.list_entries().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "Comfortably Numb");
        assert_eq!(entries[0].artist, "Pink Floyd");
        assert_eq!(entries[0].album, "The Wall");
    }

    #[test]
    fn get_entry_meta_returns_stored_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let cache = CacheManager::new(test_config(dir.path()));
        let data = b"audio";
        let hash = CacheStorage::hash_content(data);
        let meta = SongMeta {
            title: "Time".to_string(),
            artist: "Pink Floyd".to_string(),
            album: "DSOTM".to_string(),
        };
        cache.put_with_meta("song_2", data, &hash, &meta).unwrap();

        let entry = cache.get_entry_meta("song_2").unwrap();
        assert_eq!(entry.song_id, "song_2");
        assert_eq!(entry.title, "Time");
        assert_eq!(entry.artist, "Pink Floyd");
        assert_eq!(entry.album, "DSOTM");
    }

    #[test]
    fn get_entry_meta_returns_none_for_missing() {
        let dir = tempfile::tempdir().unwrap();
        let cache = CacheManager::new(test_config(dir.path()));
        assert!(cache.get_entry_meta("nonexistent").is_none());
    }

    #[test]
    fn put_without_meta_has_empty_metadata_fields() {
        let dir = tempfile::tempdir().unwrap();
        let cache = CacheManager::new(test_config(dir.path()));
        let hash = CacheStorage::hash_content(b"data");
        cache.put("song_x", b"data", &hash).unwrap();

        let entries = cache.list_entries().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "");
        assert_eq!(entries[0].artist, "");
    }

    #[test]
    fn cleanup_expired_removes_old_entries() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = test_config(dir.path());
        config.ttl_days = 10;
        let cache = CacheManager::new(config);
        let hash = CacheStorage::hash_content(b"data");
        cache.put("song_fresh", b"data", &hash).unwrap();
        cache.put("song_old", b"data", &hash).unwrap();

        // Backdate song_old to 20 days ago
        let meta_path = cache.storage.meta_path("song_old");
        let old_date = (chrono::Utc::now() - chrono::Duration::days(20)).to_rfc3339();
        let meta = serde_json::json!({
            "song_id": "song_old",
            "sha256": hash,
            "cached_at": old_date,
            "last_accessed": old_date,
            "size_bytes": 4u64,
        });
        std::fs::write(meta_path, serde_json::to_string(&meta).unwrap()).unwrap();

        cache.cleanup_expired().unwrap();
        assert!(!cache.contains("song_old"));
        assert!(cache.contains("song_fresh"));
    }

    #[test]
    fn file_path_returns_local_path() {
        let dir = tempfile::tempdir().unwrap();
        let cache = CacheManager::new(test_config(dir.path()));
        let hash = CacheStorage::hash_content(b"audio");
        cache.put("song_1", b"audio", &hash).unwrap();

        let path = cache.file_path("song_1");
        assert!(path.exists());
        assert!(path.to_str().unwrap().ends_with(".audio"));
    }

    #[test]
    fn clear_older_than_days_removes_old_entries() {
        let dir = tempfile::tempdir().unwrap();
        let cache = CacheManager::new(test_config(dir.path()));
        let hash = CacheStorage::hash_content(b"data");
        cache.put("old_song", b"data", &hash).unwrap();
        cache.put("new_song", b"data", &hash).unwrap();

        // Backdate the cached_at of old_song so it looks 60 days old
        let meta_path = cache.storage.meta_path("old_song");
        let old_date = (chrono::Utc::now() - chrono::Duration::days(60)).to_rfc3339();
        let meta = serde_json::json!({
            "song_id": "old_song",
            "sha256": hash,
            "cached_at": old_date,
            "last_accessed": old_date,
            "size_bytes": 4u64,
        });
        std::fs::write(meta_path, serde_json::to_string(&meta).unwrap()).unwrap();

        let removed = cache.clear_older_than_days(30).unwrap();
        assert_eq!(removed, 1);
        assert!(!cache.contains("old_song"));
        assert!(cache.contains("new_song"));
    }
}
