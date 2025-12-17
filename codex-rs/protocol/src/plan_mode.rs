use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

use crate::plan_tool::UpdatePlanArgs;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlanRequest {
    pub goal: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PlanOutputEvent {
    pub title: String,
    pub summary: String,
    pub plan: UpdatePlanArgs,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ExitedPlanModeEvent {
    pub plan_output: Option<PlanOutputEvent>,
}
