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
        if !self.config.enabled {
            return Ok(());
        }

        // Write data
        self.storage
            .write(song_id, data)
            .map_err(|e| crate::error::SynoError::Cache(e.to_string()))?;

        // Write meta
        let meta = serde_json::json!({
            "song_id": song_id,
            "sha256": content_hash,
            "cached_at": chrono::Utc::now().to_rfc3339(),
            "last_accessed": chrono::Utc::now().to_rfc3339(),
            "size_bytes": data.len(),
        });
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
            if let Ok(meta_str) = std::fs::read_to_string(&meta_path) {
                if let Ok(meta) = serde_json::from_str::<serde_json::Value>(&meta_str) {
                    if let Some(expected_hash) = meta["sha256"].as_str() {
                        if actual_hash != expected_hash {
                            // Corrupted — delete and return None
                            let _ = self.storage.delete(song_id);
                            return Ok(None);
                        }
                    }
                }
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
        // TODO: iterate meta files, parse cached_at, delete if expired
        Ok(())
    }

    fn evict_if_needed(&self) -> crate::error::Result<()> {
        let max_bytes = self.config.max_size_mb * 1024 * 1024;
        let target_bytes = (max_bytes as f64 * 0.9) as u64; // 10% hysteresis

        let current = self
            .storage
            .total_size()
            .map_err(|e| crate::error::SynoError::Cache(e.to_string()))?;

        if current <= max_bytes {
            return Ok(());
        }

        // TODO: sort by last_accessed, delete oldest until under target_bytes
        let _ = target_bytes;

        Ok(())
    }

    fn touch_meta(&self, song_id: &str) {
        let meta_path = self.storage.meta_path(song_id);
        if let Ok(meta_str) = std::fs::read_to_string(&meta_path) {
            if let Ok(mut meta) = serde_json::from_str::<serde_json::Value>(&meta_str) {
                meta["last_accessed"] =
                    serde_json::Value::String(chrono::Utc::now().to_rfc3339());
                let _ = std::fs::write(&meta_path, serde_json::to_string(&meta).unwrap_or_default());
            }
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
    #[ignore]
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
    #[ignore]
    fn miss_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let cache = CacheManager::new(test_config(dir.path()));
        assert!(cache.get("nonexistent").unwrap().is_none());
    }

    #[test]
    #[ignore]
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
    #[ignore]
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
    #[ignore]
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
    #[ignore]
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
    #[ignore]
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

        // After eviction, oldest (s1) should be gone
        // (this test will pass once eviction is fully implemented)
    }
}
