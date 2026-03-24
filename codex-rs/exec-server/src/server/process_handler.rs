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
        self.process.initialize()
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

    pub(crate) async fn exec(&self, params: ExecParams) -> Result<ExecResponse, JSONRPCErrorError> {
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
