#![allow(clippy::expect_used)]

use super::compact::COMPACT_WARNING_MESSAGE;
use anyhow::Result;
use codex_core::CodexThread;
use codex_core::compact::SUMMARIZATION_PROMPT;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::WarningEvent;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ResponsesRequest;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use std::sync::Arc;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn window_id_advances_after_compact_persists_on_resume_and_resets_on_fork() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let request_log = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_assistant_message("msg-1", "first reply"),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_assistant_message("msg-2", "summary"),
                ev_completed("resp-2"),
            ]),
            sse(vec![ev_completed("resp-3")]),
            sse(vec![ev_completed("resp-4")]),
            sse(vec![ev_completed("resp-5")]),
        ],
    )
    .await;

    let mut builder = test_codex().with_config(|config| {
        config.model_provider.name = "Non-OpenAI Model provider".to_string();
        config.compact_prompt = Some(SUMMARIZATION_PROMPT.to_string());
    });
    let initial = builder.build(&server).await?;
    let initial_thread = Arc::clone(&initial.codex);
    let rollout_path = initial
        .session_configured
        .rollout_path
        .clone()
        .expect("rollout path");

    submit_user_turn(&initial_thread, "before compact").await?;
    submit_compact_turn(&initial_thread).await?;
    submit_user_turn(&initial_thread, "after compact").await?;
    shutdown_thread(&initial_thread).await?;

    let resumed = builder
        .resume(&server, initial.home.clone(), rollout_path.clone())
        .await?;
    submit_user_turn(&resumed.codex, "after resume").await?;
    shutdown_thread(&resumed.codex).await?;

    let forked = resumed
        .thread_manager
        .fork_thread(
            /*snapshot*/ 0usize,
            resumed.config.clone(),
            rollout_path,
            /*persist_extended_history*/ false,
            /*parent_trace*/ None,
        )
        .await?;
    submit_user_turn(&forked.thread, "after fork").await?;
    shutdown_thread(&forked.thread).await?;

    let requests = request_log.requests();
    assert_eq!(requests.len(), 5, "expected five model requests");

    let (initial_thread_id, first_generation) = window_id_parts(&requests[0]);
    let (compact_thread_id, compact_generation) = window_id_parts(&requests[1]);
    let (after_compact_thread_id, after_compact_generation) = window_id_parts(&requests[2]);
    let (after_resume_thread_id, after_resume_generation) = window_id_parts(&requests[3]);
    let (after_fork_thread_id, after_fork_generation) = window_id_parts(&requests[4]);

    assert_eq!(first_generation, 0);
    assert_eq!(compact_thread_id, initial_thread_id);
    assert_eq!(compact_generation, 0);
    assert_eq!(after_compact_thread_id, initial_thread_id);
    assert_eq!(after_compact_generation, 1);
    assert_eq!(after_resume_thread_id, initial_thread_id);
    assert_eq!(after_resume_generation, 1);
    assert_ne!(after_fork_thread_id, initial_thread_id);
    assert_eq!(after_fork_generation, 0);

    Ok(())
}

async fn submit_user_turn(codex: &Arc<CodexThread>, text: &str) -> Result<()> {
    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: text.to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            submission_type: None,
        })
        .await?;
    wait_for_event(codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    Ok(())
}

async fn submit_compact_turn(codex: &Arc<CodexThread>) -> Result<()> {
    codex.submit(Op::Compact).await?;
    let warning_event = wait_for_event(codex, |event| matches!(event, EventMsg::Warning(_))).await;
    let EventMsg::Warning(WarningEvent { message }) = warning_event else {
        panic!("expected warning event after compact");
    };
    assert_eq!(message, COMPACT_WARNING_MESSAGE);
    wait_for_event(codex, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    Ok(())
}

async fn shutdown_thread(codex: &Arc<CodexThread>) -> Result<()> {
    codex.submit(Op::Shutdown).await?;
    wait_for_event(codex, |event| matches!(event, EventMsg::ShutdownComplete)).await;
    Ok(())
}

fn window_id_parts(request: &ResponsesRequest) -> (String, u64) {
    let window_id = request
        .header("x-codex-window-id")
        .expect("missing x-codex-window-id header");
    let (thread_id, generation) = window_id
        .rsplit_once(':')
        .unwrap_or_else(|| panic!("invalid window id header: {window_id}"));
    let generation = generation
        .parse::<u64>()
        .unwrap_or_else(|err| panic!("invalid window generation in {window_id}: {err}"));
    (thread_id.to_string(), generation)
}
