use pretty_assertions::assert_eq;

use super::AsyncCommandRuntime;
use super::AsyncDeliveryTiming;
use super::AsyncHookCompletion;
use super::MAX_DELIVERED_CONTEXT_TOKENS_PER_TURN;
use super::MAX_IN_FLIGHT_COMMANDS;
use crate::engine::CommandShell;
use crate::engine::ConfiguredHandler;
use codex_protocol::ThreadId;
use codex_protocol::protocol::HookEventName;
use codex_protocol::protocol::HookExecutionMode;
use codex_protocol::protocol::HookSource;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::collections::HashMap;

fn complete(
    runtime: &AsyncCommandRuntime,
    launch_sequence: u64,
    deliver_at_generation: u64,
    text: &str,
) {
    let mut state = runtime.inner.state.lock().expect("async hook state");
    let ready_sequence = state.next_ready_sequence;
    state.next_ready_sequence += 1;
    state.completions.insert(
        launch_sequence,
        AsyncHookCompletion {
            deliver_at_generation,
            ready_sequence,
            additional_context: Some(text.to_string()),
            system_message: None,
        },
    );
}

#[test]
fn completion_after_cutoff_waits_for_following_accepted_turn() {
    let runtime = AsyncCommandRuntime::new();
    let cutoff = runtime.delivery_cutoff();
    complete(
        &runtime, /*launch_sequence*/ 0, /*deliver_at_generation*/ 1, "late",
    );

    assert_eq!(
        runtime.commit_accepted_turn_and_drain(cutoff),
        Default::default()
    );

    let delivery = runtime.commit_accepted_turn_and_drain(runtime.delivery_cutoff());
    assert_eq!(delivery.additional_contexts, vec!["late"]);
}

#[test]
fn startup_completion_skips_first_accepted_turn() {
    let runtime = AsyncCommandRuntime::new();
    complete(
        &runtime, /*launch_sequence*/ 0, /*deliver_at_generation*/ 2, "startup",
    );

    assert_eq!(
        runtime.commit_accepted_turn_and_drain(runtime.delivery_cutoff()),
        Default::default()
    );

    let delivery = runtime.commit_accepted_turn_and_drain(runtime.delivery_cutoff());
    assert_eq!(delivery.additional_contexts, vec!["startup"]);
}

#[test]
fn blocked_submission_does_not_advance_generation() {
    let runtime = AsyncCommandRuntime::new();
    complete(
        &runtime,
        /*launch_sequence*/ 0,
        /*deliver_at_generation*/ 1,
        "after block",
    );

    let delivery = runtime.commit_accepted_turn_and_drain(runtime.delivery_cutoff());
    assert_eq!(delivery.additional_contexts, vec!["after block"]);
}

#[test]
fn unfinished_earlier_launch_does_not_block_ready_output() {
    let runtime = AsyncCommandRuntime::new();
    complete(
        &runtime, /*launch_sequence*/ 1, /*deliver_at_generation*/ 1, "ready",
    );

    let delivery = runtime.commit_accepted_turn_and_drain(runtime.delivery_cutoff());
    assert_eq!(delivery.additional_contexts, vec!["ready"]);
}

#[tokio::test]
async fn launch_is_skipped_at_session_concurrency_limit() {
    let runtime = AsyncCommandRuntime::new();
    {
        let mut state = runtime.inner.state.lock().expect("async hook state");
        state.tasks = (0..MAX_IN_FLIGHT_COMMANDS)
            .map(|_| tokio::spawn(std::future::pending()))
            .collect();
    }

    runtime.spawn(
        CommandShell {
            program: String::new(),
            args: Vec::new(),
        },
        ConfiguredHandler {
            event_name: HookEventName::PreToolUse,
            matcher: None,
            command: "exit 0".to_string(),
            timeout_sec: 5,
            status_message: None,
            source_path: AbsolutePathBuf::current_dir().expect("current dir"),
            source: HookSource::User,
            display_order: 0,
            env: HashMap::new(),
            execution_mode: HookExecutionMode::Async,
        },
        String::new(),
        std::env::current_dir().expect("current dir"),
        ThreadId::new(),
        AsyncDeliveryTiming::NextAcceptedTurn,
    );

    assert_eq!(
        runtime
            .inner
            .next_launch_sequence
            .load(std::sync::atomic::Ordering::Acquire),
        0
    );
    runtime.shutdown().await;
}

#[test]
fn context_delivery_budget_leaves_remaining_completions_queued() {
    let runtime = AsyncCommandRuntime::new();
    let context = "x".repeat(MAX_DELIVERED_CONTEXT_TOKENS_PER_TURN);
    for launch_sequence in 0..5 {
        complete(
            &runtime,
            launch_sequence,
            /*deliver_at_generation*/ 1,
            &context,
        );
    }

    let first_delivery = runtime.commit_accepted_turn_and_drain(runtime.delivery_cutoff());
    assert_eq!(first_delivery.additional_contexts, vec![context.clone(); 4]);

    let second_delivery = runtime.commit_accepted_turn_and_drain(runtime.delivery_cutoff());
    assert_eq!(second_delivery.additional_contexts, vec![context]);
}
