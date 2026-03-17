use crate::api::client::SynoClient;
use crate::api::types::{PlaylistDetailData, PlaylistListData};
use crate::error::Result;

/// Operations on playlists (SYNO.AudioStation.Playlist).
pub struct PlaylistApi<'a> {
    client: &'a SynoClient,
}

impl<'a> PlaylistApi<'a> {
    pub fn new(client: &'a SynoClient) -> Self {
        Self { client }
    }

    pub async fn list(
        &self,
        offset: i64,
        limit: i64,
        library: Option<&str>,
    ) -> Result<PlaylistListData> {
        let offset_str = offset.to_string();
        let limit_str = limit.to_string();
        let mut params = vec![
            ("offset", offset_str.as_str()),
            ("limit", limit_str.as_str()),
        ];
        if let Some(lib) = library {
            params.push(("library", lib));
        }
        self.client
            .request("SYNO.AudioStation.Playlist", 3, "list", &params)
            .await
    }

    pub async fn get_info(&self, id: &str) -> Result<PlaylistDetailData> {
        let library = if id.contains("shared") {
            "shared"
        } else {
            "personal"
        };
        self.client
            .request(
                "SYNO.AudioStation.Playlist",
                3,
                "getinfo",
                &[("id", id), ("library", library), ("additional", "songs")],
            )
            .await
    }

    pub async fn create(&self, name: &str, library: &str) -> Result<()> {
        let _: serde_json::Value = self
            .client
            .request(
                "SYNO.AudioStation.Playlist",
                3,
                "create",
                &[("name", name), ("library", library)],
            )
            .await?;
        Ok(())
    }

    /// Create a playlist with initial songs.
    pub async fn create_with_songs(
        &self,
        name: &str,
        library: &str,
        song_ids: &[&str],
    ) -> Result<()> {
        let songs = song_ids.join(",");
        let _: serde_json::Value = self
            .client
            .request(
                "SYNO.AudioStation.Playlist",
                3,
                "create",
                &[("name", name), ("library", library), ("songs", &songs)],
            )
            .await?;
        Ok(())
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        let _: serde_json::Value = self
            .client
            .request("SYNO.AudioStation.Playlist", 3, "delete", &[("id", id)])
            .await?;
        Ok(())
    }

    pub async fn rename(&self, id: &str, new_name: &str) -> Result<()> {
        let _: serde_json::Value = self
            .client
            .request(
                "SYNO.AudioStation.Playlist",
                3,
                "rename",
                &[("id", id), ("new_name", new_name)],
            )
            .await?;
        Ok(())
    }

    pub async fn update_songs(&self, id: &str, song_ids: &[&str]) -> Result<()> {
        let songs = song_ids.join(",");
        let _: serde_json::Value = self
            .client
            .request(
                "SYNO.AudioStation.Playlist",
                3,
                "updatesongs",
                &[("id", id), ("songs", &songs)],
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

    async fn client_with_playlist_api(server: &MockServer) -> SynoClient {
        let mut client = SynoClient::new(&server.uri());
        client.set_sid("sid".to_string());
        let mut paths = HashMap::new();
        paths.insert(
            "SYNO.AudioStation.Playlist".to_string(),
            ApiInfo {
                path: "AudioStation/playlist.cgi".to_string(),
                min_version: 1,
                max_version: 3,
            },
        );
        client.set_api_paths(paths);
        client
    }

    #[tokio::test]
    async fn list_playlists_parses_response() {
        let server = MockServer::start().await;
        let fixture = include_str!("../../tests/fixtures/playlist_list_response.json");
        let body: serde_json::Value = serde_json::from_str(fixture).unwrap();

        Mock::given(method("GET"))
            .and(query_param("api", "SYNO.AudioStation.Playlist"))
            .and(query_param("method", "list"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;

        let client = client_with_playlist_api(&server).await;
        let api = PlaylistApi::new(&client);
        let data = api.list(0, 50, None).await.unwrap();
        assert_eq!(data.total, 3);
        assert_eq!(data.playlists[0].name, "My Favorites");
    }

    #[tokio::test]
    async fn get_playlist_info_returns_songs() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(query_param("method", "getinfo"))
            .and(query_param("id", "playlist_1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "success": true,
                "data": {
                    "playlist": {
                        "id": "playlist_1",
                        "name": "My Favorites",
                        "songs": [{"id": "music_1", "title": "Song 1", "path": ""}]
                    }
                }
            })))
            .mount(&server)
            .await;

        let client = client_with_playlist_api(&server).await;
        let api = PlaylistApi::new(&client);
        let data = api.get_info("playlist_1").await.unwrap();
        let pl = data.into_playlist().unwrap();
        assert_eq!(pl.name, "My Favorites");
        assert_eq!(pl.songs.len(), 1);
    }

    #[tokio::test]
    async fn create_playlist_sends_name() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(query_param("method", "create"))
            .and(query_param("name", "New Playlist"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"success": true, "data": {}})),
            )
            .mount(&server)
            .await;

        let client = client_with_playlist_api(&server).await;
        let api = PlaylistApi::new(&client);
        api.create("New Playlist", "personal").await.unwrap();
    }
}
