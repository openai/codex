use std::borrow::Cow;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Duration;

use rmcp::ErrorData as McpError;
use rmcp::ServiceExt;
use rmcp::handler::server::ServerHandler;
use rmcp::model::CallToolRequestParams;
use rmcp::model::CallToolResult;
use rmcp::model::JsonObject;
use rmcp::model::ListResourceTemplatesResult;
use rmcp::model::ListResourcesResult;
use rmcp::model::ListToolsResult;
use rmcp::model::PaginatedRequestParams;
use rmcp::model::RawResource;
use rmcp::model::RawResourceTemplate;
use rmcp::model::ReadResourceRequestParams;
use rmcp::model::ReadResourceResult;
use rmcp::model::Resource;
use rmcp::model::ResourceContents;
use rmcp::model::ResourceTemplate;
use rmcp::model::ServerCapabilities;
use rmcp::model::ServerInfo;
use rmcp::model::Tool;
use rmcp::model::ToolAnnotations;
use serde::Deserialize;
use serde_json::json;
use tokio::sync::Barrier;
use tokio::task;
use tokio::time::sleep;

#[derive(Clone)]
struct TestToolServer {
    tools: Arc<Vec<Tool>>,
    resources: Arc<Vec<Resource>>,
    resource_templates: Arc<Vec<ResourceTemplate>>,
}

const MEMO_URI: &str = "memo://codex/example-note";
const MEMO_CONTENT: &str = "This is a sample MCP resource served by the rmcp test server.";
const SANDBOX_STATE_META_CAPABILITY: &str = "codex/sandbox-state-meta";
const SMALL_PNG_BASE64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==";

pub fn stdio() -> (tokio::io::Stdin, tokio::io::Stdout) {
    (tokio::io::stdin(), tokio::io::stdout())
}

impl TestToolServer {
    fn new() -> Self {
        #[expect(clippy::expect_used)]
        let sandbox_meta_schema: JsonObject = serde_json::from_value(serde_json::json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        }))
        .expect("sandbox_meta tool schema should deserialize");
        let mut sandbox_meta_tool = Tool::new(
            Cow::Borrowed("sandbox_meta"),
            Cow::Borrowed("Return the MCP request metadata received by this test server."),
            Arc::new(sandbox_meta_schema),
        );
        sandbox_meta_tool.annotations = Some(ToolAnnotations::new().read_only(true));

        let tools = vec![
            Self::echo_tool(),
            Self::echo_dash_tool(),
            Self::sync_tool(),
            Self::image_tool(),
            Self::image_scenario_tool(),
            sandbox_meta_tool,
        ];
        let resources = vec![Self::memo_resource()];
        let resource_templates = vec![Self::memo_template()];
        Self {
            tools: Arc::new(tools),
            resources: Arc::new(resources),
            resource_templates: Arc::new(resource_templates),
        }
    }

    fn echo_tool() -> Tool {
        Self::build_echo_tool(
            "echo",
            "Echo back the provided message and include environment data.",
        )
    }

    fn echo_dash_tool() -> Tool {
        Self::build_echo_tool(
            "echo-tool",
            "Echo back the provided message via a tool name that is not a legal JS identifier.",
        )
    }

    fn build_echo_tool(name: &'static str, description: &'static str) -> Tool {
        #[expect(clippy::expect_used)]
        let schema: JsonObject = serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "message": { "type": "string" },
                "env_var": { "type": "string" }
            },
            "required": ["message"],
            "additionalProperties": false
        }))
        .expect("echo tool schema should deserialize");

        let mut tool = Tool::new(
            Cow::Borrowed(name),
            Cow::Borrowed(description),
            Arc::new(schema),
        );
        #[expect(clippy::expect_used)]
        let output_schema: JsonObject = serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "echo": { "type": "string" },
                "env": {
                    "anyOf": [
                        { "type": "string" },
                        { "type": "null" }
                    ]
                }
            },
            "required": ["echo", "env"],
            "additionalProperties": false
        }))
        .expect("echo tool output schema should deserialize");
        tool.output_schema = Some(Arc::new(output_schema));
        tool.annotations = Some(ToolAnnotations::new().read_only(true));
        tool
    }

    fn sync_tool() -> Tool {
        #[expect(clippy::expect_used)]
        let schema: JsonObject = serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "sleep_before_ms": { "type": "number" },
                "sleep_after_ms": { "type": "number" },
                "barrier": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "string" },
                        "participants": { "type": "number" },
                        "timeout_ms": { "type": "number" }
                    },
                    "required": ["id", "participants"],
                    "additionalProperties": false
                }
            },
            "additionalProperties": false
        }))
        .expect("sync tool schema should deserialize");

        let mut tool = Tool::new(
            Cow::Borrowed("sync"),
            Cow::Borrowed(
                "Synchronize concurrent test calls and optionally delay before or after the barrier.",
            ),
            Arc::new(schema),
        );
        #[expect(clippy::expect_used)]
        let output_schema: JsonObject = serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "result": { "type": "string" }
            },
            "required": ["result"],
            "additionalProperties": false
        }))
        .expect("sync tool output schema should deserialize");
        tool.output_schema = Some(Arc::new(output_schema));
        tool.annotations = Some(ToolAnnotations::new().read_only(true));
        tool
    }

    fn image_tool() -> Tool {
        #[expect(clippy::expect_used)]
        let schema: JsonObject = serde_json::from_value(serde_json::json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        }))
        .expect("image tool schema should deserialize");

        let mut tool = Tool::new(
            Cow::Borrowed("image"),
            Cow::Borrowed("Return a single image content block."),
            Arc::new(schema),
        );
        tool.annotations = Some(ToolAnnotations::new().read_only(true));
        tool
    }

    /// Tool intended for manual testing of Codex TUI rendering for MCP image tool results.
    ///
    /// This exists to exercise edge cases where a `CallToolResult.content` includes image blocks
    /// that aren't the first item (or includes invalid image blocks before a valid image).
    ///
    /// Manual testing approach (Codex TUI):
    /// - Build this binary: `cargo build -p codex-rmcp-client --bin test_stdio_server`
    /// - Register it:
    ///   - `codex mcp add mcpimg -- /abs/path/to/test_stdio_server`
    /// - Then in Codex TUI, ask it to call:
    ///   - `mcpimg.image_scenario({"scenario":"image_only"})`
    ///   - `mcpimg.image_scenario({"scenario":"image_only_original_detail"})`
    ///   - `mcpimg.image_scenario({"scenario":"text_then_image","caption":"Here is the image:"})`
    ///   - `mcpimg.image_scenario({"scenario":"invalid_base64_then_image"})`
    ///   - `mcpimg.image_scenario({"scenario":"invalid_image_bytes_then_image"})`
    ///   - `mcpimg.image_scenario({"scenario":"multiple_valid_images"})`
    ///   - `mcpimg.image_scenario({"scenario":"image_then_text","caption":"Here is the image:"})`
    ///   - `mcpimg.image_scenario({"scenario":"text_only","caption":"Here is the image:"})`
    /// - You should see an extra history cell: `tool result (image output)`.
    fn image_scenario_tool() -> Tool {
        #[expect(clippy::expect_used)]
        let schema: JsonObject = serde_json::from_value(serde_json::json!({
            "type": "object",
            "properties": {
                "scenario": {
                    "type": "string",
                    "enum": [
                        "image_only",
                        "image_only_original_detail",
                        "text_then_image",
                        "invalid_base64_then_image",
                        "invalid_image_bytes_then_image",
                        "multiple_valid_images",
                        "image_then_text",
                        "text_only"
                    ]
                },
                "caption": { "type": "string" },
                "data_url": {
                    "type": "string",
                    "description": "Optional data URL like data:image/png;base64,AAAA...; if omitted, uses a built-in tiny PNG."
                }
            },
            "required": ["scenario"],
            "additionalProperties": false
        }))
        .expect("image_scenario tool schema should deserialize");

        let mut tool = Tool::new(
            Cow::Borrowed("image_scenario"),
            Cow::Borrowed(
                "Return content blocks for manual testing of MCP image rendering scenarios.",
            ),
            Arc::new(schema),
        );
        tool.annotations = Some(ToolAnnotations::new().read_only(true));
        tool
    }

    fn memo_resource() -> Resource {
        let raw = RawResource {
            uri: MEMO_URI.to_string(),
            name: "example-note".to_string(),
            title: Some("Example Note".to_string()),
            description: Some("A sample MCP resource exposed for integration tests.".to_string()),
            mime_type: Some("text/plain".to_string()),
            size: None,
            icons: None,
            meta: None,
        };
        Resource::new(raw, None)
    }

    fn memo_template() -> ResourceTemplate {
        let raw = RawResourceTemplate {
            uri_template: "memo://codex/{slug}".to_string(),
            name: "codex-memo".to_string(),
            title: Some("Codex Memo".to_string()),
            description: Some(
                "Template for memo://codex/{slug} resources used in tests.".to_string(),
            ),
            mime_type: Some("text/plain".to_string()),
            icons: None,
        };
        ResourceTemplate::new(raw, None)
    }

    fn memo_text() -> &'static str {
        MEMO_CONTENT
    }
}

#[derive(Deserialize)]
struct EchoArgs {
    message: String,
    env_var: Option<String>,
}

const DEFAULT_SYNC_TIMEOUT_MS: u64 = 1_000;

static SYNC_BARRIERS: OnceLock<tokio::sync::Mutex<HashMap<String, SyncBarrierState>>> =
    OnceLock::new();

struct SyncBarrierState {
    barrier: Arc<Barrier>,
    participants: usize,
}

#[derive(Debug, Deserialize)]
struct SyncBarrierArgs {
    id: String,
    participants: usize,
    #[serde(default = "default_sync_timeout_ms")]
    timeout_ms: u64,
}

#[derive(Debug, Deserialize)]
struct SyncArgs {
    #[serde(default)]
    sleep_before_ms: Option<u64>,
    #[serde(default)]
    sleep_after_ms: Option<u64>,
    #[serde(default)]
    barrier: Option<SyncBarrierArgs>,
}

fn default_sync_timeout_ms() -> u64 {
    DEFAULT_SYNC_TIMEOUT_MS
}

fn sync_barrier_map() -> &'static tokio::sync::Mutex<HashMap<String, SyncBarrierState>> {
    SYNC_BARRIERS.get_or_init(|| tokio::sync::Mutex::new(HashMap::new()))
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
/// Scenarios for `image_scenario`, intended to exercise Codex TUI handling of MCP image outputs.
///
/// The key behavior under test is that the TUI should render an image output cell if *any*
/// decodable image block exists in the tool result content, even if the first block is text or an
/// invalid image.
enum ImageScenario {
    ImageOnly,
    ImageOnlyOriginalDetail,
    TextThenImage,
    InvalidBase64ThenImage,
    InvalidImageBytesThenImage,
    MultipleValidImages,
    ImageThenText,
    TextOnly,
}

#[derive(Deserialize, Debug)]
struct ImageScenarioArgs {
    scenario: ImageScenario,
    #[serde(default)]
    caption: Option<String>,
    #[serde(default)]
    data_url: Option<String>,
}

impl ServerHandler for TestToolServer {
    fn get_info(&self) -> ServerInfo {
        let mut capabilities = ServerCapabilities::builder()
            .enable_tools()
            .enable_tool_list_changed()
            .enable_resources()
            .build();
        capabilities.experimental = Some(BTreeMap::from([(
            SANDBOX_STATE_META_CAPABILITY.to_string(),
            JsonObject::new(),
        )]));

        ServerInfo {
            capabilities,
            ..ServerInfo::default()
        }
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::service::RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, McpError>> + Send + '_ {
        let tools = self.tools.clone();
        async move {
            Ok(ListToolsResult {
                tools: (*tools).clone(),
                next_cursor: None,
                meta: None,
            })
        }
    }

    fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::service::RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListResourcesResult, McpError>> + Send + '_ {
        let resources = self.resources.clone();
        async move {
            Ok(ListResourcesResult {
                resources: (*resources).clone(),
                next_cursor: None,
                meta: None,
            })
        }
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::service::RoleServer>,
    ) -> Result<ListResourceTemplatesResult, McpError> {
        Ok(ListResourceTemplatesResult {
            resource_templates: (*self.resource_templates).clone(),
            next_cursor: None,
            meta: None,
        })
    }

    async fn read_resource(
        &self,
        ReadResourceRequestParams { uri, .. }: ReadResourceRequestParams,
        _context: rmcp::service::RequestContext<rmcp::service::RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        if uri == MEMO_URI {
            Ok(ReadResourceResult {
                contents: vec![ResourceContents::TextResourceContents {
                    uri,
                    mime_type: Some("text/plain".to_string()),
                    text: Self::memo_text().to_string(),
                    meta: None,
                }],
            })
        } else {
            Err(McpError::resource_not_found(
                "resource_not_found",
                Some(json!({ "uri": uri })),
            ))
        }
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: rmcp::service::RequestContext<rmcp::service::RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        match request.name.as_ref() {
            "sandbox_meta" => Ok(CallToolResult {
                content: Vec::new(),
                structured_content: Some(serde_json::Value::Object(context.meta.0)),
                is_error: Some(false),
                meta: None,
            }),
            "echo" | "echo-tool" => {
                let args: EchoArgs = match request.arguments {
                    Some(arguments) => serde_json::from_value(serde_json::Value::Object(
                        arguments.into_iter().collect(),
                    ))
                    .map_err(|err| McpError::invalid_params(err.to_string(), None))?,
                    None => {
                        return Err(McpError::invalid_params(
                            format!("missing arguments for {} tool", request.name),
                            None,
                        ));
                    }
                };

                let env_snapshot: HashMap<String, String> = std::env::vars().collect();
                let env_name = args.env_var.as_deref().unwrap_or("MCP_TEST_VALUE");
                let structured_content = json!({
                    "echo": format!("ECHOING: {}", args.message),
                    "env": env_snapshot.get(env_name),
                });

                Ok(CallToolResult {
                    content: Vec::new(),
                    structured_content: Some(structured_content),
                    is_error: Some(false),
                    meta: None,
                })
            }
            "image" => {
                // Read a data URL (e.g. data:image/png;base64,AAA...) from env and convert to
                // an MCP image content block. Tests set MCP_TEST_IMAGE_DATA_URL.
                let data_url = std::env::var("MCP_TEST_IMAGE_DATA_URL").map_err(|_| {
                    McpError::invalid_params(
                        "missing MCP_TEST_IMAGE_DATA_URL env var for image tool",
                        None,
                    )
                })?;

                let (mime_type, data_b64) = parse_data_url(&data_url).ok_or_else(|| {
                    McpError::invalid_params(
                        format!("invalid data URL for image tool: {data_url}"),
                        None,
                    )
                })?;

                Ok(CallToolResult::success(vec![rmcp::model::Content::image(
                    data_b64, mime_type,
                )]))
            }
            "image_scenario" => {
                let args = Self::parse_call_args::<ImageScenarioArgs>(&request, "image_scenario")?;
                Self::image_scenario_result(args)
            }
            "sync" => {
                let args = Self::parse_call_args::<SyncArgs>(&request, "sync")?;
                Self::sync_result(args).await
            }
            other => Err(McpError::invalid_params(
                format!("unknown tool: {other}"),
                None,
            )),
        }
    }
}

impl TestToolServer {
    fn parse_call_args<T: for<'de> Deserialize<'de>>(
        request: &CallToolRequestParams,
        tool_name: &'static str,
    ) -> Result<T, McpError> {
        match request.arguments.as_ref() {
            Some(arguments) => serde_json::from_value(serde_json::Value::Object(
                arguments.clone().into_iter().collect(),
            ))
            .map_err(|err| McpError::invalid_params(err.to_string(), None)),
            None => Err(McpError::invalid_params(
                format!("missing arguments for {tool_name} tool"),
                None,
            )),
        }
    }

    fn image_scenario_result(args: ImageScenarioArgs) -> Result<CallToolResult, McpError> {
        let (mime_type, valid_data_b64) = if let Some(data_url) = &args.data_url {
            parse_data_url(data_url).ok_or_else(|| {
                McpError::invalid_params(
                    format!("invalid data_url for image_scenario tool: {data_url}"),
                    None,
                )
            })?
        } else {
            ("image/png".to_string(), SMALL_PNG_BASE64.to_string())
        };

        let caption = args
            .caption
            .unwrap_or_else(|| "Here is the image:".to_string());

        let mut content = Vec::new();
        match args.scenario {
            ImageScenario::ImageOnly => {
                content.push(rmcp::model::Content::image(valid_data_b64, mime_type));
            }
            ImageScenario::ImageOnlyOriginalDetail => {
                let mut meta = rmcp::model::Meta::new();
                meta.insert(
                    "codex/imageDetail".to_string(),
                    serde_json::json!("original"),
                );
                content.push(rmcp::model::Annotated::new(
                    rmcp::model::RawContent::Image(rmcp::model::RawImageContent {
                        data: valid_data_b64,
                        mime_type,
                        meta: Some(meta),
                    }),
                    None,
                ));
            }
            ImageScenario::TextThenImage => {
                content.push(rmcp::model::Content::text(caption));
                content.push(rmcp::model::Content::image(valid_data_b64, mime_type));
            }
            ImageScenario::InvalidBase64ThenImage => {
                content.push(rmcp::model::Content::image(
                    "not-base64".to_string(),
                    "image/png".to_string(),
                ));
                content.push(rmcp::model::Content::image(valid_data_b64, mime_type));
            }
            ImageScenario::InvalidImageBytesThenImage => {
                content.push(rmcp::model::Content::image(
                    "bm90IGFuIGltYWdl".to_string(),
                    "image/png".to_string(),
                ));
                content.push(rmcp::model::Content::image(valid_data_b64, mime_type));
            }
            ImageScenario::MultipleValidImages => {
                content.push(rmcp::model::Content::image(
                    valid_data_b64.clone(),
                    mime_type.clone(),
                ));
                content.push(rmcp::model::Content::image(valid_data_b64, mime_type));
            }
            ImageScenario::ImageThenText => {
                content.push(rmcp::model::Content::image(valid_data_b64, mime_type));
                content.push(rmcp::model::Content::text(caption));
            }
            ImageScenario::TextOnly => {
                content.push(rmcp::model::Content::text(caption));
            }
        }

        Ok(CallToolResult::success(content))
    }

    async fn sync_result(args: SyncArgs) -> Result<CallToolResult, McpError> {
        if let Some(delay) = args.sleep_before_ms
            && delay > 0
        {
            sleep(Duration::from_millis(delay)).await;
        }

        if let Some(barrier) = args.barrier {
            wait_on_sync_barrier(barrier).await?;
        }

        if let Some(delay) = args.sleep_after_ms
            && delay > 0
        {
            sleep(Duration::from_millis(delay)).await;
        }

        Ok(CallToolResult {
            content: Vec::new(),
            structured_content: Some(json!({ "result": "ok" })),
            is_error: Some(false),
            meta: None,
        })
    }
}

async fn wait_on_sync_barrier(args: SyncBarrierArgs) -> Result<(), McpError> {
    if args.participants == 0 {
        return Err(McpError::invalid_params(
            "barrier participants must be greater than zero",
            None,
        ));
    }

    if args.timeout_ms == 0 {
        return Err(McpError::invalid_params(
            "barrier timeout must be greater than zero",
            None,
        ));
    }

    let barrier_id = args.id.clone();
    let barrier = {
        let mut map = sync_barrier_map().lock().await;
        match map.entry(barrier_id.clone()) {
            Entry::Occupied(entry) => {
                let state = entry.get();
                if state.participants != args.participants {
                    let existing = state.participants;
                    return Err(McpError::invalid_params(
                        format!(
                            "barrier {barrier_id} already registered with {existing} participants"
                        ),
                        None,
                    ));
                }
                state.barrier.clone()
            }
            Entry::Vacant(entry) => {
                let barrier = Arc::new(Barrier::new(args.participants));
                entry.insert(SyncBarrierState {
                    barrier: barrier.clone(),
                    participants: args.participants,
                });
                barrier
            }
        }
    };

    let wait_result =
        match tokio::time::timeout(Duration::from_millis(args.timeout_ms), barrier.wait()).await {
            Ok(wait_result) => wait_result,
            Err(_) => {
                remove_sync_barrier_if_current(&barrier_id, &barrier).await;
                return Err(McpError::invalid_params(
                    "sync barrier wait timed out",
                    None,
                ));
            }
        };

    if wait_result.is_leader() {
        remove_sync_barrier_if_current(&barrier_id, &barrier).await;
    }

    Ok(())
}

async fn remove_sync_barrier_if_current(barrier_id: &str, barrier: &Arc<Barrier>) {
    let mut map = sync_barrier_map().lock().await;
    if let Some(state) = map.get(barrier_id)
        && Arc::ptr_eq(&state.barrier, barrier)
    {
        map.remove(barrier_id);
    }
}

fn parse_data_url(url: &str) -> Option<(String, String)> {
    let rest = url.strip_prefix("data:")?;
    let (mime_and_opts, data) = rest.split_once(',')?;
    let (mime, _opts) = mime_and_opts.split_once(';').unwrap_or((mime_and_opts, ""));
    Some((mime.to_string(), data.to_string()))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("starting rmcp test server");
    // Run the server with STDIO transport. If the client disconnects we simply
    // bubble up the error so the process exits.
    let service = TestToolServer::new();
    let running = service.serve(stdio()).await?;

    // Wait for the client to finish interacting with the server.
    running.waiting().await?;
    // Drain background tasks to ensure clean shutdown.
    task::yield_now().await;
    Ok(())
}
