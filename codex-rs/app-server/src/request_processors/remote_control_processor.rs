use crate::error_code::internal_error;
use crate::error_code::invalid_request;
use crate::transport::RemoteControlEnableError;
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
use codex_core::path_utils::write_atomically;
use serde_json::Map;
use serde_json::Value;
use std::fs;
use std::fs::OpenOptions;
use std::fs::TryLockError;
use std::io;
use std::path::PathBuf;

#[derive(Clone)]
pub(crate) struct RemoteControlRequestProcessor {
    remote_control_handle: Option<RemoteControlHandle>,
    pub(crate) daemon_settings_file: Option<PathBuf>,
}

impl RemoteControlRequestProcessor {
    pub(crate) fn new(remote_control_handle: Option<RemoteControlHandle>) -> Self {
        Self {
            remote_control_handle,
            daemon_settings_file: None,
        }
    }

    pub(crate) async fn enable(
        &self,
        ephemeral: bool,
        app_server_client_name: Option<&str>,
    ) -> Result<RemoteControlEnableResponse, JSONRPCErrorError> {
        let handle = self.handle()?;
        let status = if ephemeral {
            handle.enable_ephemeral().map_err(map_enable_error)?
        } else {
            handle
                .enable(app_server_client_name)
                .await
                .map_err(map_update_error)?
        };
        if !ephemeral {
            self.persist_daemon_preference(/*enabled*/ true)
                .await
                .map_err(map_update_error)?;
        }
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
        if !ephemeral {
            self.persist_daemon_preference(/*enabled*/ false)
                .await
                .map_err(map_update_error)?;
        }
        Ok(RemoteControlDisableResponse::from(status))
    }

    async fn persist_daemon_preference(&self, enabled: bool) -> io::Result<()> {
        let Some(path) = self.daemon_settings_file.clone() else {
            return Ok(());
        };
        tokio::task::spawn_blocking(move || {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let lock = OpenOptions::new()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(path.with_file_name("daemon.lock"))?;
            // The daemon may be waiting for this RPC during shutdown. Never wait
            // for it: a concurrent lifecycle operation wins over RPC persistence.
            match lock.try_lock() {
                Ok(()) => {}
                Err(TryLockError::WouldBlock) => return Ok(()),
                Err(TryLockError::Error(err)) => return Err(err),
            }
            let mut settings: Map<String, Value> = match fs::read(&path) {
                Ok(contents) => serde_json::from_slice(&contents)?,
                Err(err) if err.kind() == io::ErrorKind::NotFound => Map::new(),
                Err(err) => return Err(err),
            };
            settings.insert("remoteControlEnabled".to_string(), Value::Bool(enabled));
            write_atomically(&path, &serde_json::to_string_pretty(&settings)?)
        })
        .await
        .map_err(io::Error::other)?
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
        let handle = self.handle()?;
        handle
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
        let handle = self
            .remote_control_handle
            .as_ref()
            .ok_or_else(|| internal_error("remote control is unavailable for this app-server"))?;
        handle
            .ensure_remote_control_allowed()
            .map_err(|err| invalid_request(err.to_string()))?;
        Ok(handle)
    }
}

fn map_enable_error(err: RemoteControlEnableError) -> JSONRPCErrorError {
    match err {
        RemoteControlEnableError::Unavailable(err) => map_unavailable(err),
        RemoteControlEnableError::DisabledByRequirements(err) => invalid_request(err.to_string()),
        RemoteControlEnableError::AuthenticationChanged => internal_error(err.to_string()),
    }
}

fn map_unavailable(err: RemoteControlUnavailable) -> JSONRPCErrorError {
    invalid_request(err.to_string())
}

fn map_update_error(err: io::Error) -> JSONRPCErrorError {
    if matches!(
        err.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied
    ) {
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
