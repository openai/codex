use std::sync::Arc;

use codex_extension_api::FunctionCallError;
use codex_extension_api::JsonToolOutput;
use codex_extension_api::ResponsesApiTool;
use codex_extension_api::ToolCall;
use codex_extension_api::ToolExecutor;
use codex_extension_api::ToolName;
use codex_extension_api::ToolOutput;
use codex_extension_api::ToolSpec;
use codex_extension_api::parse_tool_input_schema;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_mcp::McpResourceClient;
use codex_tools::ResponsesApiNamespace;
use codex_tools::ResponsesApiNamespaceTool;
use codex_tools::default_namespace_description;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

use crate::catalog::SkillAuthority;
use crate::catalog::SkillCatalog;
use crate::catalog::SkillSourceKind;
use crate::provider::SkillListQuery;
use crate::shadow_selection_experiment::ShadowSelectionExperiment;
use crate::sources::SkillProviders;
use crate::state::SkillsThreadState;

mod list;
mod read;
mod schema;

const SKILLS_NAMESPACE: &str = "skills";
const MAX_HANDLE_BYTES: usize = 2_048;

pub(crate) fn skill_tools(
    providers: SkillProviders,
    mcp_resources: Option<Arc<McpResourceClient>>,
    thread_state: Arc<SkillsThreadState>,
    shadow_selection: Arc<ShadowSelectionExperiment>,
) -> Vec<Arc<dyn ToolExecutor<ToolCall>>> {
    let context = SkillToolContext {
        providers,
        mcp_resources,
        thread_state,
        shadow_selection,
    };
    vec![
        Arc::new(list::ListTool {
            context: context.clone(),
        }),
        Arc::new(read::ReadTool { context }),
    ]
}

#[derive(Clone)]
struct SkillToolContext {
    providers: SkillProviders,
    mcp_resources: Option<Arc<McpResourceClient>>,
    thread_state: Arc<SkillsThreadState>,
    shadow_selection: Arc<ShadowSelectionExperiment>,
}

impl SkillToolContext {
    async fn catalog(&self, turn_id: &str, authority: SkillToolAuthority) -> SkillCatalog {
        match authority {
            SkillToolAuthority::Orchestrator => {
                self.thread_state
                    .orchestrator_catalog_snapshot(
                        self.mcp_resources.as_deref(),
                        self.providers.list_orchestrator_for_turn(SkillListQuery {
                            turn_id: turn_id.to_string(),
                            executor_roots: Vec::new(),
                            host_snapshot: None,
                            include_host_skills: false,
                            include_bundled_skills: false,
                            include_orchestrator_skills: true,
                            mcp_resources: self.mcp_resources.clone(),
                            executor_capability_discovery: None,
                        }),
                    )
                    .await
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum SkillToolAuthority {
    Orchestrator,
}

impl SkillToolAuthority {
    fn from_authority(authority: &SkillAuthority) -> Option<Self> {
        if authority
            != &SkillAuthority::new(SkillSourceKind::Orchestrator, CODEX_APPS_MCP_SERVER_NAME)
        {
            return None;
        }
        Some(Self::Orchestrator)
    }

    fn into_authority(self) -> SkillAuthority {
        match self {
            Self::Orchestrator => {
                SkillAuthority::new(SkillSourceKind::Orchestrator, CODEX_APPS_MCP_SERVER_NAME)
            }
        }
    }
}

fn skill_tool_name(name: &str) -> ToolName {
    ToolName::namespaced(SKILLS_NAMESPACE, name)
}

fn skill_function_tool<I: JsonSchema, O: JsonSchema>(name: &str, description: &str) -> ToolSpec {
    let tool = ResponsesApiTool {
        name: name.to_string(),
        description: description.to_string(),
        strict: false,
        defer_loading: None,
        parameters: parse_tool_input_schema(&schema::input_schema_for::<I>())
            .unwrap_or_else(|err| panic!("generated input schema for {name} should parse: {err}")),
        output_schema: Some(schema::output_schema_for::<O>()),
    };

    ToolSpec::Namespace(ResponsesApiNamespace {
        name: SKILLS_NAMESPACE.to_string(),
        description: default_namespace_description(SKILLS_NAMESPACE),
        tools: vec![ResponsesApiNamespaceTool::Function(tool)],
    })
}

fn parse_args<T: for<'de> Deserialize<'de>>(call: &ToolCall) -> Result<T, FunctionCallError> {
    let arguments = call.function_arguments()?;
    let value = if arguments.trim().is_empty() {
        Value::Object(serde_json::Map::new())
    } else {
        serde_json::from_str(arguments)
            .map_err(|err| FunctionCallError::RespondToModel(err.to_string()))?
    };
    serde_json::from_value(value).map_err(|err| FunctionCallError::RespondToModel(err.to_string()))
}

fn validate_handle(name: &str, value: &str, max_bytes: usize) -> Result<(), FunctionCallError> {
    if is_bounded_handle(value, max_bytes) {
        return Ok(());
    }

    Err(FunctionCallError::RespondToModel(format!(
        "{name} must be non-empty, contain no control characters, and be at most {max_bytes} bytes"
    )))
}

fn is_bounded_handle(value: &str, max_bytes: usize) -> bool {
    !value.is_empty() && value.len() <= max_bytes && !value.chars().any(char::is_control)
}

fn external_json_output<T: Serialize>(value: &T) -> Result<Box<dyn ToolOutput>, FunctionCallError> {
    let value = serde_json::to_value(value).map_err(|err| {
        FunctionCallError::Fatal(format!("failed to serialize tool output: {err}"))
    })?;
    Ok(Box::new(JsonToolOutput::new(value).with_external_context()))
}
