use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::watch;

use crate::ExecServerError;
use crate::ProcessId;
use crate::protocol::ExecCapabilities;
use crate::protocol::ExecParams;
use crate::protocol::ReadResponse;
use crate::protocol::WriteResponse;

pub struct StartedExecProcess {
    pub process: Arc<dyn ExecProcess>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecStartRequest {
    pub params: ExecParams,
    pub launch: ExecLaunch,
}

impl ExecStartRequest {
    pub fn new(params: ExecParams, launch: ExecLaunch) -> Self {
        Self { params, launch }
    }
}

impl From<ExecParams> for ExecStartRequest {
    fn from(params: ExecParams) -> Self {
        Self {
            params,
            launch: ExecLaunch::Direct,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecLaunch {
    Direct,
    ZshFork,
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
    fn capabilities(&self) -> ExecCapabilities;

    async fn start(&self, request: ExecStartRequest)
    -> Result<StartedExecProcess, ExecServerError>;
}
