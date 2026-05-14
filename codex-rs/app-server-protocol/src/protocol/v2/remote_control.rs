use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

/// Current remote-control connection status and remote identity exposed to clients.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct RemoteControlStatusChangedNotification {
    pub status: RemoteControlConnectionStatus,
    pub installation_id: String,
    pub environment_id: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct RemoteControlEnableResponse {
    pub status: RemoteControlConnectionStatus,
    pub installation_id: String,
    pub environment_id: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct RemoteControlDisableResponse {
    pub status: RemoteControlConnectionStatus,
    pub installation_id: String,
    pub environment_id: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
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
            installation_id,
            environment_id,
        } = notification;
        Self {
            status,
            installation_id,
            environment_id,
        }
    }
}

impl From<RemoteControlStatusChangedNotification> for RemoteControlDisableResponse {
    fn from(notification: RemoteControlStatusChangedNotification) -> Self {
        let RemoteControlStatusChangedNotification {
            status,
            installation_id,
            environment_id,
        } = notification;
        Self {
            status,
            installation_id,
            environment_id,
        }
    }
}
