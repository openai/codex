use std::fs;
use std::path::Path;

use anyhow::Result;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_core::protocol::SandboxPolicy;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::get_responses_requests;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::test_codex::TestCodexHarness;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;
use serial_test::serial;

struct ScopedEnvVar {
    key: &'static str,
    previous: Option<std::ffi::OsString>,
}

impl ScopedEnvVar {
    fn set(key: &'static str, value: impl AsRef<std::ffi::OsStr>) -> Self {
        let previous = std::env::var_os(key);
        // `set_var` is `unsafe` on newer Rust because env mutation is process-wide.
        unsafe {
            std::env::set_var(key, value);
        }
        Self { key, previous }
    }
}

impl Drop for ScopedEnvVar {
    fn drop(&mut self) {
        match &self.previous {
            Some(val) => unsafe {
                std::env::set_var(self.key, val);
            },
            None => unsafe {
                std::env::remove_var(self.key);
            },
        }
    }
}

fn write_agent(home: &Path, name: &str) {
    let agents_dir = home.join("agents");
    fs::create_dir_all(&agents_dir).unwrap_or_else(|err| {
        panic!(
            "failed to create agents dir {}: {err}",
            agents_dir.display()
        );
    });
    let path = agents_dir.join(format!("{name}.md"));
    fs::write(
        &path,
        r#"---
description: "test agent"
color: cyan
---
You are a test subagent.
"#,
    )
    .unwrap_or_else(|err| panic!("failed to write agent file {}: {err}", path.display()));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial]
async fn subagent_directive_is_visible_to_main_and_subagent_runs_without_history() -> Result<()> {
    let harness = TestCodexHarness::new().await?;

    // Ensure subagent resolution reads from this test's CODEX_HOME.
    let _env = ScopedEnvVar::set("CODEX_HOME", harness.test().codex_home_path());
    write_agent(harness.test().codex_home_path(), "general-purpose");

    // Seed the main conversation with a prior turn so we can assert it does not leak into the
    // subagent request.
    let secret = "DO_NOT_LEAK_TO_SUBAGENT";

    let call_id = "call-subagent-1";
    let args = r#"{"name":"general-purpose","prompt":"do it"}"#;

    // Turn 1 (main): assistant response to seed history.
    // Turn 2 (main): request the subagent tool.
    // Turn 2 (subagent): returns output.
    // Turn 2 (main follow-up): produces final assistant message after tool output.
    mount_sse_sequence(
        harness.server(),
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_assistant_message("msg-1", "seed ack"),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_function_call(call_id, "run_subagent", args),
                ev_completed("resp-2"),
            ]),
            sse(vec![
                ev_response_created("resp-3"),
                ev_assistant_message("msg-2", "subagent output"),
                ev_completed("resp-3"),
            ]),
            sse(vec![
                ev_response_created("resp-4"),
                ev_assistant_message("msg-3", "main output"),
                ev_completed("resp-4"),
            ]),
        ],
    )
    .await;

    let test = harness.test();

    // Turn 1: create main history containing the secret.
    test.codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: format!("seed {secret}"),
            }],
            final_output_json_schema: None,
            cwd: test.cwd_path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: test.session_configured.model.clone(),
            effort: None,
            summary: ReasoningSummary::Auto,
        })
        .await?;

    let _ = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::TaskComplete(_) => Some(()),
        _ => None,
    })
    .await;

    // Turn 2: @ directive that should cause the main model to call run_subagent.
    test.codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "@general-purpose do it".into(),
            }],
            final_output_json_schema: None,
            cwd: test.cwd_path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: test.session_configured.model.clone(),
            effort: None,
            summary: ReasoningSummary::Auto,
        })
        .await?;

    let last_message = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::TaskComplete(ev) => Some(ev.last_agent_message.clone()),
        _ => None,
    })
    .await;

    assert_eq!(last_message, Some("main output".to_string()));

    let reqs = get_responses_requests(harness.server()).await;
    assert_eq!(
        reqs.len(),
        4,
        "expected four /responses calls (seed main, main, subagent, main)"
    );

    let bodies = reqs
        .into_iter()
        .map(|req| {
            req.body_json::<serde_json::Value>()
                .expect("valid JSON body")
        })
        .collect::<Vec<_>>();

    let bodies_text = bodies
        .iter()
        .map(serde_json::Value::to_string)
        .collect::<Vec<_>>();
    assert!(
        bodies_text
            .iter()
            .any(|t| t.contains("@general-purpose do it")),
        "expected main request to include the user's @ directive"
    );
    assert!(
        bodies_text.iter().any(|t| t.contains(secret)),
        "expected main requests to include the prior turn content"
    );

    let subagent_prompt_marker = "You are a test subagent.";
    let subagent_bodies = bodies_text
        .iter()
        .filter(|t| t.contains(subagent_prompt_marker))
        .collect::<Vec<_>>();
    assert_eq!(
        subagent_bodies.len(),
        1,
        "expected exactly one subagent request"
    );
    assert!(
        !subagent_bodies[0].contains(secret),
        "expected subagent request not to include main conversation history"
    );

    // The follow-up main request must include the tool output for the call id.
    let follow_up = bodies
        .iter()
        .find(|b| {
            b.get("input")
                .and_then(|v| v.as_array())
                .is_some_and(|items| {
                    items.iter().any(|item| {
                        item.get("type").and_then(|v| v.as_str()) == Some("function_call_output")
                            && item.get("call_id").and_then(|v| v.as_str()) == Some(call_id)
                    })
                })
        })
        .expect("expected a follow-up main request containing function_call_output");
    let follow_up_text = follow_up.to_string();
    assert!(
        follow_up_text.contains("Subagent: @general-purpose"),
        "expected tool output to include subagent label"
    );

    Ok(())
}
