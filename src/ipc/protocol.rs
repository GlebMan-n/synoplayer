use serde::{Deserialize, Serialize};

/// Command sent from CLI client to running player.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum IpcRequest {
    Pause,
    Resume,
    Stop,
    Next,
    Prev,
    Now,
    Queue,
    Volume { level: u8 },
    Shuffle { mode: String },
    Repeat { mode: String },
}

/// Response sent back from player to CLI client.
#[derive(Debug, Serialize, Deserialize)]
pub struct IpcResponse {
    pub ok: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<IpcData>,
}

/// Structured data for query responses.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IpcData {
    NowPlaying {
        title: String,
        artist: String,
        album: String,
        position_secs: u64,
        duration_secs: u64,
        volume: u8,
        shuffle: bool,
        repeat: String,
        queue_index: usize,
        queue_total: usize,
    },
    QueueList {
        current_index: usize,
        tracks: Vec<QueueTrack>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QueueTrack {
    pub index: usize,
    pub title: String,
    pub artist: String,
    pub duration_secs: u64,
}

impl IpcResponse {
    pub fn ok(message: impl Into<String>) -> Self {
        Self {
            ok: true,
            message: message.into(),
            data: None,
        }
    }

    pub fn ok_with_data(message: impl Into<String>, data: IpcData) -> Self {
        Self {
            ok: true,
            message: message.into(),
            data: Some(data),
        }
    }

    pub fn err(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            message: message.into(),
            data: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_serialize_roundtrip() {
        let requests = vec![
            IpcRequest::Pause,
            IpcRequest::Resume,
            IpcRequest::Stop,
            IpcRequest::Next,
            IpcRequest::Prev,
            IpcRequest::Now,
            IpcRequest::Queue,
            IpcRequest::Volume { level: 75 },
            IpcRequest::Shuffle {
                mode: "on".to_string(),
            },
            IpcRequest::Repeat {
                mode: "all".to_string(),
            },
        ];
        for req in &requests {
            let json = serde_json::to_string(req).unwrap();
            let parsed: IpcRequest = serde_json::from_str(&json).unwrap();
            assert_eq!(
                serde_json::to_string(&parsed).unwrap(),
                json,
                "roundtrip failed for {json}"
            );
        }
    }

    #[test]
    fn response_ok_without_data() {
        let resp = IpcResponse::ok("done");
        let json = serde_json::to_string(&resp).unwrap();
        assert!(!json.contains("data"));
        let parsed: IpcResponse = serde_json::from_str(&json).unwrap();
        assert!(parsed.ok);
        assert!(parsed.data.is_none());
    }

    #[test]
    fn response_with_now_playing() {
        let resp = IpcResponse::ok_with_data(
            "playing",
            IpcData::NowPlaying {
                title: "Song".to_string(),
                artist: "Artist".to_string(),
                album: "Album".to_string(),
                position_secs: 120,
                duration_secs: 300,
                volume: 80,
                shuffle: true,
                repeat: "all".to_string(),
                queue_index: 2,
                queue_total: 10,
            },
        );
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: IpcResponse = serde_json::from_str(&json).unwrap();
        assert!(parsed.ok);
        assert!(parsed.data.is_some());
    }

    #[test]
    fn response_with_queue_list() {
        let resp = IpcResponse::ok_with_data(
            "queue",
            IpcData::QueueList {
                current_index: 1,
                tracks: vec![
                    QueueTrack {
                        index: 0,
                        title: "First".to_string(),
                        artist: "A".to_string(),
                        duration_secs: 180,
                    },
                    QueueTrack {
                        index: 1,
                        title: "Second".to_string(),
                        artist: "B".to_string(),
                        duration_secs: 240,
                    },
                ],
            },
        );
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: IpcResponse = serde_json::from_str(&json).unwrap();
        assert!(parsed.ok);
    }

    #[test]
    fn error_response() {
        let resp = IpcResponse::err("not playing");
        assert!(!resp.ok);
        assert_eq!(resp.message, "not playing");
    }

    #[test]
    fn malformed_json_fails() {
        let result = serde_json::from_str::<IpcRequest>("not json");
        assert!(result.is_err());
    }
}
