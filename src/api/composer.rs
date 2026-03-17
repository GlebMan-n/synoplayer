use crate::api::client::SynoClient;
use crate::api::types::ComposerListData;
use crate::error::Result;

/// Operations on composers (SYNO.AudioStation.Composer).
pub struct ComposerApi<'a> {
    client: &'a SynoClient,
}

impl<'a> ComposerApi<'a> {
    pub fn new(client: &'a SynoClient) -> Self {
        Self { client }
    }

    pub async fn list(&self, offset: i64, limit: i64) -> Result<ComposerListData> {
        let offset_str = offset.to_string();
        let limit_str = limit.to_string();
        self.client
            .request(
                "SYNO.AudioStation.Composer",
                2,
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
    async fn list_composers_parses_response() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(query_param("api", "SYNO.AudioStation.Composer"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "success": true,
                "data": {
                    "composers": [{"name": "Bach"}, {"name": "Mozart"}],
                    "total": 2, "offset": 0
                }
            })))
            .mount(&server)
            .await;

        let mut client = SynoClient::new(&server.uri());
        client.set_sid("sid".to_string());
        let mut paths = HashMap::new();
        paths.insert(
            "SYNO.AudioStation.Composer".to_string(),
            ApiInfo {
                path: "AudioStation/composer.cgi".to_string(),
                min_version: 1,
                max_version: 2,
            },
        );
        client.set_api_paths(paths);

        let api = ComposerApi::new(&client);
        let data = api.list(0, 50).await.unwrap();
        assert_eq!(data.total, 2);
        assert_eq!(data.composers[0].name, "Bach");
    }
}
