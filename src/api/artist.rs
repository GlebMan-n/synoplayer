use crate::api::client::SynoClient;
use crate::api::types::ArtistListData;
use crate::error::Result;

/// Operations on artists (SYNO.AudioStation.Artist).
pub struct ArtistApi<'a> {
    client: &'a SynoClient,
}

impl<'a> ArtistApi<'a> {
    pub fn new(client: &'a SynoClient) -> Self {
        Self { client }
    }

    pub async fn list(&self, offset: i64, limit: i64) -> Result<ArtistListData> {
        let offset_str = offset.to_string();
        let limit_str = limit.to_string();
        self.client
            .request(
                "SYNO.AudioStation.Artist",
                4,
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
    async fn list_artists_parses_response() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(query_param("api", "SYNO.AudioStation.Artist"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "success": true,
                "data": {
                    "artists": [{"name": "Pink Floyd"}, {"name": "Led Zeppelin"}],
                    "total": 2,
                    "offset": 0
                }
            })))
            .mount(&server)
            .await;

        let mut client = SynoClient::new(&server.uri());
        client.set_sid("sid".to_string());
        let mut paths = HashMap::new();
        paths.insert(
            "SYNO.AudioStation.Artist".to_string(),
            ApiInfo {
                path: "AudioStation/artist.cgi".to_string(),
                min_version: 1,
                max_version: 4,
            },
        );
        client.set_api_paths(paths);

        let api = ArtistApi::new(&client);
        let data = api.list(0, 50).await.unwrap();
        assert_eq!(data.total, 2);
        assert_eq!(data.artists[0].name, "Pink Floyd");
    }
}
