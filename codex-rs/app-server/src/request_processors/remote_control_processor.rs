use crate::error_code::internal_error;
use codex_app_server_protocol::RemoteControlClientsListParams;
use codex_app_server_protocol::RemoteControlClientsListResponse;
use codex_app_server_protocol::RemoteControlClientsRevokeParams;
use codex_app_server_protocol::RemoteControlClientsRevokeResponse;
use codex_app_server_protocol::RemoteControlConnectionStatus;
use codex_app_server_protocol::RemoteControlDisableResponse;
use codex_app_server_protocol::RemoteControlEnableResponse;
use codex_app_server_protocol::RemoteControlPairingStartParams;
use codex_app_server_protocol::RemoteControlPairingStartResponse;
use codex_app_server_protocol::RemoteControlStatusReadResponse;
use codex_app_server_protocol::RpcError;

#[derive(Clone)]
pub(crate) struct RemoteControlRequestProcessor {
    installation_id: String,
}

impl RemoteControlRequestProcessor {
    pub(crate) fn new(installation_id: String) -> Self {
        Self { installation_id }
    }

    pub(crate) fn enable(&self) -> Result<RemoteControlEnableResponse, RpcError> {
        Err(remote_control_removed_error())
    }

    pub(crate) fn disable(&self) -> Result<RemoteControlDisableResponse, RpcError> {
        Ok(RemoteControlDisableResponse {
            status: RemoteControlConnectionStatus::Disabled,
            server_name: "local".to_string(),
            installation_id: self.installation_id.clone(),
            environment_id: None,
        })
    }

    pub(crate) fn status_read(&self) -> Result<RemoteControlStatusReadResponse, RpcError> {
        Ok(RemoteControlStatusReadResponse {
            status: RemoteControlConnectionStatus::Disabled,
            server_name: "local".to_string(),
            installation_id: self.installation_id.clone(),
            environment_id: None,
        })
    }

    pub(crate) async fn pairing_start(
        &self,
        _params: RemoteControlPairingStartParams,
    ) -> Result<RemoteControlPairingStartResponse, RpcError> {
        Err(remote_control_removed_error())
    }

    pub(crate) async fn clients_list(
        &self,
        _params: RemoteControlClientsListParams,
    ) -> Result<RemoteControlClientsListResponse, RpcError> {
        Err(remote_control_removed_error())
    }

    pub(crate) async fn clients_revoke(
        &self,
        _params: RemoteControlClientsRevokeParams,
    ) -> Result<RemoteControlClientsRevokeResponse, RpcError> {
        Err(remote_control_removed_error())
    }
}

fn remote_control_removed_error() -> RpcError {
    internal_error("remote control is unavailable in the native gRPC app-server experiment")
}
