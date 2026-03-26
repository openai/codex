use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::watch;

use crate::ExecServerError;
use crate::ProcessId;
use crate::protocol::ExecParams;
use crate::protocol::ReadResponse;
use crate::protocol::WriteResponse;

pub struct StartedExecProcess {
    pub process: Arc<dyn ExecProcess>,
}

#[async_trait]
pub trait ExecProcess: Send + Sync {
    fn process_id(&self) -> &ProcessId;

    fn subscribe_wake(&self) -> watch::Receiver<u64>;

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
