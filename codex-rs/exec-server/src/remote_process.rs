use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::watch;
use tracing::trace;

use crate::ExecBackend;
use crate::ExecProcess;
use crate::ExecProcessEventReceiver;
use crate::ExecServerError;
use crate::StartedExecProcess;
use crate::client::LazyRemoteExecServerClient;
use crate::client::Session;
use crate::protocol::ExecParams;
use crate::protocol::ReadResponse;
use crate::protocol::WriteResponse;

#[derive(Clone)]
pub(crate) struct RemoteProcess {
    client: LazyRemoteExecServerClient,
}

struct RemoteExecProcess {
    session: Session,
}

impl RemoteProcess {
    pub(crate) fn new(client: LazyRemoteExecServerClient) -> Self {
        trace!("remote process new");
        Self { client }
    }
}

#[async_trait]
impl ExecBackend for RemoteProcess {
    async fn start(&self, params: ExecParams) -> Result<StartedExecProcess, ExecServerError> {
        let process_id = params.process_id.clone();
        let client = self.client.get().await?;
        let session = client.register_session(&process_id).await?;
        if let Err(err) = client.exec(params).await {
            session.unregister().await;
            return Err(err);
        }

        Ok(StartedExecProcess {
            process: Arc::new(RemoteExecProcess { session }),
        })
    }
}

#[async_trait]
impl ExecProcess for RemoteExecProcess {
    fn process_id(&self) -> &crate::ProcessId {
        self.session.process_id()
    }

    fn subscribe_wake(&self) -> watch::Receiver<u64> {
        self.session.subscribe_wake()
    }

    fn subscribe_events(&self) -> ExecProcessEventReceiver {
        self.session.subscribe_events()
    }

    async fn read(
        &self,
        after_seq: Option<u64>,
        max_bytes: Option<usize>,
        wait_ms: Option<u64>,
    ) -> Result<ReadResponse, ExecServerError> {
        self.session.read(after_seq, max_bytes, wait_ms).await
    }

    async fn write(&self, chunk: Vec<u8>) -> Result<WriteResponse, ExecServerError> {
        trace!("exec process write");
        self.session.write(chunk).await
    }

    async fn terminate(&self) -> Result<(), ExecServerError> {
        trace!("exec process terminate");
        self.session.terminate().await
    }
}

impl Drop for RemoteExecProcess {
    fn drop(&mut self) {
        let session = self.session.clone();
        tokio::spawn(async move {
            session.unregister().await;
        });
    }
}
