use crate::api::client::SynoClient;
use crate::api::types::{RadioListData, RadioStation};
use crate::error::Result;

/// Internet radio operations (SYNO.AudioStation.Radio).
pub struct RadioApi<'a> {
    client: &'a SynoClient,
}

impl<'a> RadioApi<'a> {
    pub fn new(client: &'a SynoClient) -> Self {
        Self { client }
    }

    pub async fn list(&self, offset: i64, limit: i64) -> Result<RadioListData> {
        let offset_str = offset.to_string();
        let limit_str = limit.to_string();
        self.client
            .request(
                "SYNO.AudioStation.Radio",
                2,
                "list",
                &[("offset", &offset_str), ("limit", &limit_str)],
            )
            .await
    }

    pub async fn add(&self, title: &str, url: &str) -> Result<()> {
        let _: serde_json::Value = self
            .client
            .request(
                "SYNO.AudioStation.Radio",
                2,
                "add",
                &[("title", title), ("url", url)],
            )
            .await?;
        Ok(())
    }

    /// Find a station by name (case-insensitive) or return by ID prefix.
    pub async fn find(&self, name_or_id: &str) -> Result<Option<RadioStation>> {
        let data = self.list(0, 200).await?;
        let station = data
            .radios
            .into_iter()
            .find(|r| r.id == name_or_id || r.title.eq_ignore_ascii_case(name_or_id));
        Ok(station)
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
    async fn list_radio_stations() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(query_param("api", "SYNO.AudioStation.Radio"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "success": true,
                "data": {"radios": [{"id": "1", "title": "BBC Radio", "url": "http://bbc.co.uk/stream"}]}
            })))
            .mount(&server)
            .await;

        let mut client = SynoClient::new(&server.uri());
        client.set_sid("sid".to_string());
        let mut paths = HashMap::new();
        paths.insert(
            "SYNO.AudioStation.Radio".to_string(),
            ApiInfo {
                path: "AudioStation/radio.cgi".to_string(),
                min_version: 1,
                max_version: 2,
            },
        );
        client.set_api_paths(paths);

        let api = RadioApi::new(&client);
        let data = api.list(0, 50).await.unwrap();
        assert_eq!(data.radios.len(), 1);
        assert_eq!(data.radios[0].title, "BBC Radio");
        assert_eq!(data.radios[0].url, "http://bbc.co.uk/stream");
    }

    #[tokio::test]
    async fn add_radio_station_sends_params() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(query_param("api", "SYNO.AudioStation.Radio"))
            .and(query_param("method", "add"))
            .and(query_param("title", "Jazz FM"))
            .and(query_param("url", "http://jazz.fm/stream"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "success": true
            })))
            .mount(&server)
            .await;

        let mut client = SynoClient::new(&server.uri());
        client.set_sid("sid".to_string());
        let mut paths = HashMap::new();
        paths.insert(
            "SYNO.AudioStation.Radio".to_string(),
            ApiInfo {
                path: "AudioStation/radio.cgi".to_string(),
                min_version: 1,
                max_version: 2,
            },
        );
        client.set_api_paths(paths);

        let api = RadioApi::new(&client);
        api.add("Jazz FM", "http://jazz.fm/stream").await.unwrap();
    }
}
