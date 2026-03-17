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
        self.client.request("SYNO.AudioStation.Pin", 1, "list", &[]).await
    }

    pub async fn pin(&self, id: &str) -> Result<()> {
        let _: serde_json::Value = self.client.request(
            "SYNO.AudioStation.Pin", 1, "pin", &[("id", id)],
        ).await?;
        Ok(())
    }

    pub async fn unpin(&self, id: &str) -> Result<()> {
        let _: serde_json::Value = self.client.request(
            "SYNO.AudioStation.Pin", 1, "unpin", &[("id", id)],
        ).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    #[ignore]
    async fn list_pinned_items() {
        todo!()
    }

    #[tokio::test]
    #[ignore]
    async fn pin_and_unpin_item() {
        todo!()
    }
}
