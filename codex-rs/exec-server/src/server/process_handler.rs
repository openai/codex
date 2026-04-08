use codex_app_server_protocol::JSONRPCErrorError;

use crate::local_process::LocalProcess;
use crate::protocol::ExecParams;
use crate::protocol::ExecResponse;
use crate::protocol::InitializeResponse;
use crate::protocol::ReadParams;
use crate::protocol::ReadResponse;
use crate::protocol::TerminateParams;
use crate::protocol::TerminateResponse;
use crate::protocol::WriteParams;
use crate::protocol::WriteResponse;
use crate::rpc::RpcNotificationSender;
use crate::rpc::internal_error;

#[derive(Clone)]
pub(crate) struct ProcessHandler {
    process: LocalProcess,
}

impl ProcessHandler {
    pub(crate) fn new(notifications: RpcNotificationSender) -> Self {
        Self {
            process: LocalProcess::new(notifications),
        }
    }

    pub(crate) async fn shutdown(&self) {
        self.process.shutdown().await;
    }

    pub(crate) fn initialize(&self) -> Result<InitializeResponse, JSONRPCErrorError> {
        self.process.initialize()?;
        let cwd = std::env::current_dir().map_err(|err| {
            internal_error(format!(
                "failed to read exec-server current directory: {err}"
            ))
        })?;
        Ok(InitializeResponse { cwd })
    }

    pub(crate) fn initialized(&self) -> Result<(), String> {
        self.process.initialized()
    }

    pub(crate) fn require_initialized_for(
        &self,
        method_family: &str,
    ) -> Result<(), JSONRPCErrorError> {
        self.process.require_initialized_for(method_family)
    }

    pub(crate) async fn exec(
        &self,
        mut params: ExecParams,
    ) -> Result<ExecResponse, JSONRPCErrorError> {
        // TODO(exec-server): replace this process-wide inherit with an
        // exec-server-side environment policy and explicit request overrides.
        params.env = std::env::vars().collect();
        self.process.exec(params).await
    }

    pub(crate) async fn exec_read(
        &self,
        params: ReadParams,
    ) -> Result<ReadResponse, JSONRPCErrorError> {
        self.process.exec_read(params).await
    }

    pub(crate) async fn exec_write(
        &self,
        params: WriteParams,
    ) -> Result<WriteResponse, JSONRPCErrorError> {
        self.process.exec_write(params).await
    }

    pub(crate) async fn terminate(
        &self,
        params: TerminateParams,
    ) -> Result<TerminateResponse, JSONRPCErrorError> {
        self.process.terminate_process(params).await
    }
}
