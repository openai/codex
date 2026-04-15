use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::broadcast;
use tokio::sync::watch;

use crate::ExecServerError;
use crate::ProcessId;
use crate::protocol::ExecParams;
use crate::protocol::ProcessOutputChunk;
use crate::protocol::ReadResponse;
use crate::protocol::WriteResponse;

pub struct StartedExecProcess {
    pub process: Arc<dyn ExecProcess>,
}

/// Pushed process events for consumers that want to follow process output as it
/// arrives instead of polling retained output with [`ExecProcess::read`].
///
/// The stream is scoped to one [`ExecProcess`] handle. `Output` events carry
/// stdout, stderr, or pty bytes. `Exited` reports the process exit status, while
/// `Closed` means all output streams have ended and no more output events will
/// arrive. `Failed` is used when the process session cannot continue, for
/// example because the remote executor connection disconnected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecProcessEvent {
    Output(ProcessOutputChunk),
    Exited { seq: u64, exit_code: i32 },
    Closed { seq: u64 },
    Failed(String),
}

/// Handle for an executor-managed process.
///
/// Implementations must support both retained-output reads and pushed events:
/// `read` is the request/response API for callers that want to page through
/// buffered output, while `subscribe_events` is the streaming API for callers
/// that want output and lifecycle changes delivered as they happen.
#[async_trait]
pub trait ExecProcess: Send + Sync {
    fn process_id(&self) -> &ProcessId;

    fn subscribe_wake(&self) -> watch::Receiver<u64>;

    fn subscribe_events(&self) -> broadcast::Receiver<ExecProcessEvent>;

    async fn read(
        &self,
        after_seq: Option<u64>,
        max_bytes: Option<usize>,
        wait_ms: Option<u64>,
    ) -> Result<ReadResponse, ExecServerError>;

    async fn write(&self, chunk: Vec<u8>) -> Result<WriteResponse, ExecServerError>;

    async fn terminate(&self) -> Result<(), ExecServerError>;
}

#[async_trait]
pub trait ExecBackend: Send + Sync {
    async fn start(&self, params: ExecParams) -> Result<StartedExecProcess, ExecServerError>;
}
