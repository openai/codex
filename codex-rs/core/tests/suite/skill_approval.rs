#![allow(clippy::unwrap_used)]
#![cfg(unix)]

use anyhow::Result;
use codex_core::config::Config;
use codex_core::features::Feature;
use codex_protocol::models::FileSystemPermissions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ExecApprovalRequestEvent;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::mount_function_call_agent_response;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

fn write_skill_metadata(home: &Path, name: &str, contents: &str) -> Result<()> {
    let metadata_dir = home.join("skills").join(name).join("agents");
    fs::create_dir_all(&metadata_dir)?;
    fs::write(metadata_dir.join("openai.yaml"), contents)?;
    Ok(())
}

fn shell_command_arguments(command: &str) -> Result<String> {
    Ok(serde_json::to_string(&json!({
        "command": command,
        "timeout_ms": 500,
    }))?)
}

async fn submit_turn_with_policies(
    test: &TestCodex,
    prompt: &str,
    approval_policy: AskForApproval,
    sandbox_policy: SandboxPolicy,
) -> Result<()> {
    test.codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: prompt.to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: test.cwd_path().to_path_buf(),
            approval_policy,
            sandbox_policy,
            model: test.session_configured.model.clone(),
            effort: None,
            summary: None,
            collaboration_mode: None,
            personality: None,
        })
        .await?;
    Ok(())
}

fn write_skill_with_shell_script(home: &Path, name: &str, script_name: &str) -> Result<PathBuf> {
    write_skill_with_shell_script_contents(
        home,
        name,
        script_name,
        r#"#!/bin/sh
echo 'zsh-fork-stdout'
echo 'zsh-fork-stderr' >&2
"#,
    )
}

#[cfg(unix)]
fn write_skill_with_shell_script_contents(
    home: &Path,
    name: &str,
    script_name: &str,
    script_contents: &str,
) -> Result<PathBuf> {
    use std::os::unix::fs::PermissionsExt;

    let skill_dir = home.join("skills").join(name);
    let scripts_dir = skill_dir.join("scripts");
    fs::create_dir_all(&scripts_dir)?;
    fs::write(
        skill_dir.join("SKILL.md"),
        format!(
            r#"---
name: {name}
description: {name} skill
---
"#
        ),
    )?;

    let script_path = scripts_dir.join(script_name);
    fs::write(&script_path, script_contents)?;
    let mut permissions = fs::metadata(&script_path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&script_path, permissions)?;
    Ok(script_path)
}

fn find_test_zsh_path() -> Result<Option<PathBuf>> {
    use core_test_support::fetch_dotslash_file;

    let repo_root = codex_utils_cargo_bin::repo_root()?;
    let dotslash_zsh = repo_root.join("codex-rs/app-server/tests/suite/zsh");
    if !dotslash_zsh.is_file() {
        eprintln!(
            "skipping zsh-fork skill test: shared zsh DotSlash file not found at {}",
            dotslash_zsh.display()
        );
        return Ok(None);
    }

    match fetch_dotslash_file(&dotslash_zsh, None) {
        Ok(path) => Ok(Some(path)),
        Err(error) => {
            eprintln!("skipping zsh-fork skill test: failed to fetch zsh via dotslash: {error:#}");
            Ok(None)
        }
    }
}

fn supports_exec_wrapper_intercept(zsh_path: &Path) -> bool {
    let status = std::process::Command::new(zsh_path)
        .arg("-fc")
        .arg("/usr/bin/true")
        .env("EXEC_WRAPPER", "/usr/bin/false")
        .status();
    match status {
        Ok(status) => !status.success(),
        Err(_) => false,
    }
}

#[derive(Clone)]
struct ZshForkRuntime {
    zsh_path: PathBuf,
    main_execve_wrapper_exe: PathBuf,
}

impl ZshForkRuntime {
    fn apply_to_config(
        &self,
        config: &mut Config,
        approval_policy: AskForApproval,
        sandbox_policy: SandboxPolicy,
    ) {
        use codex_config::Constrained;

        config.features.enable(Feature::ShellTool);
        config.features.enable(Feature::ShellZshFork);
        config.zsh_path = Some(self.zsh_path.clone());
        config.main_execve_wrapper_exe = Some(self.main_execve_wrapper_exe.clone());
        config.permissions.allow_login_shell = false;
        config.permissions.approval_policy = Constrained::allow_any(approval_policy);
        config.permissions.sandbox_policy = Constrained::allow_any(sandbox_policy);
    }
}

fn restrictive_workspace_write_policy() -> SandboxPolicy {
    SandboxPolicy::WorkspaceWrite {
        writable_roots: Vec::new(),
        read_only_access: Default::default(),
        network_access: false,
        exclude_tmpdir_env_var: true,
        exclude_slash_tmp: true,
    }
}

fn zsh_fork_runtime(test_name: &str) -> Result<Option<ZshForkRuntime>> {
    let Some(zsh_path) = find_test_zsh_path()? else {
        return Ok(None);
    };
    if !supports_exec_wrapper_intercept(&zsh_path) {
        eprintln!(
            "skipping {test_name}: zsh does not support EXEC_WRAPPER intercepts ({})",
            zsh_path.display()
        );
        return Ok(None);
    }
    let Ok(main_execve_wrapper_exe) = codex_utils_cargo_bin::cargo_bin("codex-execve-wrapper")
    else {
        eprintln!("skipping {test_name}: unable to resolve `codex-execve-wrapper` binary");
        return Ok(None);
    };

    Ok(Some(ZshForkRuntime {
        zsh_path,
        main_execve_wrapper_exe,
    }))
}

async fn build_zsh_fork_test<F>(
    server: &wiremock::MockServer,
    runtime: ZshForkRuntime,
    approval_policy: AskForApproval,
    sandbox_policy: SandboxPolicy,
    pre_build_hook: F,
) -> Result<TestCodex>
where
    F: FnOnce(&Path) + Send + 'static,
{
    let mut builder = test_codex()
        .with_pre_build_hook(pre_build_hook)
        .with_config(move |config| {
            runtime.apply_to_config(config, approval_policy, sandbox_policy);
        });
    builder.build(server).await
}

fn skill_script_command(test: &TestCodex, script_name: &str) -> Result<(String, String)> {
    let script_path = fs::canonicalize(
        test.codex_home_path()
            .join("skills/mbolin-test-skill/scripts")
            .join(script_name),
    )?;
    let script_path_str = script_path.to_string_lossy().into_owned();
    let command = shlex::try_join([script_path_str.as_str()])?;
    Ok((script_path_str, command))
}

async fn wait_for_exec_approval_request(test: &TestCodex) -> Option<ExecApprovalRequestEvent> {
    wait_for_event_match(test.codex.as_ref(), |event| match event {
        EventMsg::ExecApprovalRequest(request) => Some(Some(request.clone())),
        EventMsg::TurnComplete(_) => Some(None),
        _ => None,
    })
    .await
}

async fn wait_for_turn_complete(test: &TestCodex) {
    wait_for_event(test.codex.as_ref(), |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
}

fn output_shows_sandbox_denial(output: &str) -> bool {
    output.contains("Permission denied")
        || output.contains("Operation not permitted")
        || output.contains("Read-only file system")
}

/// Focus on the approval payload: the skill should prompt before execution and
/// only advertise the permissions declared in its metadata.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_zsh_fork_prompts_for_skill_script_execution() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let Some(runtime) = zsh_fork_runtime("zsh-fork skill prompt test")? else {
        return Ok(());
    };

    let server = start_mock_server().await;
    let tool_call_id = "zsh-fork-skill-call";
    let test = build_zsh_fork_test(
        &server,
        runtime,
        AskForApproval::OnRequest,
        SandboxPolicy::new_workspace_write_policy(),
        |home| {
            write_skill_with_shell_script(home, "mbolin-test-skill", "hello-mbolin.sh").unwrap();
            write_skill_metadata(
                home,
                "mbolin-test-skill",
                r#"
permissions:
  file_system:
    read:
      - "./data"
    write:
      - "./output"
"#,
            )
            .unwrap();
        },
    )
    .await?;

    let (script_path_str, command) = skill_script_command(&test, "hello-mbolin.sh")?;
    let arguments = shell_command_arguments(&command)?;
    let mocks =
        mount_function_call_agent_response(&server, tool_call_id, &arguments, "shell_command")
            .await;

    submit_turn_with_policies(
        &test,
        "use $mbolin-test-skill",
        AskForApproval::OnRequest,
        SandboxPolicy::new_workspace_write_policy(),
    )
    .await?;

    let maybe_approval = wait_for_exec_approval_request(&test).await;
    let approval = match maybe_approval {
        Some(approval) => approval,
        None => {
            let call_output = mocks
                .completion
                .single_request()
                .function_call_output(tool_call_id);
            panic!(
                "expected exec approval request before completion; function_call_output={call_output:?}"
            );
        }
    };
    assert_eq!(approval.call_id, tool_call_id);
    assert_eq!(approval.command, vec![script_path_str.clone()]);
    assert_eq!(
        approval.available_decisions,
        Some(vec![
            ReviewDecision::Approved,
            ReviewDecision::ApprovedForSession,
            ReviewDecision::Abort,
        ])
    );
    assert_eq!(
        approval.additional_permissions,
        Some(PermissionProfile {
            file_system: Some(FileSystemPermissions {
                read: Some(vec![PathBuf::from("./data")]),
                write: Some(vec![PathBuf::from("./output")]),
            }),
            ..Default::default()
        })
    );

    test.codex
        .submit(Op::ExecApproval {
            id: approval.effective_approval_id(),
            turn_id: None,
            decision: ReviewDecision::Denied,
        })
        .await?;

    wait_for_turn_complete(&test).await;

    let call_output = mocks
        .completion
        .single_request()
        .function_call_output(tool_call_id);
    let output = call_output["output"].as_str().unwrap_or_default();
    assert!(
        output.contains("Execution denied: User denied execution"),
        "expected rejection marker in function_call_output: {output:?}"
    );

    Ok(())
}

/// Look for `additional_permissions == None`, then verify that both the first
/// run and the cached session-approval rerun stay inside the turn sandbox.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_zsh_fork_skill_without_permissions_inherits_turn_sandbox() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let Some(runtime) = zsh_fork_runtime("zsh-fork inherited skill sandbox test")? else {
        return Ok(());
    };

    let outside_dir = tempfile::tempdir_in(std::env::current_dir()?)?;
    let outside_path = outside_dir
        .path()
        .join("zsh-fork-skill-inherited-sandbox.txt");
    let outside_path_quoted = shlex::try_join([outside_path.to_string_lossy().as_ref()])?;
    let script_contents = format!(
        "#!/bin/sh\nprintf '%s' forbidden > {outside_path_quoted}\ncat {outside_path_quoted}\n"
    );
    let outside_path_for_hook = outside_path.clone();
    let script_contents_for_hook = script_contents.clone();
    let workspace_write_policy = restrictive_workspace_write_policy();

    let server = start_mock_server().await;
    let test = build_zsh_fork_test(
        &server,
        runtime,
        AskForApproval::OnRequest,
        workspace_write_policy.clone(),
        move |home| {
            let _ = fs::remove_file(&outside_path_for_hook);
            write_skill_with_shell_script_contents(
                home,
                "mbolin-test-skill",
                "sandboxed.sh",
                &script_contents_for_hook,
            )
            .unwrap();
        },
    )
    .await?;

    let (script_path_str, command) = skill_script_command(&test, "sandboxed.sh")?;

    let first_call_id = "zsh-fork-skill-permissions-1";
    let first_arguments = shell_command_arguments(&command)?;
    let first_mocks = mount_function_call_agent_response(
        &server,
        first_call_id,
        &first_arguments,
        "shell_command",
    )
    .await;

    submit_turn_with_policies(
        &test,
        "use $mbolin-test-skill",
        AskForApproval::OnRequest,
        workspace_write_policy.clone(),
    )
    .await?;

    let maybe_approval = wait_for_exec_approval_request(&test).await;
    let approval = match maybe_approval {
        Some(approval) => approval,
        None => panic!("expected exec approval request before completion"),
    };
    assert_eq!(approval.call_id, first_call_id);
    assert_eq!(approval.command, vec![script_path_str.clone()]);
    assert_eq!(approval.additional_permissions, None);

    test.codex
        .submit(Op::ExecApproval {
            id: approval.effective_approval_id(),
            turn_id: None,
            decision: ReviewDecision::ApprovedForSession,
        })
        .await?;

    wait_for_turn_complete(&test).await;

    let first_output = first_mocks
        .completion
        .single_request()
        .function_call_output(first_call_id)["output"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(
        output_shows_sandbox_denial(&first_output) || !first_output.contains("forbidden"),
        "expected inherited turn sandbox denial on first run, got output: {first_output:?}"
    );
    assert!(
        !outside_path.exists(),
        "first run should not write outside the turn sandbox"
    );

    let second_call_id = "zsh-fork-skill-permissions-2";
    let second_arguments = shell_command_arguments(&command)?;
    let second_mocks = mount_function_call_agent_response(
        &server,
        second_call_id,
        &second_arguments,
        "shell_command",
    )
    .await;

    submit_turn_with_policies(
        &test,
        "use $mbolin-test-skill",
        AskForApproval::OnRequest,
        workspace_write_policy,
    )
    .await?;

    let cached_approval = wait_for_exec_approval_request(&test).await;
    assert!(
        cached_approval.is_none(),
        "expected second run to reuse the cached session approval"
    );

    let second_output = second_mocks
        .completion
        .single_request()
        .function_call_output(second_call_id)["output"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(
        output_shows_sandbox_denial(&second_output) || !second_output.contains("forbidden"),
        "expected cached skill approval to retain inherited turn sandboxing, got output: {second_output:?}"
    );
    assert!(
        !outside_path.exists(),
        "cached session approval should not widen a permissionless skill to full access"
    );

    Ok(())
}

/// The validation to focus on is: writes to the skill-approved folder succeed,
/// and writes to an unrelated folder fail, both before and after cached approval.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_zsh_fork_skill_session_approval_enforces_skill_permissions() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let Some(runtime) = zsh_fork_runtime("zsh-fork explicit skill sandbox test")? else {
        return Ok(());
    };

    let outside_dir = tempfile::tempdir_in(std::env::current_dir()?)?;
    let allowed_dir = outside_dir.path().join("allowed-output");
    let blocked_dir = outside_dir.path().join("blocked-output");
    fs::create_dir_all(&allowed_dir)?;
    fs::create_dir_all(&blocked_dir)?;

    let allowed_path = allowed_dir.join("allowed.txt");
    let blocked_path = blocked_dir.join("blocked.txt");
    let allowed_path_quoted = shlex::try_join([allowed_path.to_string_lossy().as_ref()])?;
    let blocked_path_quoted = shlex::try_join([blocked_path.to_string_lossy().as_ref()])?;
    let script_contents = format!(
        "#!/bin/sh\nprintf '%s' allowed > {allowed_path_quoted}\ncat {allowed_path_quoted}\nprintf '%s' forbidden > {blocked_path_quoted}\nif [ -f {blocked_path_quoted} ]; then echo blocked-created; fi\n"
    );
    let allowed_dir_for_hook = allowed_dir.clone();
    let allowed_path_for_hook = allowed_path.clone();
    let blocked_path_for_hook = blocked_path.clone();
    let script_contents_for_hook = script_contents.clone();

    let permissions_yaml = format!(
        "permissions:\n  file_system:\n    write:\n      - \"{}\"\n",
        allowed_dir.display()
    );

    let workspace_write_policy = restrictive_workspace_write_policy();
    let server = start_mock_server().await;
    let test = build_zsh_fork_test(
        &server,
        runtime,
        AskForApproval::OnRequest,
        workspace_write_policy.clone(),
        move |home| {
            let _ = fs::remove_file(&allowed_path_for_hook);
            let _ = fs::remove_file(&blocked_path_for_hook);
            fs::create_dir_all(&allowed_dir_for_hook).unwrap();
            fs::create_dir_all(blocked_path_for_hook.parent().unwrap()).unwrap();
            write_skill_with_shell_script_contents(
                home,
                "mbolin-test-skill",
                "sandboxed.sh",
                &script_contents_for_hook,
            )
            .unwrap();
            write_skill_metadata(home, "mbolin-test-skill", &permissions_yaml).unwrap();
        },
    )
    .await?;

    let (script_path_str, command) = skill_script_command(&test, "sandboxed.sh")?;

    let first_call_id = "zsh-fork-skill-permissions-1";
    let first_arguments = shell_command_arguments(&command)?;
    let first_mocks = mount_function_call_agent_response(
        &server,
        first_call_id,
        &first_arguments,
        "shell_command",
    )
    .await;

    submit_turn_with_policies(
        &test,
        "use $mbolin-test-skill",
        AskForApproval::OnRequest,
        workspace_write_policy.clone(),
    )
    .await?;

    let maybe_approval = wait_for_exec_approval_request(&test).await;
    let approval = match maybe_approval {
        Some(approval) => approval,
        None => panic!("expected exec approval request before completion"),
    };
    assert_eq!(approval.call_id, first_call_id);
    assert_eq!(approval.command, vec![script_path_str.clone()]);
    assert_eq!(
        approval.additional_permissions,
        Some(PermissionProfile {
            file_system: Some(FileSystemPermissions {
                read: None,
                write: Some(vec![allowed_dir.clone()]),
            }),
            ..Default::default()
        })
    );

    test.codex
        .submit(Op::ExecApproval {
            id: approval.effective_approval_id(),
            turn_id: None,
            decision: ReviewDecision::ApprovedForSession,
        })
        .await?;

    wait_for_turn_complete(&test).await;

    let first_output = first_mocks
        .completion
        .single_request()
        .function_call_output(first_call_id)["output"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(
        first_output.contains("allowed"),
        "expected skill sandbox to permit writes to the approved folder, got output: {first_output:?}"
    );
    assert_eq!(fs::read_to_string(&allowed_path)?, "allowed");
    assert!(
        !blocked_path.exists(),
        "first run should not write outside the explicit skill sandbox"
    );
    assert!(
        !first_output.contains("blocked-created"),
        "blocked path should not have been created: {first_output:?}"
    );

    let second_call_id = "zsh-fork-skill-permissions-2";
    let second_arguments = shell_command_arguments(&command)?;
    let second_mocks = mount_function_call_agent_response(
        &server,
        second_call_id,
        &second_arguments,
        "shell_command",
    )
    .await;

    let _ = fs::remove_file(&allowed_path);
    let _ = fs::remove_file(&blocked_path);

    submit_turn_with_policies(
        &test,
        "use $mbolin-test-skill",
        AskForApproval::OnRequest,
        workspace_write_policy,
    )
    .await?;

    let cached_approval = wait_for_exec_approval_request(&test).await;
    assert!(
        cached_approval.is_none(),
        "expected second run to reuse the cached session approval"
    );

    let second_output = second_mocks
        .completion
        .single_request()
        .function_call_output(second_call_id)["output"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(
        second_output.contains("allowed"),
        "expected cached skill approval to retain the explicit skill sandbox, got output: {second_output:?}"
    );
    assert_eq!(fs::read_to_string(&allowed_path)?, "allowed");
    assert!(
        !blocked_path.exists(),
        "cached session approval should not widen skill execution beyond the explicit skill sandbox"
    );
    assert!(
        !second_output.contains("blocked-created"),
        "blocked path should not have been created after cached approval: {second_output:?}"
    );

    Ok(())
}

/// This stays narrow on purpose: the important check is that `WorkspaceWrite`
/// continues to deny writes outside the workspace even under `zsh-fork`.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_zsh_fork_still_enforces_workspace_write_sandbox() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let Some(runtime) = zsh_fork_runtime("zsh-fork workspace sandbox test")? else {
        return Ok(());
    };

    let server = start_mock_server().await;
    let tool_call_id = "zsh-fork-workspace-write-deny";
    let outside_path = "/tmp/codex-zsh-fork-workspace-write-deny.txt";
    let workspace_write_policy = restrictive_workspace_write_policy();
    let _ = fs::remove_file(outside_path);
    let test = build_zsh_fork_test(
        &server,
        runtime,
        AskForApproval::Never,
        workspace_write_policy.clone(),
        move |_| {
            let _ = fs::remove_file(outside_path);
        },
    )
    .await?;

    let command = format!("touch {outside_path}");
    let arguments = shell_command_arguments(&command)?;
    let mocks =
        mount_function_call_agent_response(&server, tool_call_id, &arguments, "shell_command")
            .await;

    submit_turn_with_policies(
        &test,
        "write outside workspace with zsh fork",
        AskForApproval::Never,
        workspace_write_policy,
    )
    .await?;

    wait_for_turn_complete(&test).await;

    let call_output = mocks
        .completion
        .single_request()
        .function_call_output(tool_call_id);
    let output = call_output["output"].as_str().unwrap_or_default();
    assert!(
        output_shows_sandbox_denial(output),
        "expected sandbox denial, got output: {output:?}"
    );
    assert!(
        !Path::new(outside_path).exists(),
        "command should not write outside workspace under WorkspaceWrite policy"
    );

    Ok(())
}
