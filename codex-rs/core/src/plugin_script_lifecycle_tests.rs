use std::thread;
use std::time::Duration;
use std::time::Instant;

use codex_analytics::AnalyticsEventsClient;
use codex_analytics::CodexPluginScriptLifecycleEvent;
use codex_analytics::PluginScriptLifecycleStatus;
use pretty_assertions::assert_eq;

use super::*;

fn execution() -> PluginScriptExecution {
    PluginScriptExecution::new(
        AnalyticsEventsClient::disabled(),
        PluginScriptEvent {
            session_id: "session".to_string(),
            thread_id: "thread".to_string(),
            turn_id: "turn".to_string(),
            product_client_id: "codex-test".to_string(),
            plugin_id: "plugin".to_string(),
            execution_id: "execution".to_string(),
            script_path: "scripts/run.py".to_string(),
            skill: None,
        },
    )
}

fn emitted(execution: &PluginScriptExecution) -> Vec<CodexPluginScriptLifecycleEvent> {
    execution
        .emitted
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

fn statuses(execution: &PluginScriptExecution) -> Vec<&'static str> {
    emitted(execution)
        .into_iter()
        .map(|event| match event.status {
            PluginScriptLifecycleStatus::Started => "started",
            PluginScriptLifecycleStatus::Completed => "completed",
            PluginScriptLifecycleStatus::Failed => "failed",
            PluginScriptLifecycleStatus::Interrupted => "interrupted",
        })
        .collect()
}

#[test]
fn finish_before_start_emits_no_transition() {
    let execution = execution();

    execution.finish(PluginScriptTerminalOutcome::Exited { exit_code: 0 });

    assert_eq!(statuses(&execution), Vec::<&str>::new());
}

#[test]
fn start_is_idempotent() {
    let execution = execution();

    execution.mark_started();
    execution.mark_started();

    assert_eq!(statuses(&execution), vec!["started"]);
}

#[test]
fn exit_zero_completes() {
    let execution = execution();

    execution.mark_started();
    execution.finish(PluginScriptTerminalOutcome::Exited { exit_code: 0 });

    assert_eq!(statuses(&execution), vec!["started", "completed"]);
}

#[test]
fn nonzero_exit_and_failed_outcome_fail() {
    for outcome in [
        PluginScriptTerminalOutcome::Exited { exit_code: 9 },
        PluginScriptTerminalOutcome::Failed { exit_code: None },
    ] {
        let execution = execution();
        execution.mark_started();
        execution.finish(outcome);

        assert_eq!(statuses(&execution), vec!["started", "failed"]);
    }
}

#[test]
fn cancellation_wins_over_terminal_classification() {
    let execution = execution();

    execution.mark_started();
    execution.mark_interrupted();
    execution.finish(PluginScriptTerminalOutcome::Exited { exit_code: 0 });

    assert_eq!(statuses(&execution), vec!["started", "interrupted"]);
}

#[test]
fn terminal_transition_is_idempotent() {
    let execution = execution();

    execution.mark_started();
    execution.finish(PluginScriptTerminalOutcome::Exited { exit_code: 0 });
    execution.finish(PluginScriptTerminalOutcome::Failed { exit_code: Some(9) });

    assert_eq!(statuses(&execution), vec!["started", "completed"]);
}

#[test]
fn duration_begins_at_actual_start() {
    let execution = execution();
    let before_pre_start_delay = Instant::now();

    thread::sleep(Duration::from_millis(25));
    execution.mark_started();
    execution.finish(PluginScriptTerminalOutcome::Exited { exit_code: 0 });

    let duration_ms = emitted(&execution)[1].duration_ms.expect("duration");
    let total_elapsed_ms = before_pre_start_delay.elapsed().as_millis();
    assert!(u128::from(duration_ms) < total_elapsed_ms);
}

#[test]
fn start_and_terminal_facts_retain_execution_and_session_identity() {
    let execution = execution();

    execution.mark_started();
    execution.finish(PluginScriptTerminalOutcome::Exited { exit_code: 0 });
    let events = emitted(&execution);

    assert_eq!(events[0].execution_id, events[1].execution_id);
    assert_eq!(events[0].session_id, "session");
    assert_eq!(events[1].session_id, "session");
    assert_eq!(events[0].product_client_id, "codex-test");
    assert_eq!(events[1].product_client_id, "codex-test");
}
