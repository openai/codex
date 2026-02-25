#![allow(clippy::expect_used, clippy::unwrap_used)]

use anyhow::Result;
use codex_core::features::Feature;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::skill_approval::SkillApprovalResponse;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_function_call_agent_response;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
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

fn write_skill_with_script(home: &Path, name: &str, script_body: &str) -> Result<PathBuf> {
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
    let script_path = scripts_dir.join("run.py");
    fs::write(&script_path, script_body)?;
    Ok(script_path)
}

fn shell_command_arguments(command: &str) -> Result<String> {
    Ok(serde_json::to_string(&json!({
        "command": command,
        "timeout_ms": 500,
    }))?)
}

fn assistant_response(message: &str) -> String {
    sse(vec![
        ev_response_created("resp-2"),
        ev_assistant_message("msg-1", message),
        ev_completed("resp-2"),
    ])
}

fn command_for_script(script_path: &Path) -> Result<String> {
    let runner = if cfg!(windows) { "python" } else { "python3" };
    let script_path = script_path.to_string_lossy().into_owned();
    Ok(shlex::try_join([runner, script_path.as_str()])?)
}

async fn submit_turn(test: &TestCodex, prompt: &str) -> Result<()> {
    submit_turn_with_policies(
        test,
        prompt,
        AskForApproval::Never,
        SandboxPolicy::DangerFullAccess,
    )
    .await
}

async fn submit_turn_with_policies(
    test: &TestCodex,
    prompt: &str,
    approval_policy: AskForApproval,
    sandbox_policy: SandboxPolicy,
) -> Result<()> {
    let session_model = test.session_configured.model.clone();
    test.codex
        .submit(Op::UserTurn {
            items: vec![codex_protocol::user_input::UserInput::Text {
                text: prompt.to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: test.cwd_path().to_path_buf(),
            approval_policy,
            sandbox_policy,
            model: session_model,
            effort: None,
            summary: codex_protocol::config_types::ReasoningSummary::Auto,
            collaboration_mode: None,
            personality: None,
        })
        .await?;
    Ok(())
}

async fn wait_for_turn_complete_without_skill_approval(test: &TestCodex) {
    wait_for_event_match(test.codex.as_ref(), |event| match event {
        EventMsg::SkillRequestApproval(request) => {
            panic!("unexpected skill approval request: {request:?}");
        }
        EventMsg::TurnComplete(_) => Some(()),
        _ => None,
    })
    .await;
}

#[cfg(unix)]
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

#[cfg(unix)]
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

#[cfg(unix)]
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn skill_approval_event_round_trip_for_shell_command_skill_script_exec() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let tool_call_id = "shell-skill-call";
    let mut builder = test_codex()
        .with_pre_build_hook(|home| {
            write_skill_with_script(home, "demo", "print('shell skill approved')").unwrap();
        })
        .with_config(|config| {
            config.features.enable(Feature::SkillApproval);
        });
    let test = builder.build(&server).await?;
    let script_path = test.codex_home_path().join("skills/demo/scripts/run.py");
    let command = command_for_script(&script_path)?;
    let arguments = shell_command_arguments(&command)?;
    let _mocks =
        mount_function_call_agent_response(&server, tool_call_id, &arguments, "shell_command")
            .await;

    submit_turn(&test, "run the shell skill").await?;

    let request = wait_for_event_match(test.codex.as_ref(), |event| match event {
        EventMsg::SkillRequestApproval(request) => Some(request.clone()),
        _ => None,
    })
    .await;
    assert_eq!(request.item_id, tool_call_id);
    assert_eq!(request.skill_name, "demo");

    test.codex
        .submit(Op::SkillApproval {
            id: request.item_id,
            response: SkillApprovalResponse { approved: true },
        })
        .await?;

    wait_for_event(test.codex.as_ref(), |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn skill_approval_not_emitted_without_skill_script_exec() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let tool_call_id = "non-skill-call";
    let mut builder = test_codex().with_config(|config| {
        config.features.enable(Feature::SkillApproval);
    });
    let test = builder.build(&server).await?;
    let arguments = shell_command_arguments("echo no-skill")?;
    let _mocks =
        mount_function_call_agent_response(&server, tool_call_id, &arguments, "shell_command")
            .await;

    submit_turn(&test, "run a plain command").await?;
    wait_for_turn_complete_without_skill_approval(&test).await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn skill_approval_decline_blocks_execution() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let tool_call_id = "decline-call";
    let marker_name = "declined-marker.txt";
    let mut builder = test_codex()
        .with_pre_build_hook(move |home| {
            let marker_path = home.join(marker_name);
            let marker_path = marker_path.to_string_lossy();
            let script_body = format!(
                r#"from pathlib import Path
Path({marker_path:?}).write_text('ran')
print('ran')
"#
            );
            write_skill_with_script(home, "demo", &script_body).unwrap();
        })
        .with_config(|config| {
            config.features.enable(Feature::SkillApproval);
        });
    let test = builder.build(&server).await?;
    let script_path = test.codex_home_path().join("skills/demo/scripts/run.py");
    let command = command_for_script(&script_path)?;
    let arguments = shell_command_arguments(&command)?;
    let mocks =
        mount_function_call_agent_response(&server, tool_call_id, &arguments, "shell_command")
            .await;

    submit_turn(&test, "run the skill").await?;

    let request = wait_for_event_match(test.codex.as_ref(), |event| match event {
        EventMsg::SkillRequestApproval(request) => Some(request.clone()),
        _ => None,
    })
    .await;
    assert_eq!(request.item_id, tool_call_id);

    test.codex
        .submit(Op::SkillApproval {
            id: request.item_id,
            response: SkillApprovalResponse { approved: false },
        })
        .await?;

    wait_for_event(test.codex.as_ref(), |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let marker_path = test.codex_home_path().join(marker_name);
    assert!(
        !marker_path.exists(),
        "declined skill approval should block script execution"
    );

    let call_output = mocks
        .completion
        .single_request()
        .function_call_output(tool_call_id);
    assert_eq!(
        call_output["output"].as_str(),
        Some("This script is part of the skill and the user declined the skill usage"),
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn skill_approval_cache_is_per_skill() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let first_call_id = "skill-a-1";
    let second_call_id = "skill-a-2";
    let third_call_id = "skill-b-1";
    let mut builder = test_codex()
        .with_pre_build_hook(|home| {
            write_skill_with_script(home, "alpha", "print('alpha')").unwrap();
            write_skill_with_script(home, "beta", "print('beta')").unwrap();
        })
        .with_config(|config| {
            config.features.enable(Feature::SkillApproval);
        });
    let test = builder.build(&server).await?;
    let alpha_command =
        command_for_script(&test.codex_home_path().join("skills/alpha/scripts/run.py"))?;
    let beta_command =
        command_for_script(&test.codex_home_path().join("skills/beta/scripts/run.py"))?;
    let first_alpha_arguments = shell_command_arguments(&alpha_command)?;
    let second_alpha_arguments = shell_command_arguments(&alpha_command)?;
    let beta_arguments = shell_command_arguments(&beta_command)?;

    mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(first_call_id, "shell_command", &first_alpha_arguments),
                ev_completed("resp-1"),
            ]),
            assistant_response("alpha-1"),
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(second_call_id, "shell_command", &second_alpha_arguments),
                ev_completed("resp-1"),
            ]),
            assistant_response("alpha-2"),
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(third_call_id, "shell_command", &beta_arguments),
                ev_completed("resp-1"),
            ]),
            assistant_response("beta-1"),
        ],
    )
    .await;

    submit_turn(&test, "run alpha").await?;
    let first_request = wait_for_event_match(test.codex.as_ref(), |event| match event {
        EventMsg::SkillRequestApproval(request) => Some(request.clone()),
        _ => None,
    })
    .await;
    assert_eq!(first_request.item_id, first_call_id);
    assert_eq!(first_request.skill_name, "alpha");
    test.codex
        .submit(Op::SkillApproval {
            id: first_request.item_id,
            response: SkillApprovalResponse { approved: true },
        })
        .await?;
    wait_for_event(test.codex.as_ref(), |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    submit_turn(&test, "run alpha again").await?;
    wait_for_turn_complete_without_skill_approval(&test).await;

    submit_turn(&test, "run beta").await?;
    let third_request = wait_for_event_match(test.codex.as_ref(), |event| match event {
        EventMsg::SkillRequestApproval(request) => Some(request.clone()),
        _ => None,
    })
    .await;
    assert_eq!(third_request.item_id, third_call_id);
    assert_eq!(third_request.skill_name, "beta");
    test.codex
        .submit(Op::SkillApproval {
            id: third_request.item_id,
            response: SkillApprovalResponse { approved: true },
        })
        .await?;
    wait_for_event(test.codex.as_ref(), |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    Ok(())
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

    test.codex
        .submit(Op::ExecApproval {
            id: approval.effective_approval_id(),
            turn_id: None,
            decision: ReviewDecision::Approved,
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
        output.contains("zsh-fork-stdout"),
        "expected stdout marker in function_call_output: {output:?}"
    );
    assert!(
        output.contains("zsh-fork-stderr"),
        "expected stderr marker in function_call_output: {output:?}"
    );

    Ok(())
}
