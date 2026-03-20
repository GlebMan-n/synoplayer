use crate::api::client::SynoClient;
use crate::api::playlist::PlaylistApi;
use crate::api::types::Song;
use crate::error::Result;

/// Manages favorites as a dedicated playlist on the NAS.
///
/// The Synology Pin API only supports pinning categories
/// (artists, genres, folders, playlists), not individual songs.
/// This module implements per-song favorites using a regular
/// playlist whose name is configurable (default: "Favorites").
pub struct FavoritesApi<'a> {
    playlist_api: PlaylistApi<'a>,
    playlist_name: String,
}

impl<'a> FavoritesApi<'a> {
    pub fn new(client: &'a SynoClient, playlist_name: &str) -> Self {
        Self {
            playlist_api: PlaylistApi::new(client),
            playlist_name: playlist_name.to_string(),
        }
    }

    /// List all songs in the favorites playlist.
    pub async fn list(&self) -> Result<Vec<Song>> {
        let id = match self.find_playlist_id().await? {
            Some(id) => id,
            None => return Ok(Vec::new()),
        };
        let detail = self.playlist_api.get_info(&id).await?;
        let pl = detail.into_playlist().ok_or_else(|| {
            crate::error::SynoError::Api {
                code: 0,
                message: "Playlist not found".to_string(),
            }
        })?;
        Ok(pl.all_songs().to_vec())
    }

    /// Add a song to favorites. Creates the playlist if needed.
    pub async fn add(&self, song_id: &str) -> Result<()> {
        let id = self.ensure_playlist().await?;
        // Get current songs, append new one, update
        let detail = self.playlist_api.get_info(&id).await?;
        let pl = detail.into_playlist().ok_or_else(|| {
            crate::error::SynoError::Api {
                code: 0,
                message: "Playlist not found".to_string(),
            }
        })?;
        let songs = pl.all_songs();
        let mut ids: Vec<&str> = songs
            .iter()
            .map(|s| s.id.as_str())
            .collect();
        if ids.contains(&song_id) {
            return Ok(()); // already in favorites
        }
        ids.push(song_id);
        self.playlist_api.update_songs(&id, &ids).await
    }

    /// Remove a song from favorites.
    pub async fn remove(&self, song_id: &str) -> Result<()> {
        let id = match self.find_playlist_id().await? {
            Some(id) => id,
            None => return Ok(()), // no playlist = nothing to remove
        };
        let detail = self.playlist_api.get_info(&id).await?;
        let pl = detail.into_playlist().ok_or_else(|| {
            crate::error::SynoError::Api {
                code: 0,
                message: "Playlist not found".to_string(),
            }
        })?;
        let remaining: Vec<&str> = pl
            .all_songs()
            .iter()
            .filter(|s| s.id != song_id)
            .map(|s| s.id.as_str())
            .collect();
        self.playlist_api.update_songs(&id, &remaining).await
    }

    /// Check if a song is in favorites.
    pub async fn contains(&self, song_id: &str) -> Result<bool> {
        let id = match self.find_playlist_id().await? {
            Some(id) => id,
            None => return Ok(false),
        };
        let detail = self.playlist_api.get_info(&id).await?;
        let pl = detail.into_playlist().ok_or_else(|| {
            crate::error::SynoError::Api {
                code: 0,
                message: "Playlist not found".to_string(),
            }
        })?;
        Ok(pl.all_songs().iter().any(|s| s.id == song_id))
    }

    /// Find the favorites playlist ID, or None if it doesn't exist.
    async fn find_playlist_id(&self) -> Result<Option<String>> {
        for lib in &["personal", "shared"] {
            let data = self
                .playlist_api
                .list(0, 500, Some(lib))
                .await?;
            for pl in &data.playlists {
                if pl.name == self.playlist_name {
                    return Ok(Some(pl.id.clone()));
                }
            }
        }
        Ok(None)
    }

    /// Ensure the favorites playlist exists, creating it if needed.
    async fn ensure_playlist(&self) -> Result<String> {
        if let Some(id) = self.find_playlist_id().await? {
            return Ok(id);
        }
        self.playlist_api
            .create(&self.playlist_name, "personal")
            .await?;
        // Fetch the newly created playlist's ID
        self.find_playlist_id()
            .await?
            .ok_or_else(|| {
                crate::error::SynoError::Player(
                    "Failed to create favorites playlist"
                        .to_string(),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::types::ApiInfo;
    use std::collections::HashMap;
    use wiremock::matchers::{method, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn client_with_playlist(
        server: &MockServer,
    ) -> SynoClient {
        let mut client = SynoClient::new(&server.uri());
        client.set_sid("sid".to_string());
        let mut paths = HashMap::new();
        paths.insert(
            "SYNO.AudioStation.Playlist".to_string(),
            ApiInfo {
                path: "AudioStation/playlist.cgi".to_string(),
                min_version: 1,
                max_version: 3,
            },
        );
        client.set_api_paths(paths);
        client
    }

    fn mock_list_with_favorites() -> serde_json::Value {
        serde_json::json!({
            "success": true,
            "data": {
                "total": 1,
                "offset": 0,
                "playlists": [{
                    "id": "playlist_personal_normal/99",
                    "name": "Favorites",
                    "additional": {
                        "songs": [
                            {
                                "id": "music_1",
                                "title": "Song One",
                                "path": ""
                            },
                            {
                                "id": "music_2",
                                "title": "Song Two",
                                "path": ""
                            }
                        ],
                        "songs_offset": 0,
                        "songs_total": 2
                    }
                }]
            }
        })
    }

    fn mock_list_empty() -> serde_json::Value {
        serde_json::json!({
            "success": true,
            "data": {
                "total": 0,
                "offset": 0,
                "playlists": []
            }
        })
    }

    #[tokio::test]
    async fn list_returns_songs_from_playlist() {
        let server = MockServer::start().await;
        // list call to find playlist ID
        Mock::given(method("GET"))
            .and(query_param("method", "list"))
            .and(query_param("library", "personal"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(mock_list_with_favorites()),
            )
            .mount(&server)
            .await;
        // getinfo call for song details
        Mock::given(method("GET"))
            .and(query_param("method", "getinfo"))
            .and(query_param(
                "id",
                "playlist_personal_normal/99",
            ))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({
                        "success": true,
                        "data": {
                            "playlist": {
                                "id": "playlist_personal_normal/99",
                                "name": "Favorites",
                                "songs": [
                                    {"id": "music_1", "title": "Song One", "path": ""},
                                    {"id": "music_2", "title": "Song Two", "path": ""}
                                ]
                            }
                        }
                    })),
            )
            .mount(&server)
            .await;

        let client = client_with_playlist(&server).await;
        let api = FavoritesApi::new(&client, "Favorites");
        let songs = api.list().await.unwrap();
        assert_eq!(songs.len(), 2);
        assert_eq!(songs[0].id, "music_1");
    }

    #[tokio::test]
    async fn list_returns_empty_when_no_playlist() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(query_param("method", "list"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(mock_list_empty()),
            )
            .mount(&server)
            .await;

        let client = client_with_playlist(&server).await;
        let api = FavoritesApi::new(&client, "Favorites");
        let songs = api.list().await.unwrap();
        assert!(songs.is_empty());
    }
}
