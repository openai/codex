use super::*;
use crate::agent::AgentListing;
use crate::agent::agent_resolver::resolve_agent_target;

pub(crate) struct Handler;

impl ToolHandler for Handler {
    type Output = ListAgentsResult;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Function { .. })
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            payload,
            ..
        } = invocation;
        let arguments = function_arguments(payload)?;
        let args: ListAgentsArgs = parse_arguments(&arguments)?;
        let owner_thread_id = if args.all {
            session.conversation_id
        } else if let Some(target) = args
            .id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
        {
            resolve_agent_target(&session, &turn, target).await?
        } else {
            session.conversation_id
        };
        let agents = session
            .services
            .agent_control
            .list_agents_legacy(owner_thread_id, args.recursive, args.all)
            .await
            .map_err(|err| {
                FunctionCallError::RespondToModel(format!("list_agents failed: {err}"))
            })?;
        Ok(ListAgentsResult {
            agents: agents.into_iter().map(ListedAgent::from_listing).collect(),
        })
    }
}

#[derive(Debug, Deserialize)]
struct ListAgentsArgs {
    id: Option<String>,
    #[serde(default = "default_recursive")]
    recursive: bool,
    #[serde(default)]
    all: bool,
}

fn default_recursive() -> bool {
    true
}

#[derive(Debug, Serialize)]
pub(crate) struct ListAgentsResult {
    agents: Vec<ListedAgent>,
}

#[derive(Debug, Serialize)]
struct ListedAgent {
    thread_id: String,
    parent_thread_id: Option<String>,
    status: AgentStatus,
    depth: usize,
}

impl ListedAgent {
    fn from_listing(value: AgentListing) -> Self {
        Self {
            thread_id: value.thread_id.to_string(),
            parent_thread_id: value
                .parent_thread_id
                .map(|thread_id| thread_id.to_string()),
            status: value.status,
            depth: value.depth,
        }
    }
}

impl ToolOutput for ListAgentsResult {
    fn log_preview(&self) -> String {
        tool_output_json_text(self, "list_agents")
    }

    fn success_for_logging(&self) -> bool {
        true
    }

    fn to_response_item(&self, call_id: &str, payload: &ToolPayload) -> ResponseInputItem {
        tool_output_response_item(call_id, payload, self, Some(true), "list_agents")
    }

    fn code_mode_result(&self, _payload: &ToolPayload) -> JsonValue {
        tool_output_code_mode_result(self, "list_agents")
    }
}
