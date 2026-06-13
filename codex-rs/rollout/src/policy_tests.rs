use super::*;
use codex_protocol::parse_command::ParsedCommand;
use codex_protocol::protocol::ExecCommandEndEvent;
use codex_protocol::protocol::ExecCommandSource;
use codex_protocol::protocol::ExecCommandStatus;
use pretty_assertions::assert_eq;
use serde_json::Value;
use std::time::Duration;

#[test]
fn persists_sanitized_exec_command_completion() {
    let large_output = "😀0123456789".repeat(2_000);
    let input_event = ExecCommandEndEvent {
        call_id: "exec-1".to_string(),
        process_id: Some("process-1".to_string()),
        turn_id: "turn-1".to_string(),
        completed_at_ms: 42,
        command: vec![
            "pwsh.exe".to_string(),
            "-Command".to_string(),
            "pwd".to_string(),
        ],
        cwd: serde_json::from_value(Value::String("file:///C:/Research/workspace".to_string()))
            .expect("Windows cwd URI"),
        path_convention: serde_json::from_value(Value::String("windows".to_string()))
            .expect("Windows path convention"),
        parsed_cmd: vec![ParsedCommand::Unknown {
            cmd: "pwsh.exe -Command pwd".to_string(),
        }],
        source: ExecCommandSource::Agent,
        interaction_input: None,
        stdout: large_output.clone(),
        stderr: "stderr copy".to_string(),
        aggregated_output: large_output,
        exit_code: 0,
        duration: Duration::from_millis(250),
        formatted_output: "formatted copy".to_string(),
        status: ExecCommandStatus::Completed,
    };
    let input = RolloutItem::EventMsg(EventMsg::ExecCommandEnd(input_event.clone()));

    let persisted = persisted_rollout_items(std::slice::from_ref(&input));

    let mut expected_event = input_event;
    expected_event.aggregated_output =
        truncate_persisted_exec_output(&expected_event.aggregated_output);
    expected_event.stdout.clear();
    expected_event.stderr.clear();
    expected_event.formatted_output.clear();
    let expected = vec![RolloutItem::EventMsg(EventMsg::ExecCommandEnd(
        expected_event,
    ))];
    let RolloutItem::EventMsg(EventMsg::ExecCommandEnd(persisted_event)) = &persisted[0] else {
        panic!("expected persisted command completion")
    };
    assert!(persisted_event.aggregated_output.len() <= PERSISTED_EXEC_AGGREGATED_OUTPUT_MAX_BYTES);
    assert_eq!(
        serde_json::to_value(persisted).expect("serialize persisted items"),
        serde_json::to_value(expected).expect("serialize expected items"),
    );
}
