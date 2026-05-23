use super::ReviewTarget;
use super::Turn;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ReviewStoryStartParams {
    pub thread_id: String,
    pub target: ReviewTarget,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ReviewStoryStartResponse {
    pub turn: Turn,
    pub story_snapshot_id: String,
    pub snapshot: ReviewStorySnapshot,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ReviewStoryReadParams {
    pub thread_id: String,
    pub story_snapshot_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ReviewStoryReadResponse {
    pub snapshot: Option<ReviewStorySnapshot>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ReviewStoryListParams {
    pub thread_id: String,

    #[ts(optional = nullable)]
    pub cursor: Option<String>,

    #[ts(optional = nullable)]
    pub limit: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ReviewStoryListResponse {
    pub data: Vec<ReviewStorySnapshotSummary>,
    pub next_cursor: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ReviewStorySnapshotUpdatedNotification {
    pub thread_id: String,
    pub snapshot: ReviewStorySnapshot,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ReviewStorySnapshotSummary {
    pub story_snapshot_id: String,
    pub thread_id: String,
    pub title: String,
    pub target: ReviewTarget,
    pub source_fingerprint: String,
    pub status: ReviewStorySnapshotStatus,
    pub created_at: i64,
    pub updated_at: i64,
    pub previous_story_snapshot_id: Option<String>,
    pub step_count: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ReviewStorySnapshot {
    pub story_snapshot_id: String,
    pub thread_id: String,
    pub title: String,
    pub overview: String,
    pub target: ReviewTarget,
    pub source_fingerprint: String,
    pub status: ReviewStorySnapshotStatus,
    pub created_at: i64,
    pub updated_at: i64,
    pub previous_story_snapshot_id: Option<String>,
    pub stale: bool,
    pub steps: Vec<ReviewStoryStep>,
    pub anchors: Vec<ReviewStoryAnchor>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum ReviewStorySnapshotStatus {
    Building,
    Ready,
    Partial,
    Failed,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ReviewStoryStep {
    pub step_id: String,
    pub index: u32,
    pub title: String,
    pub goal: String,
    pub summary: String,
    pub dependency_rationale: String,
    pub anchor_ids: Vec<String>,
    pub review_focus: Vec<String>,
    pub readiness: ReviewStoryStepReadiness,
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum ReviewStoryStepReadiness {
    Outline,
    Enriching,
    Ready,
    Failed,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct ReviewStoryAnchor {
    pub anchor_id: String,
    pub file_path: String,
    pub change_kind: ReviewStoryAnchorKind,
    pub summary: String,
    pub diff: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub enum ReviewStoryAnchorKind {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    Unknown,
}
