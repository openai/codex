use anyhow::Context;
use anyhow::Result;
use codex_config::types::ApprovalsReviewer;
use codex_core::config::Constrained;
use codex_exec_server::CopyOptions;
use codex_exec_server::CreateDirectoryOptions;
use codex_exec_server::FileSystemSandboxContext;
use codex_exec_server::LOCAL_ENVIRONMENT_ID;
use codex_exec_server::REMOTE_ENVIRONMENT_ID;
use codex_exec_server::RemoveOptions;
use codex_features::Feature;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::ApplyPatchApprovalRequestEvent;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::TurnEnvironmentSelection;
use codex_protocol::user_input::UserInput;
use codex_utils_absolute_path::AbsolutePathBuf;
use core_test_support::PathBufExt;
use core_test_support::PathExt;
use core_test_support::get_remote_test_env;
use core_test_support::responses::ev_apply_patch_custom_tool_call;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::test_codex::test_env;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use tempfile::TempDir;
async fn unified_exec_test(server: &wiremock::MockServer) -> Result<TestCodex> {
    let mut builder = test_codex().with_config(|config| {
        config.use_experimental_unified_exec_tool = true;
        let result = config.features.enable(Feature::UnifiedExec);
        assert!(
            result.is_ok(),
            "unified exec should enable for test: {result:?}",
        );
    });
    builder.build_remote_aware(server).await
}

async fn submit_turn_with_approval_and_environments(
    test: &TestCodex,
    prompt: &str,
    environments: Vec<TurnEnvironmentSelection>,
) -> Result<()> {
    test.codex
        .submit(Op::UserTurn {
            environments: Some(environments),
            items: vec![UserInput::Text {
                text: prompt.into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: test.cwd.path().to_path_buf(),
            approval_policy: AskForApproval::OnRequest,
            approvals_reviewer: Some(ApprovalsReviewer::User),
            sandbox_policy: SandboxPolicy::new_workspace_write_policy(),
            permission_profile: None,
            model: test.session_configured.model.clone(),
            effort: None,
            summary: None,
            service_tier: None,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    Ok(())
}

async fn expect_patch_approval(
    test: &TestCodex,
    expected_call_id: &str,
) -> ApplyPatchApprovalRequestEvent {
    let event = wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::ApplyPatchApprovalRequest(_) | EventMsg::TurnComplete(_)
        )
    })
    .await;

    match event {
        EventMsg::ApplyPatchApprovalRequest(approval) => {
            assert_eq!(approval.call_id, expected_call_id);
            approval
        }
        EventMsg::TurnComplete(_) => panic!("expected patch approval request before completion"),
        other => panic!("unexpected event: {other:?}"),
    }
}

async fn wait_for_completion_without_patch_approval(test: &TestCodex) {
    let event = wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::ApplyPatchApprovalRequest(_) | EventMsg::TurnComplete(_)
        )
    })
    .await;

    match event {
        EventMsg::TurnComplete(_) => {}
        EventMsg::ApplyPatchApprovalRequest(event) => {
            panic!("unexpected patch approval request: {:?}", event.call_id)
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_test_env_can_connect_and_use_filesystem() -> Result<()> {
    let Some(_remote_env) = get_remote_test_env() else {
        return Ok(());
    };

    let test_env = test_env().await?;
    let file_system = test_env.environment().get_filesystem();

    let file_path_abs = remote_test_file_path().abs();
    let payload = b"remote-test-env-ok".to_vec();

    file_system
        .write_file(&file_path_abs, payload.clone(), /*sandbox*/ None)
        .await?;
    let actual = file_system
        .read_file(&file_path_abs, /*sandbox*/ None)
        .await?;
    assert_eq!(actual, payload);

    file_system
        .remove(
            &file_path_abs,
            RemoveOptions {
                recursive: false,
                force: true,
            },
            /*sandbox*/ None,
        )
        .await?;

    Ok(())
}

fn absolute_path(path: PathBuf) -> AbsolutePathBuf {
    match AbsolutePathBuf::try_from(path) {
        Ok(path) => path,
        Err(error) => panic!("path should be absolute: {error}"),
    }
}

fn read_only_sandbox(readable_root: PathBuf) -> FileSystemSandboxContext {
    let readable_root = absolute_path(readable_root);
    FileSystemSandboxContext::from_permission_profile(PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: readable_root,
            },
            access: FileSystemAccessMode::Read,
        }]),
        NetworkSandboxPolicy::Restricted,
    ))
}

fn workspace_write_sandbox(writable_root: PathBuf) -> FileSystemSandboxContext {
    let writable_root = absolute_path(writable_root);
    FileSystemSandboxContext::from_permission_profile(PermissionProfile::from_runtime_permissions(
        &FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: writable_root,
            },
            access: FileSystemAccessMode::Write,
        }]),
        NetworkSandboxPolicy::Restricted,
    ))
}

fn assert_normalized_path_rejected(error: &std::io::Error) {
    match error.kind() {
        std::io::ErrorKind::NotFound => assert!(
            error.to_string().contains("No such file or directory"),
            "unexpected not-found message: {error}",
        ),
        std::io::ErrorKind::InvalidInput | std::io::ErrorKind::PermissionDenied => {
            let message = error.to_string();
            assert!(
                message.contains("is not permitted")
                    || message.contains("Operation not permitted")
                    || message.contains("Permission denied"),
                "unexpected rejection message: {message}",
            );
        }
        other => panic!("unexpected normalized-path error kind: {other:?}: {error:?}"),
    }
}

fn remote_exec(script: &str) -> Result<()> {
    let remote_env = get_remote_test_env().context("remote env should be configured")?;
    let output = Command::new("docker")
        .args(["exec", &remote_env.container_name, "sh", "-lc", script])
        .output()?;
    assert!(
        output.status.success(),
        "remote exec failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout).trim(),
        String::from_utf8_lossy(&output.stderr).trim(),
    );
    Ok(())
}

async fn exec_command_routing_output(
    test: &TestCodex,
    server: &wiremock::MockServer,
    call_id: &str,
    arguments: Value,
    environments: Option<Vec<TurnEnvironmentSelection>>,
) -> Result<String> {
    let response_mock = mount_sse_sequence(
        server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(call_id, "exec_command", &serde_json::to_string(&arguments)?),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    test.submit_turn_with_environments("route exec command", environments)
        .await?;

    response_mock
        .function_call_output_text(call_id)
        .with_context(|| format!("missing function_call_output for {call_id}"))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_command_routes_to_selected_remote_environment() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let Some(_remote_env) = get_remote_test_env() else {
        return Ok(());
    };

    let server = start_mock_server().await;
    let test = unified_exec_test(&server).await?;
    let local_cwd = TempDir::new()?;
    fs::write(local_cwd.path().join("marker.txt"), "local-routing")?;
    let local_selection = TurnEnvironmentSelection {
        environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
        cwd: local_cwd.path().abs(),
    };
    let remote_cwd = PathBuf::from(format!(
        "/tmp/codex-remote-routing-{}",
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis()
    ))
    .abs();
    let remote_marker_name = "marker.txt";
    test.fs()
        .create_directory(
            &remote_cwd,
            CreateDirectoryOptions { recursive: true },
            /*sandbox*/ None,
        )
        .await?;
    test.fs()
        .write_file(
            &remote_cwd.join(remote_marker_name),
            b"remote-routing".to_vec(),
            /*sandbox*/ None,
        )
        .await?;
    let remote_selection = TurnEnvironmentSelection {
        environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
        cwd: remote_cwd.clone(),
    };
    let multi_env_output = exec_command_routing_output(
        &test,
        &server,
        "call-multi-env",
        json!({
            "shell": "/bin/sh",
            "cmd": format!("cat {remote_marker_name}"),
            "login": false,
            "yield_time_ms": 1_000,
            "environment_id": REMOTE_ENVIRONMENT_ID,
        }),
        Some(vec![local_selection, remote_selection]),
    )
    .await?;
    assert!(
        multi_env_output.contains("remote-routing"),
        "unexpected multi-env output: {multi_env_output}",
    );
    assert!(
        !multi_env_output.contains("local-routing"),
        "multi-env command should not route to local: {multi_env_output}",
    );

    test.fs()
        .remove(
            &remote_cwd,
            RemoveOptions {
                recursive: true,
                force: true,
            },
            /*sandbox*/ None,
        )
        .await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_freeform_routes_to_selected_remote_environment() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let Some(_remote_env) = get_remote_test_env() else {
        return Ok(());
    };

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.include_apply_patch_tool = true;
    });
    let test = builder.build_remote_aware(&server).await?;
    let local_cwd = TempDir::new()?;
    let file_name = "apply_patch_remote_freeform.txt";
    let remote_cwd = PathBuf::from(format!(
        "/tmp/codex-remote-apply-patch-freeform-{}",
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis()
    ))
    .abs();
    test.fs()
        .create_directory(
            &remote_cwd,
            CreateDirectoryOptions { recursive: true },
            /*sandbox*/ None,
        )
        .await?;

    let patch = format!(
        "*** Begin Patch\n*** Environment ID: {REMOTE_ENVIRONMENT_ID}\n*** Add File: {file_name}\n+patched remote freeform\n*** End Patch"
    );
    let call_id = "apply-patch-remote-freeform";
    mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_apply_patch_custom_tool_call(call_id, &patch),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    test.submit_turn_with_environments(
        "apply patch to remote environment",
        Some(vec![
            TurnEnvironmentSelection {
                environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
                cwd: local_cwd.path().abs(),
            },
            TurnEnvironmentSelection {
                environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
                cwd: remote_cwd.clone(),
            },
        ]),
    )
    .await?;

    let remote_contents = test
        .fs()
        .read_file_text(&remote_cwd.join(file_name), /*sandbox*/ None)
        .await?;
    assert_eq!(remote_contents, "patched remote freeform\n");
    assert!(
        !local_cwd.path().join(file_name).exists(),
        "freeform apply_patch should not create the file in the local environment"
    );

    test.fs()
        .remove(
            &remote_cwd,
            RemoveOptions {
                recursive: true,
                force: true,
            },
            /*sandbox*/ None,
        )
        .await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_approvals_are_remembered_per_environment() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let Some(_remote_env) = get_remote_test_env() else {
        return Ok(());
    };

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.include_apply_patch_tool = true;
        config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
        config.approvals_reviewer = ApprovalsReviewer::User;
    });
    let test = builder.build_remote_aware(&server).await?;
    let local_cwd = TempDir::new()?;
    let remote_cwd = PathBuf::from(format!(
        "/tmp/codex-remote-apply-patch-approval-cwd-{}",
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis()
    ))
    .abs();
    test.fs()
        .create_directory(
            &remote_cwd,
            CreateDirectoryOptions { recursive: true },
            /*sandbox*/ None,
        )
        .await?;

    let target_path = PathBuf::from(format!(
        "/tmp/codex-apply-patch-approval-scope-{}.txt",
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis()
    ))
    .abs();
    let _ = fs::remove_file(&target_path);
    test.fs()
        .remove(
            &target_path,
            RemoveOptions {
                recursive: false,
                force: true,
            },
            /*sandbox*/ None,
        )
        .await?;

    let environments = vec![
        TurnEnvironmentSelection {
            environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
            cwd: local_cwd.path().abs(),
        },
        TurnEnvironmentSelection {
            environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
            cwd: remote_cwd.clone(),
        },
    ];
    let local_patch = format!(
        "*** Begin Patch\n*** Environment ID: {LOCAL_ENVIRONMENT_ID}\n*** Add File: {}\n+local\n*** End Patch",
        target_path.display()
    );
    let remote_patch = format!(
        "*** Begin Patch\n*** Environment ID: {REMOTE_ENVIRONMENT_ID}\n*** Add File: {}\n+remote\n*** End Patch",
        target_path.display()
    );
    let remote_update_patch = format!(
        "*** Begin Patch\n*** Environment ID: {REMOTE_ENVIRONMENT_ID}\n*** Update File: {}\n@@\n-remote\n+remote updated\n*** End Patch",
        target_path.display()
    );

    mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-local-1"),
                ev_apply_patch_custom_tool_call("call-local", &local_patch),
                ev_completed("resp-local-1"),
            ]),
            sse(vec![
                ev_response_created("resp-local-2"),
                ev_assistant_message("msg-local", "done"),
                ev_completed("resp-local-2"),
            ]),
            sse(vec![
                ev_response_created("resp-remote-1"),
                ev_apply_patch_custom_tool_call("call-remote", &remote_patch),
                ev_completed("resp-remote-1"),
            ]),
            sse(vec![
                ev_response_created("resp-remote-2"),
                ev_assistant_message("msg-remote", "done"),
                ev_completed("resp-remote-2"),
            ]),
            sse(vec![
                ev_response_created("resp-remote-3"),
                ev_apply_patch_custom_tool_call("call-remote-followup", &remote_update_patch),
                ev_completed("resp-remote-3"),
            ]),
            sse(vec![
                ev_response_created("resp-remote-4"),
                ev_assistant_message("msg-remote-followup", "done"),
                ev_completed("resp-remote-4"),
            ]),
        ],
    )
    .await;

    submit_turn_with_approval_and_environments(
        &test,
        "apply patch in local environment",
        environments.clone(),
    )
    .await?;
    let approval = expect_patch_approval(&test, "call-local").await;
    test.codex
        .submit(Op::PatchApproval {
            id: approval.call_id,
            decision: ReviewDecision::ApprovedForSession,
        })
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    assert_eq!(fs::read_to_string(&target_path)?, "local\n");

    submit_turn_with_approval_and_environments(
        &test,
        "apply patch in remote environment",
        environments.clone(),
    )
    .await?;
    let approval = expect_patch_approval(&test, "call-remote").await;
    test.codex
        .submit(Op::PatchApproval {
            id: approval.call_id,
            decision: ReviewDecision::ApprovedForSession,
        })
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    assert_eq!(
        test.fs()
            .read_file_text(&target_path, /*sandbox*/ None)
            .await?,
        "remote\n"
    );

    submit_turn_with_approval_and_environments(
        &test,
        "apply patch again in remote environment",
        environments,
    )
    .await?;
    wait_for_completion_without_patch_approval(&test).await;
    assert_eq!(
        test.fs()
            .read_file_text(&target_path, /*sandbox*/ None)
            .await?,
        "remote updated\n"
    );

    let _ = fs::remove_file(&target_path);
    test.fs()
        .remove(
            &target_path,
            RemoveOptions {
                recursive: false,
                force: true,
            },
            /*sandbox*/ None,
        )
        .await?;
    test.fs()
        .remove(
            &remote_cwd,
            RemoveOptions {
                recursive: true,
                force: true,
            },
            /*sandbox*/ None,
        )
        .await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_intercepted_exec_command_routes_to_selected_remote_environment() -> Result<()>
{
    skip_if_no_network!(Ok(()));
    let Some(_remote_env) = get_remote_test_env() else {
        return Ok(());
    };

    let server = start_mock_server().await;
    let test = unified_exec_test(&server).await?;
    let local_cwd = TempDir::new()?;
    let file_name = "apply_patch_remote_exec.txt";
    let remote_cwd = PathBuf::from(format!(
        "/tmp/codex-remote-apply-patch-exec-{}",
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis()
    ))
    .abs();
    test.fs()
        .create_directory(
            &remote_cwd,
            CreateDirectoryOptions { recursive: true },
            /*sandbox*/ None,
        )
        .await?;

    let patch =
        format!("*** Begin Patch\n*** Add File: {file_name}\n+patched remote exec\n*** End Patch");
    let command = format!("apply_patch <<'EOF'\n{patch}\nEOF\n");
    let call_id = "apply-patch-remote-exec";
    mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(
                    call_id,
                    "exec_command",
                    &serde_json::to_string(&json!({
                        "shell": "/bin/sh",
                        "cmd": command,
                        "login": false,
                        "yield_time_ms": 5_000,
                        "environment_id": REMOTE_ENVIRONMENT_ID,
                    }))?,
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    test.submit_turn_with_environments(
        "apply patch through exec command to remote environment",
        Some(vec![
            TurnEnvironmentSelection {
                environment_id: LOCAL_ENVIRONMENT_ID.to_string(),
                cwd: local_cwd.path().abs(),
            },
            TurnEnvironmentSelection {
                environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
                cwd: remote_cwd.clone(),
            },
        ]),
    )
    .await?;

    let remote_contents = test
        .fs()
        .read_file_text(&remote_cwd.join(file_name), /*sandbox*/ None)
        .await?;
    assert_eq!(remote_contents, "patched remote exec\n");
    assert!(
        !local_cwd.path().join(file_name).exists(),
        "intercepted apply_patch should not create the file in the local environment"
    );

    test.fs()
        .remove(
            &remote_cwd,
            RemoveOptions {
                recursive: true,
                force: true,
            },
            /*sandbox*/ None,
        )
        .await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_test_env_sandboxed_read_allows_readable_root() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let Some(_remote_env) = get_remote_test_env() else {
        return Ok(());
    };

    let test_env = test_env().await?;
    let file_system = test_env.environment().get_filesystem();

    let allowed_dir = PathBuf::from(format!("/tmp/codex-remote-readable-{}", std::process::id()));
    let file_path = allowed_dir.join("note.txt");
    file_system
        .create_directory(
            &absolute_path(allowed_dir.clone()),
            CreateDirectoryOptions { recursive: true },
            /*sandbox*/ None,
        )
        .await?;
    file_system
        .write_file(
            &absolute_path(file_path.clone()),
            b"sandboxed hello".to_vec(),
            /*sandbox*/ None,
        )
        .await?;

    let sandbox = read_only_sandbox(allowed_dir.clone());
    let contents = file_system
        .read_file(&absolute_path(file_path.clone()), Some(&sandbox))
        .await?;
    assert_eq!(contents, b"sandboxed hello");

    file_system
        .remove(
            &absolute_path(allowed_dir),
            RemoveOptions {
                recursive: true,
                force: true,
            },
            /*sandbox*/ None,
        )
        .await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_test_env_sandboxed_read_rejects_symlink_parent_dotdot_escape() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let Some(_remote_env) = get_remote_test_env() else {
        return Ok(());
    };

    let test_env = test_env().await?;
    let file_system = test_env.environment().get_filesystem();

    let root = PathBuf::from(format!("/tmp/codex-remote-dotdot-{}", std::process::id()));
    let allowed_dir = root.join("allowed");
    let outside_dir = root.join("outside");
    let secret_path = root.join("secret.txt");
    remote_exec(&format!(
        "rm -rf {root}; mkdir -p {allowed} {outside}; printf nope > {secret}; ln -s {outside} {allowed}/link",
        root = root.display(),
        allowed = allowed_dir.display(),
        outside = outside_dir.display(),
        secret = secret_path.display(),
    ))?;

    let requested_path = absolute_path(allowed_dir.join("link").join("..").join("secret.txt"));
    let sandbox = read_only_sandbox(allowed_dir.clone());
    let error = match file_system.read_file(&requested_path, Some(&sandbox)).await {
        Ok(_) => anyhow::bail!("read should fail after path normalization"),
        Err(error) => error,
    };
    assert_normalized_path_rejected(&error);

    remote_exec(&format!("rm -rf {}", root.display()))?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_test_env_remove_removes_symlink_not_target() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let Some(_remote_env) = get_remote_test_env() else {
        return Ok(());
    };

    let test_env = test_env().await?;
    let file_system = test_env.environment().get_filesystem();

    let root = PathBuf::from(format!(
        "/tmp/codex-remote-remove-link-{}",
        std::process::id()
    ));
    let allowed_dir = root.join("allowed");
    let outside_file = root.join("outside").join("keep.txt");
    let symlink_path = allowed_dir.join("link");
    remote_exec(&format!(
        "rm -rf {root}; mkdir -p {allowed} {outside_parent}; printf outside > {outside}; ln -s {outside} {symlink}",
        root = root.display(),
        allowed = allowed_dir.display(),
        outside_parent = absolute_path(
            outside_file
                .parent()
                .context("outside parent should exist")?
                .to_path_buf(),
        )
        .display(),
        outside = outside_file.display(),
        symlink = symlink_path.display(),
    ))?;

    let sandbox = workspace_write_sandbox(allowed_dir.clone());
    file_system
        .remove(
            &absolute_path(symlink_path.clone()),
            RemoveOptions {
                recursive: false,
                force: false,
            },
            Some(&sandbox),
        )
        .await?;

    let symlink_exists = file_system
        .get_metadata(&absolute_path(symlink_path), /*sandbox*/ None)
        .await
        .is_ok();
    assert!(!symlink_exists);
    let outside = file_system
        .read_file_text(&absolute_path(outside_file.clone()), /*sandbox*/ None)
        .await?;
    assert_eq!(outside, "outside");

    file_system
        .remove(
            &absolute_path(root),
            RemoveOptions {
                recursive: true,
                force: true,
            },
            /*sandbox*/ None,
        )
        .await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_test_env_copy_preserves_symlink_source() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let Some(_remote_env) = get_remote_test_env() else {
        return Ok(());
    };

    let test_env = test_env().await?;
    let file_system = test_env.environment().get_filesystem();

    let root = PathBuf::from(format!(
        "/tmp/codex-remote-copy-link-{}",
        std::process::id()
    ));
    let allowed_dir = root.join("allowed");
    let outside_file = root.join("outside").join("outside.txt");
    let source_symlink = allowed_dir.join("link");
    let copied_symlink = allowed_dir.join("copied-link");
    remote_exec(&format!(
        "rm -rf {root}; mkdir -p {allowed} {outside_parent}; printf outside > {outside}; ln -s {outside} {source}",
        root = root.display(),
        allowed = allowed_dir.display(),
        outside_parent = outside_file.parent().expect("outside parent").display(),
        outside = outside_file.display(),
        source = source_symlink.display(),
    ))?;

    let sandbox = workspace_write_sandbox(allowed_dir.clone());
    file_system
        .copy(
            &absolute_path(source_symlink),
            &absolute_path(copied_symlink.clone()),
            CopyOptions { recursive: false },
            Some(&sandbox),
        )
        .await?;

    let link_target = Command::new("docker")
        .args([
            "exec",
            &get_remote_test_env()
                .context("remote env should still be configured")?
                .container_name,
            "readlink",
            copied_symlink
                .to_str()
                .context("copied symlink path should be utf-8")?,
        ])
        .output()?;
    assert!(
        link_target.status.success(),
        "readlink failed: stdout={} stderr={}",
        String::from_utf8_lossy(&link_target.stdout).trim(),
        String::from_utf8_lossy(&link_target.stderr).trim(),
    );
    assert_eq!(
        String::from_utf8_lossy(&link_target.stdout).trim(),
        outside_file.to_string_lossy()
    );

    file_system
        .remove(
            &absolute_path(root),
            RemoveOptions {
                recursive: true,
                force: true,
            },
            /*sandbox*/ None,
        )
        .await?;
    Ok(())
}

fn remote_test_file_path() -> PathBuf {
    let nanos = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_nanos(),
        Err(_) => 0,
    };
    PathBuf::from(format!(
        "/tmp/codex-remote-test-env-{}-{nanos}.txt",
        std::process::id()
    ))
}
