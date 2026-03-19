use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// A single playback history record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub song_id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub played_at: String,
}

/// Manages local playback history stored as a JSON file.
pub struct PlayHistory {
    path: PathBuf,
    max_entries: usize,
}

const DEFAULT_MAX_ENTRIES: usize = 500;

impl Default for PlayHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl PlayHistory {
    pub fn new() -> Self {
        let path = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("synoplayer")
            .join("history.json");
        Self {
            path,
            max_entries: DEFAULT_MAX_ENTRIES,
        }
    }

    /// Record a played track.
    pub fn add(&self, entry: HistoryEntry) -> std::io::Result<()> {
        let mut entries = self.load_entries();
        entries.push(entry);

        // Trim to max size
        if entries.len() > self.max_entries {
            let excess = entries.len() - self.max_entries;
            entries.drain(..excess);
        }

        self.save_entries(&entries)
    }

    /// Get recent history entries (newest first).
    pub fn list(&self, limit: usize) -> Vec<HistoryEntry> {
        let entries = self.load_entries();
        entries.into_iter().rev().take(limit).collect()
    }

    /// Clear all history.
    pub fn clear(&self) -> std::io::Result<()> {
        if self.path.exists() {
            std::fs::remove_file(&self.path)?;
        }
        Ok(())
    }

    /// Total number of entries.
    pub fn count(&self) -> usize {
        self.load_entries().len()
    }

    fn load_entries(&self) -> Vec<HistoryEntry> {
        if !self.path.exists() {
            return vec![];
        }
        let content = match std::fs::read_to_string(&self.path) {
            Ok(c) => c,
            Err(_) => return vec![],
        };
        serde_json::from_str(&content).unwrap_or_default()
    }

    fn save_entries(&self, entries: &[HistoryEntry]) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string(entries).unwrap_or_else(|_| "[]".to_string());
        std::fs::write(&self.path, json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_history(dir: &std::path::Path) -> PlayHistory {
        PlayHistory {
            path: dir.join("history.json"),
            max_entries: 5,
        }
    }

    fn entry(id: &str, title: &str) -> HistoryEntry {
        HistoryEntry {
            song_id: id.to_string(),
            title: title.to_string(),
            artist: "Artist".to_string(),
            album: "Album".to_string(),
            played_at: "2026-03-19T12:00:00Z".to_string(),
        }
    }

    #[test]
    fn add_and_list() {
        let dir = tempfile::tempdir().unwrap();
        let history = test_history(dir.path());

        history.add(entry("s1", "Song 1")).unwrap();
        history.add(entry("s2", "Song 2")).unwrap();

        let items = history.list(10);
        assert_eq!(items.len(), 2);
        // Newest first
        assert_eq!(items[0].song_id, "s2");
        assert_eq!(items[1].song_id, "s1");
    }

    #[test]
    fn list_respects_limit() {
        let dir = tempfile::tempdir().unwrap();
        let history = test_history(dir.path());

        for i in 0..5 {
            history
                .add(entry(&format!("s{i}"), &format!("Song {i}")))
                .unwrap();
        }

        let items = history.list(3);
        assert_eq!(items.len(), 3);
    }

    #[test]
    fn max_entries_trims_oldest() {
        let dir = tempfile::tempdir().unwrap();
        let history = test_history(dir.path()); // max_entries = 5

        for i in 0..8 {
            history
                .add(entry(&format!("s{i}"), &format!("Song {i}")))
                .unwrap();
        }

        assert_eq!(history.count(), 5);
        let items = history.list(10);
        // Oldest (s0, s1, s2) should be trimmed
        assert_eq!(items[0].song_id, "s7");
        assert_eq!(items[4].song_id, "s3");
    }

    #[test]
    fn clear_removes_all() {
        let dir = tempfile::tempdir().unwrap();
        let history = test_history(dir.path());

        history.add(entry("s1", "Song")).unwrap();
        assert_eq!(history.count(), 1);

        history.clear().unwrap();
        assert_eq!(history.count(), 0);
        assert!(history.list(10).is_empty());
    }

    #[test]
    fn empty_history_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let history = test_history(dir.path());
        assert!(history.list(10).is_empty());
        assert_eq!(history.count(), 0);
    }

    #[test]
    fn entries_persist_across_instances() {
        let dir = tempfile::tempdir().unwrap();

        {
            let history = test_history(dir.path());
            history.add(entry("s1", "Song 1")).unwrap();
            history.add(entry("s2", "Song 2")).unwrap();
        }

        // New instance reads from same file
        let history = test_history(dir.path());
        assert_eq!(history.count(), 2);
    }
}
