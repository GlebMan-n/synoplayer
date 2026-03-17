use crate::api::client::SynoClient;
use crate::api::types::{ApiInfoMap, AuthData};
use crate::error::Result;

/// Authentication operations for Synology API.
///
/// Responsible only for login/logout and API discovery.
/// Does not handle credential storage (that's `credentials` module).
pub struct AuthApi<'a> {
    client: &'a mut SynoClient,
}

impl<'a> AuthApi<'a> {
    pub fn new(client: &'a mut SynoClient) -> Self {
        Self { client }
    }

    /// Discover all available APIs and cache their paths.
    pub async fn discover(&mut self) -> Result<()> {
        let info: ApiInfoMap = self.client.request(
            "SYNO.API.Info",
            1,
            "query",
            &[("query", "all")],
        ).await?;

        self.client.set_api_paths(info);
        Ok(())
    }

    /// Login to Synology DSM.
    pub async fn login(&mut self, username: &str, password: &str) -> Result<()> {
        let data: AuthData = self.client.request(
            "SYNO.API.Auth",
            6,
            "login",
            &[
                ("account", username),
                ("passwd", password),
                ("session", "AudioStation"),
                ("format", "sid"),
            ],
        ).await?;

        self.client.set_sid(data.sid);
        self.client.set_credentials(username.to_string(), password.to_string());
        Ok(())
    }

    /// Logout from Synology DSM.
    pub async fn logout(&mut self) -> Result<()> {
        if self.client.is_authenticated() {
            let _: serde_json::Value = self.client.request(
                "SYNO.API.Auth",
                6,
                "logout",
                &[("session", "AudioStation")],
            ).await.unwrap_or_default();

            self.client.clear_sid();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    #[ignore] // enable when implementing with wiremock
    async fn login_sets_sid_on_success() {
        todo!()
    }

    #[tokio::test]
    #[ignore]
    async fn login_returns_error_on_wrong_password() {
        todo!()
    }

    #[tokio::test]
    #[ignore]
    async fn login_returns_2fa_required() {
        todo!()
    }

    #[tokio::test]
    #[ignore]
    async fn logout_clears_sid() {
        todo!()
    }

    #[tokio::test]
    #[ignore]
    async fn discover_populates_api_paths() {
        todo!()
    }
}
