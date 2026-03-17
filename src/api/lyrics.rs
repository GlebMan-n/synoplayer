use crate::api::client::SynoClient;
use crate::api::types::LyricsData;
use crate::error::Result;

/// Lyrics operations (SYNO.AudioStation.Lyrics).
pub struct LyricsApi<'a> {
    client: &'a SynoClient,
}

impl<'a> LyricsApi<'a> {
    pub fn new(client: &'a SynoClient) -> Self {
        Self { client }
    }

    pub async fn get(&self, song_id: &str) -> Result<LyricsData> {
        self.client
            .request(
                "SYNO.AudioStation.Lyrics",
                2,
                "getlyrics",
                &[("id", song_id)],
            )
            .await
    }

    pub async fn set(&self, song_id: &str, lyrics: &str) -> Result<()> {
        let _: serde_json::Value = self
            .client
            .request(
                "SYNO.AudioStation.Lyrics",
                2,
                "setlyrics",
                &[("id", song_id), ("lyrics", lyrics)],
            )
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::types::ApiInfo;
    use std::collections::HashMap;
    use wiremock::matchers::{method, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn get_lyrics_parses_text() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(query_param("api", "SYNO.AudioStation.Lyrics"))
            .and(query_param("id", "music_1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "success": true,
                "data": {"lyrics": "Hello darkness my old friend\nI've come to talk with you again"}
            })))
            .mount(&server)
            .await;

        let mut client = SynoClient::new(&server.uri());
        client.set_sid("sid".to_string());
        let mut paths = HashMap::new();
        paths.insert(
            "SYNO.AudioStation.Lyrics".to_string(),
            ApiInfo {
                path: "AudioStation/lyrics.cgi".to_string(),
                min_version: 1,
                max_version: 2,
            },
        );
        client.set_api_paths(paths);

        let api = LyricsApi::new(&client);
        let data = api.get("music_1").await.unwrap();
        assert!(data.lyrics.contains("darkness"));
    }
}
