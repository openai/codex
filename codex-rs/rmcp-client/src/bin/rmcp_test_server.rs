use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use rmcp::ErrorData as McpError;
use rmcp::ServiceExt;
use rmcp::handler::server::ServerHandler;
use rmcp::model::CallToolRequestParams;
use rmcp::model::CallToolResult;
use rmcp::model::JsonObject;
use rmcp::model::ListToolsResult;
use rmcp::model::PaginatedRequestParams;
use rmcp::model::ServerCapabilities;
use rmcp::model::ServerInfo;
use rmcp::model::Tool;
use serde::Deserialize;
use serde_json::json;
use tokio::sync::RwLock;
use tokio::task;

#[derive(Clone)]
struct TestToolServer {
    tools: Arc<RwLock<Vec<Tool>>>,
}
pub fn stdio() -> (tokio::io::Stdin, tokio::io::Stdout) {
    (tokio::io::stdin(), tokio::io::stdout())
}
impl TestToolServer {
    fn new() -> Self {
        let tools = vec![Self::echo_tool(), Self::trigger_list_changed_tool()];
        Self {
            tools: Arc::new(RwLock::new(tools)),
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
}

#[derive(Deserialize)]
struct EchoArgs {
    message: String,
    #[allow(dead_code)]
    env_var: Option<String>,
}

impl TestToolServer {
    fn new_tool() -> Tool {
        #[expect(clippy::expect_used)]
        let schema: JsonObject = serde_json::from_value(json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        }))
        .expect("new_tool schema should deserialize");

        Tool::new(
            Cow::Borrowed("new_tool"),
            Cow::Borrowed("A tool added dynamically via trigger_list_changed."),
            Arc::new(schema),
        )
    }

    fn trigger_list_changed_tool() -> Tool {
        #[expect(clippy::expect_used)]
        let schema: JsonObject = serde_json::from_value(json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        }))
        .expect("trigger_list_changed schema should deserialize");

        Tool::new(
            Cow::Borrowed("trigger_list_changed"),
            Cow::Borrowed("Adds new_tool to the list and sends notifications/tools/list_changed."),
            Arc::new(schema),
        )
    }
}

impl ServerHandler for TestToolServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_tool_list_changed()
                .build(),
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
                tools: tools.read().await.clone(),
                next_cursor: None,
                meta: None,
            })
        }
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: rmcp::service::RequestContext<rmcp::service::RoleServer>,
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
                    "echo": args.message,
                    "env": env_snapshot.get("MCP_TEST_VALUE"),
                });

                Ok(CallToolResult {
                    content: Vec::new(),
                    structured_content: Some(structured_content),
                    is_error: Some(false),
                    meta: None,
                })
            }
            "trigger_list_changed" => {
                self.tools.write().await.push(Self::new_tool());
                // Yield to let rmcp flush the call_tool response before sending
                // the notification on the same transport.
                let peer = context.peer.clone();
                tokio::spawn(async move {
                    tokio::task::yield_now().await;
                    let _ = peer.notify_tool_list_changed().await;
                });
                Ok(CallToolResult::success(vec![rmcp::model::Content::text(
                    "tool list change triggered",
                )]))
            }
            other => Err(McpError::invalid_params(
                format!("unknown tool: {other}"),
                None,
            )),
        }
    }
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
