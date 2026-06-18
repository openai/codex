//! Responses API tool definitions for persisted thread goals.

use codex_tools::JsonSchema;
use codex_tools::ResponsesApiTool;
use codex_tools::ToolSpec;
use serde_json::json;
use std::collections::BTreeMap;

pub const GET_GOAL_TOOL_NAME: &str = "get_goal";
pub const CREATE_GOAL_TOOL_NAME: &str = "create_goal";
pub const UPDATE_GOAL_TOOL_NAME: &str = "update_goal";

pub fn create_get_goal_tool() -> ToolSpec {
    ToolSpec::Function(ResponsesApiTool {
        name: GET_GOAL_TOOL_NAME.to_string(),
        description: "Get the current goal for this thread, including status, budgets, token and elapsed-time usage, and remaining token budget."
            .to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(BTreeMap::new(), Some(Vec::new()), Some(false.into())),
        output_schema: None,
    })
}

pub fn create_create_goal_tool() -> ToolSpec {
    let properties = BTreeMap::from([
        (
            "objective".to_string(),
            JsonSchema::string(Some(
                "Required. The concrete objective to start pursuing. This starts a new active goal when no goal exists or replaces the current goal when it is complete."
                    .to_string(),
            )),
        ),
        (
            "token_budget".to_string(),
            JsonSchema::integer(Some(
                "Positive token budget for the new goal. Omit unless explicitly requested."
                    .to_string(),
            )),
        ),
    ]);

    ToolSpec::Function(ResponsesApiTool {
        name: CREATE_GOAL_TOOL_NAME.to_string(),
        description: format!(
            r#"Create a goal only when explicitly requested by the user or system/developer instructions; do not infer goals from ordinary tasks.
Set token_budget only when an explicit token budget is requested. Fails if an unfinished goal exists; use {UPDATE_GOAL_TOOL_NAME} only for status."#
        ),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(
            properties,
            /*required*/ Some(vec!["objective".to_string()]),
            Some(false.into()),
        ),
        output_schema: None,
    })
}

pub fn create_update_goal_tool() -> ToolSpec {
    let properties = BTreeMap::from([(
        "status".to_string(),
        JsonSchema::string_enum(
            vec![json!("complete"), json!("blocked")],
            Some(
                "Required. Set to `complete` only when the objective is achieved and no required work remains. Set to `blocked` only after the same blocking condition has recurred for at least three consecutive goal turns and the agent is at an impasse. After a previously blocked goal is resumed, the resumed run starts a fresh blocked audit."
                    .to_string(),
            ),
        ),
    )]);

    ToolSpec::Function(ResponsesApiTool {
        name: UPDATE_GOAL_TOOL_NAME.to_string(),
        description: r#"Update the existing goal.
Use this tool only to mark the goal achieved or genuinely blocked.
Set status to `complete` only when the objective has actually been achieved and no required work remains.
Set status to `blocked` only when the same blocking condition has repeated for at least three consecutive goal turns, counting the original/user-triggered turn and any automatic continuations, and the agent cannot make meaningful progress without user input or an external-state change.
If the user resumes a goal that was previously marked `blocked`, treat the resumed run as a fresh blocked audit. If the same blocking condition then repeats for at least three consecutive resumed goal turns, set status to `blocked` again.
Once the blocked threshold is satisfied, do not keep reporting that you are still blocked while leaving the goal active; set status to `blocked`.
Do not use `blocked` merely because the work is hard, slow, uncertain, incomplete, or would benefit from clarification.
Do not mark a goal complete merely because its budget is nearly exhausted or because you are stopping work.
You cannot use this tool to pause, resume, budget-limit, or usage-limit a goal; those status changes are controlled by the user or system.
When marking a budgeted goal achieved with status `complete`, report the final token usage from the tool result to the user."#
            .to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(
            properties,
            /*required*/ Some(vec!["status".to_string()]),
            Some(false.into()),
        ),
        output_schema: None,
    })
}
