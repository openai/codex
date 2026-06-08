use schemars::JsonSchema;
#[cfg(any(test, feature = "serde-compat"))]
use serde::Deserialize;
#[cfg(any(test, feature = "serde-compat"))]
use serde::Serialize;
use ts_rs::TS;

/// Current remote-control connection status and remote identity exposed to clients.
#[derive(Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct RemoteControlStatusChangedNotification {
    pub status: RemoteControlConnectionStatus,
    pub server_name: String,
    pub installation_id: String,
    pub environment_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct RemoteControlEnableResponse {
    pub status: RemoteControlConnectionStatus,
    pub server_name: String,
    pub installation_id: String,
    pub environment_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct RemoteControlDisableResponse {
    pub status: RemoteControlConnectionStatus,
    pub server_name: String,
    pub installation_id: String,
    pub environment_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct RemoteControlStatusReadResponse {
    pub status: RemoteControlConnectionStatus,
    pub server_name: String,
    pub installation_id: String,
    pub environment_id: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct RemoteControlPairingStartParams {
    #[cfg_attr(
        any(test, feature = "serde-compat"),
        serde(default, skip_serializing_if = "std::ops::Not::not")
    )]
    pub manual_code: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct RemoteControlPairingStartResponse {
    pub pairing_code: String,
    pub manual_pairing_code: Option<String>,
    pub environment_id: String,
    pub expires_at: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct RemoteControlClientsListParams {
    pub environment_id: String,
    #[ts(optional = nullable)]
    pub cursor: Option<String>,
    #[ts(optional = nullable)]
    pub limit: Option<u32>,
    #[ts(optional = nullable)]
    pub order: Option<RemoteControlClientsListOrder>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(rename_all = "camelCase", export_to = "v2/")]
pub enum RemoteControlClientsListOrder {
    Asc,
    Desc,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct RemoteControlClientsListResponse {
    pub data: Vec<RemoteControlClient>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct RemoteControlClient {
    pub client_id: String,
    pub display_name: Option<String>,
    pub device_type: Option<String>,
    pub platform: Option<String>,
    pub os_version: Option<String>,
    pub device_model: Option<String>,
    pub app_version: Option<String>,
    pub last_seen_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct RemoteControlClientsRevokeParams {
    pub environment_id: String,
    pub client_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct RemoteControlClientsRevokeResponse {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(rename_all = "camelCase", export_to = "v2/")]
pub enum RemoteControlConnectionStatus {
    Disabled,
    Connecting,
    Connected,
    Errored,
}

impl From<RemoteControlStatusChangedNotification> for RemoteControlEnableResponse {
    fn from(notification: RemoteControlStatusChangedNotification) -> Self {
        let RemoteControlStatusChangedNotification {
            status,
            server_name,
            installation_id,
            environment_id,
        } = notification;
        Self {
            status,
            server_name,
            installation_id,
            environment_id,
        }
    }
}

impl From<RemoteControlStatusChangedNotification> for RemoteControlDisableResponse {
    fn from(notification: RemoteControlStatusChangedNotification) -> Self {
        let RemoteControlStatusChangedNotification {
            status,
            server_name,
            installation_id,
            environment_id,
        } = notification;
        Self {
            status,
            server_name,
            installation_id,
            environment_id,
        }
    }
}

#[cfg(test)]
#[path = "remote_control_tests.rs"]
mod tests;
