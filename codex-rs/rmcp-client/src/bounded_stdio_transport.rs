//! Bounded, compatibility-aware transport for modern locally spawned MCP servers.

use std::future::Future;
use std::io;
use std::sync::Arc;

use memchr::memchr;
use rmcp::service::RoleClient;
use rmcp::service::RxJsonRpcMessage;
use rmcp::service::TxJsonRpcMessage;
use rmcp::transport::Transport;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::process::ChildStdin;
use tokio::process::ChildStdout;
use tokio::sync::Mutex;
use tracing::debug;
use tracing::warn;

/// Match the existing executor stdio transport and the production MCP limit.
pub(crate) const MAX_MCP_STDIO_LINE_BYTES: usize = 8 * 1024 * 1024;

pub(super) struct BoundedStdioTransport {
    stdin: Arc<Mutex<Option<ChildStdin>>>,
    stdout: BufReader<ChildStdout>,
    pending_line: Vec<u8>,
    program_name: String,
}

impl BoundedStdioTransport {
    pub(super) fn new(stdin: ChildStdin, stdout: ChildStdout, program_name: String) -> Self {
        Self {
            stdin: Arc::new(Mutex::new(Some(stdin))),
            stdout: BufReader::new(stdout),
            pending_line: Vec::new(),
            program_name,
        }
    }

    /// Pass both a partial line and buffered read-ahead to the SDK's legacy codec.
    pub(super) fn into_legacy_pipes(
        self,
    ) -> io::Result<(Box<dyn AsyncRead + Send + Unpin>, ChildStdin)> {
        let stdin = self
            .stdin
            .try_lock()
            .map_err(io::Error::other)?
            .take()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "MCP stdin closed"))?;
        let stdout = io::Cursor::new(self.pending_line).chain(self.stdout);
        Ok((Box::new(stdout), stdin))
    }

    async fn receive_message(&mut self) -> Option<RxJsonRpcMessage<RoleClient>> {
        loop {
            let bytes = match self.stdout.fill_buf().await {
                Ok(bytes) => bytes,
                Err(error) => {
                    warn!(
                        "Failed to read MCP server stdout ({}): {error}",
                        self.program_name
                    );
                    return None;
                }
            };

            if bytes.is_empty() {
                if self.pending_line.is_empty() {
                    return None;
                }
                return self.decode_pending_message();
            }

            let newline = memchr(b'\n', bytes);
            let content_len = newline.unwrap_or(bytes.len());
            if content_len > MAX_MCP_STDIO_LINE_BYTES.saturating_sub(self.pending_line.len()) {
                warn!(
                    "MCP stdio line exceeds {MAX_MCP_STDIO_LINE_BYTES} bytes ({}); closing transport",
                    self.program_name
                );
                self.pending_line.clear();
                return None;
            }

            self.pending_line.extend_from_slice(&bytes[..content_len]);
            let consumed = content_len + usize::from(newline.is_some());
            self.stdout.consume(consumed);

            if newline.is_some()
                && let Some(message) = self.decode_pending_message()
            {
                return Some(message);
            }
        }
    }

    fn decode_pending_message(&mut self) -> Option<RxJsonRpcMessage<RoleClient>> {
        let line = std::mem::take(&mut self.pending_line);
        let line = line.strip_suffix(b"\r").unwrap_or(&line);
        match serde_json::from_slice(line) {
            Ok(message) => Some(message),
            Err(error) => {
                debug!(
                    "Failed to parse local MCP server message ({}): {error}",
                    self.program_name
                );
                None
            }
        }
    }
}

impl Transport<RoleClient> for BoundedStdioTransport {
    type Error = io::Error;

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "complete JSON-RPC frames must hold the stdin lock across writes"
    )]
    fn send(
        &mut self,
        item: TxJsonRpcMessage<RoleClient>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send + 'static {
        let stdin = Arc::clone(&self.stdin);
        async move {
            let mut message = serde_json::to_vec(&item).map_err(io::Error::other)?;
            message.push(b'\n');
            let mut guard = stdin.lock().await;
            let stdin = guard
                .as_mut()
                .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "MCP stdin closed"))?;
            stdin.write_all(&message).await?;
            stdin.flush().await
        }
    }

    fn receive(&mut self) -> impl Future<Output = Option<RxJsonRpcMessage<RoleClient>>> + Send {
        self.receive_message()
    }

    async fn close(&mut self) -> Result<(), Self::Error> {
        self.stdin.lock().await.take();
        Ok(())
    }
}
