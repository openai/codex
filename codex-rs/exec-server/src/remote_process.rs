use async_trait::async_trait;
use tokio::sync::broadcast;
use tracing::trace;

use crate::ExecProcess;
use crate::ExecServerClient;
use crate::ExecServerError;
use crate::ExecServerEvent;
use crate::protocol::ExecParams;
use crate::protocol::ExecResponse;
use crate::protocol::ReadParams;
use crate::protocol::ReadResponse;
use crate::protocol::TerminateResponse;
use crate::protocol::WriteResponse;

#[derive(Clone)]
pub(crate) struct RemoteProcess {
    client: ExecServerClient,
}

impl RemoteProcess {
    pub(crate) fn new(client: ExecServerClient) -> Self {
        trace!("remote process new");
        Self { client }
    }
}

#[async_trait]
impl ExecProcess for RemoteProcess {
    async fn start(&self, params: ExecParams) -> Result<ExecResponse, ExecServerError> {
        trace!("remote process start");
        self.client.exec(params).await
    }

    async fn read(&self, params: ReadParams) -> Result<ReadResponse, ExecServerError> {
        trace!("remote process read");
        self.client.read(params).await
    }

    async fn write(
        &self,
        process_id: &str,
        chunk: Vec<u8>,
    ) -> Result<WriteResponse, ExecServerError> {
        trace!("remote process write");
        self.client.write(process_id, chunk).await
    }

    async fn terminate(&self, process_id: &str) -> Result<TerminateResponse, ExecServerError> {
        trace!("remote process terminate");
        self.client.terminate(process_id).await
    }

    fn subscribe_events(&self) -> broadcast::Receiver<ExecServerEvent> {
        trace!("remote process subscribe_events");
        self.client.event_receiver()
    }
}
