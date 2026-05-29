//! rmcp transport adapter for an executor-managed MCP stdio process.
//!
//! This module owns the lower-level byte translation after
//! `stdio_server_launcher` has already started a process through
//! `ExecBackend::start`. It does not choose where the MCP server runs and it
//! does not implement MCP lifecycle behavior. MCP protocol ownership stays in
//! `RmcpClient` and rmcp:
//!
//! 1. rmcp serializes a JSON-RPC message and calls [`Transport::send`].
//! 2. This transport appends the stdio newline delimiter and writes those bytes
//!    to executor `process/write`.
//! 3. The executor writes the bytes to the child process stdin.
//! 4. The child writes newline-delimited JSON-RPC messages to stdout.
//! 5. The executor reports stdout bytes through pushed process events.
//! 6. This transport buffers stdout until it has one full line, deserializes
//!    that line, and returns the rmcp message from [`Transport::receive`].
//!
//! Stderr is deliberately not part of the MCP byte stream. It is logged for
//! diagnostics only, matching the local stdio implementation.

use std::future::Future;
use std::io;
use std::mem::take;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use codex_exec_server::ExecOutputStream;
use codex_exec_server::ExecProcess;
use codex_exec_server::ExecProcessEvent;
use codex_exec_server::ExecProcessEventReceiver;
use codex_exec_server::ProcessId;
use codex_exec_server::ProcessOutputChunk;
use codex_exec_server::WriteStatus;
use rmcp::service::RoleClient;
use rmcp::service::RxJsonRpcMessage;
use rmcp::service::TxJsonRpcMessage;
use rmcp::transport::Transport;
use serde_json::from_slice;
use serde_json::to_vec;
use tokio::runtime::Handle;
use tokio::sync::broadcast;
use tracing::debug;
use tracing::info;
use tracing::warn;

static PROCESS_COUNTER: AtomicUsize = AtomicUsize::new(1);

// Remote public implementation.

/// A client-side rmcp transport backed by an executor-managed process.
///
/// The orchestrator owns this value and calls rmcp on it. The process it wraps
/// may be local or remote depending on the `ExecBackend` used to create it, but
/// for remote MCP stdio the process lives on the executor and all interaction
/// crosses the executor process RPC boundary.
pub(super) struct ExecutorProcessTransport {
    /// Logical process handle returned by the executor process API.
    ///
    /// `write` forwards stdin bytes. `terminate` stops the child when rmcp
    /// closes the transport.
    process: Arc<dyn ExecProcess>,

    /// Pushed output/lifecycle stream for the process.
    ///
    /// The executor process API still supports retained-output reads, but MCP
    /// stdio is naturally streaming. This receiver lets rmcp wait for stdout
    /// chunks without issuing `process/read` after each output notification.
    events: ExecProcessEventReceiver,

    /// Human-readable program name used only in diagnostics.
    program_name: String,

    /// Buffered child stdout bytes that have not yet formed a complete
    /// newline-delimited JSON-RPC message.
    stdout: Vec<u8>,

    /// Buffered stderr bytes for diagnostic logging.
    stderr: Vec<u8>,

    /// Whether the executor has reported process closure or a terminal
    /// subscription failure. Once closed, any remaining partial stdout line is
    /// flushed once and then rmcp receives EOF.
    closed: bool,

    /// Whether this transport already asked the executor to terminate the MCP
    /// server process.
    terminated: bool,

    /// Highest executor process event sequence observed by this transport.
    ///
    /// When the pushed event stream lags, use this as the retained-output read
    /// cursor to recover missed stdout/stderr chunks from the executor.
    last_seq: u64,
}

impl ExecutorProcessTransport {
    pub(super) fn new(process: Arc<dyn ExecProcess>, program_name: String) -> Self {
        // Subscribe before returning the transport to rmcp. Some test servers
        // can emit output or exit quickly after `process/start`, and the
        // process event log will replay anything that landed before this
        // subscriber was attached.
        let events = process.subscribe_events();
        Self {
            process,
            events,
            program_name,
            stdout: Vec::new(),
            stderr: Vec::new(),
            closed: false,
            terminated: false,
            last_seq: 0,
        }
    }

    pub(super) fn next_process_id() -> ProcessId {
        // Process IDs are logical handles scoped to the executor connection,
        // not OS pids. A monotonic client-side id is enough to avoid
        // collisions between MCP servers started in the same session.
        let index = PROCESS_COUNTER.fetch_add(1, Ordering::Relaxed);
        ProcessId::from(format!("mcp-stdio-{index}"))
    }
}

impl Transport<RoleClient> for ExecutorProcessTransport {
    type Error = io::Error;

    fn send(
        &mut self,
        item: TxJsonRpcMessage<RoleClient>,
    ) -> impl Future<Output = std::result::Result<(), Self::Error>> + Send + 'static {
        let process = Arc::clone(&self.process);
        async move {
            // rmcp hands us a structured JSON-RPC message. Stdio transport on
            // the wire is JSON plus one newline delimiter.
            let mut bytes = to_vec(&item).map_err(io::Error::other)?;
            bytes.push(b'\n');
            let response = process.write(bytes).await.map_err(io::Error::other)?;
            match response.status {
                WriteStatus::Accepted => Ok(()),
                WriteStatus::UnknownProcess => {
                    Err(io::Error::new(io::ErrorKind::BrokenPipe, "unknown process"))
                }
                WriteStatus::StdinClosed => {
                    Err(io::Error::new(io::ErrorKind::BrokenPipe, "stdin closed"))
                }
                WriteStatus::Starting => Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "process is starting",
                )),
            }
        }
    }

    fn receive(&mut self) -> impl Future<Output = Option<RxJsonRpcMessage<RoleClient>>> + Send {
        self.receive_message()
    }

    async fn close(&mut self) -> std::result::Result<(), Self::Error> {
        self.process.terminate().await.map_err(io::Error::other)?;
        self.terminated = true;
        Ok(())
    }
}

impl ExecutorProcessTransport {
    async fn receive_message(&mut self) -> Option<RxJsonRpcMessage<RoleClient>> {
        loop {
            // rmcp stdio framing is line-oriented JSON. We first drain any
            // complete line already buffered from an earlier process event.
            if let Some(message) = self.take_stdout_message(/*allow_partial*/ self.closed) {
                return Some(message);
            }
            if self.closed {
                self.flush_stderr();
                return None;
            }

            match self.events.recv().await {
                Ok(ExecProcessEvent::Output(chunk)) => {
                    // The executor pushes raw process bytes. This is the only
                    // place where those bytes are split back into the stdout
                    // protocol stream and stderr diagnostics.
                    self.push_process_output_if_new(chunk);
                }
                Ok(ExecProcessEvent::Exited { seq, .. }) => {
                    self.note_seq(seq);
                    // Wait for `Closed` before ending the rmcp stream so any
                    // output flushed during process shutdown can still be
                    // decoded into JSON-RPC messages.
                }
                Ok(ExecProcessEvent::Closed { seq }) => {
                    self.note_seq(seq);
                    self.closed = true;
                }
                Ok(ExecProcessEvent::Failed(message)) => {
                    warn!(
                        "Remote MCP server process failed ({}): {message}",
                        self.program_name
                    );
                    self.closed = true;
                }
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    warn!(
                        "Remote MCP server output stream lagged ({}): skipped {skipped} events",
                        self.program_name
                    );
                    if let Err(error) = self.recover_lagged_events().await {
                        warn!(
                            "Failed to recover remote MCP server output stream ({}): {error}",
                            self.program_name
                        );
                        self.closed = true;
                    }
                }
                Err(broadcast::error::RecvError::Closed) => {
                    self.closed = true;
                }
            }
        }
    }

    fn note_seq(&mut self, seq: u64) {
        self.last_seq = self.last_seq.max(seq);
    }

    fn should_accept_seq(&mut self, seq: u64) -> bool {
        if seq <= self.last_seq {
            return false;
        }
        self.last_seq = seq;
        true
    }

    async fn recover_lagged_events(&mut self) -> io::Result<()> {
        let response = self
            .process
            .read(
                Some(self.last_seq),
                /*max_bytes*/ None,
                /*wait_ms*/ Some(0),
            )
            .await
            .map_err(io::Error::other)?;
        for chunk in response.chunks {
            self.push_process_output_if_new(chunk);
        }
        self.last_seq = self.last_seq.max(response.next_seq.saturating_sub(1));
        if let Some(message) = response.failure {
            warn!(
                "Remote MCP server process failed ({}): {message}",
                self.program_name
            );
            self.closed = true;
        } else if response.closed {
            self.closed = true;
        }
        Ok(())
    }

    fn push_process_output_if_new(&mut self, chunk: ProcessOutputChunk) {
        if !self.should_accept_seq(chunk.seq) {
            return;
        }
        self.push_process_output(chunk);
    }

    fn push_process_output(&mut self, chunk: ProcessOutputChunk) {
        let bytes = chunk.chunk.into_inner();
        match chunk.stream {
            // MCP stdio uses stdout as the protocol stream. PTY output is
            // accepted defensively because the executor process API has a
            // unified stream enum, but remote MCP starts with `tty=false`.
            ExecOutputStream::Stdout | ExecOutputStream::Pty => {
                self.stdout.extend_from_slice(&bytes);
            }
            // Stderr is intentionally out-of-band. It should help debug server
            // startup failures without entering rmcp framing.
            ExecOutputStream::Stderr => {
                self.push_stderr(&bytes);
            }
        }
    }

    fn take_stdout_message(&mut self, allow_partial: bool) -> Option<RxJsonRpcMessage<RoleClient>> {
        // A normal MCP stdio server emits one JSON-RPC message per newline.
        // If the process has already closed, accept a final unterminated line
        // so EOF after a complete JSON object behaves like local rmcp's
        // `decode_eof` handling.
        loop {
            let line_end = self.stdout.iter().position(|byte| *byte == b'\n');
            let line = match (line_end, allow_partial && !self.stdout.is_empty()) {
                (Some(index), _) => {
                    let mut line = self.stdout.drain(..=index).collect::<Vec<_>>();
                    line.pop();
                    line
                }
                (None, true) => self.stdout.drain(..).collect(),
                (None, false) => return None,
            };
            let line = Self::trim_trailing_carriage_return(line);
            match from_slice::<RxJsonRpcMessage<RoleClient>>(&line) {
                Ok(message) => return Some(message),
                Err(error) => {
                    debug!(
                        "Failed to parse remote MCP server message ({}): {error}",
                        self.program_name
                    );
                }
            }
        }
    }

    fn push_stderr(&mut self, bytes: &[u8]) {
        // Keep stderr line-oriented in logs so a chatty MCP server does not
        // produce one log record per byte chunk.
        self.stderr.extend_from_slice(bytes);
        while let Some(index) = self.stderr.iter().position(|byte| *byte == b'\n') {
            let mut line = self.stderr.drain(..=index).collect::<Vec<_>>();
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            info!(
                "MCP server stderr ({}): {}",
                self.program_name,
                String::from_utf8_lossy(&line)
            );
        }
    }

    fn flush_stderr(&mut self) {
        if self.stderr.is_empty() {
            return;
        }
        let line = take(&mut self.stderr);
        info!(
            "MCP server stderr ({}): {}",
            self.program_name,
            String::from_utf8_lossy(&line)
        );
    }

    fn trim_trailing_carriage_return(mut line: Vec<u8>) -> Vec<u8> {
        if line.last() == Some(&b'\r') {
            line.pop();
        }
        line
    }
}

impl Drop for ExecutorProcessTransport {
    fn drop(&mut self) {
        if self.terminated {
            return;
        }

        let process = Arc::clone(&self.process);
        let program_name = self.program_name.clone();
        let Ok(handle) = Handle::try_current() else {
            warn!(
                "Could not schedule remote MCP server process termination on drop ({}): no Tokio runtime is available",
                self.program_name
            );
            return;
        };

        std::mem::drop(handle.spawn(async move {
            if let Err(error) = process.terminate().await {
                warn!(
                    "Failed to terminate remote MCP server process on drop ({program_name}): {error}"
                );
            }
        }));
    }
}
