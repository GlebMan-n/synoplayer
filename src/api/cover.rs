use crate::api::client::SynoClient;
use crate::error::{Result, SynoError};

/// Cover art operations (SYNO.AudioStation.Cover).
pub struct CoverApi<'a> {
    client: &'a SynoClient,
}

impl<'a> CoverApi<'a> {
    pub fn new(client: &'a SynoClient) -> Self {
        Self { client }
    }

    /// Build URL for song cover image (returns binary image).
    pub fn song_cover_url(&self, song_id: &str) -> Result<String> {
        let base = self.client.build_url("SYNO.AudioStation.Cover")?;
        let sid = self.client.sid().ok_or(SynoError::NotAuthenticated)?;
        Ok(format!(
            "{}?api=SYNO.AudioStation.Cover&version=3&method=getsongcover&id={}&_sid={}",
            base, song_id, sid
        ))
    }

    /// Download cover image bytes.
    pub async fn get_song_cover(&self, song_id: &str) -> Result<bytes::Bytes> {
        let url = self.song_cover_url(song_id)?;
        let response = self.client.http().get(&url).send().await?;
        let bytes = response.bytes().await?;
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    #[ignore]
    async fn get_cover_returns_image_bytes() {
        todo!()
    }
}
