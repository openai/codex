//! Implements the collaboration tool surface for spawning and managing sub-agents.
//!
//! This handler translates model tool calls into `AgentControl` operations and keeps spawned
//! agents aligned with the live turn that created them. Sub-agents start from the turn's effective
//! config, inherit runtime-only state such as provider, approval policy, sandbox, and cwd, and
//! then optionally layer role-specific config on top.

use crate::agent::AgentStatus;
use crate::agent::exceeds_thread_spawn_depth_limit;
use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::context::boxed_tool_output;
pub(crate) use crate::tools::handlers::multi_agents_common::*;
use crate::tools::handlers::multi_agents_spec::MULTI_AGENT_V1_NAMESPACE;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExecutor;
use codex_protocol::ThreadId;
use codex_protocol::items::CollabAgentTool;
use codex_protocol::items::CollabAgentToolCallItem;
use codex_protocol::items::CollabAgentToolCallStatus;
use codex_protocol::items::TurnItem;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::CollabAgentRef;
use codex_protocol::user_input::UserInput;
use codex_tools::ToolName;
use codex_tools::ToolSearchInfo;
use codex_tools::ToolSearchSourceInfo;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;

const MULTI_AGENT_TOOL_SEARCH_SOURCE_NAME: &str = "Multi-agent tools";
const MULTI_AGENT_TOOL_SEARCH_SOURCE_DESCRIPTION: &str = "Spawn and manage sub-agents.";

pub(crate) fn parse_agent_id_target(target: &str) -> Result<ThreadId, FunctionCallError> {
    ThreadId::from_string(target).map_err(|err| {
        FunctionCallError::RespondToModel(format!("invalid agent id {target}: {err:?}"))
    })
}

pub(crate) fn parse_agent_id_targets(
    targets: Vec<String>,
) -> Result<Vec<ThreadId>, FunctionCallError> {
    if targets.is_empty() {
        return Err(FunctionCallError::RespondToModel(
            "agent ids must be non-empty".to_string(),
        ));
    }

    targets
        .into_iter()
        .map(|target| parse_agent_id_target(&target))
        .collect()
}

fn multi_agent_tool_search_info(
    search_text: &str,
    spec: codex_tools::ToolSpec,
) -> Option<ToolSearchInfo> {
    ToolSearchInfo::from_spec(
        search_text.to_string(),
        spec,
        Some(ToolSearchSourceInfo {
            name: MULTI_AGENT_TOOL_SEARCH_SOURCE_NAME.to_string(),
            description: Some(MULTI_AGENT_TOOL_SEARCH_SOURCE_DESCRIPTION.to_string()),
        }),
    )
}

pub(crate) use close_agent::Handler as CloseAgentHandler;
pub(crate) use resume_agent::Handler as ResumeAgentHandler;
pub(crate) use send_input::Handler as SendInputHandler;
pub(crate) use spawn::Handler as SpawnAgentHandler;
pub(crate) use wait::Handler as WaitAgentHandler;

pub(crate) mod close_agent;
mod resume_agent;
mod send_input;
mod spawn;
pub(crate) mod wait;

pub(crate) fn collab_tool_call_status(
    status: &AgentStatus,
    receiver_thread_id: Option<ThreadId>,
) -> CollabAgentToolCallStatus {
    match status {
        AgentStatus::Errored(_) | AgentStatus::NotFound => CollabAgentToolCallStatus::Failed,
        _ if receiver_thread_id.is_some() => CollabAgentToolCallStatus::Completed,
        _ => CollabAgentToolCallStatus::Failed,
    }
}

#[cfg(test)]
#[path = "multi_agents_tests.rs"]
mod tests;
