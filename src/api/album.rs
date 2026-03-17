use crate::api::client::SynoClient;
use crate::api::types::AlbumListData;
use crate::error::Result;

/// Operations on albums (SYNO.AudioStation.Album).
pub struct AlbumApi<'a> {
    client: &'a SynoClient,
}

impl<'a> AlbumApi<'a> {
    pub fn new(client: &'a SynoClient) -> Self {
        Self { client }
    }

    pub async fn list(&self, offset: i64, limit: i64) -> Result<AlbumListData> {
        let offset_str = offset.to_string();
        let limit_str = limit.to_string();
        self.client.request(
            "SYNO.AudioStation.Album",
            3,
            "list",
            &[("offset", &offset_str), ("limit", &limit_str)],
        ).await
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    #[ignore]
    async fn list_albums_parses_response() {
        todo!()
    }
}
