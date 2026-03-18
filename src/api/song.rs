use crate::api::client::SynoClient;
use crate::api::types::{Song, SongListData};
use crate::error::Result;

/// Operations on songs (SYNO.AudioStation.Song).
pub struct SongApi<'a> {
    client: &'a SynoClient,
}

impl<'a> SongApi<'a> {
    pub fn new(client: &'a SynoClient) -> Self {
        Self { client }
    }

    pub async fn list(&self, offset: i64, limit: i64) -> Result<SongListData> {
        let offset_str = offset.to_string();
        let limit_str = limit.to_string();
        self.client
            .request(
                "SYNO.AudioStation.Song",
                3,
                "list",
                &[
                    ("offset", &offset_str),
                    ("limit", &limit_str),
                    ("additional", "song_tag,song_audio,song_rating"),
                ],
            )
            .await
    }

    /// List songs with optional filters.
    pub async fn list_filtered(
        &self,
        offset: i64,
        limit: i64,
        artist: Option<&str>,
        album: Option<&str>,
        genre: Option<&str>,
    ) -> Result<SongListData> {
        let offset_str = offset.to_string();
        let limit_str = limit.to_string();
        let mut params = vec![
            ("offset", offset_str.as_str()),
            ("limit", limit_str.as_str()),
            ("additional", "song_tag,song_audio,song_rating"),
        ];
        if let Some(a) = artist {
            params.push(("artist", a));
        }
        if let Some(a) = album {
            params.push(("album", a));
        }
        if let Some(g) = genre {
            params.push(("genre", g));
        }
        self.client
            .request("SYNO.AudioStation.Song", 3, "list", &params)
            .await
    }

    pub async fn search(&self, keyword: &str, offset: i64, limit: i64) -> Result<SongListData> {
        let offset_str = offset.to_string();
        let limit_str = limit.to_string();
        self.client
            .request(
                "SYNO.AudioStation.Song",
                3,
                "search",
                &[
                    ("keyword", keyword),
                    ("offset", &offset_str),
                    ("limit", &limit_str),
                    ("additional", "song_tag,song_audio,song_rating"),
                ],
            )
            .await
    }

    pub async fn get_info(&self, id: &str) -> Result<Song> {
        // API returns SongListData with single song for getinfo too
        let data: SongListData = self
            .client
            .request(
                "SYNO.AudioStation.Song",
                3,
                "getinfo",
                &[
                    ("id", id),
                    ("additional", "song_tag,song_audio,song_rating"),
                ],
            )
            .await?;

        data.songs
            .into_iter()
            .next()
            .ok_or_else(|| crate::error::SynoError::Api {
                code: 100,
                message: format!("Song not found: {id}"),
            })
    }

    pub async fn set_rating(&self, id: &str, rating: i32) -> Result<()> {
        let rating_str = rating.to_string();
        let _: serde_json::Value = self
            .client
            .request(
                "SYNO.AudioStation.Song",
                2,
                "setrating",
                &[("id", id), ("rating", &rating_str)],
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

    async fn client_with_song_api(server: &MockServer) -> SynoClient {
        let mut client = SynoClient::new(&server.uri());
        client.set_sid("test_sid".to_string());
        let mut paths = HashMap::new();
        paths.insert(
            "SYNO.AudioStation.Song".to_string(),
            ApiInfo {
                path: "AudioStation/song.cgi".to_string(),
                min_version: 1,
                max_version: 3,
            },
        );
        client.set_api_paths(paths);
        client
    }

    #[tokio::test]
    async fn list_songs_parses_response() {
        let server = MockServer::start().await;
        let fixture = include_str!("../../tests/fixtures/song_list_response.json");
        let body: serde_json::Value = serde_json::from_str(fixture).unwrap();

        Mock::given(method("GET"))
            .and(query_param("api", "SYNO.AudioStation.Song"))
            .and(query_param("method", "list"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;

        let client = client_with_song_api(&server).await;
        let api = SongApi::new(&client);
        let data = api.list(0, 50).await.unwrap();

        assert_eq!(data.total, 1234);
        assert_eq!(data.songs.len(), 2);
        assert_eq!(data.songs[0].id, "music_12345");
    }

    #[tokio::test]
    async fn search_songs_sends_keyword() {
        let server = MockServer::start().await;
        let fixture = include_str!("../../tests/fixtures/song_list_response.json");
        let body: serde_json::Value = serde_json::from_str(fixture).unwrap();

        Mock::given(method("GET"))
            .and(query_param("api", "SYNO.AudioStation.Song"))
            .and(query_param("method", "search"))
            .and(query_param("keyword", "Pink Floyd"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;

        let client = client_with_song_api(&server).await;
        let api = SongApi::new(&client);
        let data = api.search("Pink Floyd", 0, 50).await.unwrap();

        assert!(!data.songs.is_empty());
    }

    #[tokio::test]
    async fn set_rating_sends_correct_params() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(query_param("api", "SYNO.AudioStation.Song"))
            .and(query_param("method", "setrating"))
            .and(query_param("id", "music_123"))
            .and(query_param("rating", "5"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"success": true, "data": {}})),
            )
            .mount(&server)
            .await;

        let client = client_with_song_api(&server).await;
        let api = SongApi::new(&client);
        api.set_rating("music_123", 5).await.unwrap();
    }
}
