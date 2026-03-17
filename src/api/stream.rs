use crate::api::client::SynoClient;
use crate::error::{Result, SynoError};

/// Audio streaming operations (SYNO.AudioStation.Stream).
///
/// Returns raw byte streams, does not decode audio.
pub struct StreamApi<'a> {
    client: &'a SynoClient,
}

impl<'a> StreamApi<'a> {
    pub fn new(client: &'a SynoClient) -> Self {
        Self { client }
    }

    /// Build the streaming URL for a song.
    pub fn stream_url(&self, song_id: &str) -> Result<String> {
        let base = self.client.build_url("SYNO.AudioStation.Stream")?;
        let sid = self.client.sid().ok_or(SynoError::NotAuthenticated)?;

        Ok(format!(
            "{}/0.mp3?api=SYNO.AudioStation.Stream&version=2&method=stream&id={}&_sid={}",
            base, song_id, sid
        ))
    }

    /// Build the transcoding URL for a song.
    pub fn transcode_url(&self, song_id: &str) -> Result<String> {
        let base = self.client.build_url("SYNO.AudioStation.Stream")?;
        let sid = self.client.sid().ok_or(SynoError::NotAuthenticated)?;

        Ok(format!(
            "{}/0.mp3?api=SYNO.AudioStation.Stream&version=2&method=transcode&id={}&_sid={}",
            base, song_id, sid
        ))
    }

    /// Stream audio bytes for a song.
    pub async fn stream_bytes(&self, song_id: &str) -> Result<bytes::Bytes> {
        let url = self.stream_url(song_id)?;
        let response = self.client.http().get(&url).send().await?;
        let bytes = response.bytes().await?;
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::types::ApiInfo;
    use std::collections::HashMap;

    fn client_with_stream_path() -> SynoClient {
        let mut client = SynoClient::new("https://nas.local:5001");
        client.set_sid("test_sid".to_string());
        let mut paths = HashMap::new();
        paths.insert(
            "SYNO.AudioStation.Stream".to_string(),
            ApiInfo {
                path: "AudioStation/stream.cgi".to_string(),
                min_version: 1,
                max_version: 2,
            },
        );
        client.set_api_paths(paths);
        client
    }

    #[test]
    fn stream_url_contains_song_id_and_sid() {
        let client = client_with_stream_path();
        let api = StreamApi::new(&client);
        let url = api.stream_url("music_123").unwrap();
        assert!(url.contains("id=music_123"));
        assert!(url.contains("_sid=test_sid"));
        assert!(url.contains("method=stream"));
    }

    #[test]
    fn transcode_url_uses_transcode_method() {
        let client = client_with_stream_path();
        let api = StreamApi::new(&client);
        let url = api.transcode_url("music_456").unwrap();
        assert!(url.contains("method=transcode"));
        assert!(url.contains("id=music_456"));
    }

    #[test]
    fn stream_url_fails_without_auth() {
        let client = SynoClient::new("https://nas.local:5001");
        let api = StreamApi::new(&client);
        assert!(api.stream_url("music_123").is_err());
    }

    #[tokio::test]
    #[ignore]
    async fn stream_bytes_returns_audio_data() {
        todo!()
    }
}
