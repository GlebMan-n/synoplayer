use crate::api::client::SynoClient;
use crate::api::types::GenreListData;
use crate::error::Result;

/// Operations on genres (SYNO.AudioStation.Genre).
pub struct GenreApi<'a> {
    client: &'a SynoClient,
}

impl<'a> GenreApi<'a> {
    pub fn new(client: &'a SynoClient) -> Self {
        Self { client }
    }

    pub async fn list(&self, offset: i64, limit: i64) -> Result<GenreListData> {
        let offset_str = offset.to_string();
        let limit_str = limit.to_string();
        self.client
            .request(
                "SYNO.AudioStation.Genre",
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
    async fn list_genres_parses_response() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(query_param("api", "SYNO.AudioStation.Genre"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "success": true,
                "data": {
                    "genres": [{"name": "Rock"}, {"name": "Metal"}, {"name": "Jazz"}],
                    "total": 3, "offset": 0
                }
            })))
            .mount(&server)
            .await;

        let mut client = SynoClient::new(&server.uri());
        client.set_sid("sid".to_string());
        let mut paths = HashMap::new();
        paths.insert(
            "SYNO.AudioStation.Genre".to_string(),
            ApiInfo {
                path: "AudioStation/genre.cgi".to_string(),
                min_version: 1,
                max_version: 3,
            },
        );
        client.set_api_paths(paths);

        let api = GenreApi::new(&client);
        let data = api.list(0, 50).await.unwrap();
        assert_eq!(data.total, 3);
        assert_eq!(data.genres[0].name, "Rock");
        assert_eq!(data.genres[2].name, "Jazz");
    }
}
