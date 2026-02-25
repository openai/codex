#![allow(clippy::expect_used, clippy::unwrap_used)]

use anyhow::Result;
use codex_core::features::Feature;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
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
        format!("---\nname: {name}\ndescription: {name} skill\n---\n"),
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
    let session_model = test.session_configured.model.clone();
    test.codex
        .submit(Op::UserTurn {
            items: vec![codex_protocol::user_input::UserInput::Text {
                text: prompt.to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: test.cwd_path().to_path_buf(),
            approval_policy: codex_protocol::protocol::AskForApproval::Never,
            sandbox_policy: codex_protocol::protocol::SandboxPolicy::DangerFullAccess,
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
                "from pathlib import Path\nPath({marker_path:?}).write_text('ran')\nprint('ran')\n"
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
