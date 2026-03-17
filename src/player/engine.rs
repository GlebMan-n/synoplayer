/// Audio playback engine.
///
/// Responsible only for feeding audio data to the output device.
/// Does not know about Synology API or caching.
///
/// Placeholder — will be implemented in Etap 2 with rodio.
pub struct AudioEngine {
    // Will contain rodio::Sink and OutputStreamHandle
}

impl AudioEngine {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for AudioEngine {
    fn default() -> Self {
        Self::new()
    }
}
