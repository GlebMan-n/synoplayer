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
    #[tokio::test]
    #[ignore]
    async fn list_artists_parses_response() {
        todo!()
    }
}
