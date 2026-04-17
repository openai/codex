//! Responses API tool definitions for persisted thread goals.
//!
//! These specs expose goal read/update primitives to the model while keeping
//! usage accounting system-managed.

use crate::JsonSchema;
use crate::ResponsesApiTool;
use crate::ToolSpec;
use serde_json::json;
use std::collections::BTreeMap;

pub const GET_GOAL_TOOL_NAME: &str = "get_goal";
pub const SET_GOAL_TOOL_NAME: &str = "set_goal";
pub const UPDATE_GOAL_TOOL_NAME: &str = "update_goal";

pub fn create_get_goal_tool() -> ToolSpec {
    ToolSpec::Function(ResponsesApiTool {
        name: GET_GOAL_TOOL_NAME.to_string(),
        description: "Get the current long-running goal for this thread, including status, budgets, token and elapsed-time usage, and remaining token budget."
            .to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(BTreeMap::new(), Some(Vec::new()), Some(false.into())),
        output_schema: None,
    })
}

pub fn create_set_goal_tool() -> ToolSpec {
    let properties = BTreeMap::from([
        (
            "objective".to_string(),
            JsonSchema::string(Some(
                "Required. The concrete objective to start pursuing. This starts a fresh active goal, replacing any previous goal and resetting usage accounting unless the existing non-terminal goal already has the same objective."
                    .to_string(),
            )),
        ),
        (
            "token_budget".to_string(),
            JsonSchema::integer(Some(
                "Optional positive token budget for the new active goal.".to_string(),
            )),
        ),
    ]);

    ToolSpec::Function(ResponsesApiTool {
        name: SET_GOAL_TOOL_NAME.to_string(),
        description: r#"Start a new long-running goal for this thread.
This tool always creates or replaces an active goal. It resets time/token usage accounting to zero when the objective is new.
If the existing non-terminal goal already has the same objective, the call is treated as an idempotent update that keeps the goal active and preserves usage accounting.
Use update_goal, not set_goal, to pause, resume, mark achieved, mark budget-limited, or change the token budget for an existing goal.
The system owns usage fields, so this tool cannot set them directly."#
            .to_string(),
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
    let nullable_integer = |description: &str| {
        JsonSchema::any_of(
            vec![
                JsonSchema::integer(Some(description.to_string())),
                JsonSchema::null(Some("Clear this budget.".to_string())),
            ],
            Some(description.to_string()),
        )
    };
    let properties = BTreeMap::from([
        (
            "status".to_string(),
            JsonSchema::string_enum(
                vec![
                    json!("active"),
                    json!("paused"),
                    json!("budgetLimited"),
                    json!("complete"),
                ],
                Some(
                    "Optional. Set to active, paused, budgetLimited, or complete. Use complete only when the objective is achieved and no required work remains. Use budgetLimited when the objective has not been achieved and cannot be achieved within the remaining budget, or when the remaining budget is too small for productive continuation."
                        .to_string(),
                ),
            ),
        ),
        (
            "token_budget".to_string(),
            nullable_integer("Optional positive token budget. Use null to clear the existing token budget while preserving usage accounting."),
        ),
    ]);

    ToolSpec::Function(ResponsesApiTool {
        name: UPDATE_GOAL_TOOL_NAME.to_string(),
        description: r#"Update the existing long-running goal while preserving time/token usage accounting.
Use this tool to pause, resume, mark the goal achieved, mark it budget-limited, or change the token budget.
Set status to `complete` only when the objective has actually been achieved and no required work remains.
Set status to `budgetLimited` when a budgeted goal has not been achieved and cannot be achieved within the remaining budget, or when the budget is exhausted or nearly exhausted.
Do not mark a goal complete merely because its budget is nearly exhausted or because you are stopping work.
When marking a budgeted goal achieved with status `complete`, report the final token usage from the tool result to the user.
The system owns usage fields, so this tool cannot set them directly."#
            .to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(properties, /*required*/ None, Some(false.into())),
        output_schema: None,
    })
}
