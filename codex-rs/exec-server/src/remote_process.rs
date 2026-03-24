use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use async_trait::async_trait;
use tokio::sync::broadcast;

use crate::ExecBackend;
use crate::ExecProcess;
use crate::ExecServerClient;
use crate::ExecServerError;
use crate::ExecSessionEvent;
use crate::ProcessId;
use crate::protocol::ExecParams;

#[derive(Clone)]
pub(crate) struct RemoteProcess {
    client: ExecServerClient,
}

struct RemoteExecProcess {
    process_id: ProcessId,
    events_tx: broadcast::Sender<ExecSessionEvent>,
    initial_events_rx: StdMutex<Option<broadcast::Receiver<ExecSessionEvent>>>,
    backend: RemoteProcess,
}

impl RemoteProcess {
    pub(crate) fn new(client: ExecServerClient) -> Self {
        Self { client }
    }

    async fn write(&self, process_id: &str, chunk: Vec<u8>) -> Result<(), ExecServerError> {
        let response = self.client.write(process_id, chunk).await?;
        if response.accepted {
            Ok(())
        } else {
            Err(ExecServerError::Protocol(format!(
                "exec-server did not accept stdin for process {process_id}"
            )))
        }
    }

    async fn terminate_process(&self, process_id: &str) -> Result<(), ExecServerError> {
        self.client.terminate(process_id).await?;
        Ok(())
    }
}

#[async_trait]
impl ExecBackend for RemoteProcess {
    async fn start(&self, params: ExecParams) -> Result<Arc<dyn ExecProcess>, ExecServerError> {
        let process_id = params.process_id.clone();
        let (events_tx, events_rx) = self.client.register_session(&process_id).await?;
        if let Err(err) = self.client.exec(params).await {
            self.client.unregister_session(&process_id).await;
            return Err(err);
        }

        Ok(Arc::new(RemoteExecProcess {
            process_id: process_id.into(),
            events_tx,
            initial_events_rx: StdMutex::new(Some(events_rx)),
            backend: self.clone(),
        }))
    }
}

#[async_trait]
impl ExecProcess for RemoteExecProcess {
    fn process_id(&self) -> &ProcessId {
        &self.process_id
    }

    fn subscribe(&self) -> broadcast::Receiver<ExecSessionEvent> {
        let mut initial_events_rx = self
            .initial_events_rx
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        initial_events_rx
            .take()
            .unwrap_or_else(|| self.events_tx.subscribe())
    }

    async fn write(&self, chunk: Vec<u8>) -> Result<(), ExecServerError> {
        self.backend.write(&self.process_id, chunk).await
    }

    async fn terminate(&self) -> Result<(), ExecServerError> {
        self.backend.terminate_process(&self.process_id).await
    }
}
