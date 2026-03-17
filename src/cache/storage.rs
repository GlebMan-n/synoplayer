use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// Low-level disk I/O for cached audio files.
///
/// Responsible only for reading/writing files and computing hashes.
/// Does not make eviction or TTL decisions.
pub struct CacheStorage {
    base_path: PathBuf,
}

impl CacheStorage {
    pub fn new(base_path: PathBuf) -> Self {
        Self { base_path }
    }

    pub fn base_path(&self) -> &Path {
        &self.base_path
    }

    /// Compute the file path for a song ID.
    pub fn audio_path(&self, song_id: &str) -> PathBuf {
        let hash = Self::hash_id(song_id);
        self.base_path.join(format!("{hash}.audio"))
    }

    /// Compute the metadata file path for a song ID.
    pub fn meta_path(&self, song_id: &str) -> PathBuf {
        let hash = Self::hash_id(song_id);
        self.base_path.join(format!("{hash}.meta"))
    }

    /// SHA-256 hash of a song ID (used as filename).
    pub fn hash_id(song_id: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(song_id.as_bytes());
        let result = hasher.finalize();
        format!("{:x}", result)[..16].to_string()
    }

    /// SHA-256 hash of file content (for integrity verification).
    pub fn hash_content(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        format!("{:x}", hasher.finalize())
    }

    /// Write audio data to cache.
    pub fn write(&self, song_id: &str, data: &[u8]) -> std::io::Result<PathBuf> {
        std::fs::create_dir_all(&self.base_path)?;
        let path = self.audio_path(song_id);
        std::fs::write(&path, data)?;
        Ok(path)
    }

    /// Read audio data from cache.
    pub fn read(&self, song_id: &str) -> std::io::Result<Vec<u8>> {
        let path = self.audio_path(song_id);
        std::fs::read(path)
    }

    /// Check if a cached file exists.
    pub fn exists(&self, song_id: &str) -> bool {
        self.audio_path(song_id).exists()
    }

    /// Delete a cached file.
    pub fn delete(&self, song_id: &str) -> std::io::Result<()> {
        let audio = self.audio_path(song_id);
        let meta = self.meta_path(song_id);
        if audio.exists() {
            std::fs::remove_file(audio)?;
        }
        if meta.exists() {
            std::fs::remove_file(meta)?;
        }
        Ok(())
    }

    /// Total size of all files in cache directory (bytes).
    pub fn total_size(&self) -> std::io::Result<u64> {
        if !self.base_path.exists() {
            return Ok(0);
        }
        let mut total = 0u64;
        for entry in std::fs::read_dir(&self.base_path)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                total += entry.metadata()?.len();
            }
        }
        Ok(total)
    }

    /// Count of .audio files in cache.
    pub fn file_count(&self) -> std::io::Result<usize> {
        if !self.base_path.exists() {
            return Ok(0);
        }
        let count = std::fs::read_dir(&self.base_path)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "audio"))
            .count();
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_id_is_deterministic() {
        let h1 = CacheStorage::hash_id("music_123");
        let h2 = CacheStorage::hash_id("music_123");
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 16);
    }

    #[test]
    fn hash_id_differs_for_different_ids() {
        let h1 = CacheStorage::hash_id("music_1");
        let h2 = CacheStorage::hash_id("music_2");
        assert_ne!(h1, h2);
    }

    #[test]
    fn write_and_read_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let storage = CacheStorage::new(dir.path().to_path_buf());
        let data = b"fake audio data";
        storage.write("song_1", data).unwrap();
        let read_back = storage.read("song_1").unwrap();
        assert_eq!(read_back, data);
    }

    #[test]
    fn exists_returns_true_after_write() {
        let dir = tempfile::tempdir().unwrap();
        let storage = CacheStorage::new(dir.path().to_path_buf());
        assert!(!storage.exists("song_1"));
        storage.write("song_1", b"data").unwrap();
        assert!(storage.exists("song_1"));
    }

    #[test]
    fn delete_removes_file() {
        let dir = tempfile::tempdir().unwrap();
        let storage = CacheStorage::new(dir.path().to_path_buf());
        storage.write("song_1", b"data").unwrap();
        storage.delete("song_1").unwrap();
        assert!(!storage.exists("song_1"));
    }

    #[test]
    fn total_size_sums_files() {
        let dir = tempfile::tempdir().unwrap();
        let storage = CacheStorage::new(dir.path().to_path_buf());
        storage.write("s1", &vec![0u8; 1000]).unwrap();
        storage.write("s2", &vec![0u8; 2000]).unwrap();
        let size = storage.total_size().unwrap();
        assert_eq!(size, 3000);
    }

    #[test]
    fn file_count_counts_audio_files() {
        let dir = tempfile::tempdir().unwrap();
        let storage = CacheStorage::new(dir.path().to_path_buf());
        storage.write("s1", b"data").unwrap();
        storage.write("s2", b"data").unwrap();
        assert_eq!(storage.file_count().unwrap(), 2);
    }

    #[test]
    fn read_nonexistent_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let storage = CacheStorage::new(dir.path().to_path_buf());
        assert!(storage.read("nonexistent").is_err());
    }

    #[test]
    fn content_hash_detects_changes() {
        let h1 = CacheStorage::hash_content(b"original");
        let h2 = CacheStorage::hash_content(b"modified");
        assert_ne!(h1, h2);
    }
}
