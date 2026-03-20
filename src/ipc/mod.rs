pub mod client;
pub mod protocol;
pub mod server;

use std::path::PathBuf;

/// Get the IPC socket path.
///
/// Uses `$XDG_RUNTIME_DIR/synoplayer.sock` if available,
/// falls back to `~/.cache/synoplayer.sock`.
pub fn socket_path() -> PathBuf {
    dirs::runtime_dir()
        .or_else(dirs::cache_dir)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("synoplayer.sock")
}

/// Guard that removes the socket file on drop.
pub struct SocketGuard {
    path: PathBuf,
}

impl SocketGuard {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl Drop for SocketGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Check if a socket file exists and is stale (not connectable).
/// Removes stale sockets. Returns Ok(true) if another instance is running.
pub fn check_existing_socket(path: &std::path::Path) -> anyhow::Result<bool> {
    if !path.exists() {
        return Ok(false);
    }
    // Try connecting — if it works, another instance is running
    match std::os::unix::net::UnixStream::connect(path) {
        Ok(_) => Ok(true),
        Err(_) => {
            // Stale socket — remove it
            let _ = std::fs::remove_file(path);
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn socket_path_is_absolute() {
        let path = socket_path();
        assert!(path.is_absolute());
        assert!(path.to_string_lossy().contains("synoplayer.sock"));
    }

    #[test]
    fn socket_guard_removes_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.sock");
        std::fs::write(&path, "").unwrap();
        assert!(path.exists());
        {
            let _guard = SocketGuard::new(path.clone());
        }
        assert!(!path.exists());
    }

    #[test]
    fn check_existing_socket_nonexistent() {
        let result = check_existing_socket(std::path::Path::new("/tmp/nonexistent_test.sock"));
        assert!(!result.unwrap());
    }

    #[test]
    fn check_existing_socket_stale() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("stale.sock");
        std::fs::write(&path, "").unwrap(); // regular file, not a socket
        let result = check_existing_socket(&path);
        assert!(!result.unwrap());
        assert!(!path.exists()); // was removed
    }
}
