#![allow(clippy::unwrap_used)]
#![cfg(unix)]

use anyhow::Result;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ExecApprovalRequestEvent;
use codex_protocol::protocol::GranularApprovalConfig;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::mount_function_call_agent_response;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use core_test_support::zsh_fork::build_zsh_fork_test;
use core_test_support::zsh_fork::restrictive_workspace_write_policy;
use core_test_support::zsh_fork::zsh_fork_runtime;
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
    Ok(serde_json::to_string(&serde_json::json!({
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
            approvals_reviewer: None,
            sandbox_policy,
            model: test.session_configured.model.clone(),
            effort: None,
            summary: None,
            service_tier: None,
            collaboration_mode: None,
            personality: None,
            submission_type: None,
        })
        .await?;
    Ok(())
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

fn skill_script_command(test: &TestCodex, script_name: &str) -> Result<String> {
    let script_path = fs::canonicalize(
        test.codex_home_path()
            .join("skills/mbolin-test-skill/scripts")
            .join(script_name),
    )?;
    Ok(shlex::try_join([script_path.to_string_lossy().as_ref()])?)
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

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_zsh_fork_skill_scripts_ignore_declared_permissions() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let Some(runtime) = zsh_fork_runtime("zsh-fork skill script ignores permissions test")? else {
        return Ok(());
    };

    let approval_policy = AskForApproval::Granular(GranularApprovalConfig {
        sandbox_approval: true,
        rules: true,
        skill_approval: false,
        request_permissions: true,
        mcp_elicitations: true,
    });
    let workspace_write_policy = restrictive_workspace_write_policy();
    let outside_dir = tempfile::tempdir_in(std::env::current_dir()?)?;
    let allowed_dir = outside_dir.path().join("allowed-output");
    fs::create_dir_all(&allowed_dir)?;
    let allowed_path = allowed_dir.join("allowed.txt");
    let allowed_path_quoted = shlex::try_join([allowed_path.to_string_lossy().as_ref()])?;
    let script_contents = format!(
        "#!/bin/sh\nprintf '%s' allowed > {allowed_path_quoted}\nif [ -f {allowed_path_quoted} ]; then cat {allowed_path_quoted}; fi\n"
    );
    let permissions_yaml = format!(
        "permissions:\n  file_system:\n    write:\n      - \"{}\"\n",
        allowed_dir.display()
    );

    let server = start_mock_server().await;
    let allowed_path_for_hook = allowed_path.clone();
    let script_contents_for_hook = script_contents.clone();
    let test = build_zsh_fork_test(
        &server,
        runtime,
        approval_policy,
        workspace_write_policy.clone(),
        move |home| {
            let _ = fs::remove_file(&allowed_path_for_hook);
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

    let command = skill_script_command(&test, "sandboxed.sh")?;
    let call_id = "zsh-fork-skill-script-ignores-permissions";
    let arguments = shell_command_arguments(&command)?;
    let mocks =
        mount_function_call_agent_response(&server, call_id, &arguments, "shell_command").await;

    submit_turn_with_policies(
        &test,
        "use $mbolin-test-skill",
        approval_policy,
        workspace_write_policy,
    )
    .await?;

    let approval = wait_for_exec_approval_request(&test).await;
    assert!(
        approval.is_none(),
        "expected skill script execution to skip the removed skill approval path"
    );

    wait_for_turn_complete(&test).await;

    let call_output = mocks
        .completion
        .single_request()
        .function_call_output(call_id);
    let output = call_output["output"].as_str().unwrap_or_default();
    assert!(
        !output.contains("Execution denied: Execution forbidden by policy"),
        "skill script should now be governed by the turn sandbox, not the removed skill approval gate: {output:?}"
    );
    assert!(
        output_shows_sandbox_denial(output) || !output.contains("allowed"),
        "expected the turn sandbox to block the out-of-workspace write, got output: {output:?}"
    );
    assert!(
        !allowed_path.exists(),
        "declared skill permissions should not widen script execution beyond the turn sandbox"
    );

    Ok(())
}

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
