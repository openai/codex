use anyhow::Result;
use codex_core::windows_sandbox::WindowsSandboxLevelExt;
use codex_exec_server::CreateDirectoryOptions;
use codex_features::Feature;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::PermissionProfile;
use codex_protocol::models::PermissionProfileSnapshot;
use codex_protocol::protocol::EnvironmentConfig;
use codex_protocol::protocol::EnvironmentConfigState;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::protocol::TurnEnvironmentSelections;
use core_test_support::responses::ResponseMock;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call_with_namespace;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once_match;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::submit_thread_settings;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::time::Duration;
use test_case::test_case;

const FIRST_PROMPT: &str = "spawn the first worker";
const FIRST_TASK: &str = "first worker task";
const SECOND_TASK: &str = "second worker task";
const MULTI_AGENT_V2_NAMESPACE: &str = "collaboration";

fn body_contains(request: &wiremock::Request, text: &str) -> bool {
    serde_json::from_slice::<serde_json::Value>(&request.body)
        .is_ok_and(|body| body.to_string().contains(text))
}

fn has_function_call_output(request: &wiremock::Request, call_id: &str) -> bool {
    serde_json::from_slice::<serde_json::Value>(&request.body).is_ok_and(|body| {
        body.get("input")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|items| {
                items.iter().any(|item| {
                    item.get("type").and_then(serde_json::Value::as_str)
                        == Some("function_call_output")
                        && item.get("call_id").and_then(serde_json::Value::as_str) == Some(call_id)
                })
            })
    })
}

async fn mount_root_collaboration_call(
    server: &wiremock::MockServer,
    prompt: &'static str,
    call_id: &'static str,
    tool_name: &'static str,
    arguments: serde_json::Value,
) {
    let response_id = format!("resp-{call_id}");
    mount_sse_once_match(
        server,
        move |request: &wiremock::Request| body_contains(request, prompt),
        sse(vec![
            ev_response_created(&response_id),
            ev_function_call_with_namespace(
                call_id,
                MULTI_AGENT_V2_NAMESPACE,
                tool_name,
                &arguments.to_string(),
            ),
            ev_completed(&response_id),
        ]),
    )
    .await;

    let completion_id = format!("resp-{call_id}-complete");
    mount_sse_once_match(
        server,
        move |request: &wiremock::Request| has_function_call_output(request, call_id),
        sse(vec![
            ev_response_created(&completion_id),
            ev_assistant_message(&format!("msg-{call_id}"), "collaboration completed"),
            ev_completed(&completion_id),
        ]),
    )
    .await;
}

async fn mount_completed_worker(
    server: &wiremock::MockServer,
    task: &'static str,
    parent_call_id: &'static str,
) -> ResponseMock {
    let response_id = format!("resp-worker-{parent_call_id}");
    mount_sse_once_match(
        server,
        move |request: &wiremock::Request| {
            body_contains(request, task) && !has_function_call_output(request, parent_call_id)
        },
        sse(vec![
            ev_response_created(&response_id),
            ev_assistant_message(&format!("msg-worker-{parent_call_id}"), "worker completed"),
            ev_completed(&response_id),
        ]),
    )
    .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn v2_nested_spawn_checks_shared_active_execution_capacity() -> Result<()> {
    let server = start_mock_server().await;
    let first_args = serde_json::to_string(&json!({
        "message": FIRST_TASK,
        "task_name": "first",
    }))?;
    mount_sse_once_match(
        &server,
        |request: &wiremock::Request| body_contains(request, FIRST_PROMPT),
        sse(vec![
            ev_response_created("first-response"),
            ev_function_call_with_namespace(
                "first-call",
                MULTI_AGENT_V2_NAMESPACE,
                "spawn_agent",
                &first_args,
            ),
            ev_completed("first-response"),
        ]),
    )
    .await;
    let second_args = serde_json::to_string(&json!({
        "message": SECOND_TASK,
        "task_name": "second",
    }))?;
    mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            body_contains(request, FIRST_TASK) && !has_function_call_output(request, "first-call")
        },
        sse(vec![
            ev_response_created("first-worker-response"),
            ev_function_call_with_namespace(
                "second-call",
                MULTI_AGENT_V2_NAMESPACE,
                "spawn_agent",
                &second_args,
            ),
            ev_completed("first-worker-response"),
        ]),
    )
    .await;
    let second_followup = mount_sse_once_match(
        &server,
        |request: &wiremock::Request| has_function_call_output(request, "second-call"),
        sse(vec![
            ev_response_created("second-followup-response"),
            ev_assistant_message("second-followup-message", "blocked"),
            ev_completed("second-followup-response"),
        ]),
    )
    .await;
    mount_sse_once_match(
        &server,
        |request: &wiremock::Request| has_function_call_output(request, "first-call"),
        sse(vec![
            ev_response_created("first-followup-response"),
            ev_assistant_message("first-followup-message", "spawned"),
            ev_completed("first-followup-response"),
        ]),
    )
    .await;

    let mut builder = test_codex()
        .with_model("gpt-5.6-sol")
        .with_config(|config| {
            config
                .features
                .enable(Feature::Collab)
                .expect("test config should allow feature update");
            config
                .features
                .enable(Feature::MultiAgentV2)
                .expect("test config should allow feature update");
            config.multi_agent_v2.max_concurrent_threads_per_session = 2;
        });
    let test = builder.build(&server).await?;
    test.submit_turn(FIRST_PROMPT).await?;

    let second_output = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Some(output) = second_followup.function_call_output_text("second-call") {
                return output;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;
    assert_eq!(
        second_output,
        "collab spawn failed: agent thread limit reached"
    );
    assert_eq!(test.thread_manager.list_thread_ids().await.len(), 2);

    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ResidencyReload {
    Sender,
    OwnerNarrowsPermissions,
    OwnerPreservesStricterChild,
    OwnerRevokesWorkspaceRoot,
}

#[test_case(ResidencyReload::Sender; "sender preserves stricter child")]
#[test_case(ResidencyReload::OwnerNarrowsPermissions; "owner narrows cached permissions")]
#[test_case(ResidencyReload::OwnerPreservesStricterChild; "owner preserves stricter child")]
#[test_case(ResidencyReload::OwnerRevokesWorkspaceRoot; "owner revokes cached workspace root")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn v2_residency_reload_preserves_inherited_environment_and_tools(
    reload: ResidencyReload,
) -> Result<()> {
    const EVICT_PROMPT: &str = "spawn the replacement worker";
    const FOLLOWUP_PROMPT: &str = "continue the original worker";
    const FOLLOWUP_TASK: &str = "continue work in the original environment";

    let server = start_mock_server().await;
    mount_root_collaboration_call(
        &server,
        FIRST_PROMPT,
        "first-call",
        "spawn_agent",
        json!({ "message": FIRST_TASK, "task_name": "first", "fork_turns": "none" }),
    )
    .await;
    mount_completed_worker(&server, FIRST_TASK, "first-call").await;

    mount_root_collaboration_call(
        &server,
        EVICT_PROMPT,
        "replacement-call",
        "spawn_agent",
        json!({ "message": SECOND_TASK, "task_name": "replacement", "fork_turns": "none" }),
    )
    .await;
    mount_completed_worker(&server, SECOND_TASK, "replacement-call").await;

    mount_root_collaboration_call(
        &server,
        FOLLOWUP_PROMPT,
        "followup-call",
        "followup_task",
        json!({ "target": "first", "message": FOLLOWUP_TASK }),
    )
    .await;
    let reloaded_worker_request =
        mount_completed_worker(&server, FOLLOWUP_TASK, "followup-call").await;

    let mut builder = test_codex()
        .with_model("gpt-5.6-sol")
        .with_exec_server_url("none")
        .with_config(|config| {
            for feature in [Feature::Collab, Feature::MultiAgentV2] {
                config
                    .features
                    .enable(feature)
                    .expect("test config should allow feature update");
            }
            config.multi_agent_v2.max_concurrent_threads_per_session = 2;
            config
                .permissions
                .set_permission_profile(PermissionProfile::workspace_write())
                .expect("thread permissions should allow workspace writes");
        });
    let test = builder.build_with_remote_and_local_env(&server).await?;
    let mut child_environment = test.executor_environment().selection().clone();
    let (child_permissions, parent_permissions) = match reload {
        ResidencyReload::Sender => (
            PermissionProfile::read_only(),
            PermissionProfile::workspace_write(),
        ),
        ResidencyReload::OwnerNarrowsPermissions => {
            (PermissionProfile::Disabled, PermissionProfile::read_only())
        }
        ResidencyReload::OwnerPreservesStricterChild => {
            (PermissionProfile::read_only(), PermissionProfile::Disabled)
        }
        ResidencyReload::OwnerRevokesWorkspaceRoot => (
            PermissionProfile::workspace_write(),
            PermissionProfile::workspace_write(),
        ),
    };
    if reload == ResidencyReload::OwnerRevokesWorkspaceRoot {
        child_environment.cwd = test.workspace_path_uri("retained")?;
        child_environment.workspace_roots = vec![
            child_environment.cwd.clone(),
            test.workspace_path_uri("revoked")?,
        ];
        for root in &child_environment.workspace_roots {
            test.fs()
                .create_directory(
                    root,
                    CreateDirectoryOptions {
                        recursive: true,
                        follow_symlinks: true,
                    },
                    /*sandbox*/ None,
                )
                .await?;
        }
    } else {
        child_environment.config = EnvironmentConfigState::Ready(EnvironmentConfig {
            allow_login_shell: test.config.permissions.allow_login_shell,
            permission_profile: PermissionProfileSnapshot::legacy(child_permissions),
            shell_environment_policy: Default::default(),
            windows_sandbox_level: WindowsSandboxLevel::from_config(&test.config),
            windows_sandbox_private_desktop: test
                .config
                .permissions
                .windows_sandbox_private_desktop,
            use_legacy_landlock: test.config.features.use_legacy_landlock(),
            exec_policy: None,
            mcp_policy: None,
            network_policy: None,
            selected_capability_roots: Vec::new(),
        });
    }
    if let Some(exec_server_url) = test.executor_environment().exec_server_url() {
        test.thread_manager
            .environment_manager()
            .upsert_environment(
                child_environment.environment_id.clone(),
                exec_server_url.to_string(),
                /*connect_timeout*/ None,
            )?;
    }
    let mut created_threads = test.thread_manager.subscribe_thread_created();

    submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            environments: Some(TurnEnvironmentSelections::new(
                test.config.cwd.clone(),
                vec![child_environment.clone()],
            )),
            ..Default::default()
        },
    )
    .await?;
    test.submit_text_turn(FIRST_PROMPT).await?;
    let first_thread_id = created_threads.recv().await?;
    let first_thread = test.thread_manager.get_thread(first_thread_id).await?;
    wait_for_event(first_thread.as_ref(), |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let mut parent_environment = child_environment.clone();
    if reload == ResidencyReload::OwnerRevokesWorkspaceRoot {
        parent_environment.workspace_roots.truncate(1);
    } else {
        let EnvironmentConfigState::Ready(parent_config) = &mut parent_environment.config else {
            unreachable!("child environment config should be ready");
        };
        parent_config.permission_profile = PermissionProfileSnapshot::legacy(parent_permissions);
    }
    if reload == ResidencyReload::Sender {
        submit_thread_settings(
            &test.codex,
            ThreadSettingsOverrides {
                environments: Some(TurnEnvironmentSelections::new(
                    test.config.cwd.clone(),
                    vec![parent_environment.clone()],
                )),
                ..Default::default()
            },
        )
        .await?;
    }
    test.submit_text_turn(EVICT_PROMPT).await?;
    let replacement_thread_id = created_threads.recv().await?;
    let replacement_thread = test
        .thread_manager
        .get_thread(replacement_thread_id)
        .await?;
    wait_for_event(replacement_thread.as_ref(), |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    assert!(
        test.thread_manager
            .get_thread(first_thread_id)
            .await
            .is_err()
    );

    if reload != ResidencyReload::Sender {
        submit_thread_settings(
            &test.codex,
            ThreadSettingsOverrides {
                environments: Some(TurnEnvironmentSelections::new(
                    test.config.cwd.clone(),
                    vec![parent_environment.clone()],
                )),
                ..Default::default()
            },
        )
        .await?;
        assert_eq!(
            test.codex.config_snapshot().await.environments.environments,
            vec![parent_environment]
        );
        let result = test
            .thread_manager
            .ensure_multi_agent_v2_child_loaded(first_thread_id)
            .await;
        let expected_error = match reload {
            ResidencyReload::OwnerRevokesWorkspaceRoot => {
                Some("no longer matches a ready parent environment")
            }
            ResidencyReload::OwnerNarrowsPermissions
            | ResidencyReload::OwnerPreservesStricterChild
                if test.executor_environment().environment().is_remote() =>
            {
                Some("permissions changed on a remote executor")
            }
            ResidencyReload::Sender
            | ResidencyReload::OwnerNarrowsPermissions
            | ResidencyReload::OwnerPreservesStricterChild => None,
        };
        if let Some(expected_error) = expected_error {
            let error = result.expect_err("reload must reject stale owner authority");
            assert!(
                error.to_string().contains(expected_error),
                "unexpected reload error: {error}"
            );
            assert!(
                test.thread_manager
                    .get_thread(first_thread_id)
                    .await
                    .is_err()
            );
            return Ok(());
        }
        result?;
        let EnvironmentConfigState::Ready(child_config) = &mut child_environment.config else {
            unreachable!("successfully reloaded child environment config should be ready");
        };
        child_config.permission_profile =
            PermissionProfileSnapshot::legacy(PermissionProfile::read_only());
    }

    test.submit_text_turn(FOLLOWUP_PROMPT).await?;
    let reloaded_worker = test.thread_manager.get_thread(first_thread_id).await?;
    wait_for_event(reloaded_worker.as_ref(), |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    assert_eq!(
        reloaded_worker
            .config_snapshot()
            .await
            .environments
            .environments,
        vec![child_environment]
    );
    assert_eq!(
        reloaded_worker.config_snapshot().await.permission_profile,
        PermissionProfile::read_only()
    );

    let worker_tools = |response_mock: &ResponseMock| {
        response_mock
            .requests()
            .into_iter()
            .find_map(|request| {
                let body = request.body_json();
                if body["client_metadata"]["thread_id"] != json!(first_thread_id) {
                    return None;
                }
                body.get("tools")
                    .or_else(|| {
                        body["input"]
                            .as_array()?
                            .iter()
                            .find(|item| item["type"] == "additional_tools")?
                            .get("tools")
                    })
                    .cloned()
            })
            .expect("expected a model request for the original worker")
    };
    let reloaded_tools = worker_tools(&reloaded_worker_request);
    assert!(reloaded_tools.to_string().contains("### `exec_command`"));

    Ok(())
}
