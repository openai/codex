use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use async_trait::async_trait;
use tokio::sync::broadcast;

use crate::ExecBackend;
use crate::ExecProcess;
use crate::ExecServerClient;
use crate::ExecServerError;
use crate::ExecSessionEvent;
use crate::protocol::ExecParams;

#[derive(Clone)]
pub(crate) struct RemoteProcess {
    client: ExecServerClient,
}

struct RemoteExecProcess {
    process_id: String,
    events: StdMutex<broadcast::Receiver<ExecSessionEvent>>,
    backend: RemoteProcess,
}

impl RemoteProcess {
    pub(crate) fn new(client: ExecServerClient) -> Self {
        Self { client }
    }

    async fn write_stdin(&self, process_id: &str, chunk: Vec<u8>) -> Result<(), ExecServerError> {
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
        let events = self.client.register_session(&process_id).await?;
        if let Err(err) = self.client.exec(params).await {
            self.client.unregister_session(&process_id).await;
            return Err(err);
        }

        Ok(Arc::new(RemoteExecProcess {
            process_id,
            events: StdMutex::new(events),
            backend: self.clone(),
        }))
    }
}

#[async_trait]
impl ExecProcess for RemoteExecProcess {
    fn process_id(&self) -> &str {
        &self.process_id
    }

    fn subscribe(&self) -> broadcast::Receiver<ExecSessionEvent> {
        self
            .events
            .lock()
            .expect("remote exec process events mutex should not be poisoned")
            .resubscribe()
    }

    async fn write_stdin(&self, chunk: Vec<u8>) -> Result<(), ExecServerError> {
        self.backend.write_stdin(&self.process_id, chunk).await
    }

    async fn terminate(&self) -> Result<(), ExecServerError> {
        self.backend.terminate_process(&self.process_id).await
    }
}
