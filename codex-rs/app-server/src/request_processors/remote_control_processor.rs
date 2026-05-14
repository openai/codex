use crate::error_code::internal_error;
use crate::error_code::invalid_request;
use crate::transport::RemoteControlHandle;
use crate::transport::RemoteControlUnavailable;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::RemoteControlDisableResponse;
use codex_app_server_protocol::RemoteControlEnableResponse;

#[derive(Clone)]
pub(crate) struct RemoteControlRequestProcessor {
    remote_control_handle: Option<RemoteControlHandle>,
}

impl RemoteControlRequestProcessor {
    pub(crate) fn new(remote_control_handle: Option<RemoteControlHandle>) -> Self {
        Self {
            remote_control_handle,
        }
    }

    pub(crate) fn enable(&self) -> Result<RemoteControlEnableResponse, JSONRPCErrorError> {
        let handle = self.handle()?;
        handle
            .enable()
            .map(RemoteControlEnableResponse::from)
            .map_err(map_unavailable)
    }

    pub(crate) fn disable(&self) -> Result<RemoteControlDisableResponse, JSONRPCErrorError> {
        let handle = self.handle()?;
        Ok(RemoteControlDisableResponse::from(handle.disable()))
    }

    fn handle(&self) -> Result<&RemoteControlHandle, JSONRPCErrorError> {
        self.remote_control_handle
            .as_ref()
            .ok_or_else(|| internal_error("remote control is unavailable for this app-server"))
    }
}

fn map_unavailable(err: RemoteControlUnavailable) -> JSONRPCErrorError {
    invalid_request(err.to_string())
}
