//! Enterprise authentication and revocation preserve ordinary tools and accumulated history.

use anyhow::Result;
use codex_config::test_support::CloudConfigBundleFixture;
use codex_login::CodexAuth;
use codex_protocol::openai_models::ToolMode;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::McpStartupStatus;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event_match;
use core_test_support::wait_for_event_with_timeout;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn enterprise_failure_preserves_ordinary_mcp_history() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let mut histories = Vec::new();
    for enabled in [false, true] {
        let server = responses::start_mock_server().await;
        let mcp = responses::start_mock_server().await;
        AppsTestServer::mount(&mcp).await?;
        let origin = mcp.uri();
        let policy = format!(
            r#"
[features]
use_xaa = {enabled}
apps = false
secret_auth_storage = false
[mcp_enterprise_managed_auth.idp]
issuer = "{origin}/idp"
client_id = "enterprise-client"
[mcp_servers.enterprise]
url = "{origin}/enterprise"
auth = "ema_auth"
[mcp_servers.ordinary]
url = "{origin}/api/codex/ps/mcp"
http_headers = {{ Authorization = "Bearer ordinary-access" }}
"#
        );
        let test = test_codex()
            .with_auth(CodexAuth::from_api_key("test-api-key"))
            .with_cloud_config_bundle(CloudConfigBundleFixture::loader_with_enterprise_config(
                policy,
            ))
            .with_config(super::configure_scenario_catalog)
            .with_model_info_override("gpt-6-astra", |model| {
                model.tool_mode = Some(ToolMode::Direct);
                model.use_responses_lite = false;
                model.supports_search_tool = false;
            })
            .build_with_auto_env(&server)
            .await?;
        if enabled {
            let error = wait_for_event_match(&test.codex, |event| match event {
                EventMsg::McpStartupUpdate(update) if update.server == "enterprise" => {
                    match &update.status {
                        McpStartupStatus::Failed { error, .. } => Some(error.clone()),
                        _ => None,
                    }
                }
                _ => None,
            })
            .await;
            insta::assert_snapshot!("enterprise_sign_in_required", error);
        }
        // Allow MCP's 30-second startup deadline before checking the result.
        let startup = wait_for_event_with_timeout(
            &test.codex,
            |event| matches!(event, EventMsg::McpStartupComplete(_)),
            std::time::Duration::from_secs(35),
        )
        .await;
        let EventMsg::McpStartupComplete(startup) = startup else {
            unreachable!("waited for MCP startup completion");
        };
        assert_eq!(startup.ready, ["ordinary"]);
        assert_eq!(
            startup
                .failed
                .into_iter()
                .map(|failure| failure.server)
                .collect::<Vec<_>>(),
            if enabled {
                vec!["enterprise"]
            } else {
                Vec::new()
            }
        );
        let mock = responses::mount_sse_sequence(
            &server,
            vec![
                responses::sse(vec![
                    responses::ev_function_call_with_namespace(
                        "calendar-call",
                        "mcp__ordinary",
                        "calendar_list_events",
                        "{}",
                    ),
                    responses::ev_completed("call-response"),
                ]),
                responses::sse(vec![
                    responses::ev_assistant_message("answer", "Here are your calendar events."),
                    responses::ev_completed("answer-response"),
                ]),
            ],
        )
        .await;
        test.submit_text_turn("List my calendar events.").await?;
        let requests = mock.requests();
        assert_eq!(requests.len(), 2);
        histories.push(context_snapshot::format_request_history_snapshot(
            "Ordinary MCP keeps the same tool schemas, calls, and history when enterprise authentication fails.",
            &requests,
            &ContextSnapshotOptions::default().rewrite_known_segments().include_request_settings(),
        ));
        assert!(
            mcp.received_requests()
                .await
                .expect("requests")
                .iter()
                .all(|request| {
                    !request.url.path().contains("enterprise")
                        && !request.url.path().contains("idp")
                })
        );
        test.codex.shutdown_and_wait().await?;
    }
    assert_eq!(histories[0], histories[1]);
    insta::assert_snapshot!(
        "enterprise_failure_preserves_ordinary_mcp_history",
        histories[1]
    );
    Ok(())
}

#[derive(Clone, Copy)]
enum EnterpriseLifecycle {
    Policy,
    Removal,
    RefreshPermissions,
}

#[test_case::test_case(EnterpriseLifecycle::Policy; "policy")]
#[test_case::test_case(EnterpriseLifecycle::Removal; "removal")]
#[test_case::test_case(EnterpriseLifecycle::RefreshPermissions; "permission_refresh")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn enterprise_lifecycle_preserves_history_and_ordinary_tools(
    scenario: EnterpriseLifecycle,
) -> Result<()> {
    use base64::Engine;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use codex_http_client::HttpClientFactory;
    use codex_http_client::OutboundProxyPolicy;
    use codex_login::AuthCredentialsStoreMode;
    use codex_login::AuthKeyringBackendKind;
    use codex_login::AuthManager;
    use codex_login::AuthRouteConfig;
    use serde_json::json;
    use std::sync::Arc;
    use wiremock::Mock;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let mcp = responses::start_mock_server().await;
    AppsTestServer::mount(&mcp).await?;
    if matches!(scenario, EnterpriseLifecycle::RefreshPermissions) {
        // Keep the enterprise client on the protocol that carries input requests.
        Mock::given(method("POST"))
            .and(path("/api/codex/ps/mcp"))
            .and(wiremock::matchers::header("authorization", "Bearer enterprise-access"))
            .and(wiremock::matchers::body_partial_json(json!({"method": "server/discover"})))
            .respond_with(|request: &wiremock::Request| {
                let body: serde_json::Value = request.body_json().expect("discovery request");
                ResponseTemplate::new(200).set_body_json(json!({
                    "jsonrpc": "2.0", "id": body["id"], "result": {
                        "resultType": "complete", "supportedVersions": ["2026-07-28"],
                        "capabilities": {"tools": {}}, "ttlMs": 0, "cacheScope": "private",
                        "_meta": {"io.modelcontextprotocol/serverInfo": {"name": "enterprise", "version": "1"}}
                    }
                }))
            })
            .with_priority(1)
            .mount(&mcp)
            .await;
    }
    let origin = mcp.uri();
    let resource = format!("{origin}/api/codex/ps/mcp");
    let issuer = format!("{origin}/idp");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    for (endpoint, metadata) in [
        (
            "/.well-known/oauth-protected-resource/api/codex/ps/mcp",
            json!({
                "resource": resource, "authorization_servers": [format!("{origin}/as")],
            }),
        ),
        (
            "/.well-known/oauth-authorization-server/as",
            json!({
                "issuer": format!("{origin}/as"), "token_endpoint": format!("{origin}/as/token"),
                "grant_types_supported": ["urn:ietf:params:oauth:grant-type:jwt-bearer"],
                "token_endpoint_auth_methods_supported": ["none"],
            }),
        ),
        (
            "/.well-known/oauth-authorization-server/idp",
            json!({
                "issuer": issuer, "token_endpoint": format!("{issuer}/token"),
                "authorization_endpoint": format!("{issuer}/authorize"),
                "token_endpoint_auth_methods_supported": ["none"],
            }),
        ),
    ] {
        Mock::given(method("GET"))
            .and(path(endpoint))
            .respond_with(ResponseTemplate::new(200).set_body_json(metadata))
            .with_priority(1)
            .mount(&mcp)
            .await;
    }
    let assertion = format!(
        "{}.{}.c2lnbmF0dXJl",
        URL_SAFE_NO_PAD.encode(r#"{"alg":"ES256","typ":"oauth-id-jag+jwt"}"#),
        URL_SAFE_NO_PAD.encode(
            json!({
                "iss": issuer, "aud": format!("{origin}/as"), "sub": "user",
                "client_id": "mcp-client", "jti": "grant", "iat": now,
                "exp": now + 3600, "resource": resource,
            })
            .to_string()
        )
    );
    Mock::given(method("POST")).and(path("/idp/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": assertion, "issued_token_type": "urn:ietf:params:oauth:token-type:id-jag",
            "token_type": "N_A", "resource": resource,
        }))).expect(1).mount(&mcp).await;
    Mock::given(method("POST"))
        .and(path("/as/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "enterprise-access", "token_type": "Bearer", "expires_in": 3600,
        })))
        .expect(1)
        .mount(&mcp)
        .await;
    let home = Arc::new(tempfile::TempDir::new()?);
    let id_token = format!("{}.{}.c2lnbmF0dXJl",
        URL_SAFE_NO_PAD.encode(r#"{"alg":"ES256"}"#),
        URL_SAFE_NO_PAD.encode(json!({
            "iss": issuer, "sub": "user", "aud": "account-client", "exp": now + 3600,
            "https://api.openai.com/auth": {"chatgpt_user_id":"user","chatgpt_account_id":"workspace","chatgpt_plan_type":"business"},
        }).to_string()));
    std::fs::write(
        home.path().join("auth.json"),
        serde_json::to_vec(&json!({
            "auth_mode": "chatgpt", "last_refresh": chrono::Utc::now(),
            "tokens": {"id_token": id_token, "access_token":"account-access", "refresh_token":"account-refresh",
                "account_id":"workspace", "login_owner_id":"completed-login",
                "oauth_provenance":{"issuer":issuer,"client_id":"account-client"}},
        }))?,
    )?;
    let auth = AuthManager::shared(
        home.path().to_path_buf(),
        /*enable_codex_api_key_env*/ false,
        AuthCredentialsStoreMode::File,
        /*forced_chatgpt_workspace_id*/ None,
        /*chatgpt_base_url*/ None,
        AuthKeyringBackendKind::default(),
        AuthRouteConfig::from_http_client_factory(HttpClientFactory::new(
            OutboundProxyPolicy::ReqwestDefault,
        )),
    )
    .await;
    let policy = format!(
        r#"
[features]
use_xaa = true
apps = false
secret_auth_storage = false
[mcp_enterprise_managed_auth.idp]
issuer = "{issuer}"
client_id = "enterprise-client"
[mcp_servers.enterprise]
url = "{resource}"
auth = "ema_auth"
oauth = {{ client_id = "mcp-client", authorization_server_issuer = "{origin}/as" }}
[mcp_servers.ordinary]
url = "{resource}"
http_headers = {{ Authorization = "Bearer ordinary-access" }}
"#
    );
    let delivered_policy = codex_config::CloudConfigBundlePolicy::default();
    let mut snapshot = codex_config::CloudConfigBundleSnapshot {
        bundle: Ok(Some(
            CloudConfigBundleFixture::enterprise_config(policy).into_bundle(),
        )),
        binding: None,
        config_server: None,
    };
    delivered_policy.publish_snapshot(&mut snapshot);
    let test = test_codex()
        .with_home(home)
        .with_auth_manager(auth)
        .with_cloud_config_bundle(codex_config::CloudConfigBundleLoader::from_snapshot(
            snapshot,
        ))
        .with_config(move |config| {
            super::configure_scenario_catalog(config);
            if matches!(scenario, EnterpriseLifecycle::RefreshPermissions) {
                config.permissions.approval_policy = codex_config::Constrained::allow_any(
                    codex_protocol::protocol::AskForApproval::Never,
                );
                config
                    .permissions
                    .set_permission_profile(codex_protocol::models::PermissionProfile::Disabled)
                    .expect("full access");
                config.approvals_reviewer = codex_protocol::config_types::ApprovalsReviewer::User;
                config
                    .features
                    .enable(codex_features::Feature::ToolCallMcpElicitation)
                    .expect("MCP approval prompts");
            }
        })
        .with_model_info_override("gpt-6-astra", |model| {
            model.tool_mode = Some(ToolMode::Direct);
            model.use_responses_lite = false;
            model.supports_search_tool = false;
        })
        .build_with_auto_env(&server)
        .await?;
    let startup = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::McpStartupComplete(summary) => Some(summary.clone()),
        _ => None,
    })
    .await;
    assert!(startup.failed.is_empty(), "{startup:?}");
    if matches!(scenario, EnterpriseLifecycle::RefreshPermissions) {
        use codex_protocol::protocol::AskForApproval;
        use codex_protocol::protocol::ElicitationAction;
        use codex_protocol::protocol::Op;
        use codex_protocol::protocol::ThreadSettingsOverrides;
        use core_test_support::wait_for_event;

        // Force a new manager while still in full access. A queued settings update
        // is a barrier before status reads flush the hard refresh.
        let requests_before_refresh = mcp.received_requests().await.expect("MCP requests").len();
        test.codex.submit(Op::RefreshMcpServers).await?;
        test.codex
            .update_thread_settings(ThreadSettingsOverrides::default())
            .await?;
        test.codex
            .mcp_server_status_snapshot(
                "ordinary",
                codex_mcp::McpSnapshotDetail::ToolsAndAuthOnly,
                /*expected_registration*/ None,
            )
            .await?;
        let discovery_requests = mcp
            .received_requests()
            .await
            .expect("MCP requests")
            .into_iter()
            .skip(requests_before_refresh)
            .filter(|request| {
                request.body_json::<serde_json::Value>().is_ok_and(|body| {
                    matches!(
                        body["method"].as_str(),
                        Some("initialize" | "server/discover" | "tools/list")
                    )
                })
            })
            .map(|request| {
                request
                    .headers
                    .get("authorization")
                    .and_then(|header| header.to_str().ok())
                    .map(str::to_owned)
            })
            .collect::<Vec<_>>();
        assert!(
            !discovery_requests.is_empty(),
            "hard refresh must reconnect ordinary MCP"
        );
        assert!(
            discovery_requests
                .iter()
                .all(|authorization| authorization.as_deref() == Some("Bearer ordinary-access")),
            "hard refresh must retain EMA without rediscovery or relisting: {discovery_requests:?}"
        );
        test.codex
            .update_thread_settings(ThreadSettingsOverrides {
                approval_policy: Some(AskForApproval::OnRequest),
                ..Default::default()
            })
            .await?;

        Mock::given(method("POST"))
            .and(path("/api/codex/ps/mcp"))
            .and(wiremock::matchers::header("authorization", "Bearer enterprise-access"))
            .and(wiremock::matchers::body_partial_json(json!({
                "method": "tools/call", "params": {"arguments": {"query": "permission"}}
            })))
            .respond_with(|request: &wiremock::Request| {
                let body: serde_json::Value = request.body_json().expect("MCP request");
                let result = if body["params"].get("inputResponses").is_none() {
                    json!({
                        "resultType": "input_required", "content": [], "isError": false,
                        "inputRequests": {"approval": {"method": "elicitation/create", "params": {
                            "mode": "form", "message": "Approve the calendar action",
                            "requestedSchema": {"type": "object", "properties": {}},
                            "_meta": {"codex_request_type": "approval_request", "codex_approval_kind": "mcp_tool_call",
                                "codex_sensitive_action": true, "tool_name": "calendar_list_events", "tool_params": {}}
                        }}}, "requestState": "awaiting-approval"
                    })
                } else {
                    assert_eq!(body["params"]["inputResponses"]["approval"]["action"], "accept");
                    json!({"resultType": "complete", "content": [{"type": "text", "text": "approved"}], "isError": false})
                };
                ResponseTemplate::new(200).set_body_json(json!({"jsonrpc": "2.0", "id": body["id"], "result": result}))
            })
            .with_priority(1)
            .expect(2)
            .mount(&mcp)
            .await;
        let mut call = Box::pin(test.codex.call_mcp_tool(
            "enterprise",
            "calendar_list_events",
            Some(json!({"query": "permission"})),
            /*meta*/ None,
        ));
        let request = tokio::select! {
            result = &mut call => anyhow::bail!("EMA bypassed the updated approval policy: {result:?}"),
            request = wait_for_event_match(&test.codex, |event| match event {
                EventMsg::ElicitationRequest(request) => Some(request.clone()),
                _ => None,
            }) => request,
        };
        assert_eq!(request.server_name, "enterprise");
        test.codex
            .submit(Op::ResolveElicitation {
                server_name: request.server_name,
                request_id: request.id,
                decision: ElicitationAction::Accept,
                content: Some(json!({})),
                meta: None,
            })
            .await?;
        assert_eq!(call.await?.is_error, Some(false));

        // Tool policy changes retain this EMA authority, while replacing its tool view.
        let current = test.codex.config().await;
        let mut next = current.as_ref().clone();
        let mut servers = next.mcp_servers.get().clone();
        let enterprise = servers.get_mut("enterprise").expect("enterprise server");
        enterprise.disabled_tools = Some(vec!["calendar_create_event".into()]);
        enterprise.tools.insert(
            "calendar_list_events".into(),
            codex_config::McpServerToolConfig {
                approval_mode: Some(codex_config::AppToolApproval::Prompt),
                ..Default::default()
            },
        );
        next.mcp_servers.set(servers)?;
        assert_eq!(
            test.codex.refresh_mcp_config(current, next).await,
            codex_core::ConfigRefreshOutcome::Published
        );
        let error = test
            .codex
            .call_mcp_tool(
                "enterprise",
                "calendar_create_event",
                /*arguments*/ None,
                /*meta*/ None,
            )
            .await
            .expect_err("the disabled EMA tool must not dispatch");
        assert_eq!(
            error.to_string(),
            "tool 'calendar_create_event' is disabled for MCP server 'enterprise'"
        );

        // A read-only tool now explicitly requires approval. Missing updated metadata
        // would let this turn complete without asking.
        let mock = responses::mount_sse_sequence(
            &server,
            vec![
                responses::sse(vec![
                    responses::ev_function_call_with_namespace(
                        "restricted-call",
                        "mcp__enterprise",
                        "calendar_list_events",
                        "{}",
                    ),
                    responses::ev_completed("restricted-response"),
                ]),
                responses::sse(vec![
                    responses::ev_assistant_message("done", "Cancelled."),
                    responses::ev_completed("done-response"),
                ]),
            ],
        )
        .await;
        test.codex
            .start_or_steer_turn(codex_core::TurnInputRequest::user_input(vec![
                codex_protocol::user_input::UserInput::Text {
                    text: "List my work events.".into(),
                    text_elements: Vec::new(),
                },
            ]))
            .await?;
        let event = wait_for_event(&test.codex, |event| {
            matches!(
                event,
                EventMsg::ElicitationRequest(_) | EventMsg::TurnComplete(_)
            )
        })
        .await;
        let EventMsg::ElicitationRequest(request) = event else {
            anyhow::bail!("EMA ignored the refreshed per-tool approval setting");
        };
        assert_eq!(request.server_name, "enterprise");
        assert_eq!(mock.requests().len(), 1, "the tool must wait for approval");
        let request_to_model = mock.single_request();
        assert_eq!(
            (
                request_to_model
                    .tool_by_name("mcp__enterprise", "calendar_create_event")
                    .is_some(),
                request_to_model
                    .tool_by_name("mcp__enterprise", "calendar_list_events")
                    .is_some(),
                request_to_model
                    .tool_by_name("mcp__ordinary", "calendar_create_event")
                    .is_some(),
            ),
            (false, true, true),
        );
        test.codex
            .submit(Op::ResolveElicitation {
                server_name: request.server_name,
                request_id: request.id,
                decision: ElicitationAction::Decline,
                content: None,
                meta: None,
            })
            .await?;
        wait_for_event(&test.codex, |event| {
            matches!(event, EventMsg::TurnComplete(_))
        })
        .await;
        let ordinary = test
            .codex
            .call_mcp_tool(
                "ordinary",
                "calendar_create_event",
                Some(json!({"title": "control", "starts_at": "2026-06-18T12:00:00Z"})),
                /*meta*/ None,
            )
            .await?;
        assert_eq!(ordinary.is_error, Some(false));
        let calls = core_test_support::apps_test_server::recorded_apps_tool_calls(&mcp).await;
        assert_eq!(
            calls
                .iter()
                .map(|call| call["params"]["name"].as_str().expect("tool name"))
                .collect::<Vec<_>>(),
            [
                "calendar_list_events",
                "calendar_list_events",
                "calendar_create_event"
            ]
        );
        test.codex.shutdown_and_wait().await?;
        return Ok(());
    }
    let mock = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_function_call_with_namespace(
                    "enterprise-call",
                    "mcp__enterprise",
                    "calendar_list_events",
                    "{}",
                ),
                responses::ev_completed("enterprise-response"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("answer-1", "Here are your work events."),
                responses::ev_completed("answer-1-response"),
            ]),
            responses::sse(vec![
                responses::ev_function_call_with_namespace(
                    "ordinary-call",
                    "mcp__ordinary",
                    "calendar_list_events",
                    "{}",
                ),
                responses::ev_completed("ordinary-response"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("answer-2", "Here are your other events."),
                responses::ev_completed("answer-2-response"),
            ]),
        ],
    )
    .await;
    test.submit_text_turn("List my work events.").await?;
    let pending_request = Mock::given(method("POST"))
        .and(path("/api/codex/ps/mcp"))
        .and(wiremock::matchers::body_partial_json(json!({
            "method": "tools/call", "params": {"arguments": {"query": "blocked"}}
        })))
        .respond_with(ResponseTemplate::new(200).set_delay(std::time::Duration::from_secs(60)))
        .with_priority(1)
        .expect(1)
        .mount_as_scoped(&mcp)
        .await;
    let thread = Arc::clone(&test.codex);
    let request_started = std::time::Instant::now();
    let pending = tokio::spawn(async move {
        thread
            .call_mcp_tool(
                "enterprise",
                "calendar_list_events",
                Some(json!({"query":"blocked"})),
                /*meta*/ None,
            )
            .await
    });
    pending_request.wait_until_satisfied().await;
    match scenario {
        EnterpriseLifecycle::Policy => {
            let denied =
                CloudConfigBundleFixture::enterprise_requirement("[features]\nuse_xaa = false\n")
                    .into_bundle();
            delivered_policy.observe_remote_bundle(&denied);
            let mut snapshot = codex_config::CloudConfigBundleSnapshot {
                bundle: Ok(Some(denied)),
                binding: None,
                config_server: None,
            };
            delivered_policy.publish_snapshot(&mut snapshot);
        }
        EnterpriseLifecycle::Removal => {
            // Archive/delete removes the registry entry before asynchronous shutdown.
            // A retained caller must lose EMA access at removal, even before shutdown.
            let removed = test
                .thread_manager
                .remove_thread_for_client(&test.session_configured.thread_id)
                .await?
                .expect("registered thread");
            assert!(Arc::ptr_eq(&removed, &test.codex));
            let result = removed
                .call_mcp_tool(
                    "enterprise",
                    "calendar_list_events",
                    /*arguments*/ None,
                    /*meta*/ None,
                )
                .await?;
            assert_eq!(serde_json::to_value(result)?["isError"], true);
        }
        EnterpriseLifecycle::RefreshPermissions => unreachable!("handled before revocation"),
    }
    // No config reload or shutdown: the existing owner cancels admitted work.
    let result = serde_json::to_value(pending.await??)?;
    assert!(
        request_started.elapsed() < std::time::Duration::from_secs(10),
        "revocation must finish while the 60-second response is still pending"
    );
    assert_eq!(result["isError"], true);
    assert_eq!(
        result["_meta"]["mcp/www_authenticate"],
        "Bearer error=\"invalid_token\""
    );
    // The pending response is already cancelled. Apply the config update before
    // building the next turn's tool catalog, preserving the original history check.
    test.codex.disable_mcp_enterprise_auth().await;
    test.submit_text_turn("List my other calendar events.")
        .await?;
    let requests = mock.requests();
    assert_eq!(requests.len(), 4);
    let calls = mcp
        .received_requests()
        .await
        .expect("MCP requests")
        .into_iter()
        .filter(|request| {
            serde_json::from_slice::<serde_json::Value>(&request.body)
                .is_ok_and(|body| body["method"] == "tools/call")
        })
        .map(|request| {
            request.headers["authorization"]
                .to_str()
                .expect("valid authorization header")
                .to_owned()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        calls,
        [
            "Bearer enterprise-access",
            "Bearer enterprise-access",
            "Bearer ordinary-access"
        ]
    );
    insta::assert_snapshot!(
        "enterprise_revocation_preserves_history_and_ordinary_tools",
        context_snapshot::format_request_history_snapshot(
            "Enterprise access disappears after revocation. Prior calls remain in history and ordinary tools still work.",
            &requests,
            &ContextSnapshotOptions::default()
                .rewrite_known_segments()
                .include_request_settings(),
        )
    );
    test.codex.shutdown_and_wait().await?;
    Ok(())
}
