use crate::api::client::SynoClient;
use crate::api::types::PinListData;
use crate::error::Result;

/// Favorites / Pin operations (SYNO.AudioStation.Pin).
pub struct PinApi<'a> {
    client: &'a SynoClient,
}

impl<'a> PinApi<'a> {
    pub fn new(client: &'a SynoClient) -> Self {
        Self { client }
    }

    pub async fn list(&self) -> Result<PinListData> {
        self.client
            .request("SYNO.AudioStation.Pin", 1, "list", &[("limit", "200")])
            .await
    }

    pub async fn pin(&self, id: &str) -> Result<()> {
        let _: serde_json::Value = self
            .client
            .request("SYNO.AudioStation.Pin", 1, "pin", &[("id", id)])
            .await?;
        Ok(())
    }

    pub async fn unpin(&self, id: &str) -> Result<()> {
        let _: serde_json::Value = self
            .client
            .request("SYNO.AudioStation.Pin", 1, "unpin", &[("id", id)])
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

    async fn client_with_pin(server: &MockServer) -> SynoClient {
        let mut client = SynoClient::new(&server.uri());
        client.set_sid("sid".to_string());
        let mut paths = HashMap::new();
        paths.insert(
            "SYNO.AudioStation.Pin".to_string(),
            ApiInfo {
                path: "entry.cgi".to_string(),
                min_version: 1,
                max_version: 1,
            },
        );
        client.set_api_paths(paths);
        client
    }

    #[tokio::test]
    async fn list_pinned_items() {
        let server = MockServer::start().await;
        let fixture = include_str!("../../tests/fixtures/pin_list_response.json");
        let body: serde_json::Value = serde_json::from_str(fixture).unwrap();

        Mock::given(method("GET"))
            .and(query_param("api", "SYNO.AudioStation.Pin"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;

        let client = client_with_pin(&server).await;
        let api = PinApi::new(&client);
        let data = api.list().await.unwrap();
        assert_eq!(data.items.len(), 2);
    }

    #[tokio::test]
    async fn pin_sends_id() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(query_param("api", "SYNO.AudioStation.Pin"))
            .and(query_param("method", "pin"))
            .and(query_param("id", "music_1"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"success": true})),
            )
            .mount(&server)
            .await;

        let client = client_with_pin(&server).await;
        let api = PinApi::new(&client);
        api.pin("music_1").await.unwrap();
    }

    #[tokio::test]
    async fn unpin_sends_id() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(query_param("api", "SYNO.AudioStation.Pin"))
            .and(query_param("method", "unpin"))
            .and(query_param("id", "music_1"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"success": true})),
            )
            .mount(&server)
            .await;

        let client = client_with_pin(&server).await;
        let api = PinApi::new(&client);
        api.unpin("music_1").await.unwrap();
    }

    #[tokio::test]
    async fn list_returns_total_count() {
        let server = MockServer::start().await;
        let fixture = include_str!("../../tests/fixtures/pin_list_response.json");
        let body: serde_json::Value = serde_json::from_str(fixture).unwrap();

        Mock::given(method("GET"))
            .and(query_param("api", "SYNO.AudioStation.Pin"))
            .and(query_param("method", "list"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;

        let client = client_with_pin(&server).await;
        let api = PinApi::new(&client);
        let data = api.list().await.unwrap();
        assert_eq!(data.total, 2);
        assert_eq!(data.items.len(), 2);
    }

    #[tokio::test]
    async fn list_parses_item_types() {
        let server = MockServer::start().await;
        let fixture = include_str!("../../tests/fixtures/pin_list_response.json");
        let body: serde_json::Value = serde_json::from_str(fixture).unwrap();

        Mock::given(method("GET"))
            .and(query_param("api", "SYNO.AudioStation.Pin"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;

        let client = client_with_pin(&server).await;
        let api = PinApi::new(&client);
        let data = api.list().await.unwrap();

        assert_eq!(data.items[0].item_type, "song");
        assert_eq!(data.items[0].title, "Comfortably Numb");
        assert_eq!(data.items[1].item_type, "album");
        assert_eq!(data.items[1].name, "My Fav Album");
    }
}
