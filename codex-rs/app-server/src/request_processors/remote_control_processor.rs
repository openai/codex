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
use codex_app_server_protocol::RemoteControlPairingStatusParams;
use codex_app_server_protocol::RemoteControlPairingStatusResponse;
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

    pub(crate) async fn enable(
        &self,
        ephemeral: bool,
        app_server_client_name: Option<&str>,
    ) -> Result<RemoteControlEnableResponse, JSONRPCErrorError> {
        let handle = self.handle()?;
        let status = if ephemeral {
            handle.enable_ephemeral().map_err(map_unavailable)?
        } else {
            handle
                .enable(app_server_client_name)
                .await
                .map_err(map_update_error)?
        };
        Ok(RemoteControlEnableResponse::from(status))
    }

    pub(crate) async fn disable(
        &self,
        ephemeral: bool,
        app_server_client_name: Option<&str>,
    ) -> Result<RemoteControlDisableResponse, JSONRPCErrorError> {
        let handle = self.handle()?;
        let status = if ephemeral {
            handle.disable_ephemeral().await
        } else {
            handle
                .disable(app_server_client_name)
                .await
                .map_err(map_update_error)?
        };
        Ok(RemoteControlDisableResponse::from(status))
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
        app_server_client_name: Option<&str>,
    ) -> Result<RemoteControlPairingStartResponse, JSONRPCErrorError> {
        self.handle()?
            .start_pairing(params, app_server_client_name)
            .await
            .map_err(map_pairing_start_error)
    }

    pub(crate) async fn pairing_status(
        &self,
        params: RemoteControlPairingStatusParams,
    ) -> Result<RemoteControlPairingStatusResponse, JSONRPCErrorError> {
        validate_pairing_status_params(&params)?;
        self.handle()?
            .pairing_status(params)
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

fn map_update_error(err: io::Error) -> JSONRPCErrorError {
    if err.kind() == io::ErrorKind::NotFound {
        invalid_request(err.to_string())
    } else {
        internal_error(err.to_string())
    }
}

fn map_pairing_start_error(err: io::Error) -> JSONRPCErrorError {
    if err.kind() == io::ErrorKind::InvalidInput {
        invalid_request(err.to_string())
    } else {
        internal_error(err.to_string())
    }
}

fn validate_pairing_status_params(
    params: &RemoteControlPairingStatusParams,
) -> Result<(), JSONRPCErrorError> {
    match (&params.pairing_code, &params.manual_pairing_code) {
        (Some(_), None) | (None, Some(_)) => Ok(()),
        (Some(_), Some(_)) => Err(invalid_request(
            "remoteControl/pairing/status accepts either pairingCode or manualPairingCode, not both",
        )),
        (None, None) => Err(invalid_request(
            "remoteControl/pairing/status requires pairingCode or manualPairingCode",
        )),
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
