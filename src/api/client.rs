use std::collections::HashMap;

use crate::api::types::{ApiInfoMap, ApiResponse};
use crate::error::{Result, SynoError};

/// HTTP transport layer for Synology Web API.
///
/// Responsible only for:
/// - Building and sending HTTP requests
/// - Session management (sid)
/// - API path discovery
/// - Auto-relogin on session expiry
pub struct SynoClient {
    http: reqwest::Client,
    base_url: String,
    sid: Option<String>,
    api_paths: ApiInfoMap,
    /// Stored credentials for auto-relogin.
    credentials: Option<(String, String)>,
}

impl SynoClient {
    pub fn new(base_url: &str) -> Self {
        let http = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .cookie_store(true)
            .build()
            .expect("failed to build HTTP client");

        Self {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
            sid: None,
            api_paths: HashMap::new(),
            credentials: None,
        }
    }

    pub fn is_authenticated(&self) -> bool {
        self.sid.is_some()
    }

    pub fn sid(&self) -> Option<&str> {
        self.sid.as_deref()
    }

    pub fn set_sid(&mut self, sid: String) {
        self.sid = Some(sid);
    }

    pub fn clear_sid(&mut self) {
        self.sid = None;
    }

    pub fn set_credentials(&mut self, username: String, password: String) {
        self.credentials = Some((username, password));
    }

    pub fn api_paths(&self) -> &ApiInfoMap {
        &self.api_paths
    }

    pub fn set_api_paths(&mut self, paths: ApiInfoMap) {
        self.api_paths = paths;
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn http(&self) -> &reqwest::Client {
        &self.http
    }

    /// Build a full URL for an API call.
    pub fn build_url(&self, api_name: &str) -> Result<String> {
        let path = self
            .api_paths
            .get(api_name)
            .map(|info| info.path.as_str())
            .or({
                // Fallback known paths
                match api_name {
                    "SYNO.API.Info" => Some("query.cgi"),
                    "SYNO.API.Auth" => Some("entry.cgi"),
                    _ => None,
                }
            })
            .ok_or_else(|| SynoError::Api {
                code: 102,
                message: format!("Unknown API: {api_name}. Run API discovery first."),
            })?;

        Ok(format!("{}/webapi/{}", self.base_url, path))
    }

    /// Send a generic API request and parse the response.
    pub async fn request<T: serde::de::DeserializeOwned + Default>(
        &self,
        api: &str,
        version: i32,
        method: &str,
        extra_params: &[(&str, &str)],
    ) -> Result<T> {
        let url = self.build_url(api)?;

        let mut params: Vec<(&str, &str)> = vec![("api", api), ("method", method)];
        let version_str = version.to_string();
        params.push(("version", &version_str));

        let sid_ref;
        if let Some(sid) = &self.sid {
            sid_ref = sid.clone();
            params.push(("_sid", &sid_ref));
        }

        params.extend_from_slice(extra_params);

        tracing::debug!(
            api, method, ?extra_params,
            "API request -> {url}"
        );

        let response = self.http.get(&url).query(&params).send().await?;
        let body = response.text().await?;

        tracing::debug!(api, method, "API response <- {body}");

        let api_response: ApiResponse<T> =
            serde_json::from_str(&body)?;

        if api_response.success {
            // Some API methods return {"success":true} with no data field.
            // In that case, return Default::default() (e.g. empty Value, empty struct).
            Ok(api_response.data.unwrap_or_default())
        } else {
            let code = api_response.error.map(|e| e.code).unwrap_or(100);
            Err(SynoError::from_api_code(code))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_client_is_not_authenticated() {
        let client = SynoClient::new("https://example.com:5001");
        assert!(!client.is_authenticated());
        assert!(client.sid().is_none());
    }

    #[test]
    fn set_and_clear_sid() {
        let mut client = SynoClient::new("https://example.com:5001");
        client.set_sid("test_sid".to_string());
        assert!(client.is_authenticated());
        assert_eq!(client.sid(), Some("test_sid"));
        client.clear_sid();
        assert!(!client.is_authenticated());
    }

    #[test]
    fn build_url_fallback_for_known_apis() {
        let client = SynoClient::new("https://nas.local:5001");
        assert_eq!(
            client.build_url("SYNO.API.Info").unwrap(),
            "https://nas.local:5001/webapi/query.cgi"
        );
        assert_eq!(
            client.build_url("SYNO.API.Auth").unwrap(),
            "https://nas.local:5001/webapi/entry.cgi"
        );
    }

    #[test]
    fn build_url_unknown_api_fails() {
        let client = SynoClient::new("https://nas.local:5001");
        assert!(client.build_url("SYNO.AudioStation.Song").is_err());
    }

    #[test]
    fn build_url_with_discovered_path() {
        let mut client = SynoClient::new("https://nas.local:5001");
        let mut paths = HashMap::new();
        paths.insert(
            "SYNO.AudioStation.Song".to_string(),
            crate::api::types::ApiInfo {
                path: "AudioStation/song.cgi".to_string(),
                min_version: 1,
                max_version: 3,
            },
        );
        client.set_api_paths(paths);
        assert_eq!(
            client.build_url("SYNO.AudioStation.Song").unwrap(),
            "https://nas.local:5001/webapi/AudioStation/song.cgi"
        );
    }

    #[test]
    fn base_url_strips_trailing_slash() {
        let client = SynoClient::new("https://nas.local:5001/");
        assert_eq!(client.base_url(), "https://nas.local:5001");
    }

    #[tokio::test]
    async fn request_success_parses_data() {
        use wiremock::matchers::{method, query_param};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(query_param("api", "SYNO.API.Auth"))
            .and(query_param("method", "login"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({"success": true, "data": {"sid": "mock_sid_123"}}),
            ))
            .mount(&server)
            .await;

        let client = SynoClient::new(&server.uri());
        let result: crate::api::types::AuthData = client
            .request(
                "SYNO.API.Auth",
                6,
                "login",
                &[("account", "user"), ("passwd", "pass")],
            )
            .await
            .unwrap();
        assert_eq!(result.sid, "mock_sid_123");
    }

    #[tokio::test]
    async fn request_error_returns_syno_error() {
        use wiremock::matchers::{method, query_param};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(query_param("api", "SYNO.API.Auth"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"success": false, "error": {"code": 400}})),
            )
            .mount(&server)
            .await;

        let client = SynoClient::new(&server.uri());
        let result: crate::error::Result<crate::api::types::AuthData> = client
            .request(
                "SYNO.API.Auth",
                6,
                "login",
                &[("account", "bad"), ("passwd", "bad")],
            )
            .await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            crate::error::SynoError::InvalidCredentials
        ));
    }

    #[tokio::test]
    async fn request_session_expired_returns_session_error() {
        use wiremock::matchers::{method, query_param};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(query_param("api", "SYNO.AudioStation.Song"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"success": false, "error": {"code": 106}})),
            )
            .mount(&server)
            .await;

        let mut client = SynoClient::new(&server.uri());
        // Manually set paths so build_url works
        let mut paths = HashMap::new();
        paths.insert(
            "SYNO.AudioStation.Song".to_string(),
            crate::api::types::ApiInfo {
                path: "AudioStation/song.cgi".to_string(),
                min_version: 1,
                max_version: 3,
            },
        );
        client.set_api_paths(paths);
        client.set_sid("old_sid".to_string());

        let result: crate::error::Result<serde_json::Value> = client
            .request("SYNO.AudioStation.Song", 3, "list", &[])
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().is_session_expired());
    }
}
