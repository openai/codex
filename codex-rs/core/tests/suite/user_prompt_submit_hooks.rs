#![cfg(not(target_os = "windows"))]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use anyhow::Result;
use codex_core::features::Feature;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;

use std::fs;
use std::path::Path;

fn write_hooks_file(home: &Path, contents: &str) {
    fs::write(home.join("hooks.json"), contents).expect("write hooks.json");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_prompt_submit_hook_injects_context_before_model_request() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let hook_context = "Follow the team checklist.";
    let hook_json = format!(
        r#"{{
  "hooks": {{
    "UserPromptSubmit": [
      {{
        "matcher": "^hello",
        "hooks": [
          {{
            "type": "command",
            "command": "printf '{hook_context}'",
            "statusMessage": "checking prompt"
          }}
        ]
      }}
    ]
  }}
}}"#
    );
    let mut builder = test_codex()
        .with_pre_build_hook(move |home| write_hooks_file(home, &hook_json))
        .with_config(|config| {
            config
                .features
                .enable(Feature::CodexHooks)
                .expect("enable codex hooks feature");
        });
    let test = builder.build(&server).await?;

    let mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-1"),
        ]),
    )
    .await;

    let session_model = test.session_configured.model.clone();
    test.codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "hello from the beach".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: test.cwd_path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model,
            effort: None,
            summary: None,
            service_tier: None,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    let started = wait_for_event_match(test.codex.as_ref(), |event| match event {
        EventMsg::HookStarted(event) => Some(event.clone()),
        _ => None,
    })
    .await;
    assert_eq!(
        started.run.event_name,
        codex_protocol::protocol::HookEventName::UserPromptSubmit
    );

    let completed = wait_for_event_match(test.codex.as_ref(), |event| match event {
        EventMsg::HookCompleted(event) => Some(event.clone()),
        _ => None,
    })
    .await;
    assert_eq!(
        completed.run.event_name,
        codex_protocol::protocol::HookEventName::UserPromptSubmit
    );

    wait_for_event(test.codex.as_ref(), |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let request = mock.single_request();
    assert!(
        request
            .message_input_texts("developer")
            .iter()
            .any(|text| text.contains(hook_context)),
        "expected hook context in developer input"
    );
    assert!(
        request
            .message_input_texts("user")
            .iter()
            .any(|text| text == "hello from the beach"),
        "expected original user prompt in request"
    );

    Ok(())
}
