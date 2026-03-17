use crate::api::client::SynoClient;
use crate::api::types::{ApiInfoMap, AuthData};
use crate::error::Result;

/// Authentication operations for Synology API.
///
/// Responsible only for login/logout and API discovery.
/// Does not handle credential storage (that's `credentials` module).
pub struct AuthApi<'a> {
    pub client: &'a mut SynoClient,
}

impl<'a> AuthApi<'a> {
    pub fn new(client: &'a mut SynoClient) -> Self {
        Self { client }
    }

    /// Discover all available APIs and cache their paths.
    pub async fn discover(&mut self) -> Result<()> {
        let info: ApiInfoMap = self
            .client
            .request("SYNO.API.Info", 1, "query", &[("query", "all")])
            .await?;

        self.client.set_api_paths(info);
        Ok(())
    }

    /// Login to Synology DSM.
    pub async fn login(&mut self, username: &str, password: &str) -> Result<()> {
        let data: AuthData = self
            .client
            .request(
                "SYNO.API.Auth",
                6,
                "login",
                &[
                    ("account", username),
                    ("passwd", password),
                    ("session", "AudioStation"),
                    ("format", "sid"),
                ],
            )
            .await?;

        self.client.set_sid(data.sid);
        self.client
            .set_credentials(username.to_string(), password.to_string());
        Ok(())
    }

    /// Logout from Synology DSM.
    pub async fn logout(&mut self) -> Result<()> {
        if self.client.is_authenticated() {
            let _: serde_json::Value = self
                .client
                .request("SYNO.API.Auth", 6, "logout", &[("session", "AudioStation")])
                .await
                .unwrap_or_default();

            self.client.clear_sid();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn login_sets_sid_on_success() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(query_param("api", "SYNO.API.Auth"))
            .and(query_param("method", "login"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({"success": true, "data": {"sid": "session_abc"}}),
            ))
            .mount(&server)
            .await;

        let mut client = SynoClient::new(&server.uri());
        let mut auth = AuthApi::new(&mut client);
        auth.login("admin", "password123").await.unwrap();

        assert!(auth.client.is_authenticated());
        assert_eq!(auth.client.sid(), Some("session_abc"));
    }

    #[tokio::test]
    async fn login_returns_error_on_wrong_password() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(query_param("api", "SYNO.API.Auth"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"success": false, "error": {"code": 400}})),
            )
            .mount(&server)
            .await;

        let mut client = SynoClient::new(&server.uri());
        let mut auth = AuthApi::new(&mut client);
        let result = auth.login("admin", "wrong").await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            crate::error::SynoError::InvalidCredentials
        ));
        assert!(!auth.client.is_authenticated());
    }

    #[tokio::test]
    async fn login_returns_2fa_required() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(query_param("api", "SYNO.API.Auth"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"success": false, "error": {"code": 403}})),
            )
            .mount(&server)
            .await;

        let mut client = SynoClient::new(&server.uri());
        let mut auth = AuthApi::new(&mut client);
        let result = auth.login("admin", "pass").await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            crate::error::SynoError::TwoFactorRequired
        ));
    }

    #[tokio::test]
    async fn logout_clears_sid() {
        let server = MockServer::start().await;
        // Login mock
        Mock::given(method("GET"))
            .and(query_param("method", "login"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"success": true, "data": {"sid": "ses123"}})),
            )
            .mount(&server)
            .await;
        // Logout mock
        Mock::given(method("GET"))
            .and(query_param("method", "logout"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"success": true, "data": {}})),
            )
            .mount(&server)
            .await;

        let mut client = SynoClient::new(&server.uri());
        let mut auth = AuthApi::new(&mut client);
        auth.login("admin", "pass").await.unwrap();
        assert!(auth.client.is_authenticated());

        auth.logout().await.unwrap();
        assert!(!auth.client.is_authenticated());
    }

    #[tokio::test]
    async fn discover_populates_api_paths() {
        let server = MockServer::start().await;
        let fixture = include_str!("../../tests/fixtures/api_info_response.json");
        let body: serde_json::Value = serde_json::from_str(fixture).unwrap();

        Mock::given(method("GET"))
            .and(query_param("api", "SYNO.API.Info"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;

        let mut client = SynoClient::new(&server.uri());
        let mut auth = AuthApi::new(&mut client);
        auth.discover().await.unwrap();

        let paths = auth.client.api_paths();
        assert!(paths.contains_key("SYNO.AudioStation.Song"));
        assert!(paths.contains_key("SYNO.AudioStation.Stream"));
        assert_eq!(
            paths["SYNO.AudioStation.Song"].path,
            "AudioStation/song.cgi"
        );
    }
}
