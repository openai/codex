use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use codex_app_server_protocol::JSONRPCErrorError;

use crate::protocol::InitializeResponse;
use crate::server::jsonrpc::invalid_request;

pub(crate) struct ExecServerHandler {
    initialize_requested: AtomicBool,
    initialized: AtomicBool,
}

impl ExecServerHandler {
    pub(crate) fn new() -> Self {
        Self {
            initialize_requested: AtomicBool::new(false),
            initialized: AtomicBool::new(false),
        }
    }

    pub(crate) async fn shutdown(&self) {}

    pub(crate) fn initialize(&self) -> Result<InitializeResponse, JSONRPCErrorError> {
        if self.initialize_requested.swap(true, Ordering::SeqCst) {
            return Err(invalid_request(
                "initialize may only be sent once per connection".to_string(),
            ));
        }
        Ok(InitializeResponse {})
    }

    pub(crate) fn initialized(&self) -> Result<(), String> {
        if !self.initialize_requested.load(Ordering::SeqCst) {
            return Err("received `initialized` notification before `initialize`".into());
        }
        self.initialized.store(true, Ordering::SeqCst);
        Ok(())
    }
}
