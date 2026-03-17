use crate::api::types::Song;

/// Repeat mode for the play queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeatMode {
    Off,
    One,
    All,
}

/// Manages an ordered list of songs for playback.
///
/// Responsible only for ordering, current position, and navigation.
/// Does not handle actual playback.
pub struct PlayQueue {
    songs: Vec<Song>,
    current_index: Option<usize>,
    repeat: RepeatMode,
    shuffled: bool,
}

impl PlayQueue {
    pub fn new() -> Self {
        Self {
            songs: Vec::new(),
            current_index: None,
            repeat: RepeatMode::Off,
            shuffled: false,
        }
    }

    pub fn from_songs(songs: Vec<Song>) -> Self {
        let has_songs = !songs.is_empty();
        Self {
            songs,
            current_index: if has_songs { Some(0) } else { None },
            repeat: RepeatMode::Off,
            shuffled: false,
        }
    }

    pub fn add(&mut self, song: Song) {
        self.songs.push(song);
        if self.current_index.is_none() {
            self.current_index = Some(0);
        }
    }

    pub fn clear(&mut self) {
        self.songs.clear();
        self.current_index = None;
    }

    pub fn current(&self) -> Option<&Song> {
        self.current_index.and_then(|i| self.songs.get(i))
    }

    pub fn current_index(&self) -> Option<usize> {
        self.current_index
    }

    pub fn len(&self) -> usize {
        self.songs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.songs.is_empty()
    }

    pub fn list(&self) -> &[Song] {
        &self.songs
    }

    pub fn repeat(&self) -> RepeatMode {
        self.repeat
    }

    pub fn set_repeat(&mut self, mode: RepeatMode) {
        self.repeat = mode;
    }

    /// Move to next track. Returns true if moved, false if at end.
    pub fn next(&mut self) -> bool {
        let Some(current) = self.current_index else {
            return false;
        };
        if self.songs.is_empty() {
            return false;
        }

        match self.repeat {
            RepeatMode::One => true, // stay on same track
            RepeatMode::All => {
                self.current_index = Some((current + 1) % self.songs.len());
                true
            }
            RepeatMode::Off => {
                if current + 1 < self.songs.len() {
                    self.current_index = Some(current + 1);
                    true
                } else {
                    false // end of queue
                }
            }
        }
    }

    /// Move to previous track. Returns true if moved.
    pub fn prev(&mut self) -> bool {
        let Some(current) = self.current_index else {
            return false;
        };
        if self.songs.is_empty() {
            return false;
        }

        match self.repeat {
            RepeatMode::One => true,
            RepeatMode::All => {
                self.current_index = Some(if current == 0 {
                    self.songs.len() - 1
                } else {
                    current - 1
                });
                true
            }
            RepeatMode::Off => {
                if current > 0 {
                    self.current_index = Some(current - 1);
                    true
                } else {
                    false // already at start
                }
            }
        }
    }

    /// Shuffle the queue, keeping the current track at position 0.
    pub fn shuffle(&mut self) {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        use std::time::SystemTime;

        if self.songs.len() <= 1 {
            return;
        }

        // Move current song to front
        if let Some(idx) = self.current_index {
            if idx != 0 {
                self.songs.swap(0, idx);
            }
        }

        // Simple Fisher-Yates shuffle for remaining elements
        let mut hasher = DefaultHasher::new();
        SystemTime::now().hash(&mut hasher);
        let mut seed = hasher.finish();

        for i in (2..self.songs.len()).rev() {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let j = 1 + (seed as usize % i);
            self.songs.swap(i, j);
        }

        self.current_index = Some(0);
        self.shuffled = true;
    }

    pub fn is_shuffled(&self) -> bool {
        self.shuffled
    }

    /// Remove a song by index.
    pub fn remove(&mut self, index: usize) -> Option<Song> {
        if index >= self.songs.len() {
            return None;
        }
        let song = self.songs.remove(index);

        // Adjust current_index
        if self.songs.is_empty() {
            self.current_index = None;
        } else if let Some(current) = self.current_index {
            if index < current {
                self.current_index = Some(current - 1);
            } else if index == current && current >= self.songs.len() {
                self.current_index = Some(self.songs.len() - 1);
            }
        }

        Some(song)
    }
}

impl Default for PlayQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::types::Song;

    fn song(id: &str) -> Song {
        Song {
            id: id.to_string(),
            title: format!("Song {id}"),
            path: String::new(),
            additional: None,
        }
    }

    #[test]
    fn new_queue_is_empty() {
        let q = PlayQueue::new();
        assert!(q.is_empty());
        assert_eq!(q.len(), 0);
        assert!(q.current().is_none());
    }

    #[test]
    fn add_and_current() {
        let mut q = PlayQueue::new();
        q.add(song("a"));
        q.add(song("b"));
        assert_eq!(q.len(), 2);
        assert_eq!(q.current().unwrap().id, "a");
    }

    #[test]
    fn from_songs_sets_current_to_first() {
        let q = PlayQueue::from_songs(vec![song("x"), song("y"), song("z")]);
        assert_eq!(q.current().unwrap().id, "x");
        assert_eq!(q.len(), 3);
    }

    #[test]
    fn from_empty_songs_has_no_current() {
        let q = PlayQueue::from_songs(vec![]);
        assert!(q.current().is_none());
    }

    #[test]
    fn next_advances() {
        let mut q = PlayQueue::from_songs(vec![song("a"), song("b"), song("c")]);
        assert!(q.next());
        assert_eq!(q.current().unwrap().id, "b");
        assert!(q.next());
        assert_eq!(q.current().unwrap().id, "c");
    }

    #[test]
    fn next_at_end_returns_false() {
        let mut q = PlayQueue::from_songs(vec![song("a"), song("b")]);
        q.next();
        assert!(!q.next()); // at end
        assert_eq!(q.current().unwrap().id, "b"); // stays
    }

    #[test]
    fn prev_goes_back() {
        let mut q = PlayQueue::from_songs(vec![song("a"), song("b"), song("c")]);
        q.next(); // b
        q.next(); // c
        assert!(q.prev());
        assert_eq!(q.current().unwrap().id, "b");
    }

    #[test]
    fn prev_at_start_returns_false() {
        let mut q = PlayQueue::from_songs(vec![song("a"), song("b")]);
        assert!(!q.prev());
        assert_eq!(q.current().unwrap().id, "a");
    }

    #[test]
    fn repeat_one_stays_on_track() {
        let mut q = PlayQueue::from_songs(vec![song("a"), song("b")]);
        q.set_repeat(RepeatMode::One);
        assert!(q.next());
        assert_eq!(q.current().unwrap().id, "a");
    }

    #[test]
    fn repeat_all_wraps_around() {
        let mut q = PlayQueue::from_songs(vec![song("a"), song("b")]);
        q.set_repeat(RepeatMode::All);
        q.next(); // b
        q.next(); // wrap → a
        assert_eq!(q.current().unwrap().id, "a");
    }

    #[test]
    fn repeat_all_prev_wraps_around() {
        let mut q = PlayQueue::from_songs(vec![song("a"), song("b"), song("c")]);
        q.set_repeat(RepeatMode::All);
        q.prev(); // wrap → c
        assert_eq!(q.current().unwrap().id, "c");
    }

    #[test]
    fn shuffle_preserves_all_songs() {
        let ids: Vec<String> = (0..10).map(|i| format!("s{i}")).collect();
        let songs: Vec<Song> = ids.iter().map(|id| song(id)).collect();
        let mut q = PlayQueue::from_songs(songs);
        q.shuffle();
        assert_eq!(q.len(), 10);
        let shuffled_ids: Vec<&str> = q.list().iter().map(|s| s.id.as_str()).collect();
        for id in &ids {
            assert!(shuffled_ids.contains(&id.as_str()));
        }
    }

    #[test]
    fn shuffle_keeps_current_at_front() {
        let mut q = PlayQueue::from_songs(vec![song("a"), song("b"), song("c"), song("d")]);
        q.next(); // current = b
        q.shuffle();
        assert_eq!(q.current().unwrap().id, "b");
        assert_eq!(q.current_index(), Some(0));
    }

    #[test]
    fn clear_empties_queue() {
        let mut q = PlayQueue::from_songs(vec![song("a"), song("b")]);
        q.clear();
        assert!(q.is_empty());
        assert!(q.current().is_none());
    }

    #[test]
    fn remove_adjusts_index() {
        let mut q = PlayQueue::from_songs(vec![song("a"), song("b"), song("c")]);
        q.next(); // current = b (index 1)
        q.remove(0); // remove a, b shifts to index 0
        assert_eq!(q.current().unwrap().id, "b");
        assert_eq!(q.current_index(), Some(0));
    }

    #[test]
    fn remove_current_adjusts() {
        let mut q = PlayQueue::from_songs(vec![song("a"), song("b"), song("c")]);
        q.next(); // current = b (index 1)
        q.remove(1); // remove b
        // current_index should still be 1 → now pointing to "c"
        assert_eq!(q.current().unwrap().id, "c");
    }

    #[test]
    fn remove_last_when_current_adjusts() {
        let mut q = PlayQueue::from_songs(vec![song("a"), song("b")]);
        q.next(); // current = b (index 1)
        q.remove(1); // remove b, now only a
        assert_eq!(q.current().unwrap().id, "a");
    }

    #[test]
    fn next_on_empty_returns_false() {
        let mut q = PlayQueue::new();
        assert!(!q.next());
    }

    #[test]
    fn prev_on_empty_returns_false() {
        let mut q = PlayQueue::new();
        assert!(!q.prev());
    }
}
