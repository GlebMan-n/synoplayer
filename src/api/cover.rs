use crate::api::client::SynoClient;
use crate::error::{Result, SynoError};

/// Cover art operations (SYNO.AudioStation.Cover).
pub struct CoverApi<'a> {
    client: &'a SynoClient,
}

impl<'a> CoverApi<'a> {
    pub fn new(client: &'a SynoClient) -> Self {
        Self { client }
    }

    /// Build URL for song cover image (returns binary image).
    pub fn song_cover_url(&self, song_id: &str) -> Result<String> {
        let base = self.client.build_url("SYNO.AudioStation.Cover")?;
        let sid = self.client.sid().ok_or(SynoError::NotAuthenticated)?;
        Ok(format!(
            "{}?api=SYNO.AudioStation.Cover&version=3&method=getsongcover&id={}&_sid={}",
            base, song_id, sid
        ))
    }

    /// Download cover image bytes.
    pub async fn get_song_cover(&self, song_id: &str) -> Result<bytes::Bytes> {
        let url = self.song_cover_url(song_id)?;
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

    #[test]
    fn cover_url_contains_song_id() {
        let mut client = SynoClient::new("https://nas:5001");
        client.set_sid("sid".to_string());
        let mut paths = HashMap::new();
        paths.insert(
            "SYNO.AudioStation.Cover".to_string(),
            ApiInfo {
                path: "AudioStation/cover.cgi".to_string(),
                min_version: 1,
                max_version: 3,
            },
        );
        client.set_api_paths(paths);

        let api = CoverApi::new(&client);
        let url = api.song_cover_url("music_123").unwrap();
        assert!(url.contains("music_123"));
        assert!(url.contains("getsongcover"));
    }
}
