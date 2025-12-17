use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

use crate::plan_tool::UpdatePlanArgs;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PlanProposal {
    pub title: String,
    pub summary: String,
    pub plan: UpdatePlanArgs,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PlanApprovalRequestEvent {
    pub call_id: String,
    pub proposal: PlanProposal,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(tag = "type", rename_all = "snake_case")]
pub enum PlanApprovalResponse {
    Approved,
    Revised { feedback: String },
    Rejected,
}
