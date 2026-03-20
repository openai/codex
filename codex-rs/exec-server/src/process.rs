use async_trait::async_trait;
use tokio::sync::broadcast;

use crate::ExecServerError;
use crate::protocol::ExecExitedNotification;
use crate::protocol::ExecOutputDeltaNotification;
use crate::protocol::ExecParams;
use crate::protocol::ExecResponse;
use crate::protocol::ReadParams;
use crate::protocol::ReadResponse;
use crate::protocol::TerminateResponse;
use crate::protocol::WriteResponse;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecServerEvent {
    OutputDelta(ExecOutputDeltaNotification),
    Exited(ExecExitedNotification),
}

#[async_trait]
pub trait ExecProcess: Send + Sync {
    async fn start(&self, params: ExecParams) -> Result<ExecResponse, ExecServerError>;

    async fn read(&self, params: ReadParams) -> Result<ReadResponse, ExecServerError>;

    async fn write(
        &self,
        process_id: &str,
        chunk: Vec<u8>,
    ) -> Result<WriteResponse, ExecServerError>;

    async fn terminate(&self, process_id: &str) -> Result<TerminateResponse, ExecServerError>;

    fn subscribe_events(&self) -> broadcast::Receiver<ExecServerEvent>;
}
