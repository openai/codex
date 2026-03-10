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
const PROTOCOL_VERSION: &str = "2025-11-25";
const SERVER_NAME: &str = "codex-apps-test";
const SERVER_VERSION: &str = "1.0.0";

#[derive(Clone)]
pub struct AppsTestServer {
    pub chatgpt_base_url: String,
}

impl AppsTestServer {
    pub async fn mount(server: &MockServer) -> Result<Self> {
        Self::mount_with_connector_name(server, CONNECTOR_NAME).await
    }

    pub async fn mount_with_connector_name(
        server: &MockServer,
        connector_name: &str,
    ) -> Result<Self> {
        mount_oauth_metadata(server).await;
        mount_streamable_http_json_rpc(server, connector_name.to_string()).await;
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

async fn mount_streamable_http_json_rpc(server: &MockServer, connector_name: String) {
    Mock::given(method("POST"))
        .and(path_regex("^/api/codex/apps/?$"))
        .respond_with(CodexAppsJsonRpcResponder { connector_name })
        .mount(server)
        .await;
}

struct CodexAppsJsonRpcResponder {
    connector_name: String,
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
                ResponseTemplate::new(200).set_body_json(json!({
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
                                    "connector_name": self.connector_name.clone()
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
                                    "connector_name": self.connector_name.clone()
                                }
                            }
                        ],
                        "nextCursor": null
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
