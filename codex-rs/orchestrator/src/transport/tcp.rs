/// TCP transport implementation (127.0.0.1 only)
use super::Connection;
/// TCP transport implementation (127.0.0.1 only)
use super::Transport;
/// TCP transport implementation (127.0.0.1 only)
use super::TransportInfo;
use anyhow::Context;
use anyhow::Result;
use async_trait::async_trait;
use std::path::Path;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::net::TcpStream;

/// TCP transport bound to 127.0.0.1
pub struct TcpTransport {
    listener: TcpListener,
    port: u16,
}

impl TcpTransport {
    /// Create a new TCP transport
    pub async fn new(port: u16, codex_dir: &Path) -> Result<Self> {
        let addr = if port == 0 {
            // Ephemeral port
            "127.0.0.1:0"
        } else {
            &format!("127.0.0.1:{}", port)
        };

        let listener = TcpListener::bind(addr)
            .await
            .context("Failed to bind TCP listener")?;

        let actual_port = listener.local_addr()?.port();

        // Save port to .codex/orchestrator.port
        let port_file = codex_dir.join("orchestrator.port");
        std::fs::write(&port_file, actual_port.to_string()).context("Failed to write port file")?;

        tracing::info!("TCP transport listening on 127.0.0.1:{}", actual_port);

        Ok(Self {
            listener,
            port: actual_port,
        })
    }
}

#[async_trait]
impl Transport for TcpTransport {
    fn info(&self) -> TransportInfo {
        TransportInfo::Tcp {
            host: "127.0.0.1".to_string(),
            port: self.port,
        }
    }

    async fn accept(&mut self) -> Result<Box<dyn Connection>> {
        let (stream, addr) = self
            .listener
            .accept()
            .await
            .context("Failed to accept TCP connection")?;

        // Verify connection is from localhost
        if !addr.ip().is_loopback() {
            return Err(anyhow::anyhow!(
                "Rejected non-localhost connection from {}",
                addr
            ));
        }

        Ok(Box::new(TcpConnection { stream }))
    }

    async fn shutdown(&mut self) -> Result<()> {
        // TCP listener will be dropped automatically
        Ok(())
    }
}

/// TCP connection wrapper
struct TcpConnection {
    stream: TcpStream,
}

#[async_trait]
impl Connection for TcpConnection {
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
            .context("Failed to shutdown TCP stream")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::io::AsyncReadExt;
    use tokio::io::AsyncWriteExt;

    #[tokio::test]
    async fn test_tcp_transport_creation() {
        let temp_dir = TempDir::new().unwrap();
        let codex_dir = temp_dir.path().join(".codex");
        std::fs::create_dir_all(&codex_dir).unwrap();

        let transport = TcpTransport::new(0, &codex_dir).await.unwrap();

        match transport.info() {
            TransportInfo::Tcp { host, port } => {
                assert_eq!(host, "127.0.0.1");
                assert!(port > 0);
            }
            _ => panic!("Expected TCP transport info"),
        }
    }

    #[tokio::test]
    async fn test_tcp_connection() {
        let temp_dir = TempDir::new().unwrap();
        let codex_dir = temp_dir.path().join(".codex");
        std::fs::create_dir_all(&codex_dir).unwrap();

        let mut transport = TcpTransport::new(0, &codex_dir).await.unwrap();
        let port = transport.port;

        // Spawn client
        let client_handle = tokio::spawn(async move {
            let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port))
                .await
                .unwrap();

            // Send message
            let msg = b"Hello, orchestrator!";
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
        assert_eq!(received, b"Hello, orchestrator!");

        // Send response
        conn.write_message(b"Hello, client!").await.unwrap();

        // Wait for client
        let client_response = client_handle.await.unwrap();
        assert_eq!(client_response, b"Hello, client!");
    }
}
