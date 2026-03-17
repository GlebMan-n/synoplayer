use std::time::Duration;

/// Metadata about the currently playing track.
#[derive(Debug, Clone)]
pub struct TrackInfo {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: Duration,
}

/// Player state machine.
///
/// Valid transitions:
/// - Stopped → Playing (play)
/// - Playing → Paused (pause)
/// - Playing → Stopped (stop)
/// - Paused → Playing (resume)
/// - Paused → Stopped (stop)
#[derive(Debug, Clone)]
pub enum PlayerState {
    Stopped,
    Playing {
        track: TrackInfo,
        position: Duration,
    },
    Paused {
        track: TrackInfo,
        position: Duration,
    },
}

impl PlayerState {
    pub fn play(track: TrackInfo) -> Self {
        PlayerState::Playing {
            track,
            position: Duration::ZERO,
        }
    }

    pub fn pause(&mut self) {
        if let PlayerState::Playing { track, position } = self {
            *self = PlayerState::Paused {
                track: track.clone(),
                position: *position,
            };
        }
    }

    pub fn resume(&mut self) {
        if let PlayerState::Paused { track, position } = self {
            *self = PlayerState::Playing {
                track: track.clone(),
                position: *position,
            };
        }
    }

    pub fn stop(&mut self) {
        *self = PlayerState::Stopped;
    }

    pub fn is_stopped(&self) -> bool {
        matches!(self, PlayerState::Stopped)
    }

    pub fn is_playing(&self) -> bool {
        matches!(self, PlayerState::Playing { .. })
    }

    pub fn is_paused(&self) -> bool {
        matches!(self, PlayerState::Paused { .. })
    }

    pub fn track(&self) -> Option<&TrackInfo> {
        match self {
            PlayerState::Playing { track, .. } | PlayerState::Paused { track, .. } => Some(track),
            PlayerState::Stopped => None,
        }
    }

    pub fn position(&self) -> Option<Duration> {
        match self {
            PlayerState::Playing { position, .. } | PlayerState::Paused { position, .. } => {
                Some(*position)
            }
            PlayerState::Stopped => None,
        }
    }

    pub fn set_position(&mut self, new_pos: Duration) {
        match self {
            PlayerState::Playing { position, .. } | PlayerState::Paused { position, .. } => {
                *position = new_pos;
            }
            PlayerState::Stopped => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_track() -> TrackInfo {
        TrackInfo {
            id: "music_1".to_string(),
            title: "Test Song".to_string(),
            artist: "Test Artist".to_string(),
            album: "Test Album".to_string(),
            duration: Duration::from_secs(240),
        }
    }

    #[test]
    #[ignore]
    fn initial_state_is_stopped() {
        let state = PlayerState::Stopped;
        assert!(state.is_stopped());
        assert!(!state.is_playing());
        assert!(!state.is_paused());
        assert!(state.track().is_none());
        assert!(state.position().is_none());
    }

    #[test]
    #[ignore]
    fn play_creates_playing_state() {
        let state = PlayerState::play(sample_track());
        assert!(state.is_playing());
        assert_eq!(state.track().unwrap().id, "music_1");
        assert_eq!(state.position().unwrap(), Duration::ZERO);
    }

    #[test]
    #[ignore]
    fn pause_from_playing() {
        let mut state = PlayerState::play(sample_track());
        state.set_position(Duration::from_secs(30));
        state.pause();
        assert!(state.is_paused());
        assert_eq!(state.position().unwrap(), Duration::from_secs(30));
    }

    #[test]
    #[ignore]
    fn resume_from_paused() {
        let mut state = PlayerState::play(sample_track());
        state.set_position(Duration::from_secs(60));
        state.pause();
        state.resume();
        assert!(state.is_playing());
        assert_eq!(state.position().unwrap(), Duration::from_secs(60));
    }

    #[test]
    #[ignore]
    fn pause_from_stopped_does_nothing() {
        let mut state = PlayerState::Stopped;
        state.pause();
        assert!(state.is_stopped());
    }

    #[test]
    #[ignore]
    fn resume_from_stopped_does_nothing() {
        let mut state = PlayerState::Stopped;
        state.resume();
        assert!(state.is_stopped());
    }

    #[test]
    #[ignore]
    fn stop_from_playing() {
        let mut state = PlayerState::play(sample_track());
        state.stop();
        assert!(state.is_stopped());
        assert!(state.track().is_none());
    }

    #[test]
    #[ignore]
    fn stop_from_paused() {
        let mut state = PlayerState::play(sample_track());
        state.pause();
        state.stop();
        assert!(state.is_stopped());
    }

    #[test]
    #[ignore]
    fn track_info_accessible_in_playing_and_paused() {
        let state = PlayerState::play(sample_track());
        assert_eq!(state.track().unwrap().title, "Test Song");
        assert_eq!(state.track().unwrap().artist, "Test Artist");
        assert_eq!(state.track().unwrap().album, "Test Album");
        assert_eq!(state.track().unwrap().duration, Duration::from_secs(240));
    }
}
