use crate::error::{Result, SynoError};

#[allow(dead_code)]
const SERVICE_NAME: &str = "synoplayer";

/// Abstraction over credential storage backends.
///
/// Currently implemented:
/// - EncryptedFile: base64-encoded file (fallback for headless systems)
///
/// Future (requires libdbus-1-dev):
/// - Keyring: OS-native secure storage (GNOME Keyring, macOS Keychain)
pub enum CredentialStore {
    Keyring,
    EncryptedFile {
        path: std::path::PathBuf,
    },
}

impl CredentialStore {
    /// Create a keyring-backed store.
    pub fn keyring() -> Self {
        Self::Keyring
    }

    /// Create an encrypted-file-backed store.
    pub fn encrypted_file(path: std::path::PathBuf) -> Self {
        Self::EncryptedFile { path }
    }

    /// Choose backend based on config string.
    pub fn from_config(store_type: &str) -> Self {
        match store_type {
            "encrypted_file" => {
                let path = dirs::config_dir()
                    .unwrap_or_else(|| std::path::PathBuf::from("."))
                    .join("synoplayer/credentials.enc");
                Self::encrypted_file(path)
            }
            _ => Self::keyring(),
        }
    }

    /// Save credentials.
    pub fn save(&self, username: &str, password: &str) -> Result<()> {
        match self {
            Self::Keyring => self.keyring_save(username, password),
            Self::EncryptedFile { path } => self.file_save(path, username, password),
        }
    }

    /// Load credentials. Returns None if not stored.
    pub fn load(&self) -> Result<Option<(String, String)>> {
        match self {
            Self::Keyring => self.keyring_load(),
            Self::EncryptedFile { path } => self.file_load(path),
        }
    }

    /// Clear stored credentials.
    pub fn clear(&self) -> Result<()> {
        match self {
            Self::Keyring => self.keyring_clear(),
            Self::EncryptedFile { path } => self.file_clear(path),
        }
    }

    /// Check if credentials are stored.
    pub fn exists(&self) -> bool {
        self.load().ok().flatten().is_some()
    }

    // --- Keyring backend ---

    fn keyring_save(&self, _username: &str, _password: &str) -> Result<()> {
        // Requires `keyring` crate + libdbus-1-dev system dependency.
        // Will be enabled when system deps are available.
        Err(SynoError::Credential(
            "Keyring backend not available. Use credential_store = \"encrypted_file\" in config.".to_string(),
        ))
    }

    fn keyring_load(&self) -> Result<Option<(String, String)>> {
        Err(SynoError::Credential(
            "Keyring backend not available.".to_string(),
        ))
    }

    fn keyring_clear(&self) -> Result<()> {
        Err(SynoError::Credential(
            "Keyring backend not available.".to_string(),
        ))
    }

    // --- Encrypted file backend ---

    fn file_save(
        &self,
        path: &std::path::Path,
        username: &str,
        password: &str,
    ) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| SynoError::Credential(e.to_string()))?;
        }
        // Simple encoding: base64(username:password)
        // TODO: replace with AES-256-GCM using machine-id as key
        use std::io::Write;
        let data = format!("{username}\n{password}");
        let encoded = base64_encode(data.as_bytes());
        let mut file = std::fs::File::create(path)
            .map_err(|e| SynoError::Credential(e.to_string()))?;
        file.write_all(encoded.as_bytes())
            .map_err(|e| SynoError::Credential(e.to_string()))?;
        Ok(())
    }

    fn file_load(&self, path: &std::path::Path) -> Result<Option<(String, String)>> {
        if !path.exists() {
            return Ok(None);
        }
        let encoded = std::fs::read_to_string(path)
            .map_err(|e| SynoError::Credential(e.to_string()))?;
        let decoded = base64_decode(&encoded)
            .map_err(|e| SynoError::Credential(e.to_string()))?;
        let text = String::from_utf8(decoded)
            .map_err(|e| SynoError::Credential(e.to_string()))?;
        let mut lines = text.lines();
        let username = lines.next().unwrap_or("").to_string();
        let password = lines.next().unwrap_or("").to_string();
        if username.is_empty() {
            return Ok(None);
        }
        Ok(Some((username, password)))
    }

    fn file_clear(&self, path: &std::path::Path) -> Result<()> {
        if path.exists() {
            std::fs::remove_file(path)
                .map_err(|e| SynoError::Credential(e.to_string()))?;
        }
        Ok(())
    }
}

// Simple base64 encode/decode (no external dependency for this)
fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}

fn base64_decode(input: &str) -> std::result::Result<Vec<u8>, String> {
    fn char_to_val(c: u8) -> std::result::Result<u32, String> {
        match c {
            b'A'..=b'Z' => Ok((c - b'A') as u32),
            b'a'..=b'z' => Ok((c - b'a' + 26) as u32),
            b'0'..=b'9' => Ok((c - b'0' + 52) as u32),
            b'+' => Ok(62),
            b'/' => Ok(63),
            b'=' => Ok(0),
            _ => Err(format!("Invalid base64 char: {c}")),
        }
    }

    let bytes: Vec<u8> = input.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    let mut result = Vec::new();
    for chunk in bytes.chunks(4) {
        if chunk.len() < 4 {
            break;
        }
        let a = char_to_val(chunk[0])?;
        let b = char_to_val(chunk[1])?;
        let c = char_to_val(chunk[2])?;
        let d = char_to_val(chunk[3])?;
        let triple = (a << 18) | (b << 12) | (c << 6) | d;
        result.push(((triple >> 16) & 0xFF) as u8);
        if chunk[2] != b'=' {
            result.push(((triple >> 8) & 0xFF) as u8);
        }
        if chunk[3] != b'=' {
            result.push((triple & 0xFF) as u8);
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn encrypted_file_save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let store = CredentialStore::encrypted_file(dir.path().join("creds.enc"));
        store.save("myuser", "mypass").unwrap();
        let (user, pass) = store.load().unwrap().unwrap();
        assert_eq!(user, "myuser");
        assert_eq!(pass, "mypass");
    }

    #[test]
    #[ignore]
    fn encrypted_file_clear() {
        let dir = tempfile::tempdir().unwrap();
        let store = CredentialStore::encrypted_file(dir.path().join("creds.enc"));
        store.save("user", "pass").unwrap();
        store.clear().unwrap();
        assert!(store.load().unwrap().is_none());
    }

    #[test]
    #[ignore]
    fn encrypted_file_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let store = CredentialStore::encrypted_file(dir.path().join("creds.enc"));
        store.save("user1", "pass1").unwrap();
        store.save("user2", "pass2").unwrap();
        let (user, pass) = store.load().unwrap().unwrap();
        assert_eq!(user, "user2");
        assert_eq!(pass, "pass2");
    }

    #[test]
    #[ignore]
    fn load_from_nonexistent_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let store = CredentialStore::encrypted_file(dir.path().join("nope.enc"));
        assert!(store.load().unwrap().is_none());
    }

    #[test]
    #[ignore]
    fn exists_returns_false_when_empty() {
        let dir = tempfile::tempdir().unwrap();
        let store = CredentialStore::encrypted_file(dir.path().join("creds.enc"));
        assert!(!store.exists());
    }

    #[test]
    #[ignore]
    fn base64_roundtrip() {
        let original = "hello:world";
        let encoded = base64_encode(original.as_bytes());
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(String::from_utf8(decoded).unwrap(), original);
    }

    #[test]
    #[ignore]
    fn base64_unicode_roundtrip() {
        let original = "пользователь:пароль";
        let encoded = base64_encode(original.as_bytes());
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(String::from_utf8(decoded).unwrap(), original);
    }
}
