use anyhow::Context;
use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::create_final_assistant_message_sse_response;
use app_test_support::create_request_permissions_sse_response;
use app_test_support::write_models_cache_with_models;
use codex_app_server_protocol::AdditionalPermissionProfile;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::CommandExecutionApprovalDecision;
use codex_app_server_protocol::CommandExecutionRequestApprovalResponse;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::PermissionGrantScope;
use codex_app_server_protocol::PermissionsRequestApprovalResponse;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ServerRequestResolvedNotification;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnCompletedNotification;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput as V2UserInput;
use codex_exec_server::ReadFileOptions;
use codex_features::Feature;
use codex_models_manager::model_info::model_info_from_slug;
use codex_protocol::models::FileSystemPermissions;
use codex_protocol::request_permissions::PermissionGrantScope as CorePermissionGrantScope;
use codex_protocol::request_permissions::RequestPermissionProfile;
use codex_protocol::request_permissions::RequestPermissionsResponse;
use codex_utils_path_uri::LegacyAppPathString;
use codex_utils_path_uri::PathUri;
use core_test_support::responses;
use core_test_support::skip_if_wine_exec;
use pretty_assertions::assert_eq;
use serde_json::json;
use test_case::test_case;
use tokio::time::timeout;

const DEFAULT_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// An awaited permission approval applies to the next command in the same code-mode cell.
/// Strict review must also apply immediately, even to a command needing no additional access.
#[test_case(PermissionGrantScope::Turn, false; "turn")]
#[test_case(PermissionGrantScope::Session, false; "session")]
#[test_case(PermissionGrantScope::Turn, true; "strict_review")]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn code_mode_uses_permissions_approved_in_the_same_cell(
    scope: PermissionGrantScope,
    strict_auto_review: bool,
) -> Result<()> {
    skip_if_wine_exec!(
        Ok(()),
        "Wine does not emulate Windows restricted-token and ACL sandbox semantics"
    );
    let codex_home = tempfile::TempDir::new()?;
    let server = responses::start_mock_server().await;
    MockResponsesConfig::new(&server.uri())
        .with_model("gpt-5.4")
        .with_approval_policy("on-request")
        .with_sandbox_mode("read-only")
        .with_root_config(r#"approvals_reviewer = "user""#)
        .enable_feature(Feature::RequestPermissionsTool)
        .enable_feature(Feature::CodeModeOnly)
        .enable_feature(Feature::GuardianApproval)
        .write(codex_home.path())?;
    write_models_cache_with_models(codex_home.path(), vec![model_info_from_slug("gpt-5.4")])
        .await?;
    let mut app = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;
    let cwd = app.auto_env()?.selection().cwd.clone();
    let fs = app.auto_env()?.environment().get_filesystem();
    let ThreadStartResponse { thread, .. } = app.start_thread(ThreadStartParams::default()).await?;

    let command = if strict_auto_review {
        "echo same-cell-review"
    } else {
        "echo same-cell-grant > granted.txt"
    };
    let mut events = vec![responses::sse(vec![
        responses::ev_response_created("same-cell"),
        responses::ev_custom_tool_call(
            "same-cell",
            "exec",
            &format!(
                r#"// @exec: {{"yield_time_ms": 60000}}
await tools.request_permissions({{permissions: {{file_system: {{write: ["."]}}}}}});
text(await tools.exec_command({{cmd: "{command}"}}));"#,
            ),
        ),
        responses::ev_completed("same-cell"),
    ])];
    if strict_auto_review {
        events.push(responses::sse(vec![
            responses::ev_response_created("guardian-review"),
            responses::ev_assistant_message(
                "guardian-review",
                r#"{"outcome":"deny","rationale":"same-cell command requires review"}"#,
            ),
            responses::ev_completed("guardian-review"),
        ]));
    }
    events.push(create_final_assistant_message_sse_response("done")?);
    let mock = responses::mount_sse_sequence(&server, events).await;
    let TurnStartResponse { turn } = app
        .request(|request_id| ClientRequest::TurnStart {
            request_id,
            params: TurnStartParams {
                thread_id: thread.id.clone(),
                input: vec![V2UserInput::Text {
                    text: "request permission, then run the command in the same cell".to_string(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            },
        })
        .await?;
    let request = timeout(
        DEFAULT_READ_TIMEOUT,
        app.read_stream_until_request_message(),
    )
    .await??;
    let ServerRequest::PermissionsRequestApproval { request_id, params } = request else {
        anyhow::bail!("expected permission approval, got {request:?}");
    };
    assert_eq!((&params.thread_id, &params.turn_id), (&thread.id, &turn.id));
    app.send_response(
        request_id,
        json!({
            "permissions": params.permissions,
            "scope": scope,
            "strictAutoReview": strict_auto_review,
        }),
    )
    .await?;
    let mut command_approved = false;
    loop {
        match timeout(DEFAULT_READ_TIMEOUT, app.read_next_message()).await?? {
            JSONRPCMessage::Request(request)
                if cfg!(windows) && !strict_auto_review && !command_approved =>
            {
                // Without a Windows sandbox backend, command approval is still required.
                // Check the grant before approving so stale same-cell permissions still fail.
                let ServerRequest::CommandExecutionRequestApproval {
                    request_id,
                    params: command_params,
                } = ServerRequest::try_from(request)?
                else {
                    anyhow::bail!("expected Windows command approval");
                };
                assert_eq!(
                    (
                        command_params.thread_id,
                        command_params.turn_id,
                        command_params.environment_id,
                        command_params.cwd,
                        command_params.additional_permissions,
                    ),
                    (
                        thread.id.clone(),
                        turn.id.clone(),
                        params.environment_id.clone(),
                        Some(params.cwd.clone()),
                        Some(AdditionalPermissionProfile {
                            network: params.permissions.network.clone(),
                            file_system: params.permissions.file_system.clone(),
                        }),
                    ),
                );
                app.send_response(
                    request_id,
                    serde_json::to_value(CommandExecutionRequestApprovalResponse {
                        decision: CommandExecutionApprovalDecision::Accept,
                    })?,
                )
                .await?;
                command_approved = true;
            }
            JSONRPCMessage::Request(request) => {
                anyhow::bail!(
                    "the approved command must not ask for another approval: {request:?}"
                );
            }
            JSONRPCMessage::Notification(notification)
                if notification.method == "turn/completed" =>
            {
                let completed: TurnCompletedNotification =
                    serde_json::from_value(notification.params.expect("turn/completed params"))?;
                assert_eq!(
                    (
                        completed.thread_id,
                        completed.turn.id,
                        completed.turn.status
                    ),
                    (thread.id, turn.id, TurnStatus::Completed),
                );
                break;
            }
            _ => {}
        }
    }

    let requests = mock.requests();
    let output = requests
        .last()
        .expect("model continuation")
        .custom_tool_call_output("same-cell");
    if strict_auto_review {
        assert_eq!(
            requests
                .iter()
                .filter(|request| {
                    request.body_json()["client_metadata"]["x-openai-subagent"] == "guardian"
                })
                .count(),
            1,
            "strict review must apply to the next command in the same cell; output: {output}",
        );
        assert!(
            output
                .to_string()
                .contains("same-cell command requires review")
        );
    } else {
        let contents = fs
            .read_file(
                &cwd.join("granted.txt")?,
                ReadFileOptions::default(),
                /*sandbox*/ None,
            )
            .await
            .with_context(|| {
                format!("the approved same-cell command must write the file; output: {output}")
            })?;
        assert_eq!(String::from_utf8(contents)?.trim(), "same-cell-grant");
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn request_permissions_round_trip() -> Result<()> {
    let codex_home = tempfile::TempDir::new()?;
    let project_root_entry = json!({
        "path": {
            "type": "special",
            "value": {"kind": "project_roots", "subpath": "output"}
        },
        "access": "write"
    });
    let responses = vec![
        create_request_permissions_sse_response("call1")?,
        responses::sse(vec![
            responses::ev_response_created("resp-2"),
            responses::ev_function_call(
                "call2",
                "request_permissions",
                &json!({
                    "permissions": {"file_system": {"entries": [project_root_entry]}}
                })
                .to_string(),
            ),
            responses::ev_completed("resp-2"),
        ]),
        create_final_assistant_message_sse_response("done")?,
    ];
    let server = responses::start_mock_server().await;
    let mock = responses::mount_sse_sequence(&server, responses).await;
    MockResponsesConfig::new(&server.uri())
        .with_approval_policy("on-request")
        .enable_feature(Feature::RequestPermissionsTool)
        .write(codex_home.path())?;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;
    let cwd = mcp.auto_env()?.selection().cwd.clone();
    let workspace_root = cwd.parent().expect("test cwd has a parent");
    let mut environment = mcp.auto_env_params()?;
    environment.runtime_workspace_roots = Some(vec![workspace_root.clone().into()]);

    let ThreadStartResponse { thread, .. } = mcp
        .start_thread(ThreadStartParams {
            model: Some("mock-model".to_string()),
            ..Default::default()
        })
        .await?;

    let TurnStartResponse { turn, .. } = mcp
        .request(|request_id| ClientRequest::TurnStart {
            request_id,
            params: TurnStartParams {
                thread_id: thread.id.clone(),
                client_user_message_id: None,
                environments: Some(vec![environment]),
                input: vec![V2UserInput::Text {
                    text: "pick a directory".to_string(),
                    text_elements: Vec::new(),
                }],
                model: Some("mock-model".to_string()),
                ..Default::default()
            },
        })
        .await?;

    let server_req = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_request_message(),
    )
    .await??;
    let ServerRequest::PermissionsRequestApproval { request_id, params } = server_req else {
        panic!("expected PermissionsRequestApproval request, got: {server_req:?}");
    };

    assert_eq!(params.thread_id, thread.id);
    assert_eq!(params.turn_id, turn.id);
    assert_eq!(params.item_id, "call1");
    assert_eq!(params.cwd.as_str(), cwd.inferred_native_path_string());
    let request_cwd: PathUri = params
        .cwd
        .clone()
        .try_into()
        .expect("request cwd should remain target-native");
    assert_eq!(request_cwd, cwd);
    assert_eq!(params.reason, Some("Select a workspace root".to_string()));
    let requested_file_system = params
        .permissions
        .file_system
        .expect("request should include file system permissions");
    let requested_writes = requested_file_system
        .write
        .clone()
        .expect("request should include write permissions");
    assert_eq!(requested_writes.len(), 2);
    assert_eq!(
        requested_file_system.entries,
        Some(vec![
            codex_app_server_protocol::FileSystemSandboxEntry {
                path: codex_app_server_protocol::FileSystemPath::Path {
                    path: requested_writes[0].clone(),
                },
                access: codex_app_server_protocol::FileSystemAccessMode::Write,
            },
            codex_app_server_protocol::FileSystemSandboxEntry {
                path: codex_app_server_protocol::FileSystemPath::Path {
                    path: requested_writes[1].clone(),
                },
                access: codex_app_server_protocol::FileSystemAccessMode::Write,
            },
        ])
    );
    mcp.send_response(
        request_id,
        serde_json::to_value(PermissionsRequestApprovalResponse {
            permissions: codex_app_server_protocol::GrantedPermissionProfile {
                network: None,
                file_system: Some(codex_app_server_protocol::AdditionalFileSystemPermissions {
                    read: None,
                    write: Some(vec![requested_writes[0].clone()]),
                    glob_scan_max_depth: None,
                    entries: None,
                }),
            },
            scope: PermissionGrantScope::Turn,
            strict_auto_review: None,
        })?,
    )
    .await?;

    let server_req = timeout(
        DEFAULT_READ_TIMEOUT,
        mcp.read_stream_until_request_message(),
    )
    .await??;
    let ServerRequest::PermissionsRequestApproval { request_id, params } = server_req else {
        panic!("expected PermissionsRequestApproval request, got: {server_req:?}");
    };
    assert_eq!(params.item_id, "call2");
    let resolved_request_id = request_id.clone();
    let outside_request: LegacyAppPathString = cwd.join("output")?.into();
    mcp.send_response(
        request_id,
        json!({
            "permissions": {"fileSystem": {"entries": [
                project_root_entry,
                {"path": {"type": "path", "path": outside_request}, "access": "write"}
            ]}}
        }),
    )
    .await?;

    let mut saw_resolved = false;
    loop {
        let message = timeout(DEFAULT_READ_TIMEOUT, mcp.read_next_message()).await??;
        let JSONRPCMessage::Notification(notification) = message else {
            continue;
        };
        match notification.method.as_str() {
            "serverRequest/resolved" => {
                let resolved: ServerRequestResolvedNotification = serde_json::from_value(
                    notification
                        .params
                        .clone()
                        .expect("serverRequest/resolved params"),
                )?;
                assert_eq!(resolved.thread_id, thread.id);
                if resolved.request_id == resolved_request_id {
                    saw_resolved = true;
                }
            }
            "turn/completed" => {
                assert!(saw_resolved, "serverRequest/resolved should arrive first");
                break;
            }
            _ => {}
        }
    }

    let (output, _) = mock.requests()[2]
        .function_call_output_content_and_success("call2")
        .expect("permission tool output");
    let response: RequestPermissionsResponse =
        serde_json::from_str(&output.expect("permission response text"))?;
    assert_eq!(
        response,
        RequestPermissionsResponse {
            permissions: RequestPermissionProfile {
                file_system: Some(FileSystemPermissions::from_read_write_path_uris(
                    /*read*/ None,
                    Some(vec![workspace_root.join("output")?]),
                )),
                ..Default::default()
            },
            scope: CorePermissionGrantScope::Turn,
            strict_auto_review: false,
        }
    );

    Ok(())
}
