use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tokio::sync::mpsc;

use crate::ipc::protocol::{IpcRequest, IpcResponse};
use crate::ipc::{SocketGuard, check_existing_socket, socket_path};

/// A pending IPC request with a channel to send the response back.
pub struct IpcCommand {
    pub request: IpcRequest,
    pub reply: tokio::sync::oneshot::Sender<IpcResponse>,
}

/// Start the IPC server. Returns a receiver for incoming commands and a socket guard.
///
/// The server accepts connections on a Unix socket and forwards parsed commands
/// through the channel. The caller (TUI or CLI playback loop) processes commands
/// and sends responses back through the oneshot channel in each `IpcCommand`.
pub fn start(
) -> anyhow::Result<(mpsc::UnboundedReceiver<IpcCommand>, SocketGuard)> {
    let path = socket_path();
    if check_existing_socket(&path)? {
        anyhow::bail!("Another synoplayer instance is already running.");
    }

    let listener = UnixListener::bind(&path)?;
    let guard = SocketGuard::new(path);

    let (tx, rx) = mpsc::unbounded_channel::<IpcCommand>();

    tokio::spawn(accept_loop(listener, tx));

    Ok((rx, guard))
}

async fn accept_loop(listener: UnixListener, tx: mpsc::UnboundedSender<IpcCommand>) {
    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let tx = tx.clone();
                tokio::spawn(handle_connection(stream, tx));
            }
            Err(e) => {
                tracing::warn!("IPC accept error: {e}");
            }
        }
    }
}

async fn handle_connection(
    stream: tokio::net::UnixStream,
    tx: mpsc::UnboundedSender<IpcCommand>,
) {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    if let Err(e) = reader.read_line(&mut line).await {
        tracing::warn!("IPC read error: {e}");
        return;
    }

    let request = match serde_json::from_str::<IpcRequest>(line.trim()) {
        Ok(req) => req,
        Err(e) => {
            let resp = IpcResponse::err(format!("Invalid request: {e}"));
            let json = serde_json::to_string(&resp).unwrap_or_default();
            let _ = writer.write_all(format!("{json}\n").as_bytes()).await;
            return;
        }
    };

    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    let cmd = IpcCommand {
        request,
        reply: reply_tx,
    };

    if tx.send(cmd).is_err() {
        let resp = IpcResponse::err("Player is shutting down");
        let json = serde_json::to_string(&resp).unwrap_or_default();
        let _ = writer.write_all(format!("{json}\n").as_bytes()).await;
        return;
    }

    // Wait for the response from the player loop
    let response = match reply_rx.await {
        Ok(resp) => resp,
        Err(_) => IpcResponse::err("No response from player"),
    };

    let json = serde_json::to_string(&response).unwrap_or_default();
    let _ = writer.write_all(format!("{json}\n").as_bytes()).await;
}

/// Helper to start IPC server only if possible (non-fatal).
/// Returns None if another instance is running or binding fails.
pub fn try_start(
) -> Option<(mpsc::UnboundedReceiver<IpcCommand>, SocketGuard)> {
    match start() {
        Ok(result) => Some(result),
        Err(e) => {
            tracing::debug!("IPC server not started: {e}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn server_start_and_connect() {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("test.sock");

        // Override socket path by binding directly
        let listener = UnixListener::bind(&sock).unwrap();
        let _guard = SocketGuard::new(sock.clone());

        let (tx, mut rx) = mpsc::unbounded_channel::<IpcCommand>();
        tokio::spawn(accept_loop(listener, tx));

        // Spawn handler that responds to commands
        tokio::spawn(async move {
            while let Some(cmd) = rx.recv().await {
                let resp = IpcResponse::ok("test response");
                let _ = cmd.reply.send(resp);
            }
        });

        // Connect as client
        let stream = tokio::net::UnixStream::connect(&sock).await.unwrap();
        let (reader, mut writer) = stream.into_split();

        let req = serde_json::to_string(&IpcRequest::Now).unwrap();
        writer
            .write_all(format!("{req}\n").as_bytes())
            .await
            .unwrap();

        let mut buf_reader = BufReader::new(reader);
        let mut line = String::new();
        buf_reader.read_line(&mut line).await.unwrap();

        let response: IpcResponse = serde_json::from_str(line.trim()).unwrap();
        assert!(response.ok);
        assert_eq!(response.message, "test response");
    }

    #[tokio::test]
    async fn server_handles_malformed_json() {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("bad.sock");

        let listener = UnixListener::bind(&sock).unwrap();
        let _guard = SocketGuard::new(sock.clone());

        let (tx, _rx) = mpsc::unbounded_channel::<IpcCommand>();
        tokio::spawn(accept_loop(listener, tx));

        let stream = tokio::net::UnixStream::connect(&sock).await.unwrap();
        let (reader, mut writer) = stream.into_split();

        writer
            .write_all(b"not valid json\n")
            .await
            .unwrap();

        let mut buf_reader = BufReader::new(reader);
        let mut line = String::new();
        buf_reader.read_line(&mut line).await.unwrap();

        let response: IpcResponse = serde_json::from_str(line.trim()).unwrap();
        assert!(!response.ok);
        assert!(response.message.contains("Invalid request"));
    }
}
