use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::Arc;

use codex_analytics::AnalyticsEventsClient;
use codex_analytics::InvocationType;
use codex_analytics::build_track_events_context;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionMetrics;
use codex_extension_api::FunctionCallError;
use codex_extension_api::JsonToolOutput;
use codex_extension_api::ResponsesApiTool;
use codex_extension_api::ThreadOriginator;
use codex_extension_api::ToolCall;
use codex_extension_api::ToolExecutor;
use codex_extension_api::ToolName;
use codex_extension_api::ToolOutput;
use codex_extension_api::ToolSpec;
use codex_extension_api::parse_tool_input_schema;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_mcp::McpResourceClient;
use codex_otel::SessionTelemetry;
use codex_otel::SkillInvocationEvent;
use codex_otel::SkillInvocationType;
use codex_otel::sanitize_metric_tag_value;
use codex_tools::ResponsesApiNamespace;
use codex_tools::ResponsesApiNamespaceTool;
use codex_tools::default_namespace_description;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

use crate::aliases::SkillLocator;
use crate::catalog::SkillAuthority;
use crate::catalog::SkillCatalogEntry;
use crate::catalog::SkillSourceKind;
use crate::catalog::SkillSourceLocation;
use crate::sources::SkillProviders;
use crate::state::SkillsSessionState;
use crate::state::SkillsThreadState;
use crate::step_state::SkillToolAuthoritySelector;
use crate::step_state::SkillsStepState;
use crate::telemetry::ActiveSkillTurnMetrics;
use crate::telemetry::SkillTurnMetrics;

mod installations;
mod list;
mod read;
mod schema;

const SKILLS_NAMESPACE: &str = "skills";
const MAX_HANDLE_BYTES: usize = 2_048;
const MAX_SKILL_RESPONSE_BYTES: usize = 512 * 1024;

pub(crate) fn skill_tools(
    providers: SkillProviders,
    session_store: &ExtensionData,
    thread_store: &ExtensionData,
    step_store: &ExtensionData,
) -> Vec<Arc<dyn for<'call> ToolExecutor<ToolCall<'call>>>> {
    let Some(thread_state) = thread_store.get::<SkillsThreadState>() else {
        return Vec::new();
    };
    let Some(step_state) = step_store.get::<SkillsStepState>() else {
        return Vec::new();
    };
    if !step_state.tools_enabled {
        return Vec::new();
    }
    let mcp_resources = session_store
        .get::<SkillsSessionState>()
        .and_then(|state| state.mcp_resources.clone());
    let analytics = SkillAnalytics::from_stores(session_store, thread_store);
    let context = SkillToolContext {
        providers,
        mcp_resources,
        thread_state,
        analytics,
        step_state,
    };
    let mut tools: Vec<Arc<dyn for<'call> ToolExecutor<ToolCall<'call>>>> = vec![
        Arc::new(list::ListTool {
            context: context.clone(),
        }),
        Arc::new(read::ReadTool {
            context: context.clone(),
        }),
    ];
    if context.step_state.discovery_v2 {
        tools.push(Arc::new(installations::InstallationsTool { context }));
    }
    tools
}

#[derive(Clone)]
pub(crate) struct SkillAnalytics {
    client: AnalyticsEventsClient,
    telemetry: Option<Arc<SessionTelemetry>>,
    metrics: Option<Arc<dyn ExtensionMetrics>>,
    turn_metrics: Option<Arc<SkillTurnMetrics>>,
    thread_id: String,
    product_client_id: String,
}

impl SkillAnalytics {
    pub(crate) fn from_stores(
        session_store: &ExtensionData,
        thread_store: &ExtensionData,
    ) -> Option<Self> {
        let client = session_store.get::<AnalyticsEventsClient>()?;
        let originator = thread_store.get::<ThreadOriginator>()?;

        Some(Self {
            client: client.as_ref().clone(),
            telemetry: session_store.get::<SessionTelemetry>(),
            metrics: session_store
                .get::<SkillsSessionState>()
                .and_then(|state| state.extension_metrics.clone()),
            // Code-mode callbacks retain these tools after another turn becomes active.
            turn_metrics: thread_store
                .get_or_init(ActiveSkillTurnMetrics::default)
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .upgrade(),
            thread_id: thread_store.level_id().to_string(),
            product_client_id: originator.0.clone(),
        })
    }

    pub(crate) fn track_skill_invocation(
        &self,
        skill: &SkillCatalogEntry,
        source: &SkillSourceLocation,
        model: String,
        turn_id: String,
        invocation_type: InvocationType,
    ) {
        let invocation = skill.invocation(source, invocation_type);
        if let Some(telemetry) = &self.telemetry {
            telemetry
                .as_ref()
                .clone()
                .with_model(&model, &model)
                .skill_invocation(SkillInvocationEvent {
                    turn_id: &turn_id,
                    skill_name: &skill.name,
                    scope: skill.analytics_scope,
                    plugin_id: invocation.plugin_id.as_deref(),
                    invocation_type: match invocation_type {
                        InvocationType::Explicit => SkillInvocationType::Explicit,
                        InvocationType::Implicit => SkillInvocationType::Implicit,
                    },
                });
        }
        let turn_metrics = self
            .turn_metrics
            .as_ref()
            .filter(|turn| turn.turn_id == turn_id);
        if let Some(turn_metrics) = &turn_metrics {
            turn_metrics.record_plugin(invocation.plugin_id.as_deref());
        }
        if let Some(metrics) = &self.metrics {
            let skill_name_tag = sanitize_metric_tag_value(skill.name.as_str());
            let plugin_id_tag = sanitize_metric_tag_value(
                invocation.plugin_id.as_deref().unwrap_or("unattributed"),
            );
            let model_slug_tag = sanitize_metric_tag_value(model.as_str());
            let reasoning_effort = turn_metrics
                .as_ref()
                .map(|turn| turn.reasoning_effort.as_str())
                .unwrap_or("unknown");
            let invoke_type = match invocation_type {
                InvocationType::Explicit => "explicit",
                InvocationType::Implicit => "implicit",
            };
            metrics.counter(
                "codex.skill.injected",
                /*inc*/ 1,
                &[
                    ("status", "ok"),
                    ("skill", skill_name_tag.as_str()),
                    ("invoke_type", invoke_type),
                    ("plugin_id", plugin_id_tag.as_str()),
                    ("model_slug", model_slug_tag.as_str()),
                    ("reasoning_effort", reasoning_effort),
                ],
            );
        }
        self.client.track_skill_invocations(
            build_track_events_context(
                model,
                self.thread_id.clone(),
                turn_id,
                self.product_client_id.clone(),
                turn_metrics.and_then(|turn| turn.turn_metadata.clone()),
            ),
            vec![invocation],
        );
    }
}

#[derive(Clone)]
struct SkillToolContext {
    providers: SkillProviders,
    mcp_resources: Option<Arc<McpResourceClient>>,
    thread_state: Arc<SkillsThreadState>,
    analytics: Option<SkillAnalytics>,
    step_state: Arc<SkillsStepState>,
}

impl SkillToolContext {
    fn matching_packages<'a>(
        &'a self,
        package: &SkillLocator,
    ) -> Vec<(&'a SkillCatalogEntry, SkillSourceLocation)> {
        let catalog = &self.step_state.catalog.merged;
        let matching_entries = |resolved_package: &str, alias_source: Option<&SkillSourceKind>| {
            catalog
                .entries
                .iter()
                .filter_map(|entry| {
                    entry.source_locations().find_map(|source| {
                        if source.package.0 != resolved_package
                            || alias_source.is_some_and(|kind| *kind != source.authority.kind)
                        {
                            return None;
                        }
                        SkillToolAuthority::from_authority(&source.authority)?;
                        Some((entry, source))
                    })
                })
                .collect::<Vec<_>>()
        };
        let exact = matching_entries(package.as_str(), /*alias_source*/ None);
        if !exact.is_empty() || package.alias_source().is_none() {
            return exact;
        }
        let resolved_package = self.step_state.catalog.aliases.resolve(package);
        resolved_package.map_or_else(Vec::new, |resolved| {
            matching_entries(&resolved, package.alias_source())
        })
    }
}

#[derive(Deserialize, Hash, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum InstallationSource {
    Cloud,
    Executor { environment_id: String },
}

impl InstallationSource {
    fn validate(&self) -> Result<(), FunctionCallError> {
        if let Self::Executor { environment_id } = self {
            validate_handle("environment_id", environment_id, MAX_HANDLE_BYTES)?;
        }
        Ok(())
    }

    fn matches(&self, source: &SkillSourceLocation) -> bool {
        source.enabled
            && match self {
                Self::Cloud => source.authority.kind == SkillSourceKind::Cloud,
                Self::Executor { environment_id } => {
                    source.authority.kind == SkillSourceKind::Executor
                        && source
                            .main_prompt
                            .environment_path()
                            .is_some_and(|(id, _)| id == environment_id)
                }
            }
    }
}

#[derive(Clone, Debug, Eq, Hash, JsonSchema, PartialEq, Serialize)]
#[serde(untagged)]
#[schemars(deny_unknown_fields)]
enum InstallationLocation {
    Cloud {
        package: String,
        resource: String,
    },
    Executor {
        environment_id: String,
        skill_root: String,
    },
}

impl InstallationLocation {
    fn from_source(source: &SkillSourceLocation) -> Option<Self> {
        if !source.enabled {
            return None;
        }
        match &source.authority.kind {
            SkillSourceKind::Cloud => Some(Self::Cloud {
                package: source.package.0.clone(),
                resource: source.main_prompt.as_str().to_string(),
            }),
            SkillSourceKind::Executor => {
                let (environment_id, path) = source.main_prompt.environment_path()?;
                Some(Self::Executor {
                    environment_id: environment_id.to_string(),
                    skill_root: path.parent()?.inferred_native_path_string(),
                })
            }
            SkillSourceKind::Host | SkillSourceKind::Custom(_) => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum SkillToolAuthority {
    Cloud,
    Executor { id: String },
}

impl SkillToolAuthority {
    fn selector(&self) -> SkillToolAuthoritySelector {
        match self {
            Self::Cloud => SkillToolAuthoritySelector::Cloud,
            Self::Executor { .. } => SkillToolAuthoritySelector::Executor,
        }
    }

    pub(crate) fn from_authority(authority: &SkillAuthority) -> Option<Self> {
        match &authority.kind {
            SkillSourceKind::Cloud if authority.id == CODEX_APPS_MCP_SERVER_NAME => {
                Some(Self::Cloud)
            }
            SkillSourceKind::Executor => Some(Self::Executor {
                id: authority.id.clone(),
            }),
            SkillSourceKind::Host | SkillSourceKind::Cloud | SkillSourceKind::Custom(_) => None,
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
        output_schema: Some(schema::output_schema_for::<O>().into()),
    };

    ToolSpec::Namespace(ResponsesApiNamespace {
        name: SKILLS_NAMESPACE.to_string(),
        description: default_namespace_description(SKILLS_NAMESPACE),
        tools: vec![ResponsesApiNamespaceTool::Function(tool)],
    })
}

fn parse_args<T: for<'de> Deserialize<'de>>(call: &ToolCall<'_>) -> Result<T, FunctionCallError> {
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

fn pagination_cursor(value: &(impl Hash + ?Sized), offset: usize) -> String {
    format!("{:016x}:{offset}", value_fingerprint(value))
}

fn parse_pagination_cursor(
    cursor: Option<&str>,
    value: &(impl Hash + ?Sized),
    tool: &str,
) -> Result<usize, FunctionCallError> {
    let Some(cursor) = cursor else {
        return Ok(0);
    };
    let invalid = || FunctionCallError::RespondToModel(format!("{tool} cursor is invalid"));
    let (fingerprint, offset) = cursor.split_once(':').ok_or_else(invalid)?;
    if u64::from_str_radix(fingerprint, 16).ok() != Some(value_fingerprint(value)) {
        return Err(FunctionCallError::RespondToModel(format!(
            "{tool} cursor is stale; restart from the first page"
        )));
    }
    offset.parse::<usize>().map_err(|_| invalid())
}

fn value_fingerprint(value: &(impl Hash + ?Sized)) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn serialized_len(value: &impl Serialize) -> Result<usize, FunctionCallError> {
    serde_json::to_vec(value)
        .map(|value| value.len())
        .map_err(|err| FunctionCallError::Fatal(err.to_string()))
}

fn skill_json_output<T: Serialize>(
    value: &T,
    authority: SkillToolAuthoritySelector,
) -> Result<Box<dyn ToolOutput>, FunctionCallError> {
    let value = serde_json::to_value(value).map_err(|err| {
        FunctionCallError::Fatal(format!("failed to serialize tool output: {err}"))
    })?;
    let output = JsonToolOutput::new(value);
    Ok(match authority {
        SkillToolAuthoritySelector::Cloud => Box::new(output.with_external_context()),
        SkillToolAuthoritySelector::Executor => Box::new(output),
    })
}
