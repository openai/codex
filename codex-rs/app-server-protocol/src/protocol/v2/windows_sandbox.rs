use codex_utils_absolute_path::AbsolutePathBuf;
use schemars::JsonSchema;
#[cfg(any(test, feature = "serde-compat"))]
use serde::Deserialize;
#[cfg(any(test, feature = "serde-compat"))]
use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct WindowsWorldWritableWarningNotification {
    pub sample_paths: Vec<String>,
    pub extra_count: usize,
    pub failed_scan: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub enum WindowsSandboxSetupMode {
    Elevated,
    Unelevated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub enum WindowsSandboxReadiness {
    Ready,
    NotConfigured,
    UpdateRequired,
}

#[derive(Debug, Clone, PartialEq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct WindowsSandboxSetupStartParams {
    pub mode: WindowsSandboxSetupMode,
    #[ts(optional = nullable)]
    pub cwd: Option<AbsolutePathBuf>,
}

#[derive(Debug, Clone, PartialEq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct WindowsSandboxSetupStartResponse {
    pub started: bool,
}

#[derive(Debug, Clone, PartialEq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct WindowsSandboxReadinessResponse {
    pub status: WindowsSandboxReadiness,
}

#[derive(Debug, Clone, PartialEq, JsonSchema, TS)]
#[cfg_attr(any(test, feature = "serde-compat"), derive(Serialize, Deserialize))]
#[cfg_attr(any(test, feature = "serde-compat"), serde(rename_all = "camelCase"))]
#[ts(export_to = "v2/")]
pub struct WindowsSandboxSetupCompletedNotification {
    pub mode: WindowsSandboxSetupMode,
    pub success: bool,
    pub error: Option<String>,
}
