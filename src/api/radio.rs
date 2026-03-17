use crate::api::client::SynoClient;
use crate::error::Result;

/// Internet radio operations (SYNO.AudioStation.Radio).
pub struct RadioApi<'a> {
    client: &'a SynoClient,
}

impl<'a> RadioApi<'a> {
    pub fn new(client: &'a SynoClient) -> Self {
        Self { client }
    }

    pub async fn list(&self, offset: i64, limit: i64) -> Result<serde_json::Value> {
        let offset_str = offset.to_string();
        let limit_str = limit.to_string();
        self.client.request(
            "SYNO.AudioStation.Radio", 2, "list",
            &[("offset", &offset_str), ("limit", &limit_str)],
        ).await
    }

    pub async fn add(&self, title: &str, url: &str) -> Result<()> {
        let _: serde_json::Value = self.client.request(
            "SYNO.AudioStation.Radio", 2, "add",
            &[("title", title), ("url", url)],
        ).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    #[ignore]
    async fn list_radio_stations() {
        todo!()
    }
}
