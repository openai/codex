use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Instant;

use crate::context::NodeReplReviewEvidence;
use crate::context::NodeReplReviewEvidenceMode;
use crate::context::node_repl_review_evidence_mode;
use crate::function_tool::FunctionCallError;
use crate::mcp_tool_call::handle_mcp_tool_call;
use crate::original_image_detail::can_request_original_image_detail;
use crate::session::session::Session;
use crate::tools::context::McpToolOutput;
use crate::tools::context::ToolCallSource;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::context::boxed_tool_output;
use crate::tools::flat_tool_name;
use crate::tools::hook_names::HookToolName;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::PostToolUsePayload;
use crate::tools::registry::PreToolUsePayload;
use crate::tools::registry::ToolExecutor;
use crate::tools::registry::ToolTelemetryTags;
use codex_mcp::ToolInfo;
use codex_protocol::user_input::UserInput;
use codex_tools::ResponsesApiNamespace;
use codex_tools::ResponsesApiNamespaceTool;
use codex_tools::ToolName;
use codex_tools::ToolSearchInfo;
use codex_tools::ToolSearchSourceInfo;
use codex_tools::ToolSpec;
use codex_tools::agent_plugin_mcp_tool_to_responses_api_tool;
use codex_tools::mcp_tool_to_responses_api_tool;
use codex_utils_image::PromptImageMode;
use codex_utils_image::load_data_url_for_prompt_uncached;
use codex_utils_string::take_bytes_at_char_boundary;
use futures::future::BoxFuture;
use serde_json::Map;
use serde_json::Value;

const LEGACY_MCP_TOOL_NAME_PREFIX: &str = "mcp__";
const MCP_TOOL_NAME_DELIMITER: &str = "__";
const MAX_AGENT_PLUGIN_MCP_NAMESPACE_DESCRIPTION_BYTES: usize = 1_000;
const MAX_MCP_NAMESPACE_DESCRIPTION_BYTES: usize = 512 * 1024;

pub struct McpHandler {
    tool_info: ToolInfo,
    spec: Arc<ToolSpec>,
    code_mode_tool_definitions: OnceLock<Vec<codex_code_mode::ToolDefinition>>,
}

impl McpHandler {
    pub fn new(tool_info: ToolInfo) -> Result<Self, serde_json::Error> {
        Self::with_agent_plugin(tool_info, /*agent_plugin*/ false)
    }

    pub fn new_agent_plugin(tool_info: ToolInfo) -> Result<Self, serde_json::Error> {
        Self::with_agent_plugin(tool_info, /*agent_plugin*/ true)
    }

    fn with_agent_plugin(
        mut tool_info: ToolInfo,
        agent_plugin: bool,
    ) -> Result<Self, serde_json::Error> {
        if agent_plugin {
            tool_info.namespace_description =
                tool_info
                    .namespace_description
                    .as_deref()
                    .map(|description| {
                        take_bytes_at_char_boundary(
                            description,
                            MAX_AGENT_PLUGIN_MCP_NAMESPACE_DESCRIPTION_BYTES,
                        )
                        .to_string()
                    });
        }
        let spec = Arc::new(create_tool_spec(&tool_info, agent_plugin)?);
        Ok(Self {
            tool_info,
            spec,
            code_mode_tool_definitions: OnceLock::new(),
        })
    }

    pub(crate) fn model_spec_bytes(&self) -> Result<usize, serde_json::Error> {
        serde_json::to_vec(&self.spec).map(|spec| spec.len())
    }

    fn hook_tool_name(&self) -> HookToolName {
        HookToolName::new(ensure_mcp_prefix(&join_tool_name(&self.tool_name())))
    }
}

fn join_tool_name(tool_name: &ToolName) -> String {
    match tool_name.namespace.as_deref() {
        Some(namespace) => {
            let namespace = namespace.trim_end_matches('_');
            let name = tool_name.name.trim_start_matches('_');
            format!("{namespace}{MCP_TOOL_NAME_DELIMITER}{name}")
        }
        None => tool_name.name.clone(),
    }
}

fn ensure_mcp_prefix(name: &str) -> String {
    if name.starts_with(LEGACY_MCP_TOOL_NAME_PREFIX) {
        name.to_string()
    } else {
        format!("{LEGACY_MCP_TOOL_NAME_PREFIX}{name}")
    }
}

impl ToolExecutor<ToolInvocation> for McpHandler {
    fn tool_name(&self) -> ToolName {
        self.tool_info.canonical_tool_name()
    }

    fn spec(&self) -> ToolSpec {
        self.spec.as_ref().clone()
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        // Correctly implemented MCP servers should tolerate parallel calls to
        // tools that advertise themselves as read-only.
        self.tool_info.supports_parallel_tool_calls
            || self
                .tool_info
                .tool
                .annotations
                .as_ref()
                .and_then(|annotations| annotations.read_only_hint)
                .unwrap_or(false)
    }

    fn search_info(&self) -> Option<ToolSearchInfo> {
        let source_name = self
            .tool_info
            .connector_name
            .as_deref()
            .map(str::trim)
            .filter(|connector_name| !connector_name.is_empty())
            .unwrap_or_else(|| self.tool_info.server_name.trim());
        let source_info = (!source_name.is_empty()).then(|| ToolSearchSourceInfo {
            name: source_name.to_string(),
            description: self
                .tool_info
                .namespace_description
                .as_deref()
                .map(str::trim)
                .filter(|description| !description.is_empty())
                .map(str::to_string),
        });

        ToolSearchInfo::from_spec(
            build_mcp_search_text(&self.tool_info),
            self.spec(),
            source_info,
        )
    }

    fn handle(&self, invocation: ToolInvocation) -> codex_tools::ToolExecutorFuture<'_> {
        Box::pin(self.handle_call(invocation))
    }
}

impl McpHandler {
    async fn handle_call(
        &self,
        invocation: ToolInvocation,
    ) -> Result<Box<dyn crate::tools::context::ToolOutput>, FunctionCallError> {
        let ToolInvocation {
            session,
            step_context,
            cancellation_token,
            call_id,
            tool_name,
            payload,
            ..
        } = invocation;
        let turn = Arc::clone(&step_context.turn);

        let payload = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "mcp handler received unsupported payload".to_string(),
                ));
            }
        };

        let started = Instant::now();
        let result = handle_mcp_tool_call(
            Arc::clone(&session),
            &step_context,
            &cancellation_token,
            call_id.clone(),
            &self.tool_info,
            self.hook_tool_name(),
            tool_name,
            payload,
        )
        .await;

        Ok(boxed_tool_output(McpToolOutput {
            result: result.result,
            tool_input: result.tool_input,
            wall_time: started.elapsed(),
            original_image_detail_supported: can_request_original_image_detail(&turn.model_info),
            truncation_policy: turn.model_info.truncation_policy.into(),
        }))
    }
}

impl CoreToolRuntime for McpHandler {
    fn immutable_spec(&self) -> Option<&Arc<ToolSpec>> {
        Some(&self.spec)
    }

    fn cached_code_mode_definitions(&self) -> Option<&[codex_code_mode::ToolDefinition]> {
        Some(
            self.code_mode_tool_definitions
                .get_or_init(|| {
                    let mut definitions = codex_tools::collect_code_mode_tool_definitions(
                        std::iter::once(self.spec.as_ref()),
                    );
                    for definition in &mut definitions {
                        definition.input_schema = None;
                        definition.output_schema = None;
                    }
                    definitions
                })
                .as_slice(),
        )
    }

    fn wait_until_ready<'a>(&'a self, session: &'a Arc<Session>) -> Option<BoxFuture<'a, ()>> {
        Some(Box::pin(async move {
            session
                .wait_for_mcp_server(&self.tool_info.server_name)
                .await;
        }))
    }

    fn mcp_server_name(&self) -> Option<&str> {
        Some(&self.tool_info.server_name)
    }

    fn on_tool_result_accepted(&self, invocation: &ToolInvocation, result: &dyn ToolOutput) {
        let ToolCallSource::CodeMode { cell_id, .. } = &invocation.source else {
            return;
        };
        let evidence_mode = node_repl_review_evidence_mode(&invocation.turn);
        let image_capture_enabled = invocation
            .session
            .services
            .thread_extension_data
            .get::<NodeReplReviewEvidence>()
            .is_some_and(|evidence| evidence.image_capture_enabled());
        if self.tool_info.server_name != "node_repl"
            || !result.success_for_logging()
            || evidence_mode == NodeReplReviewEvidenceMode::Disabled && !image_capture_enabled
        {
            return;
        }

        let result = result.code_mode_result(&invocation.payload);
        let Some(content) = result.get("content").and_then(Value::as_array) else {
            return;
        };
        let is_encrypted = |item: &Value| {
            item.get("_meta")
                .and_then(|meta| meta.get("codex/encryptedContent"))
                .and_then(Value::as_bool)
                == Some(true)
        };
        let mut captured_image_bytes = 0_usize;
        let mut items = content
            .iter()
            .filter_map(|item| {
                if is_encrypted(item) {
                    return None;
                }
                match item.get("type").and_then(Value::as_str) {
                    Some("text") => item
                        .get("text")
                        .and_then(Value::as_str)
                        .filter(|text| !text.trim().is_empty())
                        .map(|text| UserInput::Text {
                            text: text.to_string(),
                            text_elements: Vec::new(),
                        }),
                    Some("image")
                        if evidence_mode == NodeReplReviewEvidenceMode::Multimodal
                            || image_capture_enabled =>
                    {
                        let payload = item.get("data").and_then(Value::as_str)?;
                        let mime_type = item.get("mimeType").and_then(Value::as_str)?;
                        if payload.is_empty()
                            || !mime_type
                                .get(..6)
                                .is_some_and(|prefix| prefix.eq_ignore_ascii_case("image/"))
                        {
                            return None;
                        }
                        let image_bytes = "data:;base64,"
                            .len()
                            .saturating_add(mime_type.len())
                            .saturating_add(payload.len());
                        let next_image_bytes = captured_image_bytes.saturating_add(image_bytes);
                        if next_image_bytes > NodeReplReviewEvidence::MAX_RETAINED_BYTES {
                            return None;
                        }
                        let detail = item
                            .get("_meta")
                            .and_then(|meta| meta.get("codex/imageDetail"))
                            .and_then(|detail| serde_json::from_value(detail.clone()).ok());
                        let image_url =
                            format!("data:{};base64,{payload}", mime_type.to_ascii_lowercase());
                        load_data_url_for_prompt_uncached(&image_url, PromptImageMode::Original)
                            .ok()?;
                        captured_image_bytes = next_image_bytes;
                        Some(UserInput::Image { image_url, detail })
                    }
                    _ => None,
                }
            })
            .collect::<Vec<_>>();
        if !items
            .iter()
            .any(|item| matches!(item, UserInput::Text { .. }))
            && !content.iter().any(is_encrypted)
            && let Some(content) = result.get("structuredContent")
            && !content.is_null()
            && let Ok(text) = serde_json::to_string(content)
        {
            items.insert(
                /*index*/ 0,
                UserInput::Text {
                    text,
                    text_elements: Vec::new(),
                },
            );
        }
        invocation
            .session
            .services
            .thread_extension_data
            .get_or_init(NodeReplReviewEvidence::default)
            .record(
                self.tool_info.tool.name.as_ref(),
                cell_id,
                &invocation.call_id,
                items,
            );
    }

    fn telemetry_tags(&self, _invocation: &ToolInvocation) -> ToolTelemetryTags {
        let mut tags = vec![("mcp_server", self.tool_info.server_name.clone())];
        if let Some(origin) = &self.tool_info.server_origin {
            tags.push(("mcp_server_origin", origin.clone()));
        }
        tags
    }

    fn pre_tool_use_payload(&self, invocation: &ToolInvocation) -> Option<PreToolUsePayload> {
        let ToolPayload::Function { arguments } = &invocation.payload else {
            return None;
        };

        Some(PreToolUsePayload {
            tool_name: self.hook_tool_name(),
            tool_input: mcp_hook_tool_input(arguments),
        })
    }

    fn with_updated_hook_input(
        &self,
        mut invocation: ToolInvocation,
        updated_input: Value,
    ) -> Result<ToolInvocation, FunctionCallError> {
        invocation.payload = match invocation.payload {
            ToolPayload::Function { .. } => ToolPayload::Function {
                arguments: serde_json::to_string(&updated_input).map_err(|err| {
                    FunctionCallError::RespondToModel(format!(
                        "failed to serialize rewritten MCP arguments: {err}"
                    ))
                })?,
            },
            payload => {
                return Err(FunctionCallError::RespondToModel(format!(
                    "tool {} does not support hook input rewriting for payload {payload:?}",
                    self.tool_name()
                )));
            }
        };
        Ok(invocation)
    }
    fn post_tool_use_payload(
        &self,
        invocation: &ToolInvocation,
        result: &dyn crate::tools::context::ToolOutput,
    ) -> Option<PostToolUsePayload> {
        let ToolPayload::Function { .. } = &invocation.payload else {
            return None;
        };

        let tool_response =
            result.post_tool_use_response(&invocation.call_id, &invocation.payload)?;
        Some(PostToolUsePayload {
            tool_name: self.hook_tool_name(),
            tool_use_id: invocation.call_id.clone(),
            tool_input: result.post_tool_use_input(&invocation.payload)?,
            tool_response,
        })
    }
}

fn create_tool_spec(
    tool_info: &ToolInfo,
    agent_plugin: bool,
) -> Result<ToolSpec, serde_json::Error> {
    let tool_name = tool_info.canonical_tool_name();
    let tool = if agent_plugin {
        agent_plugin_mcp_tool_to_responses_api_tool(&tool_name, &tool_info.tool)?
    } else {
        mcp_tool_to_responses_api_tool(&tool_name, &tool_info.tool)?
    };
    let description = tool_info
        .namespace_description
        .as_deref()
        .map(str::trim)
        .filter(|description| !description.is_empty())
        .map(str::to_string)
        .or_else(|| {
            tool_info
                .connector_name
                .as_deref()
                .map(str::trim)
                .filter(|connector_name| !connector_name.is_empty())
                .map(|connector_name| format!("Tools for working with {connector_name}."))
        })
        .unwrap_or_default();

    Ok(ToolSpec::Namespace(ResponsesApiNamespace {
        name: tool_info.callable_namespace.clone(),
        description: take_bytes_at_char_boundary(&description, MAX_MCP_NAMESPACE_DESCRIPTION_BYTES)
            .to_string(),
        tools: vec![ResponsesApiNamespaceTool::Function(tool)],
    }))
}

fn mcp_hook_tool_input(raw_arguments: &str) -> Value {
    if raw_arguments.trim().is_empty() {
        return Value::Object(Map::new());
    }

    serde_json::from_str(raw_arguments).unwrap_or_else(|_| Value::String(raw_arguments.to_string()))
}

fn build_mcp_search_text(info: &ToolInfo) -> String {
    let tool_name = info.canonical_tool_name();
    let mut schema_properties = info
        .tool
        .input_schema
        .get("properties")
        .and_then(serde_json::Value::as_object)
        .map(|map| map.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    schema_properties.sort();
    let mut parts = vec![
        flat_tool_name(&tool_name).into_owned(),
        info.callable_name.clone(),
        info.tool.name.to_string(),
        info.server_name.clone(),
    ];
    if let Some(title) = info.tool.title.as_deref().map(str::trim)
        && !title.is_empty()
    {
        parts.push(title.to_string());
    }
    if let Some(description) = info.tool.description.as_deref().map(str::trim)
        && !description.is_empty()
    {
        parts.push(description.to_string());
    }
    if let Some(connector_name) = info.connector_name.as_deref().map(str::trim)
        && !connector_name.is_empty()
    {
        parts.push(connector_name.to_string());
    }
    if let Some(namespace_description) = info.namespace_description.as_deref().map(str::trim)
        && !namespace_description.is_empty()
    {
        parts.push(namespace_description.to_string());
    }
    parts.extend(
        info.plugin_display_names
            .iter()
            .map(String::as_str)
            .map(str::trim)
            .filter(|display_name| !display_name.is_empty())
            .map(str::to_string),
    );
    parts.extend(schema_properties);
    parts.join(" ")
}

#[cfg(test)]
#[path = "mcp_search_tests.rs"]
mod search_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::step_context::StepContext;
    use crate::session::tests::make_session_and_context;
    use crate::tools::context::ToolCallSource;
    use crate::tools::hook_names::HookToolName;
    use crate::tools::registry::PostToolUsePayload;
    use crate::tools::registry::PreToolUsePayload;
    use crate::turn_diff_tracker::TurnDiffTracker;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::time::Duration;
    use tokio::sync::Mutex;

    #[tokio::test]
    async fn mcp_pre_tool_use_payload_uses_prefixed_tool_name_and_raw_args() {
        let payload = ToolPayload::Function {
            arguments: json!({
                "entities": [{
                    "name": "Ada",
                    "entityType": "person"
                }]
            })
            .to_string(),
        };
        let (session, turn) = make_session_and_context().await;
        let turn = Arc::new(turn);
        let handler = McpHandler::new(tool_info("memory", "memory", "create_entities"))
            .expect("MCP tool spec should build");
        assert_eq!(
            handler.pre_tool_use_payload(&ToolInvocation {
                session: session.into(),
                step_context: StepContext::for_test(Arc::clone(&turn)),
                turn,
                cancellation_token: tokio_util::sync::CancellationToken::new(),
                tracker: Arc::new(Mutex::new(TurnDiffTracker::new())),
                call_id: "call-mcp-pre".to_string(),
                tool_name: codex_tools::ToolName::namespaced("memory", "create_entities"),
                source: ToolCallSource::Direct,
                payload,
            }),
            Some(PreToolUsePayload {
                tool_name: HookToolName::new("mcp__memory__create_entities"),
                tool_input: json!({
                    "entities": [{
                        "name": "Ada",
                        "entityType": "person"
                    }]
                }),
            })
        );
    }

    #[tokio::test]
    async fn mcp_pre_tool_use_payload_keeps_builtin_like_tool_names_namespaced() {
        let payload = ToolPayload::Function {
            arguments: json!({ "message": "hello" }).to_string(),
        };
        let (session, turn) = make_session_and_context().await;
        let turn = Arc::new(turn);
        let handler = McpHandler::new(tool_info("foo", "mcp__foo", "exec_command"))
            .expect("MCP tool spec should build");

        assert_eq!(
            handler.pre_tool_use_payload(&ToolInvocation {
                session: session.into(),
                step_context: StepContext::for_test(Arc::clone(&turn)),
                turn,
                cancellation_token: tokio_util::sync::CancellationToken::new(),
                tracker: Arc::new(Mutex::new(TurnDiffTracker::new())),
                call_id: "call-mcp-pre-builtin-like".to_string(),
                tool_name: codex_tools::ToolName::namespaced("mcp__foo", "exec_command"),
                source: ToolCallSource::Direct,
                payload,
            }),
            Some(PreToolUsePayload {
                tool_name: HookToolName::new("mcp__foo__exec_command"),
                tool_input: json!({ "message": "hello" }),
            })
        );
    }

    #[tokio::test]
    async fn mcp_updated_input_rewrites_builtin_like_tool_names_as_mcp() {
        let payload = ToolPayload::Function {
            arguments: json!({ "message": "hello" }).to_string(),
        };
        let (session, turn) = make_session_and_context().await;
        let turn = Arc::new(turn);
        let handler = McpHandler::new(tool_info("foo", "mcp__foo", "exec_command"))
            .expect("MCP tool spec should build");

        let invocation = handler
            .with_updated_hook_input(
                ToolInvocation {
                    session: session.into(),
                    step_context: StepContext::for_test(Arc::clone(&turn)),
                    turn,
                    cancellation_token: tokio_util::sync::CancellationToken::new(),
                    tracker: Arc::new(Mutex::new(TurnDiffTracker::new())),
                    call_id: "call-mcp-rewrite-builtin-like".to_string(),
                    tool_name: codex_tools::ToolName::namespaced("mcp__foo", "exec_command"),
                    source: ToolCallSource::Direct,
                    payload,
                },
                json!({ "message": "rewritten" }),
            )
            .expect("MCP rewrite should succeed");

        let ToolPayload::Function { arguments } = invocation.payload else {
            panic!("builtin-like MCP tool should stay function-shaped");
        };
        assert_eq!(arguments, json!({ "message": "rewritten" }).to_string());
    }

    #[tokio::test]
    async fn mcp_post_tool_use_payload_uses_prefixed_tool_name_args_and_result() {
        let payload = ToolPayload::Function {
            arguments: json!({ "path": "/tmp/notes.txt" }).to_string(),
        };
        let output = McpToolOutput {
            result: codex_protocol::mcp::CallToolResult {
                content: vec![json!({
                    "type": "text",
                    "text": "notes"
                })],
                structured_content: Some(json!({ "bytes": 5 })),
                is_error: None,
                meta: None,
            },
            tool_input: json!({
                "path": {
                    "file_id": "file_123"
                }
            }),
            wall_time: Duration::from_millis(42),
            original_image_detail_supported: true,
            truncation_policy: codex_utils_output_truncation::TruncationPolicy::Bytes(1024),
        };
        let (session, turn) = make_session_and_context().await;
        let turn = Arc::new(turn);
        let handler = McpHandler::new(tool_info("filesystem", "filesystem", "read_file"))
            .expect("MCP tool spec should build");
        let invocation = ToolInvocation {
            session: session.into(),
            step_context: StepContext::for_test(Arc::clone(&turn)),
            turn,
            cancellation_token: tokio_util::sync::CancellationToken::new(),
            tracker: Arc::new(Mutex::new(TurnDiffTracker::new())),
            call_id: "call-mcp-post".to_string(),
            tool_name: codex_tools::ToolName::namespaced("filesystem", "read_file"),
            source: ToolCallSource::Direct,
            payload,
        };
        assert_eq!(
            handler.post_tool_use_payload(&invocation, &output),
            Some(PostToolUsePayload {
                tool_name: HookToolName::new("mcp__filesystem__read_file"),
                tool_use_id: "call-mcp-post".to_string(),
                tool_input: json!({
                    "path": {
                        "file_id": "file_123"
                    }
                }),
                tool_response: json!({
                    "content": [{
                        "type": "text",
                        "text": "notes"
                    }],
                    "structuredContent": { "bytes": 5 }
                }),
            })
        );
    }

    #[test]
    fn mcp_code_mode_definitions_are_cached_lazily() {
        let handler = McpHandler::new(tool_info("filesystem", "mcp__filesystem", "read_file"))
            .expect("MCP tool spec should build");

        assert!(handler.code_mode_tool_definitions.get().is_none());
        assert!(Arc::ptr_eq(
            handler
                .immutable_spec()
                .expect("MCP spec should be immutable"),
            &handler.spec,
        ));

        let first = handler
            .cached_code_mode_definitions()
            .expect("MCP definitions should be cached");
        assert_eq!(first.len(), 1);
        assert!(first[0].input_schema.is_none());
        assert!(first[0].output_schema.is_none());

        let second = handler
            .cached_code_mode_definitions()
            .expect("MCP definitions should be cached");
        assert!(std::ptr::eq(first, second));
    }

    #[test]
    fn mcp_read_only_hint_supports_parallel_calls_without_server_opt_in() {
        let mut read_only_info = tool_info("foo", "mcp__foo__", "read");
        read_only_info.tool.annotations = Some(rmcp::model::ToolAnnotations::new().read_only(true));

        assert!(
            McpHandler::new(read_only_info)
                .expect("MCP tool spec should build")
                .supports_parallel_tool_calls()
        );
    }

    #[test]
    fn mcp_parallel_calls_require_read_only_hint_or_server_opt_in() {
        let missing_hint_info = tool_info("foo", "mcp__foo__", "unannotated");
        assert!(
            !McpHandler::new(missing_hint_info)
                .expect("MCP tool spec should build")
                .supports_parallel_tool_calls()
        );

        let mut writable_info = tool_info("foo", "mcp__foo__", "write");
        writable_info.tool.annotations = Some(rmcp::model::ToolAnnotations::new().read_only(false));
        assert!(
            !McpHandler::new(writable_info)
                .expect("MCP tool spec should build")
                .supports_parallel_tool_calls()
        );

        let mut server_opt_in_info = tool_info("foo", "mcp__foo__", "server_opt_in");
        server_opt_in_info.supports_parallel_tool_calls = true;
        assert!(
            McpHandler::new(server_opt_in_info)
                .expect("MCP tool spec should build")
                .supports_parallel_tool_calls()
        );
    }

    fn tool_info(server_name: &str, callable_namespace: &str, tool_name: &str) -> ToolInfo {
        ToolInfo {
            server_name: server_name.to_string(),
            supports_parallel_tool_calls: false,
            server_origin: None,
            callable_name: tool_name.to_string(),
            callable_namespace: callable_namespace.to_string(),
            namespace_description: None,
            tool: rmcp::model::Tool::new_with_raw(
                tool_name.to_string(),
                None,
                Arc::new(rmcp::model::object(serde_json::json!({
                    "type": "object",
                }))),
            ),
            openai_file_input_optional_fields: Default::default(),
            connector_id: None,
            connector_name: None,
            plugin_display_names: Vec::new(),
        }
    }
}
