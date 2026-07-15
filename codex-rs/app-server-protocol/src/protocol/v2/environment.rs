use codex_utils_path_uri::PathUri;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct EnvironmentAddParams {
    pub environment_id: String,
    pub exec_server_url: String,
    /// Optional WebSocket connection timeout. The server default applies when omitted.
    #[ts(type = "number | null")]
    #[ts(optional = nullable)]
    pub connect_timeout_ms: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct EnvironmentAddResponse {}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase", export_to = "v2/")]
pub struct EnvironmentConnectionNotification {
    pub thread_id: String,
    pub environment_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct EnvironmentInfoParams {
    pub environment_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct EnvironmentInfoResponse {
    pub shell: EnvironmentShellInfo,
    /// Default working directory reported by the environment, as a canonical file URI.
    pub cwd: Option<PathUri>,
}

/// Parameters for reading the current status of one configured environment.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct EnvironmentStatusParams {
    /// Environment id to inspect.
    pub environment_id: String,
}

/// Current status for the requested environment.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct EnvironmentStatusResponse {
    /// Current status observed without starting or recovering the environment.
    pub status: EnvironmentStatusKind,
    /// Human-readable detail for `disconnected` and `unknown`; omitted for other statuses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub error: Option<String>,
}

/// Current status observed by app-server without starting or recovering an environment.
///
/// For a currently ready remote environment, app-server asks the existing
/// exec-server connection for `environment/status` without allowing recovery.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase", export_to = "v2/")]
pub enum EnvironmentStatusKind {
    /// The environment is local, or an already-connected remote exec-server answered
    /// `environment/status` over its existing initialized connection.
    Ready,
    /// The configured environment has no ready connection and no observed connection failure.
    /// This includes lazy environments that have never been started and initial startup that has
    /// not finished.
    Pending,
    /// A connection attempt, prior connection, or fail-fast `environment/status` probe observed
    /// a failure. This does not promise the failure is terminal: later normal environment use may
    /// recover it. This call does not trigger recovery; `error` contains the observed reason.
    Disconnected,
    /// The requested environment id is not configured in app-server.
    Unknown,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct EnvironmentShellInfo {
    /// Stable shell name, for example `zsh`, `bash`, `powershell`, `sh`, or `cmd`.
    pub name: String,
    /// Target-native shell executable path or command name.
    pub path: String,
}
