use crate::api::client::SynoClient;
use crate::api::types::LyricsData;
use crate::error::Result;

/// Lyrics operations (SYNO.AudioStation.Lyrics).
pub struct LyricsApi<'a> {
    client: &'a SynoClient,
}

impl<'a> LyricsApi<'a> {
    pub fn new(client: &'a SynoClient) -> Self {
        Self { client }
    }

    pub async fn get(&self, song_id: &str) -> Result<LyricsData> {
        self.client.request(
            "SYNO.AudioStation.Lyrics", 2, "getlyrics", &[("id", song_id)],
        ).await
    }

    pub async fn set(&self, song_id: &str, lyrics: &str) -> Result<()> {
        let _: serde_json::Value = self.client.request(
            "SYNO.AudioStation.Lyrics", 2, "setlyrics",
            &[("id", song_id), ("lyrics", lyrics)],
        ).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    #[ignore]
    async fn get_lyrics_parses_text() {
        todo!()
    }
}
