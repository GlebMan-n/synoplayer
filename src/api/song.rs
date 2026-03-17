use crate::api::client::SynoClient;
use crate::api::types::{SongListData, Song};
use crate::error::Result;

/// Operations on songs (SYNO.AudioStation.Song).
pub struct SongApi<'a> {
    client: &'a SynoClient,
}

impl<'a> SongApi<'a> {
    pub fn new(client: &'a SynoClient) -> Self {
        Self { client }
    }

    pub async fn list(&self, offset: i64, limit: i64) -> Result<SongListData> {
        let offset_str = offset.to_string();
        let limit_str = limit.to_string();
        self.client.request(
            "SYNO.AudioStation.Song",
            3,
            "list",
            &[
                ("offset", &offset_str),
                ("limit", &limit_str),
                ("additional", "song_tag,song_audio,song_rating"),
            ],
        ).await
    }

    pub async fn search(&self, keyword: &str, offset: i64, limit: i64) -> Result<SongListData> {
        let offset_str = offset.to_string();
        let limit_str = limit.to_string();
        self.client.request(
            "SYNO.AudioStation.Song",
            3,
            "search",
            &[
                ("keyword", keyword),
                ("offset", &offset_str),
                ("limit", &limit_str),
                ("additional", "song_tag,song_audio,song_rating"),
            ],
        ).await
    }

    pub async fn get_info(&self, id: &str) -> Result<Song> {
        // API returns SongListData with single song for getinfo too
        let data: SongListData = self.client.request(
            "SYNO.AudioStation.Song",
            3,
            "getinfo",
            &[
                ("id", id),
                ("additional", "song_tag,song_audio,song_rating"),
            ],
        ).await?;

        data.songs.into_iter().next().ok_or_else(|| {
            crate::error::SynoError::Api {
                code: 100,
                message: format!("Song not found: {id}"),
            }
        })
    }

    pub async fn set_rating(&self, id: &str, rating: i32) -> Result<()> {
        let rating_str = rating.to_string();
        let _: serde_json::Value = self.client.request(
            "SYNO.AudioStation.Song",
            2,
            "setrating",
            &[("id", id), ("rating", &rating_str)],
        ).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    #[ignore]
    async fn list_songs_parses_response() {
        todo!()
    }

    #[tokio::test]
    #[ignore]
    async fn search_songs_sends_keyword() {
        todo!()
    }

    #[tokio::test]
    #[ignore]
    async fn set_rating_sends_correct_params() {
        todo!()
    }
}
