use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use rmcp::ErrorData as McpError;
use rmcp::ServiceExt;
use rmcp::handler::server::ServerHandler;
use rmcp::model::CallToolRequestParam;
use rmcp::model::CallToolResult;
use rmcp::model::JsonObject;
use rmcp::model::ListResourceTemplatesResult;
use rmcp::model::ListResourcesResult;
use rmcp::model::ListToolsResult;
use rmcp::model::PaginatedRequestParam;
use rmcp::model::RawResource;
use rmcp::model::RawResourceTemplate;
use rmcp::model::ReadResourceRequestParam;
use rmcp::model::ReadResourceResult;
use rmcp::model::Resource;
use rmcp::model::ResourceContents;
use rmcp::model::ResourceTemplate;
use rmcp::model::ServerCapabilities;
use rmcp::model::ServerInfo;
use rmcp::model::Tool;
use serde::Deserialize;
use serde_json::json;
use tokio::task;

#[derive(Clone)]
struct TestToolServer {
    tools: Arc<Vec<Tool>>,
    resources: Arc<Vec<Resource>>,
    resource_templates: Arc<Vec<ResourceTemplate>>,
}

const MEMO_URI: &str = "memo://codex/example-note";
const MEMO_CONTENT: &str = "This is a sample MCP resource served by the rmcp test server.";
const SMALL_PNG_BASE64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==";

pub fn stdio() -> (tokio::io::Stdin, tokio::io::Stdout) {
    (tokio::io::stdin(), tokio::io::stdout())
}

impl TestToolServer {
    fn new() -> Self {
        let tools = vec![
            Self::echo_tool(),
            Self::image_tool(),
            Self::image_scenario_tool(),
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

        Tool::new(
            Cow::Borrowed("echo"),
            Cow::Borrowed("Echo back the provided message and include environment data."),
            Arc::new(schema),
        )
    }

    fn image_tool() -> Tool {
        #[expect(clippy::expect_used)]
        let schema: JsonObject = serde_json::from_value(serde_json::json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        }))
        .expect("image tool schema should deserialize");

        Tool::new(
            Cow::Borrowed("image"),
            Cow::Borrowed("Return a single image content block."),
            Arc::new(schema),
        )
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

        Tool::new(
            Cow::Borrowed("image_scenario"),
            Cow::Borrowed(
                "Return content blocks for manual testing of MCP image rendering scenarios.",
            ),
            Arc::new(schema),
        )
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
    #[allow(dead_code)]
    env_var: Option<String>,
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
        ServerInfo {
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_tool_list_changed()
                .enable_resources()
                .build(),
            ..ServerInfo::default()
        }
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParam>,
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
        _request: Option<PaginatedRequestParam>,
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
        _request: Option<PaginatedRequestParam>,
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
        ReadResourceRequestParam { uri }: ReadResourceRequestParam,
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
        request: CallToolRequestParam,
        _context: rmcp::service::RequestContext<rmcp::service::RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        match request.name.as_ref() {
            "echo" => {
                let args: EchoArgs = match request.arguments {
                    Some(arguments) => serde_json::from_value(serde_json::Value::Object(
                        arguments.into_iter().collect(),
                    ))
                    .map_err(|err| McpError::invalid_params(err.to_string(), None))?,
                    None => {
                        return Err(McpError::invalid_params(
                            "missing arguments for echo tool",
                            None,
                        ));
                    }
                };

                let env_snapshot: HashMap<String, String> = std::env::vars().collect();
                let structured_content = json!({
                    "echo": format!("ECHOING: {}", args.message),
                    "env": env_snapshot.get("MCP_TEST_VALUE"),
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
            other => Err(McpError::invalid_params(
                format!("unknown tool: {other}"),
                None,
            )),
        }
    }
}

impl TestToolServer {
    fn parse_call_args<T: for<'de> Deserialize<'de>>(
        request: &CallToolRequestParam,
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
