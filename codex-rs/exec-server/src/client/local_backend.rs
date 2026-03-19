use std::sync::Arc;

use crate::protocol::InitializeResponse;
use crate::server::ExecServerHandler;

use super::ExecServerError;

#[derive(Clone)]
pub(super) struct LocalBackend {
    handler: Arc<ExecServerHandler>,
}

impl LocalBackend {
    pub(super) fn new(handler: ExecServerHandler) -> Self {
        Self {
            handler: Arc::new(handler),
        }
    }

    pub(super) async fn shutdown(&self) {
        self.handler.shutdown().await;
    }

    pub(super) async fn initialize(&self) -> Result<InitializeResponse, ExecServerError> {
        self.handler
            .initialize()
            .map_err(|error| ExecServerError::Server {
                code: error.code,
                message: error.message,
            })
    }

    pub(super) async fn initialized(&self) -> Result<(), ExecServerError> {
        self.handler
            .initialized()
            .map_err(ExecServerError::Protocol)
    }
}
