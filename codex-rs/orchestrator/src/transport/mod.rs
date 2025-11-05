/// Transport layer abstraction for orchestrator communication
///
/// Supports Unix Domain Sockets, Windows Named Pipes, and TCP (127.0.0.1)
pub mod tcp;

#[cfg(unix)]
pub mod uds;

#[cfg(windows)]
pub mod named_pipe;

use anyhow::Result;
use async_trait::async_trait;
use serde::Deserialize;
use serde::Serialize;

/// Transport preference order
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransportPreference {
    /// Auto-detect best transport
    Auto,
    /// Unix Domain Socket (Unix only)
    Uds,
    /// Named Pipe (Windows only)
    Pipe,
    /// TCP on 127.0.0.1
    Tcp,
}

impl Default for TransportPreference {
    fn default() -> Self {
        Self::Auto
    }
}

/// Transport configuration
#[derive(Debug, Clone)]
pub struct TransportConfig {
    pub preference: TransportPreference,
    pub tcp_port: u16, // 0 for ephemeral
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            preference: TransportPreference::Auto,
            tcp_port: 0,
        }
    }
}

/// Transport connection information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransportInfo {
    #[cfg(unix)]
    Uds {
        socket_path: std::path::PathBuf,
    },
    #[cfg(windows)]
    NamedPipe {
        pipe_name: String,
    },
    Tcp {
        host: String,
        port: u16,
    },
}

/// Trait for transport implementations
#[async_trait]
pub trait Transport: Send + Sync {
    /// Get transport info
    fn info(&self) -> TransportInfo;

    /// Accept incoming connection
    async fn accept(&mut self) -> Result<Box<dyn Connection>>;

    /// Shutdown transport
    async fn shutdown(&mut self) -> Result<()>;
}

/// Trait for transport connections
#[async_trait]
pub trait Connection: Send + Sync {
    /// Read message from connection
    async fn read_message(&mut self) -> Result<Vec<u8>>;

    /// Write message to connection
    async fn write_message(&mut self, data: &[u8]) -> Result<()>;

    /// Close connection
    async fn close(&mut self) -> Result<()>;
}

/// Create transport based on preference
pub async fn create_transport(
    config: TransportConfig,
    codex_dir: &std::path::Path,
) -> Result<Box<dyn Transport>> {
    match config.preference {
        TransportPreference::Auto => {
            // Auto-detect: UDS (Unix) → Pipe (Windows) → TCP (fallback)
            #[cfg(unix)]
            {
                uds::UdsTransport::new(codex_dir)
                    .await
                    .map(|t| Box::new(t) as Box<dyn Transport>)
            }
            #[cfg(windows)]
            {
                named_pipe::NamedPipeTransport::new(codex_dir)
                    .await
                    .map(|t| Box::new(t) as Box<dyn Transport>)
            }
            #[cfg(not(any(unix, windows)))]
            {
                tcp::TcpTransport::new(config.tcp_port, codex_dir)
                    .await
                    .map(|t| Box::new(t) as Box<dyn Transport>)
            }
        }
        TransportPreference::Uds => {
            #[cfg(unix)]
            {
                uds::UdsTransport::new(codex_dir)
                    .await
                    .map(|t| Box::new(t) as Box<dyn Transport>)
            }
            #[cfg(not(unix))]
            {
                Err(anyhow::anyhow!("UDS not supported on this platform"))
            }
        }
        TransportPreference::Pipe => {
            #[cfg(windows)]
            {
                named_pipe::NamedPipeTransport::new(codex_dir)
                    .await
                    .map(|t| Box::new(t) as Box<dyn Transport>)
            }
            #[cfg(not(windows))]
            {
                Err(anyhow::anyhow!(
                    "Named pipes not supported on this platform"
                ))
            }
        }
        TransportPreference::Tcp => tcp::TcpTransport::new(config.tcp_port, codex_dir)
            .await
            .map(|t| Box::new(t) as Box<dyn Transport>),
    }
}
