use anyhow::Context;
use anyhow::Result;
use codex_config::permissions_toml::FilesystemPermissionToml;
use codex_config::permissions_toml::PermissionProfileToml;
use codex_config::types::ApprovalsReviewer;
use codex_core::TurnInputRequest;
use codex_core::sandboxing::SandboxPermissions;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Settings;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ExecApprovalRequestEvent;
use codex_protocol::protocol::GuardianAssessmentAction;
use codex_protocol::protocol::GuardianAssessmentStatus;
use codex_protocol::protocol::GuardianCommandSource;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ResponseMock;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::local_selections;
use core_test_support::test_codex::turn_permission_fields;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_with_timeout;
use core_test_support::zsh_fork::build_zsh_fork_test;
use core_test_support::zsh_fork::restrictive_workspace_write_profile;
use core_test_support::zsh_fork::zsh_fork_runtime;
use pretty_assertions::assert_eq;
use regex_lite::Regex;
use serde_json::Value;
use serde_json::json;
use std::fs;
use std::path::Path;
use std::time::Duration;
use toml_edit::Key as TomlKey;
use wiremock::MockServer;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unified_exec_zsh_fork_parent_approval_preserves_denied_reads() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let denied_dir = tempfile::tempdir_in(std::env::current_dir()?)?;
    let denied_path = denied_dir.path().join("secret.env");
    let secret = "unified-exec-zsh-fork-denied-read-secret";
    fs::write(&denied_path, format!("{secret}\n"))?;
    let permission_profile = denied_read_permission_profile(&denied_path)?;
    assert!(
        permission_profile
            .file_system_sandbox_policy()
            .has_denied_read_restrictions(),
        "test must exercise a permission profile with denied reads"
    );

    let approval_policy = AskForApproval::OnRequest;
    let command = format!("cat {denied_path:?}");
    let Some((server, test)) = build_unified_exec_zsh_fork_test_or_skip(
        "unified-exec zsh-fork denied-read approval test",
        approval_policy,
        permission_profile,
        move |_home| {},
    )
    .await?
    else {
        return Ok(());
    };

    let call_id = "uexec-zsh-fork-parent-approval-denied-read";
    let results = mount_unified_exec_command(
        &server,
        "uexec-zsh-fork-denied-read",
        call_id,
        &command,
        "attempt a denied read for the test",
    )
    .await?;
    submit_turn_with_session_permissions(
        &test,
        "run approved unified exec denied read through zsh fork",
        approval_policy,
        ApprovalsReviewer::User,
    )
    .await?;
    approve_expected_exec(&test, &command).await?;
    wait_for_completion_without_approval(&test).await;

    let result = command_result(&results, call_id);
    assert_ne!(
        result.exit_code.unwrap_or(0),
        0,
        "denied-read command should stay sandboxed after parent approval"
    );
    assert!(
        !result.stdout.contains(secret),
        "denied-read command unexpectedly printed the secret: {}",
        result.stdout
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unified_exec_zsh_fork_parent_approval_escalates_intercepted_exec() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let approval_policy = AskForApproval::OnRequest;
    let permission_profile = restrictive_workspace_write_profile();
    let outside_dir = tempfile::tempdir_in(std::env::current_dir()?)?;
    let outside_path = outside_dir
        .path()
        .join("unified-exec-zsh-fork-parent-approval.txt");
    let command = format!("printf hi > {outside_path:?}");

    let outside_path_for_hook = outside_path.clone();
    let Some((server, test)) = build_unified_exec_zsh_fork_test_or_skip(
        "unified-exec zsh-fork parent approval test",
        approval_policy,
        permission_profile,
        move |_home| {
            let _ = fs::remove_file(&outside_path_for_hook);
        },
    )
    .await?
    else {
        return Ok(());
    };

    let call_id = "uexec-zsh-fork-parent-approval";
    let results = mount_unified_exec_command(
        &server,
        "uexec-zsh-fork-parent-approval",
        call_id,
        &command,
        "write outside the workspace for the test",
    )
    .await?;
    submit_turn_with_session_permissions(
        &test,
        "run approved unified exec through zsh fork",
        approval_policy,
        ApprovalsReviewer::User,
    )
    .await?;
    approve_expected_exec(&test, &command).await?;
    wait_for_completion_without_approval(&test).await;

    let result = command_result(&results, call_id);
    assert_eq!(
        result.exit_code.unwrap_or(0),
        0,
        "approved unified exec zsh-fork command should complete: {}",
        result.stdout
    );
    let contents = fs::read_to_string(&outside_path)
        .with_context(|| format!("read {}", outside_path.display()))?;
    assert_eq!(
        contents, "hi",
        "approved parent sandbox override should allow zsh-fork shell redirection to write outside the workspace"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unified_exec_zsh_fork_parent_approval_keeps_explicit_prompt_rule() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let approval_policy = AskForApproval::OnRequest;
    let permission_profile = restrictive_workspace_write_profile();
    let outside_dir = tempfile::tempdir_in(std::env::current_dir()?)?;
    let outside_path = outside_dir
        .path()
        .join("unified-exec-zsh-fork-explicit-prompt-rule.txt");
    let intercepted_command = format!("touch {outside_path:?}");
    let command = format!("{intercepted_command} && {intercepted_command}");
    let rules = r#"prefix_rule(pattern=["touch"], decision="prompt")"#.to_string();

    let outside_path_for_hook = outside_path.clone();
    let Some((server, test)) = build_unified_exec_zsh_fork_test_or_skip(
        "unified-exec zsh-fork prompt rule approval test",
        approval_policy,
        permission_profile,
        move |home| {
            let _ = fs::remove_file(&outside_path_for_hook);
            let rules_dir = home.join("rules");
            fs::create_dir_all(&rules_dir).unwrap();
            fs::write(rules_dir.join("default.rules"), &rules).unwrap();
        },
    )
    .await?
    else {
        return Ok(());
    };

    let call_id = "uexec-zsh-fork-parent-approval-explicit-prompt-rule";
    let results = mount_unified_exec_command(
        &server,
        "uexec-zsh-fork-prompt-rule",
        call_id,
        &command,
        "write outside the workspace for the test",
    )
    .await?;
    submit_turn_with_session_permissions(
        &test,
        "run approved unified exec prompt rule through zsh fork",
        approval_policy,
        ApprovalsReviewer::User,
    )
    .await?;
    approve_expected_exec(&test, &command).await?;

    let mut intercepted_approval_ids = Vec::new();
    for _ in 0..2 {
        let approval_event = wait_for_event_with_timeout(
            &test.codex,
            |event| {
                matches!(
                    event,
                    EventMsg::ExecApprovalRequest(_) | EventMsg::TurnComplete(_)
                )
            },
            Duration::from_secs(10),
        )
        .await;
        let EventMsg::ExecApprovalRequest(inner_approval) = approval_event else {
            panic!("expected explicit prompt rule approval before completion");
        };
        assert_eq!(
            (
                inner_approval.call_id.as_str(),
                inner_approval.environment_id.as_deref(),
                inner_approval.available_decisions.as_deref(),
            ),
            (
                call_id,
                Some(codex_exec_server::LOCAL_ENVIRONMENT_ID),
                Some([ReviewDecision::Approved, ReviewDecision::Abort].as_slice()),
            )
        );
        let approval_id = inner_approval
            .approval_id
            .as_ref()
            .context("intercepted execve should have a subprocess approval id")?;
        assert_ne!(approval_id, call_id);
        assert!(
            !intercepted_approval_ids.contains(approval_id),
            "identical intercepted commands must receive distinct approval ids"
        );
        intercepted_approval_ids.push(approval_id.clone());
        assert!(
            inner_approval
                .command
                .iter()
                .any(|arg| arg.ends_with("/touch"))
                && inner_approval
                    .command
                    .iter()
                    .any(|arg| arg == outside_path.to_string_lossy().as_ref()),
            "expected explicit prompt rule approval for intercepted touch, got: {:?}",
            inner_approval.command
        );

        approve_exec(&test, inner_approval.effective_approval_id()).await?;
    }
    wait_for_completion(&test).await;

    let result = command_result(&results, call_id);
    assert_eq!(
        result.exit_code.unwrap_or(0),
        0,
        "approved unified exec zsh-fork prompt-rule command should complete: {}",
        result.stdout
    );
    assert!(
        outside_path.exists(),
        "approved intercepted touch should create the out-of-workspace file"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unified_exec_zsh_fork_guardian_reviews_intercepted_execve() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let approval_policy = AskForApproval::OnRequest;
    let permission_profile = restrictive_workspace_write_profile();
    let outside_dir = tempfile::tempdir_in(std::env::current_dir()?)?;
    let outside_path = outside_dir
        .path()
        .join("unified-exec-zsh-fork-guardian-execve.txt");
    let command = format!("touch {outside_path:?}");
    let rules = r#"prefix_rule(pattern=["touch"], decision="prompt")"#.to_string();

    let outside_path_for_hook = outside_path.clone();
    let Some((server, test)) = build_unified_exec_zsh_fork_test_or_skip(
        "unified-exec zsh-fork guardian execve approval test",
        approval_policy,
        permission_profile,
        move |home| {
            let _ = fs::remove_file(&outside_path_for_hook);
            let rules_dir = home.join("rules");
            fs::create_dir_all(&rules_dir).unwrap();
            fs::write(rules_dir.join("default.rules"), &rules).unwrap();
        },
    )
    .await?
    else {
        return Ok(());
    };

    let call_id = "uexec-zsh-fork-guardian-execve";
    let parent_tool_call = exec_command_event(
        call_id,
        &command,
        Some(30_000),
        SandboxPermissions::RequireEscalated,
        "exercise Guardian review of intercepted execve",
    )?;
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-guardian-execve-parent"),
                parent_tool_call,
                ev_completed("resp-guardian-execve-parent"),
            ]),
            sse(vec![
                ev_response_created("resp-guardian-parent-review"),
                ev_assistant_message("msg-guardian-parent-review", r#"{"outcome":"allow"}"#),
                ev_completed("resp-guardian-parent-review"),
            ]),
            sse(vec![
                ev_response_created("resp-guardian-execve-review"),
                ev_assistant_message("msg-guardian-execve-review", r#"{"outcome":"allow"}"#),
                ev_completed("resp-guardian-execve-review"),
            ]),
            sse(vec![
                ev_response_created("resp-guardian-execve-done"),
                ev_assistant_message("msg-guardian-execve-done", "done"),
                ev_completed("resp-guardian-execve-done"),
            ]),
        ],
    )
    .await;

    submit_turn_with_session_permissions(
        &test,
        "run unified exec with an intercepted command requiring Guardian review",
        approval_policy,
        ApprovalsReviewer::AutoReview,
    )
    .await?;

    let mut intercepted_assessments = Vec::new();
    loop {
        let event = tokio::time::timeout(Duration::from_secs(30), test.codex.next_event())
            .await
            .context("timed out waiting for intercepted execve Guardian review")??;
        match event.msg {
            EventMsg::GuardianAssessment(assessment)
                if assessment.status == GuardianAssessmentStatus::Approved
                    && matches!(assessment.action, GuardianAssessmentAction::Execve { .. }) =>
            {
                intercepted_assessments.push(assessment);
            }
            EventMsg::ExecApprovalRequest(_) => {
                panic!("Guardian-reviewed intercepted execution must not prompt the user")
            }
            EventMsg::TurnComplete(_) => break,
            _ => {}
        }
    }

    let [assessment] = intercepted_assessments.as_slice() else {
        panic!(
            "expected one approved intercepted execve Guardian assessment, got {intercepted_assessments:?}"
        );
    };
    let GuardianAssessmentAction::Execve {
        source,
        program,
        argv,
        cwd,
    } = &assessment.action
    else {
        unreachable!("intercepted assessments contain only execve actions");
    };
    let expected_cwd = test.config.cwd.canonicalize()?;
    assert_eq!(
        (
            *source,
            argv.as_slice(),
            cwd,
            assessment.target_item_id.as_deref(),
        ),
        (
            GuardianCommandSource::UnifiedExec,
            [
                "touch".to_string(),
                outside_path.to_string_lossy().into_owned()
            ]
            .as_slice(),
            &expected_cwd,
            Some(call_id),
        )
    );
    assert!(
        program.ends_with("/touch"),
        "unexpected execve program: {program}"
    );

    let guardian_requests = responses
        .requests()
        .into_iter()
        .filter(|request| {
            request.body_json()["client_metadata"]["x-openai-subagent"].as_str() == Some("guardian")
        })
        .collect::<Vec<_>>();
    assert_eq!(guardian_requests.len(), 2);
    assert!(guardian_requests[1].body_contains_text(&outside_path.to_string_lossy()));
    assert!(
        outside_path.exists(),
        "Guardian-approved intercepted touch should create the out-of-workspace file"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unified_exec_zsh_fork_guardian_reviews_persistent_terminal_in_current_turn() -> Result<()>
{
    skip_if_no_network!(Ok(()));

    let approval_policy = AskForApproval::OnRequest;
    let permission_profile = restrictive_workspace_write_profile();
    let outside_dir = tempfile::tempdir_in(std::env::current_dir()?)?;
    let outside_path = outside_dir
        .path()
        .join("unified-exec-zsh-fork-current-turn.txt");
    let rules = r#"prefix_rule(pattern=["touch"], decision="prompt")"#.to_string();

    let outside_path_for_hook = outside_path.clone();
    let Some((server, test)) = build_unified_exec_zsh_fork_test_or_skip(
        "unified-exec zsh-fork current-turn guardian approval test",
        approval_policy,
        permission_profile,
        move |home| {
            let _ = fs::remove_file(&outside_path_for_hook);
            let rules_dir = home.join("rules");
            fs::create_dir_all(&rules_dir).unwrap();
            fs::write(rules_dir.join("default.rules"), &rules).unwrap();
        },
    )
    .await?
    else {
        return Ok(());
    };

    let open_call_id = "uexec-zsh-fork-cross-turn-open";
    let open_command = "while IFS= read -r command; do eval \"$command\"; done";
    let open_args = json!({
        "cmd": open_command,
        "yield_time_ms": 250,
        "tty": true,
        "sandbox_permissions": SandboxPermissions::RequireEscalated,
        "justification": "start an interactive terminal for the next turn",
    });
    let write_call_id = "uexec-zsh-fork-cross-turn-write";
    let write_args = json!({
        "chars": format!("touch {outside_path:?}\nexit\n"),
        "session_id": 1000,
        "yield_time_ms": 5_000,
    });
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-cross-turn-open"),
                ev_function_call(
                    open_call_id,
                    "exec_command",
                    &serde_json::to_string(&open_args)?,
                ),
                ev_completed("resp-cross-turn-open"),
            ]),
            sse(vec![
                ev_response_created("resp-cross-turn-first-done"),
                ev_assistant_message("msg-cross-turn-first-done", "terminal is running"),
                ev_completed("resp-cross-turn-first-done"),
            ]),
            sse(vec![
                ev_response_created("resp-cross-turn-write"),
                ev_function_call(
                    write_call_id,
                    "write_stdin",
                    &serde_json::to_string(&write_args)?,
                ),
                ev_completed("resp-cross-turn-write"),
            ]),
            sse(vec![
                ev_response_created("resp-cross-turn-guardian"),
                ev_assistant_message("msg-cross-turn-guardian", r#"{"outcome":"allow"}"#),
                ev_completed("resp-cross-turn-guardian"),
            ]),
            sse(vec![
                ev_response_created("resp-cross-turn-second-done"),
                ev_assistant_message("msg-cross-turn-second-done", "done"),
                ev_completed("resp-cross-turn-second-done"),
            ]),
        ],
    )
    .await;

    submit_turn_with_session_permissions(
        &test,
        "start a persistent terminal with user approvals",
        approval_policy,
        ApprovalsReviewer::User,
    )
    .await?;
    approve_expected_exec(&test, open_command).await?;
    let first_completion = wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let EventMsg::TurnComplete(first_completion) = first_completion else {
        unreachable!("completion wait only returns turn-complete events");
    };

    submit_turn_with_session_permissions(
        &test,
        "run a command in the persistent terminal with Guardian approvals",
        approval_policy,
        ApprovalsReviewer::AutoReview,
    )
    .await?;

    let mut current_turn_id = None;
    let mut intercepted_assessment = None;
    loop {
        let event = tokio::time::timeout(Duration::from_secs(30), test.codex.next_event())
            .await
            .context("timed out waiting for current-turn intercepted execve Guardian review")??;
        match event.msg {
            EventMsg::TurnStarted(started) => current_turn_id = Some(started.turn_id),
            EventMsg::GuardianAssessment(assessment)
                if assessment.status == GuardianAssessmentStatus::Approved
                    && matches!(assessment.action, GuardianAssessmentAction::Execve { .. }) =>
            {
                intercepted_assessment = Some(assessment);
            }
            EventMsg::ExecApprovalRequest(_) => {
                panic!("persistent-terminal approval used the previous turn's user reviewer")
            }
            EventMsg::TurnComplete(_) => break,
            _ => {}
        }
    }

    let current_turn_id = current_turn_id.context("expected the second turn to start")?;
    assert_ne!(current_turn_id, first_completion.turn_id);
    let assessment = intercepted_assessment
        .context("expected an approved Guardian assessment for the persistent terminal")?;
    assert_eq!(assessment.turn_id, current_turn_id);
    assert_eq!(assessment.target_item_id.as_deref(), Some(open_call_id));
    assert!(
        outside_path.exists(),
        "Guardian-approved command from the second turn should create the file"
    );

    let guardian_requests = responses
        .requests()
        .into_iter()
        .filter(|request| {
            request.body_json()["client_metadata"]["x-openai-subagent"].as_str() == Some("guardian")
        })
        .collect::<Vec<_>>();
    assert_eq!(guardian_requests.len(), 1);
    assert!(guardian_requests[0].body_contains_text(&outside_path.to_string_lossy()));

    Ok(())
}

struct CommandResult {
    exit_code: Option<i64>,
    stdout: String,
}

async fn build_unified_exec_zsh_fork_test_or_skip<F>(
    test_name: &str,
    approval_policy: AskForApproval,
    permission_profile: PermissionProfile,
    pre_build_hook: F,
) -> Result<Option<(MockServer, TestCodex)>>
where
    F: FnOnce(&Path) + Send + 'static,
{
    let Some(runtime) = zsh_fork_runtime(test_name)? else {
        return Ok(None);
    };

    let server = start_mock_server().await;
    let test = build_zsh_fork_test(
        &server,
        runtime,
        approval_policy,
        permission_profile,
        pre_build_hook,
    )
    .await?;
    Ok(Some((server, test)))
}

fn denied_read_permission_profile(denied_path: &Path) -> Result<PermissionProfile> {
    let denied_path_key = TomlKey::new(denied_path.to_string_lossy().into_owned());
    permission_profile_from_toml(&format!(
        r#"
[filesystem]
"/" = "read"
":project_roots" = "write"
{denied_path_key} = "deny"

[network]
enabled = false
"#
    ))
}

fn permission_profile_from_toml(profile: &str) -> Result<PermissionProfile> {
    let profile = toml::from_str::<PermissionProfileToml>(profile)
        .context("test permission profile should deserialize")?;
    let filesystem = profile
        .filesystem
        .as_ref()
        .context("test permission profile should include filesystem entries")?;
    let entries = filesystem
        .entries
        .iter()
        .map(|(path, permission)| {
            let FilesystemPermissionToml::Access(access) = permission else {
                anyhow::bail!("unexpected scoped filesystem permission in test profile: {path}");
            };
            let path = match path.as_str() {
                "/" => FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                ":project_roots" => FileSystemPath::Special {
                    value: FileSystemSpecialPath::project_roots(/*subpath*/ None),
                },
                _ if *access == FileSystemAccessMode::Deny => FileSystemPath::GlobPattern {
                    pattern: path.clone(),
                },
                _ => anyhow::bail!("unexpected filesystem entry in test profile: {path}"),
            };
            Ok(FileSystemSandboxEntry {
                path,
                access: *access,
                missing_path_behavior: None,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let mut file_system_sandbox_policy = FileSystemSandboxPolicy::restricted(entries);
    file_system_sandbox_policy.glob_scan_max_depth = filesystem.glob_scan_max_depth;
    let network_sandbox_policy = match profile.network.as_ref().and_then(|network| network.enabled)
    {
        Some(true) => NetworkSandboxPolicy::Enabled,
        Some(false) | None => NetworkSandboxPolicy::Restricted,
    };

    Ok(PermissionProfile::from_runtime_permissions(
        &file_system_sandbox_policy,
        network_sandbox_policy,
    ))
}

async fn mount_unified_exec_command(
    server: &MockServer,
    response_prefix: &str,
    call_id: &str,
    command: &str,
    justification: &str,
) -> Result<ResponseMock> {
    let first_response_id = format!("resp-{response_prefix}-1");
    let second_response_id = format!("resp-{response_prefix}-2");
    let message_id = format!("msg-{response_prefix}-1");
    let event = exec_command_event(
        call_id,
        command,
        Some(30_000),
        SandboxPermissions::RequireEscalated,
        justification,
    )?;
    let _ = mount_sse_once(
        server,
        sse(vec![
            ev_response_created(&first_response_id),
            event,
            ev_completed(&first_response_id),
        ]),
    )
    .await;
    let results = mount_sse_once(
        server,
        sse(vec![
            ev_assistant_message(&message_id, "done"),
            ev_completed(&second_response_id),
        ]),
    )
    .await;
    Ok(results)
}

async fn submit_turn_with_session_permissions(
    test: &TestCodex,
    prompt: &str,
    approval_policy: AskForApproval,
    approvals_reviewer: ApprovalsReviewer,
) -> Result<()> {
    let session_model = test.session_configured.model.clone();
    let (sandbox_policy, permission_profile) = turn_permission_fields(
        test.session_configured.permission_profile.clone(),
        test.cwd.path(),
    );
    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: prompt.into(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                environments: Some(local_selections(test.config.cwd.clone())),
                approval_policy: Some(approval_policy),
                approvals_reviewer: Some(approvals_reviewer),
                sandbox_policy: Some(sandbox_policy),
                permission_profile,
                collaboration_mode: Some(CollaborationMode {
                    mode: ModeKind::Default,
                    settings: Settings {
                        model: session_model,
                        reasoning_effort: None,
                        developer_instructions: None,
                    },
                }),
                ..Default::default()
            }),
        )
        .await?;

    Ok(())
}

async fn approve_expected_exec(test: &TestCodex, expected_command: &str) -> Result<()> {
    let approval = expect_exec_approval(test, expected_command).await;
    approve_exec(test, approval.effective_approval_id()).await
}

async fn approve_exec(test: &TestCodex, approval_id: String) -> Result<()> {
    test.codex
        .submit(Op::ExecApproval {
            id: approval_id,
            turn_id: None,
            decision: ReviewDecision::Approved,
        })
        .await?;
    Ok(())
}

fn command_result(results: &ResponseMock, call_id: &str) -> CommandResult {
    parse_result(&results.single_request().function_call_output(call_id))
}

fn exec_command_event(
    call_id: &str,
    cmd: &str,
    yield_time_ms: Option<u64>,
    sandbox_permissions: SandboxPermissions,
    justification: &str,
) -> Result<Value> {
    let mut args = json!({
        "cmd": cmd.to_string(),
    });
    if let Some(yield_time_ms) = yield_time_ms {
        args["yield_time_ms"] = json!(yield_time_ms);
    }
    if sandbox_permissions.requests_sandbox_override() {
        args["sandbox_permissions"] = json!(sandbox_permissions);
        args["justification"] = json!(justification);
    }
    let args_str = serde_json::to_string(&args)?;
    Ok(ev_function_call(call_id, "exec_command", &args_str))
}

fn parse_result(item: &Value) -> CommandResult {
    let Some(output_str) = item.get("output").and_then(Value::as_str) else {
        return CommandResult {
            exit_code: None,
            stdout: String::new(),
        };
    };
    match serde_json::from_str::<Value>(output_str) {
        Ok(parsed) => {
            let exit_code = parsed["metadata"]["exit_code"].as_i64();
            let stdout = parsed["output"].as_str().unwrap_or_default().to_string();
            CommandResult { exit_code, stdout }
        }
        Err(_) => parsed_regex_result(r"(?s)^Exit code:\s*(-?\d+).*?Output:\n(.*)$", output_str)
            .or_else(|| {
                parsed_regex_result(
                    r"(?s)^.*?Process exited with code (\d+)\n.*?Output:\n(.*)$",
                    output_str,
                )
            })
            .unwrap_or_else(|| CommandResult {
                exit_code: None,
                stdout: output_str.to_string(),
            }),
    }
}

fn parsed_regex_result(pattern: &str, output_str: &str) -> Option<CommandResult> {
    let regex = Regex::new(pattern).ok()?;
    let captures = regex.captures(output_str)?;
    let exit_code = captures.get(1)?.as_str().parse::<i64>().ok()?;
    let output = captures.get(2)?.as_str();
    Some(CommandResult {
        exit_code: Some(exit_code),
        stdout: output.to_string(),
    })
}

async fn expect_exec_approval(
    test: &TestCodex,
    expected_command: &str,
) -> ExecApprovalRequestEvent {
    let event = wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::ExecApprovalRequest(_) | EventMsg::TurnComplete(_)
        )
    })
    .await;

    match event {
        EventMsg::ExecApprovalRequest(approval) => {
            let last_arg = approval
                .command
                .last()
                .map(std::string::String::as_str)
                .unwrap_or_default();
            assert_eq!(
                last_arg, expected_command,
                "approval request should be for the parent unified-exec command"
            );
            approval
        }
        EventMsg::TurnComplete(_) => panic!("expected approval request before completion"),
        other => panic!("unexpected event: {other:?}"),
    }
}

async fn wait_for_completion_without_approval(test: &TestCodex) {
    let event = wait_for_event(&test.codex, |event| {
        matches!(
            event,
            EventMsg::ExecApprovalRequest(_) | EventMsg::TurnComplete(_)
        )
    })
    .await;

    match event {
        EventMsg::TurnComplete(_) => {}
        EventMsg::ExecApprovalRequest(event) => {
            panic!("unexpected approval request: {:?}", event.command)
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

async fn wait_for_completion(test: &TestCodex) {
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
}
