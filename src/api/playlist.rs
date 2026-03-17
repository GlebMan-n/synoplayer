use crate::api::client::SynoClient;
use crate::api::types::{PlaylistDetailData, PlaylistListData};
use crate::error::Result;

/// Operations on playlists (SYNO.AudioStation.Playlist).
pub struct PlaylistApi<'a> {
    client: &'a SynoClient,
}

impl<'a> PlaylistApi<'a> {
    pub fn new(client: &'a SynoClient) -> Self {
        Self { client }
    }

    pub async fn list(&self, offset: i64, limit: i64) -> Result<PlaylistListData> {
        let offset_str = offset.to_string();
        let limit_str = limit.to_string();
        self.client
            .request(
                "SYNO.AudioStation.Playlist",
                3,
                "list",
                &[("offset", &offset_str), ("limit", &limit_str)],
            )
            .await
    }

    pub async fn get_info(&self, id: &str) -> Result<PlaylistDetailData> {
        self.client
            .request(
                "SYNO.AudioStation.Playlist",
                3,
                "getinfo",
                &[
                    ("id", id),
                    ("additional", "song_tag,song_audio,song_rating"),
                ],
            )
            .await
    }

    pub async fn create(&self, name: &str, library: &str) -> Result<()> {
        let _: serde_json::Value = self
            .client
            .request(
                "SYNO.AudioStation.Playlist",
                3,
                "create",
                &[("name", name), ("library", library)],
            )
            .await?;
        Ok(())
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        let _: serde_json::Value = self
            .client
            .request("SYNO.AudioStation.Playlist", 3, "delete", &[("id", id)])
            .await?;
        Ok(())
    }

    pub async fn rename(&self, id: &str, new_name: &str) -> Result<()> {
        let _: serde_json::Value = self
            .client
            .request(
                "SYNO.AudioStation.Playlist",
                3,
                "rename",
                &[("id", id), ("new_name", new_name)],
            )
            .await?;
        Ok(())
    }

    pub async fn update_songs(&self, id: &str, song_ids: &[&str]) -> Result<()> {
        let songs = song_ids.join(",");
        let _: serde_json::Value = self
            .client
            .request(
                "SYNO.AudioStation.Playlist",
                3,
                "updatesongs",
                &[("id", id), ("songs", &songs)],
            )
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    #[ignore]
    async fn list_playlists_parses_response() {
        todo!()
    }

    #[tokio::test]
    #[ignore]
    async fn get_playlist_info_returns_songs() {
        todo!()
    }

    #[tokio::test]
    #[ignore]
    async fn create_playlist_sends_name() {
        todo!()
    }
}
