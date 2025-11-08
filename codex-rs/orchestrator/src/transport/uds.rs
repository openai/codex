/// Unix Domain Socket transport implementation
use super::Connection;
/// Unix Domain Socket transport implementation
use super::Transport;
/// Unix Domain Socket transport implementation
use super::TransportInfo;
use anyhow::Context;
use anyhow::Result;
use async_trait::async_trait;
use std::path::Path;
use std::path::PathBuf;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::UnixListener;
use tokio::net::UnixStream;

const SOCKET_FILENAME: &str = "orchestrator.sock";

/// Unix Domain Socket transport
pub struct UdsTransport {
    listener: UnixListener,
    socket_path: PathBuf,
}

impl UdsTransport {
    /// Create a new UDS transport
    pub async fn new(codex_dir: &Path) -> Result<Self> {
        let socket_path = codex_dir.join(SOCKET_FILENAME);

        // Remove existing socket if present
        if socket_path.exists() {
            std::fs::remove_file(&socket_path).context("Failed to remove existing socket")?;
        }

        let listener = UnixListener::bind(&socket_path).context("Failed to bind Unix socket")?;

        // Set restrictive permissions (0700)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&socket_path)?.permissions();
            perms.set_mode(0o700);
            std::fs::set_permissions(&socket_path, perms)?;
        }

        tracing::info!("UDS transport listening on {}", socket_path.display());

        Ok(Self {
            listener,
            socket_path,
        })
    }
}

#[async_trait]
impl Transport for UdsTransport {
    fn info(&self) -> TransportInfo {
        TransportInfo::Uds {
            socket_path: self.socket_path.clone(),
        }
    }

    async fn accept(&mut self) -> Result<Box<dyn Connection>> {
        let (stream, _addr) = self
            .listener
            .accept()
            .await
            .context("Failed to accept UDS connection")?;

        Ok(Box::new(UdsConnection { stream }))
    }

    async fn shutdown(&mut self) -> Result<()> {
        // Remove socket file
        if self.socket_path.exists() {
            std::fs::remove_file(&self.socket_path).context("Failed to remove socket file")?;
        }
        Ok(())
    }
}

impl Drop for UdsTransport {
    fn drop(&mut self) {
        // Best effort cleanup
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

/// UDS connection wrapper
struct UdsConnection {
    stream: UnixStream,
}

#[async_trait]
impl Connection for UdsConnection {
    async fn read_message(&mut self) -> Result<Vec<u8>> {
        // Read length prefix (4 bytes, little-endian)
        let mut len_bytes = [0u8; 4];
        self.stream
            .read_exact(&mut len_bytes)
            .await
            .context("Failed to read message length")?;
        let len = u32::from_le_bytes(len_bytes) as usize;

        // Read message body
        let mut buffer = vec![0u8; len];
        self.stream
            .read_exact(&mut buffer)
            .await
            .context("Failed to read message body")?;

        Ok(buffer)
    }

    async fn write_message(&mut self, data: &[u8]) -> Result<()> {
        // Write length prefix
        let len = data.len() as u32;
        self.stream
            .write_all(&len.to_le_bytes())
            .await
            .context("Failed to write message length")?;

        // Write message body
        self.stream
            .write_all(data)
            .await
            .context("Failed to write message body")?;

        self.stream
            .flush()
            .await
            .context("Failed to flush stream")?;

        Ok(())
    }

    async fn close(&mut self) -> Result<()> {
        self.stream
            .shutdown()
            .await
            .context("Failed to shutdown UDS stream")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_uds_transport_creation() {
        let temp_dir = TempDir::new().unwrap();
        let codex_dir = temp_dir.path().join(".codex");
        std::fs::create_dir_all(&codex_dir).unwrap();

        let transport = UdsTransport::new(&codex_dir).await.unwrap();

        match transport.info() {
            TransportInfo::Uds { socket_path } => {
                assert!(socket_path.exists());
                assert_eq!(socket_path.file_name().unwrap(), "orchestrator.sock");
            }
            _ => panic!("Expected UDS transport info"),
        }
    }

    #[tokio::test]
    async fn test_uds_connection() {
        let temp_dir = TempDir::new().unwrap();
        let codex_dir = temp_dir.path().join(".codex");
        std::fs::create_dir_all(&codex_dir).unwrap();

        let mut transport = UdsTransport::new(&codex_dir).await.unwrap();
        let socket_path = codex_dir.join("orchestrator.sock");

        // Spawn client
        let client_handle = tokio::spawn(async move {
            let mut stream = UnixStream::connect(&socket_path).await.unwrap();

            // Send message
            let msg = b"Hello from UDS client!";
            let len = msg.len() as u32;
            stream.write_all(&len.to_le_bytes()).await.unwrap();
            stream.write_all(msg).await.unwrap();
            stream.flush().await.unwrap();

            // Read response
            let mut len_bytes = [0u8; 4];
            stream.read_exact(&mut len_bytes).await.unwrap();
            let len = u32::from_le_bytes(len_bytes) as usize;
            let mut response = vec![0u8; len];
            stream.read_exact(&mut response).await.unwrap();

            response
        });

        // Accept connection
        let mut conn = transport.accept().await.unwrap();

        // Read message
        let received = conn.read_message().await.unwrap();
        assert_eq!(received, b"Hello from UDS client!");

        // Send response
        conn.write_message(b"Hello from server!").await.unwrap();

        // Wait for client
        let client_response = client_handle.await.unwrap();
        assert_eq!(client_response, b"Hello from server!");
    }
}
