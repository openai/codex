use pretty_assertions::assert_eq;

use super::AsyncCommandRuntime;
use super::AsyncHookCompletion;
use super::MAX_DELIVERED_OUTPUT_TOKENS_PER_TURN;
use super::MAX_IN_FLIGHT_COMMANDS;
use crate::engine::CommandShell;
use crate::engine::ConfiguredHandler;
use codex_protocol::ThreadId;
use codex_protocol::protocol::HookEventName;
use codex_protocol::protocol::HookExecutionMode;
use codex_protocol::protocol::HookSource;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::collections::HashMap;

enum TestOutput<'a> {
    AdditionalContext(&'a str),
    SystemMessage(&'a str),
}

fn complete(
    runtime: &AsyncCommandRuntime,
    launch_sequence: u64,
    deliver_at_generation: u64,
    output: TestOutput<'_>,
) {
    let mut state = runtime.inner.state.lock().expect("async hook state");
    let ready_sequence = state.next_ready_sequence;
    state.next_ready_sequence += 1;
    let (additional_context, system_message) = match output {
        TestOutput::AdditionalContext(text) => (Some(text.to_string()), None),
        TestOutput::SystemMessage(text) => (None, Some(text.to_string())),
    };
    state.completions.insert(
        launch_sequence,
        AsyncHookCompletion {
            deliver_at_generation,
            ready_sequence,
            additional_context,
            system_message,
        },
    );
}

#[test]
fn completion_after_cutoff_waits_for_following_accepted_turn() {
    let runtime = AsyncCommandRuntime::new();
    let cutoff = runtime.delivery_cutoff();
    complete(
        &runtime,
        /*launch_sequence*/ 0,
        /*deliver_at_generation*/ 1,
        TestOutput::AdditionalContext("late"),
    );

    assert_eq!(
        runtime.commit_accepted_turn_and_drain(cutoff),
        Default::default()
    );

    let delivery = runtime.commit_accepted_turn_and_drain(runtime.delivery_cutoff());
    assert_eq!(delivery.additional_contexts, vec!["late"]);
}

#[test]
fn unfinished_earlier_launch_does_not_block_ready_output() {
    let runtime = AsyncCommandRuntime::new();
    complete(
        &runtime,
        /*launch_sequence*/ 1,
        /*deliver_at_generation*/ 1,
        TestOutput::AdditionalContext("ready"),
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
fn shared_output_budget_leaves_remaining_completions_queued() {
    let runtime = AsyncCommandRuntime::new();
    let output = "x".repeat(MAX_DELIVERED_OUTPUT_TOKENS_PER_TURN);
    complete(
        &runtime,
        /*launch_sequence*/ 0,
        /*deliver_at_generation*/ 1,
        TestOutput::AdditionalContext(&output),
    );
    complete(
        &runtime,
        /*launch_sequence*/ 1,
        /*deliver_at_generation*/ 1,
        TestOutput::SystemMessage(&output),
    );
    complete(
        &runtime,
        /*launch_sequence*/ 2,
        /*deliver_at_generation*/ 1,
        TestOutput::AdditionalContext(&output),
    );
    complete(
        &runtime,
        /*launch_sequence*/ 3,
        /*deliver_at_generation*/ 1,
        TestOutput::SystemMessage(&output),
    );
    complete(
        &runtime,
        /*launch_sequence*/ 4,
        /*deliver_at_generation*/ 1,
        TestOutput::AdditionalContext(&output),
    );

    let first_delivery = runtime.commit_accepted_turn_and_drain(runtime.delivery_cutoff());
    assert_eq!(
        first_delivery,
        super::AsyncHookDelivery {
            additional_contexts: vec![output.clone(), output.clone()],
            system_messages: vec![output.clone(), output.clone()],
        }
    );

    let second_delivery = runtime.commit_accepted_turn_and_drain(runtime.delivery_cutoff());
    assert_eq!(second_delivery.additional_contexts, vec![output]);
}
