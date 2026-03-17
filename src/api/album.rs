use crate::api::client::SynoClient;
use crate::api::types::AlbumListData;
use crate::error::Result;

/// Operations on albums (SYNO.AudioStation.Album).
pub struct AlbumApi<'a> {
    client: &'a SynoClient,
}

impl<'a> AlbumApi<'a> {
    pub fn new(client: &'a SynoClient) -> Self {
        Self { client }
    }

    pub async fn list(&self, offset: i64, limit: i64) -> Result<AlbumListData> {
        let offset_str = offset.to_string();
        let limit_str = limit.to_string();
        self.client
            .request(
                "SYNO.AudioStation.Album",
                3,
                "list",
                &[("offset", &offset_str), ("limit", &limit_str)],
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
    async fn list_albums_parses_response() {
        let server = MockServer::start().await;
        let fixture = include_str!("../../tests/fixtures/album_list_response.json");
        let body: serde_json::Value = serde_json::from_str(fixture).unwrap();

        Mock::given(method("GET"))
            .and(query_param("api", "SYNO.AudioStation.Album"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;

        let mut client = SynoClient::new(&server.uri());
        client.set_sid("sid".to_string());
        let mut paths = HashMap::new();
        paths.insert(
            "SYNO.AudioStation.Album".to_string(),
            ApiInfo {
                path: "AudioStation/album.cgi".to_string(),
                min_version: 1,
                max_version: 3,
            },
        );
        client.set_api_paths(paths);

        let api = AlbumApi::new(&client);
        let data = api.list(0, 50).await.unwrap();
        assert_eq!(data.total, 42);
        assert_eq!(data.albums[0].name, "The Dark Side of the Moon");
    }
}
