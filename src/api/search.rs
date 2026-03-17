use crate::api::client::SynoClient;
use crate::api::types::SearchData;
use crate::error::Result;

/// Global search (SYNO.AudioStation.Search).
pub struct SearchApi<'a> {
    client: &'a SynoClient,
}

impl<'a> SearchApi<'a> {
    pub fn new(client: &'a SynoClient) -> Self {
        Self { client }
    }

    pub async fn search(&self, keyword: &str, offset: i64, limit: i64) -> Result<SearchData> {
        let offset_str = offset.to_string();
        let limit_str = limit.to_string();
        self.client
            .request(
                "SYNO.AudioStation.Search",
                1,
                "list",
                &[
                    ("keyword", keyword),
                    ("offset", &offset_str),
                    ("limit", &limit_str),
                    ("additional", "song_tag,song_audio,song_rating"),
                ],
            )
            .await
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
    async fn search_returns_songs_albums_artists() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(query_param("api", "SYNO.AudioStation.Search"))
            .and(query_param("keyword", "test"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "success": true,
                "data": {
                    "songs": [{"id": "music_1", "title": "Test Song", "path": ""}],
                    "albums": [{"name": "Test Album", "artist": "A", "album_artist": "", "year": 2020, "display_artist": ""}],
                    "artists": [{"name": "Test Artist"}]
                }
            })))
            .mount(&server)
            .await;

        let mut client = SynoClient::new(&server.uri());
        client.set_sid("sid".to_string());
        let mut paths = HashMap::new();
        paths.insert(
            "SYNO.AudioStation.Search".to_string(),
            ApiInfo {
                path: "AudioStation/search.cgi".to_string(),
                min_version: 1,
                max_version: 1,
            },
        );
        client.set_api_paths(paths);

        let api = SearchApi::new(&client);
        let data = api.search("test", 0, 50).await.unwrap();
        assert_eq!(data.songs.len(), 1);
        assert_eq!(data.albums.len(), 1);
        assert_eq!(data.artists.len(), 1);
    }
}
