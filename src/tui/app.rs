use std::time::{Duration, Instant};

use ratatui::widgets::TableState;

use crate::api::types::{Folder, Playlist, Song};
use crate::player::engine::AudioEngine;
use crate::player::queue::RepeatMode;
use crate::player::state::TrackInfo;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Library,
    Folders,
    Playlists,
    Queue,
}

impl Tab {
    pub const ALL: &[Tab] = &[Tab::Library, Tab::Folders, Tab::Playlists, Tab::Queue];

    pub fn label(&self) -> &'static str {
        match self {
            Tab::Library => "Library",
            Tab::Folders => "Folders",
            Tab::Playlists => "Playlists",
            Tab::Queue => "Queue",
        }
    }

    pub fn index(&self) -> usize {
        Tab::ALL.iter().position(|t| t == self).unwrap_or(0)
    }
}

/// Generic stateful list backed by ratatui TableState.
pub struct StatefulList<T> {
    pub items: Vec<T>,
    pub state: TableState,
}

impl<T> Default for StatefulList<T> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            state: TableState::default(),
        }
    }
}

impl<T> StatefulList<T> {
    pub fn with_items(items: Vec<T>) -> Self {
        let mut state = TableState::default();
        if !items.is_empty() {
            state.select(Some(0));
        }
        Self { items, state }
    }

    pub fn next(&mut self) {
        if self.items.is_empty() {
            return;
        }
        let i = match self.state.selected() {
            Some(i) => (i + 1).min(self.items.len() - 1),
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn previous(&mut self) {
        if self.items.is_empty() {
            return;
        }
        let i = match self.state.selected() {
            Some(i) => i.saturating_sub(1),
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn selected(&self) -> Option<usize> {
        self.state.selected()
    }

    pub fn selected_item(&self) -> Option<&T> {
        self.state.selected().and_then(|i| self.items.get(i))
    }

    pub fn page_down(&mut self, page: usize) {
        if self.items.is_empty() {
            return;
        }
        let i = match self.state.selected() {
            Some(i) => (i + page).min(self.items.len() - 1),
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn page_up(&mut self, page: usize) {
        if self.items.is_empty() {
            return;
        }
        let i = match self.state.selected() {
            Some(i) => i.saturating_sub(page),
            None => 0,
        };
        self.state.select(Some(i));
    }
}

/// Info about the playlist currently being viewed in Playlists tab.
pub struct PlaylistDetail {
    pub name: String,
    pub songs: StatefulList<Song>,
}

/// Currently playing track with timing info.
pub struct NowPlaying {
    pub track: TrackInfo,
    pub started_at: Instant,
    pub paused_elapsed: Duration,
    pub queue_index: usize,
}

impl NowPlaying {
    pub fn elapsed(&self) -> Duration {
        self.paused_elapsed + self.started_at.elapsed()
    }

    pub fn progress(&self) -> f64 {
        if self.track.duration.is_zero() {
            return 0.0;
        }
        (self.elapsed().as_secs_f64() / self.track.duration.as_secs_f64()).min(1.0)
    }
}

/// Top-level TUI application state.
pub struct App {
    pub running: bool,
    pub active_tab: Tab,

    // Data lists
    pub songs: StatefulList<Song>,
    pub playlists: StatefulList<Playlist>,
    pub playlist_detail: Option<PlaylistDetail>,
    pub folders: StatefulList<Folder>,
    /// Breadcrumb stack for folder navigation: (id, name).
    pub folder_stack: Vec<(String, String)>,

    // Playback
    pub queue: Vec<Song>,
    pub now_playing: Option<NowPlaying>,
    pub volume: u8,
    pub shuffle: bool,
    pub repeat_mode: RepeatMode,

    // Status bar
    pub status: String,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        Self {
            running: true,
            active_tab: Tab::Library,
            songs: StatefulList::default(),
            playlists: StatefulList::default(),
            playlist_detail: None,
            folders: StatefulList::default(),
            folder_stack: Vec::new(),
            queue: Vec::new(),
            now_playing: None,
            volume: 80,
            shuffle: false,
            repeat_mode: RepeatMode::Off,
            status: "Ready".to_string(),
        }
    }

    pub fn next_tab(&mut self) {
        let idx = self.active_tab.index();
        self.active_tab = Tab::ALL[(idx + 1) % Tab::ALL.len()];
        self.playlist_detail = None;
    }

    pub fn prev_tab(&mut self) {
        let idx = self.active_tab.index();
        self.active_tab = Tab::ALL[if idx == 0 {
            Tab::ALL.len() - 1
        } else {
            idx - 1
        }];
        self.playlist_detail = None;
    }

    /// Returns true if a track just finished (caller should advance queue).
    /// Note: does NOT clear now_playing — advance_queue needs queue_index from it.
    pub fn tick(&mut self, engine: &AudioEngine) -> bool {
        if self.now_playing.is_some() && engine.check_finished() {
            return true;
        }
        false
    }

    pub fn set_now_playing(&mut self, track: TrackInfo, queue_index: usize) {
        self.now_playing = Some(NowPlaying {
            track,
            started_at: Instant::now(),
            paused_elapsed: Duration::ZERO,
            queue_index,
        });
    }

    pub fn stop_playback(&mut self, engine: &AudioEngine) {
        engine.stop();
        self.now_playing = None;
        self.status = "Stopped.".to_string();
    }

    /// Navigate up/down in the active tab's list.
    pub fn active_list_next(&mut self) {
        match self.active_tab {
            Tab::Library => self.songs.next(),
            Tab::Folders => self.folders.next(),
            Tab::Playlists => {
                if let Some(ref mut detail) = self.playlist_detail {
                    detail.songs.next();
                } else {
                    self.playlists.next();
                }
            }
            Tab::Queue => {}
        }
    }

    pub fn active_list_previous(&mut self) {
        match self.active_tab {
            Tab::Library => self.songs.previous(),
            Tab::Folders => self.folders.previous(),
            Tab::Playlists => {
                if let Some(ref mut detail) = self.playlist_detail {
                    detail.songs.previous();
                } else {
                    self.playlists.previous();
                }
            }
            Tab::Queue => {}
        }
    }

    pub fn active_list_page_down(&mut self, page: usize) {
        match self.active_tab {
            Tab::Library => self.songs.page_down(page),
            Tab::Folders => self.folders.page_down(page),
            Tab::Playlists => {
                if let Some(ref mut d) = self.playlist_detail {
                    d.songs.page_down(page);
                } else {
                    self.playlists.page_down(page);
                }
            }
            Tab::Queue => {}
        }
    }

    pub fn active_list_page_up(&mut self, page: usize) {
        match self.active_tab {
            Tab::Library => self.songs.page_up(page),
            Tab::Folders => self.folders.page_up(page),
            Tab::Playlists => {
                if let Some(ref mut d) = self.playlist_detail {
                    d.songs.page_up(page);
                } else {
                    self.playlists.page_up(page);
                }
            }
            Tab::Queue => {}
        }
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
    fn stateful_list_next_previous() {
        let mut list = StatefulList::with_items(vec![song("a"), song("b"), song("c")]);
        assert_eq!(list.selected(), Some(0));
        list.next();
        assert_eq!(list.selected(), Some(1));
        list.next();
        assert_eq!(list.selected(), Some(2));
        list.next(); // clamp at end
        assert_eq!(list.selected(), Some(2));
        list.previous();
        assert_eq!(list.selected(), Some(1));
        list.previous();
        assert_eq!(list.selected(), Some(0));
        list.previous(); // clamp at start
        assert_eq!(list.selected(), Some(0));
    }

    #[test]
    fn stateful_list_empty() {
        let mut list: StatefulList<Song> = StatefulList::default();
        assert_eq!(list.selected(), None);
        assert!(list.selected_item().is_none());
        list.next(); // no-op
        list.previous();
        assert_eq!(list.selected(), None);
    }

    #[test]
    fn stateful_list_page_navigation() {
        let items: Vec<Song> = (0..20).map(|i| song(&format!("{i}"))).collect();
        let mut list = StatefulList::with_items(items);
        list.page_down(10);
        assert_eq!(list.selected(), Some(10));
        list.page_down(10);
        assert_eq!(list.selected(), Some(19)); // clamped
        list.page_up(5);
        assert_eq!(list.selected(), Some(14));
        list.page_up(100);
        assert_eq!(list.selected(), Some(0)); // clamped
    }

    #[test]
    fn tab_switching() {
        let mut app = App::new();
        assert_eq!(app.active_tab, Tab::Library);
        app.next_tab();
        assert_eq!(app.active_tab, Tab::Folders);
        app.next_tab();
        assert_eq!(app.active_tab, Tab::Playlists);
        app.next_tab();
        assert_eq!(app.active_tab, Tab::Queue);
        app.next_tab();
        assert_eq!(app.active_tab, Tab::Library); // wraps
        app.prev_tab();
        assert_eq!(app.active_tab, Tab::Queue); // wraps back
    }

    #[test]
    fn now_playing_progress() {
        let np = NowPlaying {
            track: TrackInfo {
                id: "t".to_string(),
                title: "T".to_string(),
                artist: "A".to_string(),
                album: String::new(),
                duration: Duration::from_secs(100),
            },
            started_at: Instant::now() - Duration::from_secs(50),
            paused_elapsed: Duration::ZERO,
            queue_index: 0,
        };
        let progress = np.progress();
        assert!(progress >= 0.49 && progress <= 0.51);
    }

    #[test]
    fn now_playing_zero_duration() {
        let np = NowPlaying {
            track: TrackInfo {
                id: "t".to_string(),
                title: "T".to_string(),
                artist: String::new(),
                album: String::new(),
                duration: Duration::ZERO,
            },
            started_at: Instant::now(),
            paused_elapsed: Duration::ZERO,
            queue_index: 0,
        };
        assert_eq!(np.progress(), 0.0);
    }
}
