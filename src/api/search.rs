use crate::api::client::SynoClient;
use crate::api::types::SearchData;
use crate::error::Result;

/// Global search (SYNO.AudioStation.Search).
pub struct SearchApi<'a> {
    client: &'a SynoClient,
}

impl<'a> SearchApi<'a> {
    pub fn new(client: &'a SynoClient) -> Self {
        Self { client }
    }

    pub async fn search(&self, keyword: &str, offset: i64, limit: i64) -> Result<SearchData> {
        let offset_str = offset.to_string();
        let limit_str = limit.to_string();
        self.client
            .request(
                "SYNO.AudioStation.Search",
                1,
                "list",
                &[
                    ("keyword", keyword),
                    ("offset", &offset_str),
                    ("limit", &limit_str),
                    ("additional", "song_tag,song_audio,song_rating"),
                ],
            )
            .await
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    #[ignore]
    async fn search_returns_songs_albums_artists() {
        todo!()
    }
}
