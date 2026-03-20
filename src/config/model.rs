use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AppConfig {
    #[serde(default = "ServerConfig::default")]
    pub server: ServerConfig,
    #[serde(default = "AuthConfig::default")]
    pub auth: AuthConfig,
    #[serde(default = "PlayerConfig::default")]
    pub player: PlayerConfig,
    #[serde(default = "CacheConfig::default")]
    pub cache: CacheConfig,
    #[serde(default = "DisplayConfig::default")]
    pub display: DisplayConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_true")]
    pub https: bool,
    #[serde(default = "default_true")]
    pub verify_ssl: bool,
    #[serde(default)]
    pub quickconnect_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuthConfig {
    #[serde(default)]
    pub username: String,
    #[serde(default = "default_credential_store")]
    pub credential_store: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlayerConfig {
    #[serde(default = "default_volume")]
    pub default_volume: u8,
    #[serde(default)]
    pub output_device: String,
    #[serde(default = "default_buffer_size")]
    pub buffer_size_kb: u32,
    #[serde(default = "default_favorites_playlist")]
    pub favorites_playlist: String,
}

fn default_favorites_playlist() -> String {
    "Favorites".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CacheConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_cache_path")]
    pub path: PathBuf,
    #[serde(default = "default_max_size")]
    pub max_size_mb: u64,
    #[serde(default = "default_ttl")]
    pub ttl_days: u32,
    #[serde(default = "default_true")]
    pub cache_on_play: bool,
    #[serde(default)]
    pub preload_playlist: bool,
    #[serde(default)]
    pub transcode_before_cache: bool,
    #[serde(default = "default_true")]
    pub verify_integrity: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct DisplayConfig {
    #[serde(default)]
    pub show_lyrics: bool,
    #[serde(default)]
    pub show_cover: bool,
}

// --- Defaults ---

fn default_host() -> String {
    "localhost".to_string()
}
fn default_port() -> u16 {
    5001
}
fn default_true() -> bool {
    true
}
fn default_credential_store() -> String {
    "keyring".to_string()
}
fn default_volume() -> u8 {
    80
}
fn default_buffer_size() -> u32 {
    256
}
fn default_cache_path() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("synoplayer/audio")
}
fn default_max_size() -> u64 {
    2048
}
fn default_ttl() -> u32 {
    30
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            https: true,
            verify_ssl: true,
            quickconnect_id: String::new(),
        }
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            username: String::new(),
            credential_store: default_credential_store(),
        }
    }
}

impl Default for PlayerConfig {
    fn default() -> Self {
        Self {
            default_volume: default_volume(),
            output_device: String::new(),
            buffer_size_kb: default_buffer_size(),
            favorites_playlist: default_favorites_playlist(),
        }
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            path: default_cache_path(),
            max_size_mb: default_max_size(),
            ttl_days: default_ttl(),
            cache_on_play: true,
            preload_playlist: false,
            transcode_before_cache: false,
            verify_integrity: true,
        }
    }
}

impl AppConfig {
    /// Build the base URL for the Synology API.
    pub fn base_url(&self) -> String {
        let scheme = if self.server.https { "https" } else { "http" };
        format!("{}://{}:{}", scheme, self.server.host, self.server.port)
    }

    /// Path to the config file.
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("synoplayer/config.toml")
    }

    /// Path to session file.
    pub fn session_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("synoplayer/session.json")
    }

    /// Load from TOML file. Returns default if file doesn't exist.
    pub fn load() -> crate::error::Result<Self> {
        let path = Self::config_path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)?;
        let config: Self =
            toml::from_str(&content).map_err(|e| crate::error::SynoError::Config(e.to_string()))?;
        Ok(config)
    }

    /// Save to TOML file.
    pub fn save(&self) -> crate::error::Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)
            .map_err(|e| crate::error::SynoError::Config(e.to_string()))?;
        std::fs::write(path, content)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_full_config() {
        let toml_str = r#"
        [server]
        host = "192.168.1.100"
        port = 5001
        https = true
        verify_ssl = false

        [auth]
        username = "admin"
        credential_store = "keyring"

        [player]
        default_volume = 75
        buffer_size_kb = 512

        [cache]
        enabled = true
        path = "/tmp/test_cache"
        max_size_mb = 1024
        ttl_days = 14
        cache_on_play = true
        preload_playlist = false
        transcode_before_cache = false
        verify_integrity = true

        [display]
        show_lyrics = true
        show_cover = false
        "#;

        let config: AppConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.server.host, "192.168.1.100");
        assert_eq!(config.server.port, 5001);
        assert!(!config.server.verify_ssl);
        assert_eq!(config.auth.username, "admin");
        assert_eq!(config.cache.max_size_mb, 1024);
        assert_eq!(config.cache.ttl_days, 14);
        assert_eq!(config.player.default_volume, 75);
        assert!(config.display.show_lyrics);
    }

    #[test]
    fn defaults_when_optional_missing() {
        let toml_str = r#"
        [server]
        host = "10.0.0.1"
        "#;

        let config: AppConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.server.port, 5001);
        assert!(config.server.https);
        assert!(config.cache.enabled);
        assert_eq!(config.cache.max_size_mb, 2048);
        assert_eq!(config.player.default_volume, 80);
    }

    #[test]
    fn empty_string_parses_to_defaults() {
        let config: AppConfig = toml::from_str("").unwrap();
        assert_eq!(config.server.host, "localhost");
        assert_eq!(config.server.port, 5001);
    }

    #[test]
    fn serialize_roundtrip() {
        let config = AppConfig::default();
        let serialized = toml::to_string(&config).unwrap();
        let deserialized: AppConfig = toml::from_str(&serialized).unwrap();
        assert_eq!(config, deserialized);
    }

    #[test]
    fn base_url_https() {
        let mut config = AppConfig::default();
        config.server.host = "nas.local".to_string();
        config.server.port = 5001;
        config.server.https = true;
        assert_eq!(config.base_url(), "https://nas.local:5001");
    }

    #[test]
    fn base_url_http() {
        let mut config = AppConfig::default();
        config.server.host = "nas.local".to_string();
        config.server.port = 5000;
        config.server.https = false;
        assert_eq!(config.base_url(), "http://nas.local:5000");
    }
}
