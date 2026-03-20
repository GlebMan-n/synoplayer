use std::process::{Child, Command};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::player::queue::PlayQueue;
use crate::player::state::{PlayerState, TrackInfo};

/// Audio playback engine.
///
/// Uses a subprocess (pw-play, paplay, aplay, or ffplay) to play audio.
/// Manages player state and queue. Does not know about Synology API.
pub struct AudioEngine {
    state: Arc<Mutex<PlayerState>>,
    queue: Arc<Mutex<PlayQueue>>,
    child: Arc<Mutex<Option<Child>>>,
    volume: Arc<Mutex<u8>>,
    play_start: Arc<Mutex<Option<Instant>>>,
    output_device: String,
}

impl AudioEngine {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(PlayerState::Stopped)),
            queue: Arc::new(Mutex::new(PlayQueue::new())),
            child: Arc::new(Mutex::new(None)),
            volume: Arc::new(Mutex::new(80)),
            play_start: Arc::new(Mutex::new(None)),
            output_device: String::new(),
        }
    }

    pub fn with_device(mut self, device: &str) -> Self {
        self.output_device = device.to_string();
        self
    }

    pub fn state(&self) -> PlayerState {
        self.state.lock().unwrap().clone()
    }

    pub fn queue(&self) -> std::sync::MutexGuard<'_, PlayQueue> {
        self.queue.lock().unwrap()
    }

    pub fn volume(&self) -> u8 {
        *self.volume.lock().unwrap()
    }

    pub fn set_volume(&self, vol: u8) {
        let vol = vol.min(100);
        *self.volume.lock().unwrap() = vol;
        apply_system_volume(vol);
    }

    /// Start playing from a URL. Sets state to Playing.
    pub fn play_url(&self, url: &str, track: TrackInfo) -> crate::error::Result<()> {
        self.stop_subprocess();

        let vol = self.volume();
        let child = spawn_audio_process(url, vol, &self.output_device)?;
        *self.child.lock().unwrap() = Some(child);
        *self.state.lock().unwrap() = PlayerState::play(track);
        *self.play_start.lock().unwrap() = Some(Instant::now());

        Ok(())
    }

    /// Pause playback (kills subprocess, remembers position).
    pub fn pause(&self) {
        self.stop_subprocess();
        let mut state = self.state.lock().unwrap();
        if let Some(start) = *self.play_start.lock().unwrap() {
            state.set_position(start.elapsed());
        }
        state.pause();
    }

    /// Resume is not truly supported with subprocess (would restart from beginning).
    /// For MVP, resume restarts the track.
    pub fn resume_url(&self, url: &str) -> crate::error::Result<()> {
        let mut state = self.state.lock().unwrap();
        if state.is_paused() {
            let vol = *self.volume.lock().unwrap();
            let child = spawn_audio_process(url, vol, &self.output_device)?;
            *self.child.lock().unwrap() = Some(child);
            state.resume();
            *self.play_start.lock().unwrap() = Some(Instant::now());
        }
        Ok(())
    }

    /// Stop playback completely.
    pub fn stop(&self) {
        self.stop_subprocess();
        self.state.lock().unwrap().stop();
        *self.play_start.lock().unwrap() = None;
    }

    /// Check if the subprocess has finished.
    /// Returns Ok(true) if track ended normally, Ok(false) if
    /// still playing, Err if the player subprocess failed.
    pub fn check_finished(&self) -> Result<bool, String> {
        let mut child_guard = self.child.lock().unwrap();
        if let Some(ref mut child) = *child_guard {
            match child.try_wait() {
                Ok(Some(status)) => {
                    *child_guard = None;
                    let elapsed = self
                        .play_start
                        .lock()
                        .unwrap()
                        .map(|s| s.elapsed())
                        .unwrap_or(Duration::ZERO);
                    self.state.lock().unwrap().stop();
                    *self.play_start.lock().unwrap() = None;

                    // If process exited with error within 3s,
                    // it likely failed to open audio device.
                    if !status.success() && elapsed < Duration::from_secs(3) {
                        Err(format!(
                            "Audio player exited with {} after {:.1}s. \
                             Check [player] output_device in config.",
                            status, elapsed.as_secs_f64()
                        ))
                    } else {
                        Ok(true)
                    }
                }
                _ => Ok(false),
            }
        } else {
            Ok(false)
        }
    }

    fn stop_subprocess(&self) {
        let mut child_guard = self.child.lock().unwrap();
        if let Some(ref mut child) = *child_guard {
            let _ = child.kill();
            let _ = child.wait();
        }
        *child_guard = None;
    }

    /// Get estimated current position.
    pub fn current_position(&self) -> Duration {
        if let Some(start) = *self.play_start.lock().unwrap()
            && self.state.lock().unwrap().is_playing()
        {
            return start.elapsed();
        }
        self.state
            .lock()
            .unwrap()
            .position()
            .unwrap_or(Duration::ZERO)
    }
}

impl Default for AudioEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        self.stop_subprocess();
    }
}

/// Check if PulseAudio or PipeWire sound server is running.
/// When a sound server owns the ALSA device, we must route
/// audio through it instead of opening ALSA directly.
fn pulse_running() -> bool {
    // pactl info succeeds when PulseAudio/PipeWire-pulse is up
    try_run("pactl", &["info"])
}

/// Detect audio output method: "pulse", "alsa", or "auto".
fn detect_output(device: &str) -> &'static str {
    if pulse_running() {
        "pulse"
    } else if !device.is_empty() {
        "alsa"
    } else {
        "auto"
    }
}

/// Check that at least one audio backend is available.
/// Returns Ok(()) or Err with a user-friendly message.
pub fn check_audio_deps() -> Result<(), String> {
    const PLAYERS: &[&str] = &[
        "ffplay", "mpv", "ffmpeg", "gst-launch-1.0",
    ];
    if PLAYERS.iter().any(|p| which_exists(p)) {
        return Ok(());
    }
    Err(
        "No supported audio player found.\n\
         Install one of the following:\n\
         \n\
         Ubuntu/Debian:\n  \
           sudo apt install ffmpeg        # recommended\n  \
           sudo apt install mpv\n\
         \n\
         Fedora:\n  \
           sudo dnf install ffmpeg-free\n  \
           sudo dnf install mpv\n\
         \n\
         Arch:\n  \
           sudo pacman -S ffmpeg\n  \
           sudo pacman -S mpv\n\
         \n\
         Alpine:\n  \
           apk add ffmpeg\n\
         \n\
         Any of these provides the audio decoding and \
         output needed for playback."
            .to_string(),
    )
}

/// Spawn an audio subprocess to play from a URL.
fn spawn_audio_process(
    url: &str,
    volume: u8,
    device: &str,
) -> crate::error::Result<Child> {
    let vol_str = volume.to_string();
    let vol_frac = format!("{:.2}", volume as f64 / 100.0);
    let output = detect_output(device);
    let alsa_dev = if device.is_empty() {
        "default"
    } else {
        device
    };

    // 1. ffplay (handles URLs, good codec support)
    if which_exists("ffplay") {
        return match output {
            "pulse" => {
                // Let ffplay use PulseAudio via SDL
                try_spawn_env_pulse(
                    "ffplay",
                    &[
                        "-nodisp", "-autoexit",
                        "-loglevel", "quiet",
                        "-volume", &vol_str, url,
                    ],
                )
            }
            _ => {
                try_spawn_env(
                    "ffplay",
                    &[
                        "-nodisp", "-autoexit",
                        "-loglevel", "quiet",
                        "-volume", &vol_str, url,
                    ],
                    device,
                )
            }
        };
    }

    // 2. mpv
    if which_exists("mpv") {
        let vol_flag = format!("--volume={vol_str}");
        let mut args = vec![
            "--no-video", "--really-quiet",
            vol_flag.as_str(), url,
        ];
        let dev_flag;
        if output == "alsa" && !device.is_empty() {
            dev_flag = format!("--audio-device=alsa/{device}");
            args.insert(0, &dev_flag);
        }
        return try_spawn("mpv", &args);
    }

    // 3. ffmpeg — try pulse first when sound server is up
    if which_exists("ffmpeg") {
        let af = format!("volume={vol_frac}");
        if output == "pulse" {
            if let Ok(child) = try_spawn(
                "ffmpeg",
                &[
                    "-i", url, "-loglevel", "quiet",
                    "-af", &af, "-f", "pulse", "default",
                ],
            ) {
                return Ok(child);
            }
        }
        // ALSA fallback (works when no sound server)
        if let Ok(child) = try_spawn(
            "ffmpeg",
            &[
                "-i", url, "-loglevel", "quiet",
                "-af", &af, "-f", "alsa", alsa_dev,
            ],
        ) {
            return Ok(child);
        }
        // Final pulse fallback
        if output != "pulse" {
            if let Ok(child) = try_spawn(
                "ffmpeg",
                &[
                    "-i", url, "-loglevel", "quiet",
                    "-af", &af, "-f", "pulse", "default",
                ],
            ) {
                return Ok(child);
            }
        }
    }

    // 4. GStreamer
    if which_exists("gst-launch-1.0") {
        let sink = match output {
            "alsa" if !device.is_empty() => {
                format!("alsasink device={device}")
            }
            "pulse" => "pulsesink".to_string(),
            _ => "autoaudiosink".to_string(),
        };
        let pipeline = format!(
            "souphttpsrc location={url} ! decodebin ! \
             audioconvert ! audioresample ! \
             volume volume={vol_frac} ! {sink}"
        );
        return try_spawn("gst-launch-1.0", &[&pipeline]);
    }

    Err(crate::error::SynoError::Player(
        "No audio player found. \
         Run `synoplayer doctor` for help."
            .to_string(),
    ))
}

/// Spawn with AUDIODEV env var set for direct ALSA access.
fn try_spawn_env(
    cmd: &str,
    args: &[&str],
    device: &str,
) -> crate::error::Result<Child> {
    let mut command = Command::new(cmd);
    command
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    if !device.is_empty() {
        command.env("AUDIODEV", device);
        command.env("SDL_AUDIODRIVER", "alsa");
    }
    command
        .spawn()
        .map_err(|e| {
            crate::error::SynoError::Player(
                format!("Failed to spawn {cmd}: {e}"),
            )
        })
}

/// Spawn with SDL_AUDIODRIVER=pulseaudio for PulseAudio route.
fn try_spawn_env_pulse(
    cmd: &str,
    args: &[&str],
) -> crate::error::Result<Child> {
    Command::new(cmd)
        .args(args)
        .env("SDL_AUDIODRIVER", "pulseaudio")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| {
            crate::error::SynoError::Player(
                format!("Failed to spawn {cmd}: {e}"),
            )
        })
}

/// Apply volume at system level for runtime changes.
fn apply_system_volume(vol: u8) {
    let pct = format!("{}%", vol);

    // pactl (PulseAudio / PipeWire-pulse)
    if try_run("pactl", &["set-sink-volume", "@DEFAULT_SINK@", &pct]) {
        return;
    }
    // wpctl (WirePlumber / PipeWire native)
    let frac = format!("{:.2}", vol as f64 / 100.0);
    if try_run("wpctl", &["set-volume", "@DEFAULT_AUDIO_SINK@", &frac]) {
        return;
    }
    // amixer (ALSA fallback)
    let _ = try_run("amixer", &["sset", "Master", &pct]);
}

/// Run a command silently, return true on success.
fn try_run(cmd: &str, args: &[&str]) -> bool {
    Command::new(cmd)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

fn try_spawn(cmd: &str, args: &[&str]) -> crate::error::Result<Child> {
    Command::new(cmd)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| crate::error::SynoError::Player(format!("Failed to spawn {cmd}: {e}")))
}

fn which_exists(name: &str) -> bool {
    Command::new("which")
        .arg(name)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_track() -> TrackInfo {
        TrackInfo {
            id: "test_1".to_string(),
            title: "Test".to_string(),
            artist: "Artist".to_string(),
            album: "Album".to_string(),
            duration: Duration::from_secs(180),
        }
    }

    #[test]
    fn new_engine_is_stopped() {
        let engine = AudioEngine::new();
        assert!(engine.state().is_stopped());
    }

    #[test]
    fn default_volume_is_80() {
        let engine = AudioEngine::new();
        assert_eq!(engine.volume(), 80);
    }

    #[test]
    fn set_volume_clamps_to_100() {
        let engine = AudioEngine::new();
        engine.set_volume(150);
        assert_eq!(engine.volume(), 100);
    }

    #[test]
    fn stop_from_stopped_is_noop() {
        let engine = AudioEngine::new();
        engine.stop();
        assert!(engine.state().is_stopped());
    }

    #[test]
    fn which_exists_finds_sh() {
        assert!(which_exists("sh"));
    }

    #[test]
    fn which_exists_fails_for_nonexistent() {
        assert!(!which_exists("nonexistent_binary_xyz"));
    }
}
