/// Windows Named Pipe transport implementation
use super::{Connection, Transport, TransportInfo};
use anyhow::Result;
use async_trait::async_trait;
use std::path::Path;

const PIPE_NAME: &str = r"\\.\pipe\codex-orchestrator";

/// Named Pipe transport (Windows only)
pub struct NamedPipeTransport {
    pipe_name: String,
}

impl NamedPipeTransport {
    /// Create a new Named Pipe transport
    pub async fn new(_codex_dir: &Path) -> Result<Self> {
        let pipe_name = PIPE_NAME.to_string();

        tracing::info!("Named Pipe transport using {}", pipe_name);

        Ok(Self { pipe_name })
    }
}

#[async_trait]
impl Transport for NamedPipeTransport {
    fn info(&self) -> TransportInfo {
        TransportInfo::NamedPipe {
            pipe_name: self.pipe_name.clone(),
        }
    }

    async fn accept(&mut self) -> Result<Box<dyn Connection>> {
        // TODO: Implement Named Pipe server using tokio-uds or windows-async-pipe
        // For now, return error
        Err(anyhow::anyhow!("Named Pipe transport not yet implemented"))
    }

    async fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }
}

/// Named Pipe connection wrapper
pub struct NamedPipeConnection {
    // TODO: Add actual pipe handle
}

#[async_trait]
impl Connection for NamedPipeConnection {
    async fn read_message(&mut self) -> Result<Vec<u8>> {
        Err(anyhow::anyhow!("Named Pipe connection not yet implemented"))
    }

    async fn write_message(&mut self, _data: &[u8]) -> Result<()> {
        Err(anyhow::anyhow!("Named Pipe connection not yet implemented"))
    }

    async fn close(&mut self) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_named_pipe_transport_creation() {
        let temp_dir = TempDir::new().unwrap();
        let codex_dir = temp_dir.path().join(".codex");
        std::fs::create_dir_all(&codex_dir).unwrap();

        let transport = NamedPipeTransport::new(&codex_dir).await.unwrap();

        match transport.info() {
            TransportInfo::NamedPipe { pipe_name } => {
                assert!(pipe_name.contains("codex-orchestrator"));
            }
            _ => panic!("Expected Named Pipe transport info"),
        }
    }
}
