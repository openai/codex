use crate::error_code::internal_error;
use crate::error_code::invalid_request;
use crate::transport::RemoteControlHandle;
use crate::transport::RemoteControlUnavailable;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::RemoteControlDisableResponse;
use codex_app_server_protocol::RemoteControlEnableResponse;
use codex_app_server_protocol::RemoteControlStatusReadResponse;

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

    pub(crate) fn status_read(&self) -> Result<RemoteControlStatusReadResponse, JSONRPCErrorError> {
        let status = self.handle()?.status();
        Ok(RemoteControlStatusReadResponse {
            status: status.status,
            server_name: status.server_name,
            installation_id: status.installation_id,
            environment_id: status.environment_id,
        })
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
