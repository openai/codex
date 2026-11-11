//! Same-named executor MCP servers retain separate tools, resources, calls, and approvals.

use super::*;
use anyhow::Context;
use codex_config::Constrained;
use codex_protocol::approvals::ElicitationAction;
use codex_protocol::approvals::ElicitationRequest;
use codex_protocol::capabilities::CapabilityRootLocation;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::models::PermissionProfile;
use codex_protocol::models::PermissionProfileSnapshot;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::protocol::TurnEnvironmentRequest;
use codex_protocol::protocol::TurnEnvironmentRequests;
use core_test_support::apps_test_server::AppsTestServer;
use core_test_support::apps_test_server::recorded_apps_tool_calls;
use core_test_support::responses::ev_function_call;
use pretty_assertions::assert_eq;
use serde_json::Value;
use std::process::Stdio;
use std::sync::Mutex;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::Command;
use wiremock::Mock;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_partial_json;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn same_named_executor_mcps_keep_calls_and_approvals_separate() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let first_endpoint = start_mock_server().await;
    let second_endpoint = start_mock_server().await;
    let mut extensions = ExtensionRegistryBuilder::new();
    codex_mcp_extension::install_plugins(
        &mut extensions,
        Arc::new(codex_exec_server::EnvironmentManager::default_for_tests()),
    );
    let test = test_codex()
        .with_exec_server_url("none")
        .with_model("gpt-5.5")
        .with_extensions(Arc::new(extensions.build()))
        .with_config(|config| {
            // Review B's restrictions without reading project files under those restrictions.
            config.project_doc_max_bytes = 0;
            configure_scenario_catalog(config);
            config
                .features
                .enable(Feature::MultiEnvironmentMcp)
                .unwrap();
            config
                .features
                .enable(Feature::ToolCallMcpElicitation)
                .unwrap();
            config.features.enable(Feature::Mcp20260728).unwrap();
            config.features.enable(Feature::Plugins).unwrap();
            config
                .features
                .enable(Feature::ExecutorCapabilityDiscovery)
                .unwrap();
            config
                .features
                .disable(Feature::ExecutorCapabilityDiscoveryV2)
                .unwrap();
            config.approvals_reviewer = ApprovalsReviewer::User;
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config
                .permissions
                .set_permission_profile(PermissionProfile::Disabled)
                .unwrap();
        })
        .with_model_info_override("gpt-5.5", |model| {
            model.tool_mode = Some(ToolMode::Direct);
            model.supports_search_tool = false;
            model.use_responses_lite = false;
            // Catalog schemas written before multi-environment resources still gain the selector.
            model
                .model_messages
                .as_mut()
                .expect("model messages")
                .tools
                .get_or_insert_with(Default::default)
                .mcp_resources = Some(
                serde_json::from_value(json!({"read_mcp_resource": {"parameters": json!({
                    "type": "object",
                    "properties": {"server": {"type": "string"}, "uri": {"type": "string"}},
                    "required": ["server", "uri"], "additionalProperties": false
                }).to_string()}}))
                .expect("resource catalog schema"),
            );
        })
        // Both named executors below run on this host with their own configuration.
        .build(&server)
        .await?;

    let mut executors = Vec::new();
    let mut environments = Vec::new();
    for (id, endpoint) in [("env-a", &first_endpoint), ("env-b", &second_endpoint)] {
        AppsTestServer::mount_with_tools(endpoint, Arc::new(Mutex::new(vec![json!({
            "name": "save", "description": "Save a document.",
            "inputSchema": {"type": "object", "properties": {"title": {"type": "string"}}, "required": ["title"]},
            "annotations": {"readOnlyHint": false, "destructiveHint": true}
        })]))).await?;
        for (method_name, result) in [
            (
                "resources/list",
                json!({"resources": [{"uri": "memo://shared", "name": "Shared note"}]}),
            ),
            (
                "resources/templates/list",
                json!({"resourceTemplates": [{"uriTemplate": "memo://{name}", "name": "Note"}]}),
            ),
            (
                "resources/read",
                json!({"contents": [{"uri": "memo://shared", "text": format!("Note from {id}")}]}),
            ),
        ] {
            Mock::given(method("POST"))
                .and(path("/api/codex/ps/mcp"))
                .and(body_partial_json(json!({"method": method_name})))
                .respond_with(move |request: &wiremock::Request| {
                    let body: Value = serde_json::from_slice(&request.body).unwrap();
                    let mut result = result.clone();
                    result["resultType"] = json!("complete");
                    ResponseTemplate::new(200).set_body_json(json!({
                        "jsonrpc": "2.0", "id": body["id"], "result": result,
                    }))
                })
                .with_priority(1)
                .mount(endpoint)
                .await;
        }
        let home = tempfile::tempdir()?;
        fs::write(
            home.path().join("config.toml"),
            format!(
                "[mcp_servers.docs]\nurl = \"{}/api/codex/ps/mcp\"\ndefault_tools_approval_mode = \"auto\"\nrequired = true\n",
                endpoint.uri(),
            ),
        )?;
        // Keep config-backed docs and add a separate plugin-backed server on each executor.
        let plugin_root = home.path().join("notes");
        fs::create_dir_all(plugin_root.join(".codex-plugin"))?;
        fs::write(
            plugin_root.join(".codex-plugin/plugin.json"),
            json!({"name": "notes", "description": "Save notes on this computer."}).to_string(),
        )?;
        fs::write(
            plugin_root.join(".mcp.json"),
            json!({"mcpServers": {"notes": {
                "url": format!("{}/api/codex/ps/mcp", endpoint.uri()),
                "default_tools_approval_mode": "approve", "required": true
            }}})
            .to_string(),
        )?;
        let mut executor = Command::from(codex_utils_process::background_command(
            codex_utils_cargo_bin::cargo_bin("codex")?,
        ))
        .args(["exec-server", "--listen", "ws://127.0.0.1:0"])
        .env("CODEX_HOME", home.path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
        let mut stdout = BufReader::new(executor.stdout.take().context("executor stdout")?).lines();
        let url = stdout.next_line().await?.context("executor listen URL")?;
        test.thread_manager
            .environment_manager()
            .upsert_environment(id.to_string(), url, /*connect_timeout*/ None)?;
        let cwd = PathUri::from_host_native_path(home.path())?;
        let mut request = TurnEnvironmentRequest {
            environment_id: id.to_string(),
            cwd: cwd.clone(),
            workspace_roots: vec![cwd],
            config: EnvironmentConfigState::FromThread,
        };
        let mut config = environment_config_for_selection(
            &test.config,
            &TurnEnvironmentSelection::new(request.clone(), &[]),
        );
        config.selected_capability_roots = vec![SelectedCapabilityRoot {
            id: format!("{id}:notes"),
            location: CapabilityRootLocation::Environment {
                environment_id: id.to_string(),
                path: PathUri::from_host_native_path(plugin_root)?,
            },
        }];
        if id == "env-b" {
            // Keep B distinct from unrestricted A without deny-read rules, which prevent
            // plugin discovery under Windows' unelevated restricted-token sandbox.
            config.permission_profile =
                PermissionProfileSnapshot::legacy(PermissionProfile::from_runtime_permissions(
                    &FileSystemSandboxPolicy::read_only(),
                    NetworkSandboxPolicy::Enabled,
                ));
        }
        request.config = EnvironmentConfigState::Ready(config);
        environments.push(request);
        executors.push((home, executor, stdout));
    }

    // B also asks for approval inside its call. Use the existing HTTP input-request
    // protocol so the test does not need a second kind of MCP server.
    Mock::given(method("POST"))
        .and(path("/api/codex/ps/mcp"))
        .and(body_partial_json(json!({"method": "server/discover"})))
        .respond_with(|request: &wiremock::Request| {
            let body: serde_json::Value = request.body_json().unwrap();
            ResponseTemplate::new(200).set_body_json(json!({
                "jsonrpc": "2.0", "id": body["id"], "result": {
                    "resultType": "complete", "supportedVersions": ["2026-07-28"],
                "capabilities": {"tools": {}, "resources": {}}, "ttlMs": 0, "cacheScope": "private",
                    "_meta": {"io.modelcontextprotocol/serverInfo": {"name": "docs", "version": "1"}}
                }
            }))
        })
        .with_priority(1)
        .mount(&second_endpoint)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/codex/ps/mcp"))
        .and(body_partial_json(json!({"method": "tools/call"})))
        .respond_with(|request: &wiremock::Request| {
            let body: serde_json::Value = request.body_json().unwrap();
            let result = if body["params"]["arguments"]["title"] == "plugin-second" {
                json!({"resultType": "complete", "content": [{"type": "text", "text": "Saved plugin-second."}]})
            } else if body["params"].get("inputResponses").is_none() {
                json!({
                    "resultType": "input_required", "content": [], "isError": false,
                    "inputRequests": {"approval": {"method": "elicitation/create", "params": {
                        "mode": "form", "message": "Approve saving this document?",
                        "requestedSchema": {"type": "object", "properties": {}},
                        "_meta": {"codex_request_type": "approval_request",
                            "codex_approval_kind": "mcp_tool_call", "tool_name": "save",
                            "tool_params": body["params"]["arguments"],
                            "codex_strict_auto_review": true, "codex_sensitive_action": true,
                            "callId": body["params"]["_meta"]["callId"]}
                    }}}, "requestState": "awaiting-approval"
                })
            } else {
                json!({"resultType": "complete", "content": [{"type": "text", "text": "Server action denied."}], "isError": true})
            };
            ResponseTemplate::new(200).set_body_json(json!({"jsonrpc": "2.0", "id": body["id"], "result": result}))
        })
        .with_priority(1)
        .expect(3)
        .mount(&second_endpoint)
        .await;

    // The stable environment discriminator comes before the shared server name.
    let first_namespace = "mcp__74afdff5f493__docs";
    let second_namespace = "mcp__14afef09931d__docs";
    let first_plugin_namespace = "mcp__74afdff5f493__notes";
    let second_plugin_namespace = "mcp__14afef09931d__notes";
    let mut responses = Vec::new();
    for (id, namespace) in [
        ("first", first_namespace),
        ("first-again", first_namespace),
        ("second", second_namespace),
    ] {
        responses.push(sse(vec![
            ev_function_call_with_namespace(
                id,
                namespace,
                "save",
                &json!({"title": id}).to_string(),
            ),
            ev_completed(id),
        ]));
    }
    responses.push(sse(vec![
        ev_assistant_message("review", r#"{"outcome":"deny"}"#),
        ev_completed("review"),
    ]));
    for (id, namespace) in [
        ("plugin-first", first_plugin_namespace),
        ("plugin-second", second_plugin_namespace),
    ] {
        responses.push(sse(vec![
            ev_function_call_with_namespace(
                id,
                namespace,
                "save",
                &json!({"title": id}).to_string(),
            ),
            ev_completed(id),
        ]));
    }
    let mut resource_calls = Vec::new();
    for (id, name, arguments) in [
        ("resources", "list_mcp_resources", json!({})),
        ("templates", "list_mcp_resource_templates", json!({})),
        (
            "read-b",
            "read_mcp_resource",
            json!({"server": "docs", "environment_id": "env-b", "uri": "memo://shared"}),
        ),
        (
            "read-ambiguous",
            "read_mcp_resource",
            json!({"server": "docs", "uri": "memo://shared"}),
        ),
    ] {
        resource_calls.push(ev_function_call(id, name, &arguments.to_string()));
    }
    resource_calls.push(ev_completed("resources"));
    responses.push(sse(resource_calls));
    responses.push(sse(vec![ev_completed("done")]));
    let mock = mount_sse_sequence(&server, responses).await;
    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![text(
                "Save twice on the first computer and once on the second, then save a note on each. List resources and templates, then read the second computer's shared note.",
            )])
            .with_thread_settings(ThreadSettingsOverrides {
                environments: Some(TurnEnvironmentRequests::new(
                    test.config.cwd.clone(),
                    environments,
                )),
                ..Default::default()
            }),
        )
        .await?;

    // Remembering A skips its second prompt, but must not approve B's same-named tool.
    let mut approval_messages = Vec::new();
    for expected_first_calls in [0, 2] {
        let event = wait_for_event(&test.codex, |event| {
            matches!(
                event,
                EventMsg::ElicitationRequest(_) | EventMsg::TurnComplete(_)
            )
        })
        .await;
        let EventMsg::ElicitationRequest(request) = event else {
            panic!("each environment must request its own approval");
        };
        let ElicitationRequest::Form { message, .. } = &request.request else {
            panic!("expected a tool approval form");
        };
        approval_messages.push(message.clone());
        assert_eq!(
            recorded_apps_tool_calls(&first_endpoint).await.len(),
            expected_first_calls
        );
        assert!(recorded_apps_tool_calls(&second_endpoint).await.is_empty());
        test.codex
            .submit(Op::ResolveElicitation {
                server_name: request.server_name,
                request_id: request.id,
                decision: ElicitationAction::Accept,
                content: None,
                meta: Some(json!({"persist": "session"})),
            })
            .await?;
    }
    let event = wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::ElicitationRequest(_) | EventMsg::TurnComplete(_)
        )
    })
    .await;
    assert!(matches!(event, EventMsg::TurnComplete(_)));
    for (endpoint, titles, plugin_id) in [
        (
            &first_endpoint,
            vec!["first", "first-again", "plugin-first"],
            "env-a:notes",
        ),
        (
            &second_endpoint,
            vec!["second", "second", "plugin-second"],
            "env-b:notes",
        ),
    ] {
        let calls = recorded_apps_tool_calls(endpoint).await;
        assert_eq!(calls[2]["params"]["_meta"]["plugin_id"], plugin_id);
        assert_eq!(
            calls
                .iter()
                .map(|call| call["params"]["arguments"]["title"].as_str().unwrap())
                .collect::<Vec<_>>(),
            titles
        );
    }
    assert_eq!(
        recorded_apps_tool_calls(&second_endpoint).await[1]["params"]["inputResponses"]["approval"]
            ["action"],
        "decline"
    );
    let requests = mock.requests();
    let history = context_snapshot::format_request_history_snapshot(
        "Two computers expose config-backed docs and plugin-backed notes; tools, resources, approvals, and reviews use their owning computer. Identical resource URIs stay unchanged and require an environment to select one.",
        &requests,
        &ContextSnapshotOptions::default()
            .rewrite_known_segments()
            .include_request_settings(),
    );
    let history = regex_lite::Regex::new(r"Wall time: [0-9]+(?:\.[0-9]+)? seconds")?
        .replace_all(&history, "Wall time: <DURATION> seconds");
    insta::assert_snapshot!(format!(
        "Approval prompts\n{}\n\n{history}",
        approval_messages.join("\n")
    ));
    test.codex.shutdown_and_wait().await?;
    Ok(())
}
