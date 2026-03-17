use thiserror::Error;

#[derive(Debug, Error)]
pub enum SynoError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("API error (code {code}): {message}")]
    Api { code: i32, message: String },

    #[error("Session expired")]
    SessionExpired,

    #[error("Not authenticated — run `synoplayer login` first")]
    NotAuthenticated,

    #[error("2FA code required")]
    TwoFactorRequired,

    #[error("Invalid credentials")]
    InvalidCredentials,

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Cache error: {0}")]
    Cache(String),

    #[error("Credential storage error: {0}")]
    Credential(String),

    #[error("Player error: {0}")]
    Player(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Server not reachable: {0}")]
    ServerUnreachable(String),
}

impl SynoError {
    /// Create an API error from a Synology error code.
    pub fn from_api_code(code: i32) -> Self {
        let message = match code {
            100 => "Unknown error",
            101 => "No parameter of API, method, or version",
            102 => "API does not exist",
            103 => "Method does not exist",
            104 => "Version not supported",
            105 => "No permission",
            106 => "Session timeout",
            107 => "Session interrupted by duplicate login",
            119 => "Invalid session (SID not found)",
            400 => "No such account or incorrect password",
            401 => "Account disabled",
            402 => "Permission denied",
            403 => "2-factor authentication code required",
            404 => "Failed to authenticate 2-factor code",
            412 => "Playlist not found or corrupted (try recreating it via Audio Station UI)",
            _ => "Unknown API error",
        };
        if code == 106 || code == 119 {
            return SynoError::SessionExpired;
        }
        if code == 400 {
            return SynoError::InvalidCredentials;
        }
        if code == 403 {
            return SynoError::TwoFactorRequired;
        }
        SynoError::Api {
            code,
            message: message.to_string(),
        }
    }

    pub fn is_session_expired(&self) -> bool {
        matches!(self, SynoError::SessionExpired)
    }
}

pub type Result<T> = std::result::Result<T, SynoError>;
