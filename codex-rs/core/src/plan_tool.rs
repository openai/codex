use std::collections::BTreeMap;
use std::sync::LazyLock;

use serde::Deserialize;
use serde::Serialize;

use crate::codex::Session;
use crate::models::FunctionCallOutputPayload;
use crate::models::ResponseInputItem;
use crate::openai_tools::JsonSchema;
use crate::openai_tools::OpenAiTool;
use crate::openai_tools::ResponsesApiTool;
use crate::protocol::Event;
use crate::protocol::EventMsg;

// Types for the TODO tool arguments matching codex-vscode/todo-mcp/src/main.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Pending,
    InProgress,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanItemArg {
    pub step: String,
    pub status: StepStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdatePlanArgs {
    #[serde(default)]
    pub explanation: Option<String>,
    pub plan: Vec<PlanItemArg>,
}

pub(crate) static PLAN_TOOL: LazyLock<OpenAiTool> = LazyLock::new(|| {
    let mut plan_item_props = BTreeMap::new();
    plan_item_props.insert("step".to_string(), JsonSchema::String { description: None });
    plan_item_props.insert(
        "status".to_string(),
        JsonSchema::String { description: None },
    );

    let plan_items_schema = JsonSchema::Array {
        items: Box::new(JsonSchema::Object {
            properties: plan_item_props,
            required: &["step", "status"],
            additional_properties: false,
        }),
    };

    let mut properties = BTreeMap::new();
    properties.insert(
        "explanation".to_string(),
        JsonSchema::String { description: None },
    );
    properties.insert("plan".to_string(), plan_items_schema);

    OpenAiTool::Function(ResponsesApiTool {
        name: "update_plan",
        description: r#"Use the update_plan tool to keep the user updated on the current plan for the task.
After understanding the user's task, call the update_plan tool with an initial plan. An example of a plan:
1. Explore the codebase to find relevant files (status: in_progress)
2. Implement the feature in the XYZ component (status: pending)
3. Commit changes and make a pull request (status: pending)
Each step should be a short, 1-sentence description.
Until all the steps are finished, there should always be exactly one in_progress step in the plan.
Call the update_plan tool whenever you finish a step, marking the completed step as `completed` and marking the next step as `in_progress`.
Before running a command, consider whether or not you have completed the previous step, and make sure to mark it as completed before moving on to the next step.
Sometimes, you may need to change plans in the middle of a task: call `update_plan` with the updated plan and make sure to provide an `explanation` of the rationale when doing so.
When all steps are completed, call update_plan one last time with all steps marked as `completed`."#.to_string(),
        strict: false,
        parameters: JsonSchema::Object {
            properties,
            required: &["plan"],
            additional_properties: false,
        },
    })
});

/// This function doesn't do anything useful. However, it gives the model a structured way to record its plan that clients can read and render.
/// So it's the _inputs_ to this function that are useful to clients, not the outputs and neither are actually useful for the model other
/// than forcing it to come up and document a plan (TBD how that affects performance).
pub(crate) async fn handle_update_plan(
    session: &Session,
    arguments: String,
    sub_id: String,
    call_id: String,
) -> ResponseInputItem {
    match parse_update_plan_arguments(arguments, &call_id) {
        Ok(args) => {
            let output = ResponseInputItem::FunctionCallOutput {
                call_id,
                output: FunctionCallOutputPayload {
                    content: "Plan updated".to_string(),
                    success: Some(true),
                },
            };
            session
                .send_event(Event {
                    id: sub_id.to_string(),
                    msg: EventMsg::PlanUpdate(args),
                })
                .await;
            output
        }
        Err(output) => *output,
    }
}

fn parse_update_plan_arguments(
    arguments: String,
    call_id: &str,
) -> Result<UpdatePlanArgs, Box<ResponseInputItem>> {
    match serde_json::from_str::<UpdatePlanArgs>(&arguments) {
        Ok(args) => Ok(args),
        Err(e) => {
            let output = ResponseInputItem::FunctionCallOutput {
                call_id: call_id.to_string(),
                output: FunctionCallOutputPayload {
                    content: format!("failed to parse function arguments: {e}"),
                    success: None,
                },
            };
            Err(Box::new(output))
        }
    }
}
