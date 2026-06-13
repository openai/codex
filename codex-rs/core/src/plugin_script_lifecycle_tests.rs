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
            thread_id: "thread".to_string(),
            turn_id: "turn".to_string(),
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
            PluginScriptLifecycleStatus::Cancelled => "cancelled",
        })
        .collect()
}

#[test]
fn finish_before_start_emits_no_transition() {
    let execution = execution();

    execution.finish(Some(0), /*failed*/ false);

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
    execution.finish(Some(0), /*failed*/ false);

    assert_eq!(statuses(&execution), vec!["started", "completed"]);
}

#[test]
fn nonzero_exit_and_executor_error_fail() {
    for (exit_code, failed) in [(Some(9), false), (None, true)] {
        let execution = execution();
        execution.mark_started();
        execution.finish(exit_code, failed);

        assert_eq!(statuses(&execution), vec!["started", "failed"]);
    }
}

#[test]
fn cancellation_wins_over_terminal_classification() {
    let execution = execution();

    execution.mark_started();
    execution.mark_cancelled();
    execution.finish(Some(0), /*failed*/ false);

    assert_eq!(statuses(&execution), vec!["started", "cancelled"]);
}

#[test]
fn terminal_transition_is_idempotent() {
    let execution = execution();

    execution.mark_started();
    execution.finish(Some(0), /*failed*/ false);
    execution.finish(Some(9), /*failed*/ true);

    assert_eq!(statuses(&execution), vec!["started", "completed"]);
}

#[test]
fn duration_begins_at_actual_start() {
    let execution = execution();
    let before_pre_start_delay = Instant::now();

    thread::sleep(Duration::from_millis(25));
    execution.mark_started();
    execution.finish(Some(0), /*failed*/ false);

    let duration_ms = emitted(&execution)[1].duration_ms.expect("duration");
    let total_elapsed_ms = before_pre_start_delay.elapsed().as_millis();
    assert!(u128::from(duration_ms) < total_elapsed_ms);
}

#[test]
fn start_and_terminal_facts_retain_one_execution_id() {
    let execution = execution();

    execution.mark_started();
    execution.finish(Some(0), /*failed*/ false);
    let events = emitted(&execution);

    assert_eq!(events[0].execution_id, events[1].execution_id);
}
