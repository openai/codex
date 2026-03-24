use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::broadcast;

use crate::ExecServerError;
use crate::protocol::ExecOutputStream;
use crate::protocol::ExecParams;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecSessionEvent {
    Output {
        seq: u64,
        stream: ExecOutputStream,
        chunk: Vec<u8>,
    },
    Exited {
        seq: u64,
        exit_code: i32,
    },
    Closed {
        seq: u64,
    },
}

#[async_trait]
pub trait ExecProcess: Send + Sync {
    fn process_id(&self) -> &str; // TODO(codex) make this a ProcessId struct

    fn subscribe(&self) -> broadcast::Receiver<ExecSessionEvent>;

    async fn write_stdin(&self, chunk: Vec<u8>) -> Result<(), ExecServerError>; // TODO(codex) rename to write()

    async fn terminate(&self) -> Result<(), ExecServerError>;
}

#[async_trait]
pub trait ExecBackend: Send + Sync {
    async fn start(&self, params: ExecParams) -> Result<Arc<dyn ExecProcess>, ExecServerError>;
}
