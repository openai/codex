#![allow(clippy::unwrap_used)]
#![cfg(unix)]

use anyhow::Result;
use codex_core::features::Feature;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::models::FileSystemPermissions;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
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
            summary: ReasoningSummary::Auto,
            collaboration_mode: None,
            personality: None,
        })
        .await?;
    Ok(())
}

fn write_skill_with_shell_script(home: &Path, name: &str, script_name: &str) -> Result<PathBuf> {
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
    fs::write(
        &script_path,
        r#"#!/bin/sh
echo 'zsh-fork-stdout'
echo 'zsh-fork-stderr' >&2
"#,
    )?;
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

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_zsh_fork_prompts_for_skill_script_execution() -> Result<()> {
    use codex_config::Constrained;
    use codex_protocol::protocol::ReviewDecision;

    skip_if_no_network!(Ok(()));

    let Some(zsh_path) = find_test_zsh_path()? else {
        return Ok(());
    };
    if !supports_exec_wrapper_intercept(&zsh_path) {
        eprintln!(
            "skipping zsh-fork skill test: zsh does not support EXEC_WRAPPER intercepts ({})",
            zsh_path.display()
        );
        return Ok(());
    }
    let Ok(main_execve_wrapper_exe) = codex_utils_cargo_bin::cargo_bin("codex-execve-wrapper")
    else {
        eprintln!("skipping zsh-fork skill test: unable to resolve `codex-execve-wrapper` binary");
        return Ok(());
    };

    let server = start_mock_server().await;
    let tool_call_id = "zsh-fork-skill-call";
    let mut builder = test_codex()
        .with_pre_build_hook(|home| {
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
        })
        .with_config(move |config| {
            config.features.enable(Feature::ShellTool);
            config.features.enable(Feature::ShellZshFork);
            config.zsh_path = Some(zsh_path.clone());
            config.main_execve_wrapper_exe = Some(main_execve_wrapper_exe);
            config.permissions.allow_login_shell = false;
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config.permissions.sandbox_policy =
                Constrained::allow_any(SandboxPolicy::new_workspace_write_policy());
        });
    let test = builder.build(&server).await?;

    let script_path = fs::canonicalize(
        test.codex_home_path()
            .join("skills/mbolin-test-skill/scripts/hello-mbolin.sh"),
    )?;
    let script_path_str = script_path.to_string_lossy().into_owned();
    let command = shlex::try_join([script_path_str.as_str()])?;
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

    let maybe_approval = wait_for_event_match(test.codex.as_ref(), |event| match event {
        EventMsg::ExecApprovalRequest(request) => Some(Some(request.clone())),
        EventMsg::TurnComplete(_) => Some(None),
        _ => None,
    })
    .await;
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

    wait_for_event(test.codex.as_ref(), |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

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

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_zsh_fork_still_enforces_workspace_write_sandbox() -> Result<()> {
    use codex_config::Constrained;
    use codex_protocol::protocol::AskForApproval;

    skip_if_no_network!(Ok(()));

    let Some(zsh_path) = find_test_zsh_path()? else {
        return Ok(());
    };
    if !supports_exec_wrapper_intercept(&zsh_path) {
        eprintln!(
            "skipping zsh-fork sandbox test: zsh does not support EXEC_WRAPPER intercepts ({})",
            zsh_path.display()
        );
        return Ok(());
    }
    let Ok(main_execve_wrapper_exe) = codex_utils_cargo_bin::cargo_bin("codex-execve-wrapper")
    else {
        eprintln!(
            "skipping zsh-fork sandbox test: unable to resolve `codex-execve-wrapper` binary"
        );
        return Ok(());
    };

    let server = start_mock_server().await;
    let tool_call_id = "zsh-fork-workspace-write-deny";
    let outside_path = "/tmp/codex-zsh-fork-workspace-write-deny.txt";
    let workspace_write_policy = SandboxPolicy::WorkspaceWrite {
        writable_roots: Vec::new(),
        read_only_access: Default::default(),
        network_access: false,
        exclude_tmpdir_env_var: true,
        exclude_slash_tmp: true,
    };
    let policy_for_config = workspace_write_policy.clone();
    let _ = fs::remove_file(outside_path);
    let mut builder = test_codex()
        .with_pre_build_hook(move |_| {
            let _ = fs::remove_file(outside_path);
        })
        .with_config(move |config| {
            config.features.enable(Feature::ShellTool);
            config.features.enable(Feature::ShellZshFork);
            config.zsh_path = Some(zsh_path.clone());
            config.main_execve_wrapper_exe = Some(main_execve_wrapper_exe);
            config.permissions.allow_login_shell = false;
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::Never);
            config.permissions.sandbox_policy = Constrained::allow_any(policy_for_config);
        });
    let test = builder.build(&server).await?;

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

    wait_for_turn_complete_without_skill_approval(&test).await;

    let call_output = mocks
        .completion
        .single_request()
        .function_call_output(tool_call_id);
    let output = call_output["output"].as_str().unwrap_or_default();
    assert!(
        output.contains("Permission denied")
            || output.contains("Operation not permitted")
            || output.contains("Read-only file system"),
        "expected sandbox denial, got output: {output:?}"
    );
    assert!(
        !Path::new(outside_path).exists(),
        "command should not write outside workspace under WorkspaceWrite policy"
    );

    Ok(())
}
