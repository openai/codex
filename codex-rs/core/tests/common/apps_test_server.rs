use anyhow::Result;
use serde_json::Value;
use serde_json::json;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::Request;
use wiremock::Respond;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;
use wiremock::matchers::path_regex;

const CONNECTOR_ID: &str = "calendar";
const CONNECTOR_NAME: &str = "Calendar";
const DISCOVERABLE_CALENDAR_ID: &str = "connector_2128aebfecb84f64a069897515042a44";
const DISCOVERABLE_GMAIL_ID: &str = "connector_68df038e0ba48191908c8434991bbac2";
const CONNECTOR_DESCRIPTION: &str = "Plan events and manage your calendar.";
const PROTOCOL_VERSION: &str = "2025-11-25";
const SERVER_NAME: &str = "codex-apps-test";
const SERVER_VERSION: &str = "1.0.0";
const SEARCHABLE_TOOL_COUNT: usize = 100;

#[derive(Clone)]
pub struct AppsTestServer {
    pub chatgpt_base_url: String,
}

impl AppsTestServer {
    pub async fn mount(server: &MockServer) -> Result<Self> {
        Self::mount_with_connector_name(server, CONNECTOR_NAME).await
    }

    pub async fn mount_searchable(server: &MockServer) -> Result<Self> {
        mount_oauth_metadata(server).await;
        mount_connectors_directory(server).await;
        mount_streamable_http_json_rpc(
            server,
            CONNECTOR_NAME.to_string(),
            CONNECTOR_DESCRIPTION.to_string(),
            /*searchable*/ true,
        )
        .await;
        Ok(Self {
            chatgpt_base_url: server.uri(),
        })
    }

    pub async fn mount_with_connector_name(
        server: &MockServer,
        connector_name: &str,
    ) -> Result<Self> {
        mount_oauth_metadata(server).await;
        mount_connectors_directory(server).await;
        mount_streamable_http_json_rpc(
            server,
            connector_name.to_string(),
            CONNECTOR_DESCRIPTION.to_string(),
            /*searchable*/ false,
        )
        .await;
        Ok(Self {
            chatgpt_base_url: server.uri(),
        })
    }
}

async fn mount_oauth_metadata(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-authorization-server/mcp"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "authorization_endpoint": format!("{}/oauth/authorize", server.uri()),
            "token_endpoint": format!("{}/oauth/token", server.uri()),
            "scopes_supported": [""],
        })))
        .mount(server)
        .await;
}

async fn mount_connectors_directory(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/connectors/directory/list"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "apps": [
                {
                    "id": DISCOVERABLE_CALENDAR_ID,
                    "name": "Google Calendar",
                    "description": "Plan events and schedules.",
                },
                {
                    "id": DISCOVERABLE_GMAIL_ID,
                    "name": "Gmail",
                    "description": "Find and summarize email threads.",
                }
            ],
            "nextToken": null
        })))
        .mount(server)
        .await;

    Mock::given(method("GET"))
        .and(path("/connectors/directory/list_workspace"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "apps": [],
            "nextToken": null
        })))
        .mount(server)
        .await;
}

async fn mount_streamable_http_json_rpc(
    server: &MockServer,
    connector_name: String,
    connector_description: String,
    searchable: bool,
) {
    Mock::given(method("POST"))
        .and(path_regex("^/api/codex/apps/?$"))
        .respond_with(CodexAppsJsonRpcResponder {
            connector_name,
            connector_description,
            searchable,
        })
        .mount(server)
        .await;
}

struct CodexAppsJsonRpcResponder {
    connector_name: String,
    connector_description: String,
    searchable: bool,
}

impl Respond for CodexAppsJsonRpcResponder {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let body: Value = match serde_json::from_slice(&request.body) {
            Ok(body) => body,
            Err(error) => {
                return ResponseTemplate::new(400).set_body_json(json!({
                    "error": format!("invalid JSON-RPC body: {error}"),
                }));
            }
        };

        let Some(method) = body.get("method").and_then(Value::as_str) else {
            return ResponseTemplate::new(400).set_body_json(json!({
                "error": "missing method in JSON-RPC request",
            }));
        };

        match method {
            "initialize" => {
                let id = body.get("id").cloned().unwrap_or(Value::Null);
                let protocol_version = body
                    .pointer("/params/protocolVersion")
                    .and_then(Value::as_str)
                    .unwrap_or(PROTOCOL_VERSION);
                ResponseTemplate::new(200).set_body_json(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "protocolVersion": protocol_version,
                        "capabilities": {
                            "tools": {
                                "listChanged": true
                            }
                        },
                        "serverInfo": {
                            "name": SERVER_NAME,
                            "version": SERVER_VERSION
                        }
                    }
                }))
            }
            "notifications/initialized" => ResponseTemplate::new(202),
            "tools/list" => {
                let id = body.get("id").cloned().unwrap_or(Value::Null);
                let mut response = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "tools": [
                            {
                                "name": "calendar_create_event",
                                "description": "Create a calendar event.",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "title": { "type": "string" },
                                        "starts_at": { "type": "string" },
                                        "timezone": { "type": "string" }
                                    },
                                    "required": ["title", "starts_at"],
                                    "additionalProperties": false
                                },
                                "_meta": {
                                    "connector_id": CONNECTOR_ID,
                                    "connector_name": self.connector_name.clone(),
                                    "connector_description": self.connector_description.clone()
                                }
                            },
                            {
                                "name": "calendar_list_events",
                                "description": "List calendar events.",
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "query": { "type": "string" },
                                        "limit": { "type": "integer" }
                                    },
                                    "additionalProperties": false
                                },
                                "_meta": {
                                    "connector_id": CONNECTOR_ID,
                                    "connector_name": self.connector_name.clone(),
                                    "connector_description": self.connector_description.clone()
                                }
                            }
                        ],
                        "nextCursor": null
                    }
                });
                if self.searchable
                    && let Some(tools) = response
                        .pointer_mut("/result/tools")
                        .and_then(Value::as_array_mut)
                {
                    for index in 2..SEARCHABLE_TOOL_COUNT {
                        tools.push(json!({
                            "name": format!("calendar_timezone_option_{index}"),
                            "description": format!("Read timezone option {index}."),
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "timezone": { "type": "string" }
                                },
                                "additionalProperties": false
                            },
                            "_meta": {
                                "connector_id": CONNECTOR_ID,
                                "connector_name": self.connector_name.clone(),
                                "connector_description": self.connector_description.clone()
                            }
                        }));
                    }
                }
                ResponseTemplate::new(200).set_body_json(response)
            }
            "tools/call" => {
                let id = body.get("id").cloned().unwrap_or(Value::Null);
                let tool_name = body
                    .pointer("/params/name")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let title = body
                    .pointer("/params/arguments/title")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let starts_at = body
                    .pointer("/params/arguments/starts_at")
                    .and_then(Value::as_str)
                    .unwrap_or_default();

                ResponseTemplate::new(200).set_body_json(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{
                            "type": "text",
                            "text": format!("called {tool_name} for {title} at {starts_at}")
                        }],
                        "isError": false
                    }
                }))
            }
            method if method.starts_with("notifications/") => ResponseTemplate::new(202),
            _ => {
                let id = body.get("id").cloned().unwrap_or(Value::Null);
                ResponseTemplate::new(200).set_body_json(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {
                        "code": -32601,
                        "message": format!("method not found: {method}")
                    }
                }))
            }
        }
    }
}
