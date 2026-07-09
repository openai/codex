use pretty_assertions::assert_eq;

use super::AsyncCommandRuntime;
use super::AsyncHookDelivery;
use crate::engine::output_parser::AsyncInformationalOutput;

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

fn complete(runtime: &AsyncCommandRuntime, launch_sequence: u64, output: TestOutput<'_>) {
    let mut state = runtime.inner.lock_state();
    let (additional_context, system_message) = match output {
        TestOutput::AdditionalContext(text) => (Some(text.to_string()), None),
        TestOutput::SystemMessage(text) => (None, Some(text.to_string())),
    };
    state.completions.insert(
        launch_sequence,
        AsyncInformationalOutput {
            additional_context,
            system_message,
        },
    );
}

#[test]
fn completion_after_turn_snapshot_waits_for_following_turn() {
    let runtime = AsyncCommandRuntime::new();
    let pending = runtime.pending_delivery();
    complete(
        &runtime,
        /*launch_sequence*/ 0,
        TestOutput::AdditionalContext("late"),
    );

    assert_eq!(pending.accept_turn(), Default::default());

    let delivery = runtime.pending_delivery().accept_turn();
    assert_eq!(delivery, context_delivery("late"));
}

#[test]
fn unfinished_earlier_launch_does_not_block_ready_output() {
    let runtime = AsyncCommandRuntime::new();
    complete(
        &runtime,
        /*launch_sequence*/ 1,
        TestOutput::AdditionalContext("ready"),
    );

    let delivery = runtime.pending_delivery().accept_turn();
    assert_eq!(delivery, context_delivery("ready"));
}

#[test]
fn all_snapshotted_completions_are_delivered_together() {
    let runtime = AsyncCommandRuntime::new();
    for (launch_sequence, output) in (0_u64..).zip([
        TestOutput::AdditionalContext("first"),
        TestOutput::SystemMessage("notice"),
        TestOutput::AdditionalContext("second"),
    ]) {
        complete(&runtime, launch_sequence, output);
    }

    let delivery = runtime.pending_delivery().accept_turn();
    assert_eq!(
        delivery,
        super::AsyncHookDelivery {
            additional_contexts: vec!["first".to_string(), "second".to_string()],
            system_messages: vec!["notice".to_string()],
        }
    );
}
