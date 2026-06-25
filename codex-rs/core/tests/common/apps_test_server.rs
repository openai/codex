use crate::test_codex::TestCodexBuilder;
use crate::test_codex::test_codex;
use anyhow::Result;
use codex_core::config::Config;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_models_manager::bundled_models_response;
use serde_json::Value;
use serde_json::json;
use std::sync::Arc;
use std::sync::Condvar;
use std::sync::Mutex;
use std::sync::PoisonError;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use tokio::sync::Notify;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::Request;
use wiremock::Respond;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;
use wiremock::matchers::path_regex;

const CONNECTOR_ID: &str = "calendar";
pub const LINK_ID: &str = "link_calendar";
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
const CALENDAR_APP_ONLY_TOOL_NAME: &str = "calendar_app_only_action";
pub const CALENDAR_EXTRACT_TEXT_TOOL_NAME: &str = "calendar_extract_text";
pub const CALENDAR_UPSTREAM_ERROR_TITLE: &str = "return an upstream Apps error";
const CALENDAR_LIST_EVENTS_TOOL_NAME: &str = "calendar_list_events";
pub const DIRECT_CALENDAR_CREATE_EVENT_TOOL: &str = "mcp__codex_apps__calendar__create_event";
pub const DIRECT_CALENDAR_APP_ONLY_TOOL: &str = "mcp__codex_apps__calendar__app_only_action";
pub const DIRECT_CALENDAR_LIST_EVENTS_TOOL: &str = "mcp__codex_apps__calendar__list_events";
pub const DIRECT_CALENDAR_EXTRACT_TEXT_TOOL: &str = "mcp__codex_apps__calendar__extract_text";
pub const CALENDAR_MCP_SERVER_NAME: &str = "codex_apps__calendar";
pub const APPS_RESOURCE_MCP_SERVER_NAME: &str = "codex_apps";
pub const SEARCH_CALENDAR_NAMESPACE: &str = "mcp__codex_apps__calendar";
pub const SEARCH_CALENDAR_APP_ONLY_TOOL: &str = "_app_only_action";
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

#[derive(Clone, Default)]
pub struct AppsTestServerStartupControl {
    initialize_attempts: Arc<AtomicUsize>,
    tools_list_attempts: Arc<AtomicUsize>,
    remaining_initialize_failures: Arc<AtomicUsize>,
}

impl AppsTestServerStartupControl {
    pub fn fail_next_initialize_attempts(&self, attempts: usize) {
        self.remaining_initialize_failures
            .store(attempts, Ordering::SeqCst);
    }

    pub fn initialize_attempts(&self) -> usize {
        self.initialize_attempts.load(Ordering::SeqCst)
    }

    pub fn tools_list_attempts(&self) -> usize {
        self.tools_list_attempts.load(Ordering::SeqCst)
    }
}

#[derive(Clone, Copy)]
pub enum AppsTestToolLoading {
    Direct,
    Searchable,
}

#[derive(Default)]
struct AppsToolsListGateState {
    entered: bool,
    released: bool,
}

#[derive(Default)]
struct AppsToolsListGateInner {
    state: Mutex<AppsToolsListGateState>,
    entered: Notify,
    released: Condvar,
}

/// Explicitly blocks the hosted Apps `tools/list` response until the test releases it.
pub struct AppsToolsListGate {
    inner: Arc<AppsToolsListGateInner>,
}

impl AppsToolsListGate {
    pub async fn wait_until_entered(&self) {
        loop {
            let entered = self.inner.entered.notified();
            if self
                .inner
                .state
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .entered
            {
                return;
            }
            entered.await;
        }
    }

    pub fn release(&self) {
        self.inner.release();
    }
}

impl Drop for AppsToolsListGate {
    fn drop(&mut self) {
        self.release();
    }
}

impl AppsToolsListGateInner {
    fn block(&self) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if !state.entered {
            state.entered = true;
            self.entered.notify_waiters();
        }
        while !state.released {
            state = self
                .released
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    fn release(&self) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.released = true;
        self.released.notify_all();
    }
}

#[derive(Clone, Copy)]
enum AppsTestToolsListBehavior {
    AlwaysAvailable,
    AvailableAfterInitialList,
    AlwaysUnavailable,
}

#[derive(Clone, Copy, Default)]
struct AppsTestToolOptions {
    searchable: bool,
    include_app_only_tool: bool,
    synthetic_only: bool,
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
            CONNECTOR_NAME,
            AppsTestToolOptions {
                searchable: true,
                ..Default::default()
            },
            AppsTestToolsListBehavior::AlwaysAvailable,
            /*tools_list_gate*/ None,
            /*startup_control*/ None,
        )
        .await;
        Ok(Self {
            chatgpt_base_url: server.uri(),
        })
    }

    pub async fn mount_searchable_with_startup_control(
        server: &MockServer,
    ) -> Result<(Self, AppsTestServerStartupControl)> {
        mount_oauth_metadata(server).await;
        mount_connectors_directory(server).await;
        let control = AppsTestServerStartupControl::default();
        mount_streamable_http_json_rpc(
            server,
            CONNECTOR_NAME,
            AppsTestToolOptions {
                searchable: true,
                ..Default::default()
            },
            AppsTestToolsListBehavior::AlwaysAvailable,
            /*tools_list_gate*/ None,
            Some(control.clone()),
        )
        .await;
        Ok((
            Self {
                chatgpt_base_url: server.uri(),
            },
            control,
        ))
    }

    pub async fn mount_with_connector_name(
        server: &MockServer,
        connector_name: &str,
    ) -> Result<Self> {
        mount_oauth_metadata(server).await;
        mount_connectors_directory(server).await;
        mount_streamable_http_json_rpc(
            server,
            connector_name,
            AppsTestToolOptions::default(),
            AppsTestToolsListBehavior::AlwaysAvailable,
            /*tools_list_gate*/ None,
            /*startup_control*/ None,
        )
        .await;
        Ok(Self {
            chatgpt_base_url: server.uri(),
        })
    }

    pub async fn mount_with_app_only_tool(
        server: &MockServer,
        tool_loading: AppsTestToolLoading,
    ) -> Result<Self> {
        mount_oauth_metadata(server).await;
        mount_connectors_directory(server).await;
        mount_streamable_http_json_rpc(
            server,
            CONNECTOR_NAME,
            AppsTestToolOptions {
                searchable: matches!(tool_loading, AppsTestToolLoading::Searchable),
                include_app_only_tool: true,
                ..Default::default()
            },
            AppsTestToolsListBehavior::AlwaysAvailable,
            /*tools_list_gate*/ None,
            /*startup_control*/ None,
        )
        .await;
        Ok(Self {
            chatgpt_base_url: server.uri(),
        })
    }

    pub async fn mount_with_tools_available_after_initial_list(
        server: &MockServer,
    ) -> Result<Self> {
        Self::mount_with_tools_list_behavior(
            server,
            AppsTestToolsListBehavior::AvailableAfterInitialList,
        )
        .await
    }

    pub async fn mount_with_synthetic_tools_available_after_initial_list(
        server: &MockServer,
    ) -> Result<Self> {
        mount_oauth_metadata(server).await;
        mount_connectors_directory(server).await;
        mount_streamable_http_json_rpc(
            server,
            CONNECTOR_NAME,
            AppsTestToolOptions {
                synthetic_only: true,
                ..Default::default()
            },
            AppsTestToolsListBehavior::AvailableAfterInitialList,
            /*tools_list_gate*/ None,
            /*startup_control*/ None,
        )
        .await;
        Ok(Self {
            chatgpt_base_url: server.uri(),
        })
    }

    pub async fn mount_with_tools_list_gate(
        server: &MockServer,
    ) -> Result<(Self, AppsToolsListGate)> {
        mount_oauth_metadata(server).await;
        mount_connectors_directory(server).await;
        let inner = Arc::new(AppsToolsListGateInner::default());
        mount_streamable_http_json_rpc(
            server,
            CONNECTOR_NAME,
            AppsTestToolOptions {
                searchable: true,
                ..Default::default()
            },
            AppsTestToolsListBehavior::AlwaysAvailable,
            Some(Arc::clone(&inner)),
            /*startup_control*/ None,
        )
        .await;
        Ok((
            Self {
                chatgpt_base_url: server.uri(),
            },
            AppsToolsListGate { inner },
        ))
    }

    pub async fn mount_without_tools(server: &MockServer) -> Result<Self> {
        Self::mount_with_tools_list_behavior(server, AppsTestToolsListBehavior::AlwaysUnavailable)
            .await
    }

    async fn mount_with_tools_list_behavior(
        server: &MockServer,
        tools_list_behavior: AppsTestToolsListBehavior,
    ) -> Result<Self> {
        mount_oauth_metadata(server).await;
        mount_connectors_directory(server).await;
        mount_streamable_http_json_rpc(
            server,
            CONNECTOR_NAME,
            AppsTestToolOptions::default(),
            tools_list_behavior,
            /*tools_list_gate*/ None,
            /*startup_control*/ None,
        )
        .await;
        Ok(Self {
            chatgpt_base_url: server.uri(),
        })
    }
}

pub fn configure_search_capable_model(config: &mut Config) {
    let mut model_catalog = bundled_models_response().expect("bundled models.json should parse");
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
        .with_extension_factory(apps_extensions)
        .with_config(move |config| configure_apps(config, apps_base_url.as_str()))
}

pub fn search_capable_apps_builder(apps_base_url: impl Into<String>) -> TestCodexBuilder {
    let apps_base_url = apps_base_url.into();
    test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_extension_factory(apps_extensions)
        .with_config(move |config| configure_search_capable_apps(config, apps_base_url.as_str()))
}

pub fn search_capable_apps_builder_with_analytics(
    apps_base_url: impl Into<String>,
) -> TestCodexBuilder {
    let apps_base_url = apps_base_url.into();
    let analytics_base_url = apps_base_url.clone();
    test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_extension_factory(
            move |auth_manager, environment_manager, plugins_manager, _config| {
                let analytics_events_client = codex_analytics::AnalyticsEventsClient::new(
                    Arc::clone(&auth_manager),
                    analytics_base_url.clone(),
                    /*analytics_enabled*/ None,
                );
                let mut extensions = codex_extension_api::ExtensionRegistryBuilder::new();
                let service = Arc::new(
                    codex_mcp_extension::CodexAppsMcpExtension::new_with_analytics(
                        auth_manager,
                        environment_manager,
                        plugins_manager,
                        analytics_events_client,
                    ),
                );
                codex_mcp_extension::install(&mut extensions, service);
                Arc::new(extensions.build())
            },
        )
        .with_config(move |config| configure_search_capable_apps(config, apps_base_url.as_str()))
}

fn apps_extensions(
    auth_manager: std::sync::Arc<codex_login::AuthManager>,
    environment_manager: std::sync::Arc<codex_exec_server::EnvironmentManager>,
    plugins_manager: std::sync::Arc<codex_core_plugins::PluginsManager>,
    _config: &Config,
) -> std::sync::Arc<codex_extension_api::ExtensionRegistry<Config>> {
    let mut extensions = codex_extension_api::ExtensionRegistryBuilder::new();
    let service = std::sync::Arc::new(codex_mcp_extension::CodexAppsMcpExtension::new(
        auth_manager,
        environment_manager,
        plugins_manager,
    ));
    codex_mcp_extension::install(&mut extensions, service);
    std::sync::Arc::new(extensions.build())
}

fn apps_tool_call_id(body: &Value) -> Option<&str> {
    body.get("params")?
        .get("_meta")?
        .get(CODEX_APPS_META_KEY)?
        .get("call_id")?
        .as_str()
}

pub async fn recorded_apps_tool_calls(server: &MockServer) -> Vec<Value> {
    server
        .received_requests()
        .await
        .expect("mock server should capture requests")
        .into_iter()
        .filter_map(|request| {
            let body: Value = serde_json::from_slice(&request.body).ok()?;
            (request.url.path() == "/api/codex/ps/mcp"
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
    connector_name: &str,
    tool_options: AppsTestToolOptions,
    tools_list_behavior: AppsTestToolsListBehavior,
    tools_list_gate: Option<Arc<AppsToolsListGateInner>>,
    startup_control: Option<AppsTestServerStartupControl>,
) {
    let AppsTestToolOptions {
        searchable,
        include_app_only_tool,
        synthetic_only,
    } = tool_options;
    Mock::given(method("POST"))
        .and(path_regex("^/api/codex/ps/mcp/?$"))
        .respond_with(CodexAppsJsonRpcResponder {
            connector_name: connector_name.to_string(),
            connector_description: CONNECTOR_DESCRIPTION.to_string(),
            searchable,
            include_app_only_tool,
            synthetic_only,
            tools_list_behavior,
            tools_list_calls: AtomicUsize::new(0),
            tools_list_gate,
            startup_control,
        })
        .mount(server)
        .await;
}

struct CodexAppsJsonRpcResponder {
    connector_name: String,
    connector_description: String,
    searchable: bool,
    include_app_only_tool: bool,
    synthetic_only: bool,
    tools_list_behavior: AppsTestToolsListBehavior,
    tools_list_calls: AtomicUsize,
    tools_list_gate: Option<Arc<AppsToolsListGateInner>>,
    startup_control: Option<AppsTestServerStartupControl>,
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
                if let Some(control) = &self.startup_control {
                    control.initialize_attempts.fetch_add(1, Ordering::SeqCst);
                    if control
                        .remaining_initialize_failures
                        .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |remaining| {
                            remaining.checked_sub(1)
                        })
                        .is_ok()
                    {
                        return ResponseTemplate::new(400).set_body_json(json!({
                            "error": "simulated non-retryable Apps startup failure",
                        }));
                    }
                }
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
                if let Some(control) = &self.startup_control {
                    control.tools_list_attempts.fetch_add(1, Ordering::SeqCst);
                }
                if let Some(gate) = &self.tools_list_gate {
                    gate.block();
                }
                let list_index = self.tools_list_calls.fetch_add(1, Ordering::SeqCst);
                let tools_available = match self.tools_list_behavior {
                    AppsTestToolsListBehavior::AlwaysAvailable => true,
                    AppsTestToolsListBehavior::AvailableAfterInitialList => list_index > 0,
                    AppsTestToolsListBehavior::AlwaysUnavailable => false,
                };
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
                                    "link_id": LINK_ID,
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
                                    "link_id": LINK_ID,
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
                                    "link_id": LINK_ID,
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
                if !tools_available
                    && let Some(tools) = response
                        .pointer_mut("/result/tools")
                        .and_then(Value::as_array_mut)
                {
                    tools.clear();
                }
                if tools_available
                    && self.synthetic_only
                    && let Some(tools) = response
                        .pointer_mut("/result/tools")
                        .and_then(Value::as_array_mut)
                {
                    for tool in tools {
                        tool.pointer_mut("/_meta/_codex_apps")
                            .and_then(Value::as_object_mut)
                            .expect("test tool has private Apps metadata")
                            .insert("synthetic_link".to_string(), Value::Bool(true));
                    }
                }
                if tools_available
                    && self.searchable
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
                if tools_available
                    && self.include_app_only_tool
                    && let Some(tools) = response
                        .pointer_mut("/result/tools")
                        .and_then(Value::as_array_mut)
                {
                    tools.push(json!({
                        "name": CALENDAR_APP_ONLY_TOOL_NAME,
                        "description": "Open a calendar app-only action.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {},
                            "additionalProperties": false
                        },
                        "_meta": {
                            "connector_id": CONNECTOR_ID,
                            "connector_name": self.connector_name.clone(),
                            "connector_description": self.connector_description.clone(),
                            "ui": {
                                "visibility": ["app"]
                            }
                        }
                    }));
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
                let is_error = title == CALENDAR_UPSTREAM_ERROR_TITLE;

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
                        "isError": is_error
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
