use crate::test_codex::TestCodexBuilder;
use crate::test_codex::test_codex;
use anyhow::Result;
use codex_core::config::Config;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_models_manager::bundled_models_response;
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
const CODEX_APPS_META_KEY: &str = "_codex_apps";
const PROTOCOL_VERSION: &str = "2025-11-25";
const SERVER_NAME: &str = "codex-apps-test";
const SERVER_VERSION: &str = "1.0.0";
const SEARCHABLE_TOOL_COUNT: usize = 100;
const CALENDAR_CREATE_EVENT_TOOL_NAME: &str = "calendar_create_event";
pub const CALENDAR_EXTRACT_TEXT_TOOL_NAME: &str = "calendar_extract_text";
const CALENDAR_LIST_EVENTS_TOOL_NAME: &str = "calendar_list_events";
pub const DIRECT_CALENDAR_CREATE_EVENT_TOOL: &str = "mcp__codex_apps__calendar_create_event";
pub const DIRECT_CALENDAR_LIST_EVENTS_TOOL: &str = "mcp__codex_apps__calendar_list_events";
pub const DIRECT_CALENDAR_EXTRACT_TEXT_TOOL: &str = "mcp__codex_apps__calendar_extract_text";
pub const SEARCH_CALENDAR_NAMESPACE: &str = "mcp__codex_apps__calendar";
pub const SEARCH_CALENDAR_CREATE_TOOL: &str = "_create_event";
pub const SEARCH_CALENDAR_EXTRACT_TEXT_TOOL: &str = "_extract_text";
pub const SEARCH_CALENDAR_LIST_TOOL: &str = "_list_events";
pub const CALENDAR_CREATE_EVENT_RESOURCE_URI: &str =
    "connector://calendar/tools/calendar_create_event";
pub const CALENDAR_CREATE_EVENT_MCP_APP_RESOURCE_URI: &str =
    "ui://widget/calendar-create-event.html";
const CALENDAR_LIST_EVENTS_RESOURCE_URI: &str = "connector://calendar/tools/calendar_list_events";
pub const DOCUMENT_EXTRACT_TEXT_RESOURCE_URI: &str =
    "connector://calendar/tools/calendar_extract_text";

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

pub fn configure_search_capable_model(config: &mut Config) {
    let mut model_catalog = bundled_models_response()
        .unwrap_or_else(|err| panic!("bundled models.json should parse: {err}"));
    let model = model_catalog
        .models
        .iter_mut()
        .find(|model| model.slug == "gpt-5.4")
        .expect("gpt-5.4 exists in bundled models.json");
    config.model = Some("gpt-5.4".to_string());
    model.supports_search_tool = true;
    config.model_catalog = Some(model_catalog);
}

fn configure_apps(config: &mut Config, apps_base_url: &str) {
    config
        .features
        .enable(Feature::Apps)
        .expect("test config should allow feature update");
    config.chatgpt_base_url = apps_base_url.to_string();
}

pub fn configure_search_capable_apps(config: &mut Config, apps_base_url: &str) {
    configure_apps(config, apps_base_url);
    configure_search_capable_model(config);
}

pub fn apps_enabled_builder(apps_base_url: impl Into<String>) -> TestCodexBuilder {
    let apps_base_url = apps_base_url.into();
    test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(move |config| configure_apps(config, apps_base_url.as_str()))
}

pub fn search_capable_apps_builder(apps_base_url: impl Into<String>) -> TestCodexBuilder {
    let apps_base_url = apps_base_url.into();
    test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(move |config| configure_search_capable_apps(config, apps_base_url.as_str()))
}

fn apps_tool_call_id(body: &Value) -> Option<&str> {
    body.get("params")?
        .get("_meta")?
        .get(CODEX_APPS_META_KEY)?
        .get("call_id")?
        .as_str()
}

async fn recorded_apps_tool_calls(server: &MockServer) -> Vec<Value> {
    server
        .received_requests()
        .await
        .expect("mock server should capture requests")
        .into_iter()
        .filter_map(|request| {
            let body: Value = serde_json::from_slice(&request.body).ok()?;
            (request.url.path() == "/api/codex/apps"
                && body.get("method").and_then(Value::as_str) == Some("tools/call"))
            .then_some(body)
        })
        .collect()
}

pub async fn recorded_apps_tool_call_by_call_id(server: &MockServer, call_id: &str) -> Value {
    let matches = recorded_apps_tool_calls(server)
        .await
        .into_iter()
        .filter(|body| apps_tool_call_id(body) == Some(call_id))
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "expected exactly one apps tools/call request for call_id {call_id}"
    );
    matches
        .into_iter()
        .next()
        .expect("matching apps tools/call request should be recorded")
}

pub async fn recorded_apps_tool_call_by_name(server: &MockServer, tool_name: &str) -> Value {
    let matches = recorded_apps_tool_calls(server)
        .await
        .into_iter()
        .filter(|body| body.pointer("/params/name").and_then(Value::as_str) == Some(tool_name))
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "expected exactly one apps tools/call request for tool {tool_name}"
    );
    matches
        .into_iter()
        .next()
        .expect("matching apps tools/call request should be recorded")
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
                                "name": CALENDAR_CREATE_EVENT_TOOL_NAME,
                                "description": "Create a calendar event.",
                                "annotations": {
                                    "readOnlyHint": false,
                                    "destructiveHint": false,
                                    "openWorldHint": false
                                },
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
                                    "connector_description": self.connector_description.clone(),
                                    "openai/outputTemplate": CALENDAR_CREATE_EVENT_MCP_APP_RESOURCE_URI,
                                    "_codex_apps": {
                                        "resource_uri": CALENDAR_CREATE_EVENT_RESOURCE_URI,
                                        "contains_mcp_source": true,
                                        "connector_id": CONNECTOR_ID
                                    }
                                }
                            },
                            {
                                "name": CALENDAR_LIST_EVENTS_TOOL_NAME,
                                "description": "List calendar events.",
                                "annotations": {
                                    "readOnlyHint": true
                                },
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
                                    "connector_description": self.connector_description.clone(),
                                    "_codex_apps": {
                                        "resource_uri": CALENDAR_LIST_EVENTS_RESOURCE_URI,
                                        "contains_mcp_source": true,
                                        "connector_id": CONNECTOR_ID
                                    }
                                }
                            },
                            {
                                "name": CALENDAR_EXTRACT_TEXT_TOOL_NAME,
                                "description": "Extract text from an uploaded document.",
                                "annotations": {
                                    "readOnlyHint": false
                                },
                                "inputSchema": {
                                    "type": "object",
                                    "properties": {
                                        "file": {
                                            "type": "object",
                                            "description": "Document file payload.",
                                            "properties": {
                                                "file_id": { "type": "string" }
                                            },
                                            "required": ["file_id"]
                                        }
                                    },
                                    "required": ["file"],
                                    "additionalProperties": false
                                },
                                "_meta": {
                                    "connector_id": CONNECTOR_ID,
                                    "connector_name": self.connector_name.clone(),
                                    "connector_description": self.connector_description.clone(),
                                    "openai/fileParams": ["file"],
                                    "_codex_apps": {
                                        "resource_uri": DOCUMENT_EXTRACT_TEXT_RESOURCE_URI,
                                        "contains_mcp_source": true,
                                        "connector_id": CONNECTOR_ID
                                    }
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
                    for index in 3..SEARCHABLE_TOOL_COUNT {
                        tools.push(json!({
                            "name": format!("calendar_timezone_option_{index}"),
                            "description": format!("Read timezone option {index}."),
                            "annotations": {
                                "readOnlyHint": true
                            },
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
                let file_id = body
                    .pointer("/params/arguments/file/file_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let codex_apps_meta = body.pointer("/params/_meta/_codex_apps").cloned();

                ResponseTemplate::new(200).set_body_json(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{
                            "type": "text",
                            "text": format!("called {tool_name} for {title} at {starts_at} with {file_id}")
                        }],
                        "structuredContent": {
                            "_codex_apps": codex_apps_meta,
                        },
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
