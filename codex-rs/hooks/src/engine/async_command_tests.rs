use pretty_assertions::assert_eq;

use super::AsyncCommandRuntime;
use super::AsyncHookCompletion;
use super::AsyncHookDelivery;
use super::MAX_DELIVERED_OUTPUT_TOKENS_PER_REQUEST;
use super::MAX_IN_FLIGHT_COMMANDS;
use crate::engine::CommandShell;
use crate::engine::ConfiguredHandler;
use crate::engine::output_parser::AsyncInformationalOutput;
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

fn context_delivery(text: impl Into<String>) -> AsyncHookDelivery {
    AsyncHookDelivery {
        additional_contexts: vec![text.into()],
        ..Default::default()
    }
}

fn complete(
    runtime: &AsyncCommandRuntime,
    launch_sequence: u64,
    deliver_at_generation: u64,
    output: TestOutput<'_>,
) {
    let mut state = runtime.inner.lock_state();
    let (additional_context, system_message) = match output {
        TestOutput::AdditionalContext(text) => (Some(text.to_string()), None),
        TestOutput::SystemMessage(text) => (None, Some(text.to_string())),
    };
    state.completions.insert(
        launch_sequence,
        AsyncHookCompletion {
            deliver_at_generation,
            output: AsyncInformationalOutput {
                additional_context,
                system_message,
            },
        },
    );
}

#[test]
fn completion_after_snapshot_waits_for_following_accepted_input() {
    let runtime = AsyncCommandRuntime::new();
    let pending = runtime.pending_delivery();
    complete(
        &runtime,
        /*launch_sequence*/ 0,
        /*deliver_at_generation*/ 1,
        TestOutput::AdditionalContext("late"),
    );

    assert_eq!(pending.accept_input(), Default::default());

    let delivery = runtime.pending_delivery().accept_input();
    assert_eq!(delivery, context_delivery("late"));
}

#[test]
fn startup_output_skips_first_accepted_input() {
    let runtime = AsyncCommandRuntime::new();
    complete(
        &runtime,
        /*launch_sequence*/ 0,
        /*deliver_at_generation*/ 2,
        TestOutput::AdditionalContext("startup"),
    );

    assert_eq!(
        runtime.pending_delivery().accept_input(),
        Default::default()
    );
    assert_eq!(
        runtime.pending_delivery().accept_input(),
        context_delivery("startup")
    );
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

    let delivery = runtime.pending_delivery().accept_input();
    assert_eq!(delivery, context_delivery("ready"));
}

#[tokio::test]
async fn launch_is_skipped_at_session_concurrency_limit() {
    let runtime = AsyncCommandRuntime::new();
    {
        let mut state = runtime.inner.lock_state();
        for _ in 0..MAX_IN_FLIGHT_COMMANDS {
            state.tasks.spawn(std::future::pending());
        }
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

    assert_eq!(runtime.inner.lock_state().next_launch_sequence, 0);
    runtime.shutdown().await;
}

#[test]
fn shared_output_budget_leaves_remaining_completions_queued() {
    let runtime = AsyncCommandRuntime::new();
    let output = "x".repeat(MAX_DELIVERED_OUTPUT_TOKENS_PER_REQUEST);
    for (launch_sequence, output) in (0_u64..).zip([
        TestOutput::AdditionalContext(&output),
        TestOutput::SystemMessage(&output),
        TestOutput::AdditionalContext(&output),
        TestOutput::SystemMessage(&output),
        TestOutput::AdditionalContext(&output),
    ]) {
        complete(
            &runtime,
            launch_sequence,
            /*deliver_at_generation*/ 1,
            output,
        );
    }

    let first_delivery = runtime.pending_delivery().accept_input();
    assert_eq!(
        first_delivery,
        super::AsyncHookDelivery {
            additional_contexts: vec![output.clone(), output.clone()],
            system_messages: vec![output.clone(), output.clone()],
        }
    );

    let second_delivery = runtime.pending_delivery().accept_input();
    assert_eq!(second_delivery, context_delivery(output));
}
