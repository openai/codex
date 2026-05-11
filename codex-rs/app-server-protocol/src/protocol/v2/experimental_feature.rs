use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;
use ts_rs::TS;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ExperimentalFeatureListParams {
    /// Opaque pagination cursor returned by a previous call.
    #[ts(optional = nullable)]
    pub cursor: Option<String>,
    /// Optional page size; defaults to a reasonable server-side value.
    #[ts(optional = nullable)]
    pub limit: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum ExperimentalFeatureStage {
    /// Feature is available for user testing and feedback.
    Beta,
    /// Feature is still being built and not ready for broad use.
    UnderDevelopment,
    /// Feature is production-ready.
    Stable,
    /// Feature is deprecated and should be avoided.
    Deprecated,
    /// Feature flag is retained only for backwards compatibility.
    Removed,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ExperimentalFeature {
    /// Stable key used in config.toml and CLI flag toggles.
    pub name: String,
    /// Lifecycle stage of this feature flag.
    pub stage: ExperimentalFeatureStage,
    /// User-facing display name shown in the experimental features UI.
    /// Null when this feature is not in beta.
    pub display_name: Option<String>,
    /// Short summary describing what the feature does.
    /// Null when this feature is not in beta.
    pub description: Option<String>,
    /// Announcement copy shown to users when the feature is introduced.
    /// Null when this feature is not in beta.
    pub announcement: Option<String>,
    /// Whether this feature is currently enabled in the loaded config.
    pub enabled: bool,
    /// Whether this feature is enabled by default.
    pub default_enabled: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ExperimentalFeatureListResponse {
    pub data: Vec<ExperimentalFeature>,
    /// Opaque cursor to pass to the next call to continue after the last item.
    /// If None, there are no more items to return.
    pub next_cursor: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ExperimentalFeatureEnablementSetParams {
    /// Process-wide runtime feature enablement keyed by canonical feature name.
    ///
    /// Only named features are updated. Omitted features are left unchanged.
    /// Send an empty map for a no-op.
    pub enablement: BTreeMap<String, bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ExperimentalFeatureEnablementSetResponse {
    /// Feature enablement entries updated by this request.
    pub enablement: BTreeMap<String, bool>,
}
