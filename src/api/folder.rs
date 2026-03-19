use crate::api::client::SynoClient;
use crate::api::types::FolderListData;
use crate::error::Result;

/// Folder navigation (SYNO.AudioStation.Folder).
pub struct FolderApi<'a> {
    client: &'a SynoClient,
}

impl<'a> FolderApi<'a> {
    pub fn new(client: &'a SynoClient) -> Self {
        Self { client }
    }

    /// List folder contents. Pass empty `id` for root.
    pub async fn list(&self, id: Option<&str>, offset: i64, limit: i64) -> Result<FolderListData> {
        let offset_str = offset.to_string();
        let limit_str = limit.to_string();
        let mut params = vec![
            ("offset", offset_str.as_str()),
            ("limit", limit_str.as_str()),
            ("sort_by", "title"),
            ("sort_direction", "asc"),
            ("additional", "song_tag,song_audio,song_rating"),
        ];
        if let Some(folder_id) = id {
            params.push(("id", folder_id));
        }
        self.client
            .request("SYNO.AudioStation.Folder", 3, "list", &params)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::types::ApiInfo;
    use std::collections::HashMap;
    use wiremock::matchers::{method, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn list_root_folders() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(query_param("api", "SYNO.AudioStation.Folder"))
            .and(query_param("method", "list"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "success": true,
                "data": {
                    "items": [
                        {"id": "dir_0", "title": "music", "is_dir": true},
                        {"id": "dir_1", "title": "podcasts", "is_dir": true}
                    ],
                    "total": 2, "offset": 0
                }
            })))
            .mount(&server)
            .await;

        let mut client = SynoClient::new(&server.uri());
        client.set_sid("sid".to_string());
        let mut paths = HashMap::new();
        paths.insert(
            "SYNO.AudioStation.Folder".to_string(),
            ApiInfo {
                path: "AudioStation/folder.cgi".to_string(),
                min_version: 1,
                max_version: 3,
            },
        );
        client.set_api_paths(paths);

        let api = FolderApi::new(&client);
        let data = api.list(None, 0, 50).await.unwrap();
        assert_eq!(data.total, 2);
        assert_eq!(data.items[0].title, "music");
        assert!(data.items[0].is_dir);
    }

    #[tokio::test]
    async fn list_subfolder() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(query_param("id", "dir_0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "success": true,
                "data": {
                    "items": [
                        {"id": "dir_0/rock", "title": "rock", "is_dir": true},
                        {"id": "music_1", "title": "track.mp3", "is_dir": false}
                    ],
                    "total": 2, "offset": 0
                }
            })))
            .mount(&server)
            .await;

        let mut client = SynoClient::new(&server.uri());
        client.set_sid("sid".to_string());
        let mut paths = HashMap::new();
        paths.insert(
            "SYNO.AudioStation.Folder".to_string(),
            ApiInfo {
                path: "AudioStation/folder.cgi".to_string(),
                min_version: 1,
                max_version: 3,
            },
        );
        client.set_api_paths(paths);

        let api = FolderApi::new(&client);
        let data = api.list(Some("dir_0"), 0, 50).await.unwrap();
        assert_eq!(data.items.len(), 2);
        assert!(!data.items[1].is_dir);
    }
}
