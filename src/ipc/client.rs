use std::io::{BufRead, Write};
use std::os::unix::net::UnixStream;

use crate::ipc::protocol::{IpcRequest, IpcResponse};

/// Send a command to the running player instance and return the response.
pub fn send_command(request: &IpcRequest) -> anyhow::Result<IpcResponse> {
    let path = super::socket_path();
    if !path.exists() {
        anyhow::bail!(
            "No running player instance.\n\
             Start playback with `synoplayer tui` or `synoplayer playlist play` first."
        );
    }

    let stream = UnixStream::connect(&path).map_err(|_| {
        anyhow::anyhow!(
            "No running player instance.\n\
             Start playback with `synoplayer tui` or `synoplayer playlist play` first."
        )
    })?;

    // Set a timeout so we don't hang forever
    stream.set_read_timeout(Some(std::time::Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(std::time::Duration::from_secs(5)))?;

    let mut writer = std::io::BufWriter::new(&stream);
    let json = serde_json::to_string(request)?;
    writeln!(writer, "{json}")?;
    writer.flush()?;

    let mut reader = std::io::BufReader::new(&stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;

    let response: IpcResponse = serde_json::from_str(line.trim())?;
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn send_to_nonexistent_socket_fails() {
        let result = send_command(&IpcRequest::Now);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("No running player instance"));
    }
}
