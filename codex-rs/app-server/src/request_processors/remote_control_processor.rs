use crate::error_code::internal_error;
use crate::error_code::invalid_request;
use crate::transport::RemoteControlHandle;
use crate::transport::RemoteControlUnavailable;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::RemoteControlClientsListParams;
use codex_app_server_protocol::RemoteControlClientsListResponse;
use codex_app_server_protocol::RemoteControlClientsRevokeParams;
use codex_app_server_protocol::RemoteControlClientsRevokeResponse;
use codex_app_server_protocol::RemoteControlDisableResponse;
use codex_app_server_protocol::RemoteControlEnableResponse;
use codex_app_server_protocol::RemoteControlPairingStartParams;
use codex_app_server_protocol::RemoteControlPairingStartResponse;
use codex_app_server_protocol::RemoteControlStatusReadResponse;
use std::io;

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

    pub(crate) async fn pairing_start(
        &self,
        params: RemoteControlPairingStartParams,
    ) -> Result<RemoteControlPairingStartResponse, JSONRPCErrorError> {
        self.handle()?
            .start_pairing(params)
            .await
            .map_err(map_pairing_start_error)
    }

    pub(crate) async fn clients_list(
        &self,
        params: RemoteControlClientsListParams,
    ) -> Result<RemoteControlClientsListResponse, JSONRPCErrorError> {
        self.handle()?
            .list_clients(params)
            .await
            .map_err(map_client_management_error)
    }

    pub(crate) async fn clients_revoke(
        &self,
        params: RemoteControlClientsRevokeParams,
    ) -> Result<RemoteControlClientsRevokeResponse, JSONRPCErrorError> {
        self.handle()?
            .revoke_client(params)
            .await
            .map_err(map_client_management_error)
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

fn map_pairing_start_error(err: io::Error) -> JSONRPCErrorError {
    if err.kind() == io::ErrorKind::InvalidInput {
        invalid_request(err.to_string())
    } else {
        internal_error(err.to_string())
    }
}

fn map_client_management_error(err: io::Error) -> JSONRPCErrorError {
    match err.kind() {
        io::ErrorKind::InvalidInput
        | io::ErrorKind::NotFound
        | io::ErrorKind::PermissionDenied
        | io::ErrorKind::WouldBlock => invalid_request(err.to_string()),
        _ => internal_error(err.to_string()),
    }
}

#[cfg(test)]
mod remote_control_processor_tests;
