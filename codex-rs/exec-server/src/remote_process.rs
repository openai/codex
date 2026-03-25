use std::sync::Arc;

use async_trait::async_trait;

use crate::ExecBackend;
use crate::ExecProcess;
use crate::ExecServerClient;
use crate::ExecServerError;
use crate::ProcessId;
use crate::StartedExecProcess;
use crate::protocol::ExecParams;

#[derive(Clone)]
pub(crate) struct RemoteProcess {
    client: ExecServerClient,
}

struct RemoteExecProcess {
    process_id: ProcessId,
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
    async fn start(&self, params: ExecParams) -> Result<StartedExecProcess, ExecServerError> {
        let process_id = params.process_id.clone();
        let events = self.client.register_session(&process_id).await?;
        if let Err(err) = self.client.exec(params).await {
            self.client.unregister_session(&process_id).await;
            return Err(err);
        }

        Ok(StartedExecProcess {
            process: Arc::new(RemoteExecProcess {
                process_id: process_id.into(),
                backend: self.clone(),
            }),
            events,
        })
    }
}

#[async_trait]
impl ExecProcess for RemoteExecProcess {
    fn process_id(&self) -> &ProcessId {
        &self.process_id
    }

    async fn write(&self, chunk: Vec<u8>) -> Result<(), ExecServerError> {
        self.backend.write(&self.process_id, chunk).await
    }

    async fn terminate(&self) -> Result<(), ExecServerError> {
        self.backend.terminate_process(&self.process_id).await
    }
}
