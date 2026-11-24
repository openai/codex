//! Verifies protocol selection, request auth, and metadata for extension-contributed MCP servers.

use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use codex_api::AuthError;
use codex_config::McpServerConfig;
use codex_core::StartThreadOptions;
use codex_core::TurnInputRequest;
use codex_core::config::Config;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::McpProtocolMode;
use codex_extension_api::McpRequestAuth;
use codex_extension_api::McpRequestAuthProvider;
use codex_extension_api::McpServerContribution;
use codex_extension_api::McpServerContributionContext;
use codex_extension_api::McpServerContributor;
use codex_features::Feature;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_mcp::McpServerExposure;
use codex_mcp::McpServerKey;
use codex_mcp::McpServerRegistration;
use codex_mcp::McpServerSource;
use codex_mcp::McpSnapshotDetail;
use codex_mcp::McpToolRequestMetaHook;
use codex_mcp::McpToolRequestMetaProvider;
use codex_mcp::ReadResourceRequestParams;
use codex_mcp::ResolvedMcpCatalog;
use codex_protocol::ThreadId;
use codex_protocol::approvals::ElicitationRequest;
use codex_protocol::mcp::ClientMcpExtensions;
use codex_protocol::mcp::OPENAI_ELICITATION_EXTENSION_ID;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::protocol::ElicitationAction;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::apps_test_server::apps_enabled_builder;
use core_test_support::apps_test_server::recorded_apps_tool_calls;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_mcp_server;
use http::HeaderMap;
use http::HeaderValue;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use test_case::test_case;
use tokio::time::timeout;
use tokio_util::task::AbortOnDropHandle;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::Request;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

struct ExtensionServers(Vec<McpServerContribution>);

impl McpServerContributor<Config> for ExtensionServers {
    fn id(&self) -> &'static str {
        "apps_protocol_test"
    }

    fn contribute<'a>(
        &'a self,
        _context: McpServerContributionContext<'a, Config>,
    ) -> ExtensionFuture<'a, Vec<McpServerContribution>> {
        Box::pin(async move { self.0.clone() })
    }
}

async fn mcp_methods(server: &MockServer) -> Vec<String> {
    server
        .received_requests()
        .await
        .expect("mock server should capture MCP startup requests")
        .into_iter()
        .filter(|request| request.url.path() == "/api/codex/ps/mcp")
        .filter_map(|request| {
            let body: Value = serde_json::from_slice(&request.body).ok()?;
            body.get("method")?.as_str().map(str::to_string)
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn extension_protocol_mode_does_not_change_other_http_servers() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    for (generic_mode, extension_mode) in [
        (McpProtocolMode::Legacy, McpProtocolMode::V20260728),
        (McpProtocolMode::V20260728, McpProtocolMode::Legacy),
    ] {
        let responses_server = responses::start_mock_server().await;
        let extension_server = responses::start_mock_server().await;
        let extension_url = format!(
            "{}/api/codex/ps/mcp",
            AppsTestServer::mount(&extension_server)
                .await?
                .chatgpt_base_url
        );
        let third_party_server = responses::start_mock_server().await;
        let third_party_url = format!(
            "{}/api/codex/ps/mcp",
            AppsTestServer::mount(&third_party_server)
                .await?
                .chatgpt_base_url
        );
        let mut extensions = ExtensionRegistryBuilder::new();
        extensions.mcp_server_contributor(Arc::new(ExtensionServers(vec![
            McpServerContribution::SetWithProtocolMode {
                name: "extension_apps".to_string(),
                config: Box::new(serde_json::from_value(json!({ "url": extension_url }))?),
                protocol_mode: extension_mode,
            },
        ])));

        let fixture = test_codex()
            .with_extensions(Arc::new(extensions.build()))
            .with_config(move |config| {
                config
                    .features
                    .disable(Feature::Apps)
                    .expect("test config should disable hosted Apps");
                if generic_mode == McpProtocolMode::V20260728 {
                    config
                        .features
                        .enable(Feature::Mcp20260728)
                        .expect("test config should enable generic MCP protocol");
                } else {
                    config
                        .features
                        .disable(Feature::Mcp20260728)
                        .expect("test config should disable generic MCP protocol");
                }
                let third_party: McpServerConfig =
                    serde_json::from_value(json!({ "url": third_party_url }))
                        .expect("third-party MCP config");
                config
                    .mcp_servers
                    .set(HashMap::from([("third_party".to_string(), third_party)]))
                    .expect("test config should accept MCP server");
            })
            .build_with_auto_env(&responses_server)
            .await?;

        wait_for_mcp_server(&fixture.codex, "extension_apps").await?;
        let legacy = vec!["initialize", "notifications/initialized", "tools/list"];
        let modern = vec![
            "server/discover",
            "initialize",
            "notifications/initialized",
            "tools/list",
        ];
        assert_eq!(
            mcp_methods(&extension_server).await,
            if extension_mode == McpProtocolMode::V20260728 {
                modern.clone()
            } else {
                legacy.clone()
            }
        );
        assert_eq!(
            mcp_methods(&third_party_server).await,
            if generic_mode == McpProtocolMode::V20260728 {
                modern
            } else {
                legacy
            }
        );
    }

    Ok(())
}

struct InvocationMetadata;

impl McpToolRequestMetaProvider for InvocationMetadata {
    fn value<'a>(
        &'a self,
        thread_id: ThreadId,
        turn_id: &'a str,
        call_id: &'a str,
    ) -> ExtensionFuture<'a, anyhow::Result<Value>> {
        Box::pin(async move {
            anyhow::ensure!(call_id == "prepared", "cannot prepare call {call_id}");
            Ok(json!([thread_id, turn_id, call_id]))
        })
    }
}

enum HookOutcome {
    Applies,
    Skipped,
    KeyConflict,
}

#[test_case("apps_protocol_test", "approved", HookOutcome::Applies; "matching registration")]
#[test_case("other_contributor", "approved", HookOutcome::Skipped; "different contributor")]
#[test_case("apps_protocol_test", "other", HookOutcome::Skipped; "different routing header")]
#[test_case("apps_protocol_test", "approved", HookOutcome::KeyConflict; "existing metadata key")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn request_metadata_is_scoped_to_the_selected_tool_and_registration(
    contributor: &str,
    target: &str,
    outcome: HookOutcome,
) -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let model = responses::start_mock_server().await;
    let gateway = responses::start_mock_server().await;
    let tools = ["prepare", "read"].map(|name| {
        json!({
            "name": name,
            "inputSchema": {"type": "object"},
            "annotations": {"readOnlyHint": true},
        })
    });
    AppsTestServer::mount_with_tools(&gateway, Arc::new(Mutex::new(Vec::from(tools)))).await?;
    let config_for = |target: &str| -> serde_json::Result<McpServerConfig> {
        serde_json::from_value(json!({
            "url": format!("{}/api/codex/ps/mcp", gateway.uri()),
            "http_headers": {"x-test-target": target},
        }))
    };
    // The two registrations differ only by name, as when a gateway hosts several tools.
    let config = config_for("approved")?;
    let servers = ["selected", "other"].map(|name| McpServerContribution::Set {
        name: name.into(),
        config: Box::new(config.clone()),
    });
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.mcp_server_contributor(Arc::new(ExtensionServers(Vec::from(servers))));
    let fixture = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .build_with_auto_env(&model)
        .await?;
    fixture
        .codex
        .thread_extension_data()
        .insert(McpToolRequestMetaHook {
            server_name: "selected".into(),
            source: McpServerSource::Extension {
                id: contributor.into(),
                host_owned_apps: false,
            },
            transport: config_for(target)?.transport,
            tool_name: "prepare".into(),
            key: if matches!(outcome, HookOutcome::KeyConflict) {
                "callId"
            } else {
                "x-test-context"
            }
            .into(),
            provider: Arc::new(InvocationMetadata),
        });
    wait_for_mcp_server(&fixture.codex, "selected").await?;

    let mut events = vec![responses::ev_response_created("calls")];
    events.extend(
        [
            ("prepared", "mcp__selected", "prepare"),
            ("refused", "mcp__selected", "prepare"),
            ("other-tool", "mcp__selected", "read"),
            ("other-server", "mcp__other", "prepare"),
        ]
        .map(|(id, ns, tool)| responses::ev_function_call_with_namespace(id, ns, tool, "{}")),
    );
    events.push(responses::ev_completed("calls"));
    let mock = responses::mount_sse_sequence(
        &model,
        vec![
            responses::sse(events),
            responses::sse(vec![
                responses::ev_assistant_message("done", "done"),
                responses::ev_completed("end"),
            ]),
        ],
    )
    .await;
    fixture.submit_turn("Call the tools").await?;

    let calls = recorded_apps_tool_calls(&gateway).await;
    let mut dispatched: Vec<_> = calls
        .iter()
        .map(|call| {
            let meta = &call["params"]["_meta"];
            (
                meta["callId"].as_str().expect("dispatch call ID"),
                meta.get("x-test-context").cloned(),
            )
        })
        .collect();
    dispatched.sort_unstable_by_key(|(id, _)| *id);
    let mut expected = vec![("other-server", None), ("other-tool", None)];
    let request = mock.last_request().expect("tool outputs");
    let error = match outcome {
        HookOutcome::Applies => {
            let body = request.body_json();
            let turn_id = body["client_metadata"]["turn_id"]
                .as_str()
                .expect("model turn ID");
            expected.push((
                "prepared",
                Some(json!([
                    fixture.session_configured.thread_id,
                    turn_id,
                    "prepared"
                ])),
            ));
            Some("cannot prepare call refused")
        }
        HookOutcome::Skipped => {
            expected.extend([("prepared", None), ("refused", None)]);
            None
        }
        HookOutcome::KeyConflict => Some("MCP request metadata already contains callId"),
    };
    if let Some(message) = error {
        let item = request.function_call_output("refused");
        let body: FunctionCallOutputBody = serde_json::from_value(item["output"].clone())?;
        let output = body.to_text().expect("tool error");
        assert!(output.contains(message), "{output}");
    }
    assert_eq!(dispatched, expected);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn hosted_apps_protocol_override_preserves_native_verification() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let responses_server = responses::start_mock_server().await;
    let apps_server = responses::start_mock_server().await;
    Mock::given(method("POST"))
        .and(path("/api/codex/ps/mcp"))
        .respond_with(|request: &Request| {
            let body: Value = request.body_json().unwrap();
            let result = match body["method"].as_str() {
                Some("server/discover") => json!({
                    "resultType": "complete",
                    "supportedVersions": ["2026-07-28"],
                    "capabilities": {"tools": {}},
                    "_meta": {"io.modelcontextprotocol/serverInfo": {
                        "name": "hosted-verification", "version": "1.0.0"
                    }},
                    "ttlMs": 0, "cacheScope": "private"
                }),
                Some("tools/list") => json!({
                    "resultType": "complete",
                    "tools": [{"name": "verify_action", "inputSchema": {"type": "object"}}]
                }),
                Some("tools/call") if body.pointer("/params/inputResponses").is_none() => {
                    assert_eq!(
                        body["params"]["_meta"]["io.modelcontextprotocol/clientCapabilities"]
                            ["extensions"][OPENAI_ELICITATION_EXTENSION_ID],
                        json!({"userVerification": {}})
                    );
                    json!({
                        "resultType": "input_required", "content": [], "isError": false,
                        "inputRequests": {"verification": {
                            "method": "openai/elicitation/create",
                            "params": {
                                "mode": "openai/userVerification", "title": "Verify action",
                                "description": "Verify the requested operation", "challenge": "AQID"
                            }
                        }},
                        "requestState": "verification-state"
                    })
                }
                Some("tools/call") => {
                    assert_eq!(body["params"]["requestState"], "verification-state");
                    assert_eq!(body["params"]["inputResponses"], json!({
                        "verification": {"action": "accept", "content": {
                            "credentialId": "AQID", "signature": "BAUG"
                        }}
                    }));
                    json!({
                        "resultType": "complete",
                        "content": [{"type": "text", "text": "verified"}], "isError": false
                    })
                }
                other => panic!("unexpected MCP method: {other:?}"),
            };
            ResponseTemplate::new(200).set_body_json(json!({
                "jsonrpc": "2.0", "id": body["id"], "result": result
            }))
        })
        .mount(&apps_server)
        .await;
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.mcp_server_contributor(Arc::new(ExtensionServers(vec![
        McpServerContribution::HostedApps {
            config: Box::new(serde_json::from_value(json!({
                "url": format!("{}/api/codex/ps/mcp", apps_server.uri())
            }))?),
            protocol_mode: Some(McpProtocolMode::V20260728),
        },
    ])));
    let fixture = apps_enabled_builder(apps_server.uri())
        .with_extensions(Arc::new(extensions.build()))
        .with_config(|config| {
            config.features.disable(Feature::Mcp20260728).unwrap();
        })
        .build_with_auto_env(&responses_server)
        .await?;
    let thread = fixture
        .thread_manager
        .start_thread(StartThreadOptions {
            client_mcp_extensions: ClientMcpExtensions::new([(
                OPENAI_ELICITATION_EXTENSION_ID.to_string(),
                json!({"userVerification": {}}),
            )]),
            environments: Some(vec![fixture.executor_environment().request()]),
            ..StartThreadOptions::new(fixture.config.clone())
        })
        .await?
        .thread;
    let caller = Arc::clone(&thread);
    let call = AbortOnDropHandle::new(tokio::spawn(async move {
        caller
            .call_mcp_tool(
                CODEX_APPS_MCP_SERVER_NAME,
                /*environment_id*/ None,
                "verify_action",
                /*arguments*/ None,
                /*meta*/ None,
            )
            .await
    }));
    let EventMsg::ElicitationRequest(request) = wait_for_event(&thread, |event| {
        matches!(event, EventMsg::ElicitationRequest(_))
    })
    .await
    else {
        unreachable!()
    };
    assert_eq!(request.server_name, CODEX_APPS_MCP_SERVER_NAME);
    assert_eq!(
        request.request,
        ElicitationRequest::UserVerification {
            meta: None,
            title: "Verify action".into(),
            description: "Verify the requested operation".into(),
            challenge: "AQID".into(),
        }
    );
    assert!(!call.is_finished());
    thread
        .submit(Op::ResolveElicitation {
            server_name: request.server_name,
            request_id: request.id,
            decision: ElicitationAction::Accept,
            content: Some(json!({"credentialId": "AQID", "signature": "BAUG"})),
            meta: None,
        })
        .await?;
    assert_eq!(
        timeout(Duration::from_secs(/*secs*/ 10), call).await???,
        codex_protocol::mcp::CallToolResult {
            content: vec![json!({"type": "text", "text": "verified"})],
            structured_content: None,
            is_error: Some(false),
            meta: None,
        },
    );
    thread.shutdown_and_wait().await?;
    fixture.codex.shutdown_and_wait().await?;
    Ok(())
}

const ACTOR_HEADER: &str = "x-test-mcp-actor";

struct RequestAuth(Mutex<Option<(&'static str, &'static str)>>);

impl RequestAuth {
    fn identity(&self) -> Result<(&'static str, &'static str), AuthError> {
        self.0
            .lock()
            .expect("test auth lock should be available")
            .ok_or_else(|| AuthError::Build("test owner is unavailable".to_owned()))
    }
}

impl McpRequestAuth for RequestAuth {
    fn owner_key(&self) -> Result<String, AuthError> {
        Ok(self.identity()?.0.to_owned())
    }

    fn add_auth_headers(&self, headers: &mut HeaderMap) -> Result<String, AuthError> {
        let (owner, actor) = self.identity()?;
        headers.insert(ACTOR_HEADER, HeaderValue::from_static(actor));
        Ok(owner.to_owned())
    }
}

async fn mount_session_mcp(server: &MockServer) {
    let sessions = AtomicUsize::new(0);
    Mock::given(method("GET"))
        .and(path("/mcp"))
        .respond_with(ResponseTemplate::new(405))
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(move |request: &Request| {
            let body: Value = request.body_json().expect("expected an MCP JSON body");
            if body.get("id").is_none() {
                return ResponseTemplate::new(202);
            }
            let mut response = ResponseTemplate::new(200);
            let result = match body["method"].as_str().expect("expected an MCP method") {
                "initialize" => {
                    let session = sessions.fetch_add(1, Ordering::Relaxed) + 1;
                    response =
                        response.insert_header("mcp-session-id", format!("session-{session}"));
                    json!({
                        "protocolVersion": body["params"]["protocolVersion"],
                        "capabilities": {"tools": {}, "resources": {}},
                        "serverInfo": {"name": "owned-tools", "version": "1"}
                    })
                }
                "tools/list" => {
                    let actor = request
                        .headers
                        .get(ACTOR_HEADER)
                        .and_then(|value| value.to_str().ok())
                        .unwrap_or("public");
                    json!({"tools": [{
                        "name": "check", "description": format!("Visible to {actor}"),
                        "inputSchema": {"type": "object"}
                    }]})
                }
                "tools/call" => json!({"content": [{"type": "text", "text": "ok"}]}),
                "resources/read" => json!({"contents": [{"uri": "test://owner", "text": "ok"}]}),
                other => panic!("unexpected MCP method: {other}"),
            };
            response.set_body_json(json!({"jsonrpc": "2.0", "id": body["id"], "result": result}))
        })
        .mount(server)
        .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_request_auth_rejects_chatgpt_auth_on_untrusted_origins() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let model = responses::start_mock_server().await;
    let endpoint = responses::start_mock_server().await;
    let config = serde_json::from_value(json!({
        "url": format!("{}/mcp", endpoint.uri()),
        "auth": "chatgpt",
    }))?;
    let provider = Arc::new(RequestAuth(Mutex::new(Some(("alice", "alice")))));
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.mcp_server_contributor(Arc::new(ExtensionServers(vec![
        McpServerContribution::SetWithAuthProvider {
            name: "owned".into(),
            config: Box::new(config),
            auth_provider: McpRequestAuthProvider::new(provider),
        },
    ])));
    let fixture = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .build_with_auto_env(&model)
        .await?;

    let startup = wait_for_mcp_server(&fixture.codex, "owned").await;
    let requests = endpoint.received_requests().await.expect("record requests");
    assert_eq!(requests.len(), 0);
    let error = startup.expect_err("conflicting auth settings must fail MCP startup");
    assert!(
        error.to_string().contains("no other credentials"),
        "{error}"
    );
    Ok(())
}

/// A scoped endpoint can be configured again without regaining a previous endpoint's auth.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_request_auth_stays_dropped_after_transport_rebuilds() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let model = responses::start_mock_server().await;
    let destination = responses::start_mock_server().await;
    mount_session_mcp(&destination).await;
    let fixture = test_codex().build_with_auto_env(&model).await?;
    let server = "owned";
    let original: McpServerConfig = serde_json::from_value(json!({
        "url": format!("{}/original", destination.uri())
    }))?;
    let replacement: McpServerConfig = serde_json::from_value(json!({
        "url": format!("{}/mcp", destination.uri())
    }))?;
    let mut builder = ResolvedMcpCatalog::builder();
    builder.set_exposure(McpServerExposure::EnvironmentQualified);
    builder.register(
        McpServerRegistration::from_extension(
            server.into(),
            "test",
            /*contribution_order*/ 0,
            original,
        )
        .with_environment_scope()
        .with_request_auth_provider(McpRequestAuthProvider::new(Arc::new(RequestAuth(
            Mutex::new(Some(("original-owner", "original-owner"))),
        )))),
    );
    let mut catalog = builder.build();
    let replacements = HashMap::from([(server.to_owned(), replacement)]);
    catalog = catalog.with_materialized_servers(replacements.clone());
    assert!(
        !catalog
            .server_by_name(server)
            .expect("expected the rebuilt registration")
            .config()
            .enabled
    );
    catalog = catalog.with_materialized_servers(replacements);
    let (config, context) = fixture.codex.current_mcp_config_and_runtime_context().await;
    let mut config = (*config).clone();
    config.mcp_server_catalog = catalog;
    let manager = fixture.codex.mcp_manager();
    let resource = codex_mcp::read_mcp_resource(
        &config,
        /*auth*/ None,
        /*runtime_auth*/ None,
        context,
        manager.codex_apps_tools_cache(),
        manager.tool_catalog_cache(),
        server,
        /*environment_id*/ None,
        ReadResourceRequestParams::new("test://owner"),
    )
    .await?;
    assert_eq!(resource.contents.len(), 1);
    let requests = destination
        .received_requests()
        .await
        .expect("expected captured MCP requests");
    assert!(!requests.is_empty());
    assert!(
        requests
            .iter()
            .all(|request| !request.headers.contains_key(ACTOR_HEADER))
    );
    Ok(())
}

/// A chat with different credentials cannot see tools cached by an earlier chat during startup.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_request_auth_skips_the_shared_startup_catalog() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let model = responses::start_mock_server().await;
    let endpoint = responses::start_mock_server().await;
    let (mcp, startup) = AppsTestServer::mount_with_startup_control(&endpoint).await?;
    let config = serde_json::from_value(json!({
        "url": format!("{}/api/codex/ps/mcp", mcp.chatgpt_base_url),
        "enabled_tools": ["calendar_create_event"],
        "startup_readiness": "catalog",
        "startup_timeout_sec": 60
    }))?;
    let provider = Arc::new(RequestAuth(Mutex::new(Some(("alice", "alice")))));
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.mcp_server_contributor(Arc::new(ExtensionServers(vec![
        McpServerContribution::SetWithAuthProvider {
            name: "owned".into(),
            config: Box::new(config),
            auth_provider: McpRequestAuthProvider::new(provider.clone()),
        },
    ])));
    let response = responses::mount_sse_once(
        &model,
        responses::sse(vec![
            responses::ev_response_created("resp"),
            responses::ev_assistant_message("msg", "done"),
            responses::ev_completed("resp"),
        ]),
    )
    .await;
    let fixture = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_model_info_override("gpt-5.4", |model| model.supports_search_tool = false)
        .with_config(|config| {
            config.mcp_optional_startup_grace = Duration::from_millis(50);
            config
                .features
                .disable(Feature::Apps)
                .expect("disable built-in Apps");
            config
                .features
                .disable(Feature::Mcp20260728)
                .expect("use the fixture’s legacy MCP protocol");
        })
        .build_with_auto_env(&model)
        .await?;
    wait_for_mcp_server(&fixture.codex, "owned").await?;
    let release_bob = startup.hold_next_successful_initialize();
    *provider
        .0
        .lock()
        .expect("test auth lock should be available") = Some(("bob", "bob"));
    let bob = fixture
        .thread_manager
        .start_thread(fixture.start_thread_options().await)
        .await?
        .thread;
    timeout(Duration::from_secs(5), async {
        while startup.initialize_attempts() < 2 {
            tokio::task::yield_now().await;
        }
    })
    .await?;
    bob.start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
        text: "Show available tools".into(),
        text_elements: Vec::new(),
    }]))
    .await?;
    timeout(
        Duration::from_secs(5),
        wait_for_event(&bob, |event| matches!(event, EventMsg::TurnComplete(_))),
    )
    .await?;
    assert!(
        responses::namespace_child_tool(
            &response.single_request().body_json(),
            "mcp__owned",
            "calendar_create_event",
        )
        .is_none()
    );
    release_bob
        .send(())
        .expect("Bob’s initialization should still be pending");
    let requests = endpoint
        .received_requests()
        .await
        .expect("expected captured MCP requests");
    assert!(requests.iter().any(|request| {
        request
            .headers
            .get(ACTOR_HEADER)
            .is_some_and(|value| value == "alice")
    }));
    assert!(requests.iter().any(|request| {
        request
            .headers
            .get(ACTOR_HEADER)
            .is_some_and(|value| value == "bob")
    }));
    bob.shutdown_and_wait().await?;
    Ok(())
}

/// Session reuse follows the owner, and an unavailable owner cannot stall the thread.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_request_auth_session_follows_owner_key() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let model = responses::start_mock_server().await;
    let endpoint = responses::start_mock_server().await;
    mount_session_mcp(&endpoint).await;
    let config = serde_json::from_value(json!({"url": format!("{}/mcp", endpoint.uri())}))?;
    let provider = Arc::new(RequestAuth(Mutex::new(None)));
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.mcp_server_contributor(Arc::new(ExtensionServers(vec![
        McpServerContribution::SetWithAuthProvider {
            name: "owned".into(),
            config: Box::new(config),
            auth_provider: McpRequestAuthProvider::new(provider.clone()),
        },
    ])));
    let fixture = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_config(|config| {
            config
                .features
                .disable(Feature::Apps)
                .expect("disable built-in Apps");
            config
                .features
                .disable(Feature::Mcp20260728)
                .expect("use the fixture’s legacy MCP protocol");
        })
        .build_with_auto_env(&model)
        .await?;

    let response = responses::mount_sse_once(
        &model,
        responses::sse(vec![
            responses::ev_response_created("resp"),
            responses::ev_assistant_message("msg", "done"),
            responses::ev_completed("resp"),
        ]),
    )
    .await;
    fixture
        .codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Continue while MCP is unavailable".into(),
            text_elements: Vec::new(),
        }]))
        .await?;
    timeout(
        Duration::from_secs(5),
        wait_for_event(&fixture.codex, |event| {
            matches!(event, EventMsg::TurnComplete(_))
        }),
    )
    .await?;
    assert!(
        responses::namespace_child_tool(
            &response.single_request().body_json(),
            "mcp__owned",
            "check",
        )
        .is_none()
    );

    let server = McpServerKey::unscoped("owned");
    for identities in [
        &[None][..],
        &[Some(("alice", "alice"))],
        &[Some(("alice", "alice-refreshed"))],
        &[Some(("bob", "bob")), Some(("alice", "alice-returned"))],
        &[Some(("bob", "bob"))],
        &[None],
        &[Some(("bob", "bob-recovered"))],
    ] {
        for &identity in identities {
            *provider
                .0
                .lock()
                .expect("test auth lock should be available") = identity;
        }
        if matches!(identities.last(), Some(None)) {
            let (_, snapshot) = timeout(
                Duration::from_secs(5),
                fixture.codex.mcp_server_status_snapshot(
                    "owned",
                    /*environment_id*/ None,
                    McpSnapshotDetail::ToolsAndAuthOnly,
                    /*expected_registration*/ None,
                ),
            )
            .await??;
            assert!(snapshot.tools_errors[&server].contains("test owner is unavailable"));
            assert!(!snapshot.tools_by_server.contains_key(&server));
            continue;
        }

        fixture
            .codex
            .call_mcp_tool(
                "owned", /*environment_id*/ None, "check", /*arguments*/ None,
                /*meta*/ None,
            )
            .await?;
    }

    let requests = endpoint
        .received_requests()
        .await
        .expect("expected captured MCP requests");
    let calls = requests
        .iter()
        .filter_map(|request| {
            let body: Value = request.body_json().ok()?;
            (body["method"] == "tools/call").then(|| {
                (
                    request.headers[ACTOR_HEADER]
                        .to_str()
                        .expect("expected an ASCII test header"),
                    request.headers["mcp-session-id"]
                        .to_str()
                        .expect("expected an ASCII test header"),
                )
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(
        calls,
        vec![
            ("alice", "session-1"),
            ("alice-refreshed", "session-1"),
            ("alice-returned", "session-1"),
            ("bob", "session-2"),
            ("bob-recovered", "session-3"),
        ]
    );
    let discovered = requests
        .iter()
        .filter_map(|request| {
            let body: Value = request.body_json().ok()?;
            (body["method"] == "tools/list").then(|| {
                request.headers[ACTOR_HEADER]
                    .to_str()
                    .expect("expected an ASCII test header")
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(discovered, vec!["alice", "bob", "bob-recovered"]);
    Ok(())
}

/// The status list can reuse a public catalog, while an account switch requires fresh auth discovery.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_status_list_rediscovers_tools_for_the_current_request_auth_owner() -> anyhow::Result<()>
{
    skip_if_no_network!(Ok(()));
    let model = responses::start_mock_server().await;
    let endpoint = responses::start_mock_server().await;
    mount_session_mcp(&endpoint).await;
    let url = format!("{}/mcp", endpoint.uri());
    let config = serde_json::from_value(json!({"url": url}))?;
    let public_config = serde_json::from_value(json!({
        "url": url,
        "http_headers": {"Authorization": "Bearer public-fixture"}
    }))?;
    let provider = Arc::new(RequestAuth(Mutex::new(Some(("alice", "alice")))));
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.mcp_server_contributor(Arc::new(ExtensionServers(vec![
        McpServerContribution::SetWithAuthProvider {
            name: "owned".into(),
            config: Box::new(config),
            auth_provider: McpRequestAuthProvider::new(provider.clone()),
        },
        McpServerContribution::Set {
            name: "public".into(),
            config: Box::new(public_config),
        },
    ])));
    let fixture = test_codex()
        .with_extensions(Arc::new(extensions.build()))
        .with_config(|config| {
            config
                .features
                .disable(Feature::Apps)
                .expect("disable built-in Apps");
            config
                .features
                .disable(Feature::Mcp20260728)
                .expect("use the fixture’s legacy MCP protocol");
        })
        .build_with_auto_env(&model)
        .await?;

    let owned = McpServerKey::unscoped("owned");
    let public = McpServerKey::unscoped("public");
    for (server, description) in [(&owned, "Visible to alice"), (&public, "Visible to public")] {
        let (_, snapshot) = fixture
            .codex
            .mcp_server_status_snapshot(
                server.name(),
                /*environment_id*/ None,
                McpSnapshotDetail::ToolsAndAuthOnly,
                /*expected_registration*/ None,
            )
            .await?;
        assert_eq!(
            snapshot.tools_by_server[server]["check"]
                .description
                .as_deref(),
            Some(description)
        );
    }

    *provider
        .0
        .lock()
        .expect("test auth lock should be available") = Some(("bob", "bob"));
    let config = fixture.codex.config().await;
    let (status_config, context) = fixture.codex.mcp_status_config_and_context(&config).await;
    let manager = fixture.codex.mcp_manager();
    let auth_manager = manager.auth_manager();
    let auth = auth_manager.auth().await;
    let page = HashSet::from([owned.clone(), public.clone()]);
    let mut snapshot = fixture
        .codex
        .matching_mcp_status_snapshot(&status_config, auth.as_ref(), &page, &context)
        .await;
    assert_eq!(snapshot.server_keys, HashSet::from([public.clone()]));

    let mut remaining = codex_mcp::effective_mcp_servers(&status_config, auth.as_ref());
    remaining.retain(|key, _| page.contains(key) && !snapshot.server_keys.contains(key));
    snapshot.extend(
        codex_mcp::collect_mcp_server_status_snapshot_with_detail(
            &status_config,
            auth.as_ref(),
            Some(&auth_manager),
            /*runtime_auth*/ None,
            context,
            manager.codex_apps_tools_cache(),
            manager.tool_catalog_cache(),
            McpSnapshotDetail::ToolsAndAuthOnly,
            remaining,
        )
        .await,
    );

    assert_eq!(
        snapshot.tools_by_server[&public]["check"]
            .description
            .as_deref(),
        Some("Visible to public")
    );
    assert_eq!(
        snapshot.tools_by_server[&owned]["check"]
            .description
            .as_deref(),
        Some("Visible to bob")
    );
    Ok(())
}
