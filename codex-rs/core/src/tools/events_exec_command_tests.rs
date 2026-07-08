use super::*;
use crate::session::tests::make_session_and_context_with_dynamic_tools_and_rx;
use pretty_assertions::assert_eq;
use tempfile::tempdir;
use test_case::test_case;

#[test_case(
    ExecCommandSource::UnifiedExecInteraction,
    Some("echo hello\n"),
    Some("1234");
    "unified interaction"
)]
#[test_case(ExecCommandSource::Agent, None, None; "non-unified command")]
#[tokio::test]
async fn command_lifecycle_is_canonical_then_legacy(
    source: ExecCommandSource,
    interaction_input: Option<&str>,
    process_id: Option<&str>,
) {
    let (session, turn, rx_event) =
        make_session_and_context_with_dynamic_tools_and_rx(Vec::new()).await;
    let dir = tempdir().expect("tempdir");
    let cwd = PathUri::from_host_native_path(dir.path()).expect("absolute cwd");
    let command = vec!["echo".to_string(), "hello".to_string()];
    let parsed_cmd = parse_command(&command);
    let call_id = "call-id";
    let exec_input = ExecCommandInput::new(
        &command,
        &cwd,
        &parsed_cmd,
        source,
        interaction_input,
        process_id,
    );
    let ctx = ToolEventCtx::new(
        session.as_ref(),
        turn.as_ref(),
        call_id,
        /*turn_diff_tracker*/ None,
    );

    emit_exec_command_begin(
        ctx,
        exec_input.command,
        exec_input.cwd,
        exec_input.parsed_cmd,
        exec_input.source,
        exec_input.interaction_input.map(str::to_owned),
        exec_input.process_id,
    )
    .await;
    emit_exec_end(
        ctx,
        exec_input,
        ExecCommandResult {
            stdout: "hello\n".to_string(),
            stderr: String::new(),
            aggregated_output: "hello\n".to_string(),
            exit_code: 0,
            duration: Duration::from_millis(25),
            formatted_output: "formatted output".to_string(),
            status: ExecCommandStatus::Completed,
        },
    )
    .await;

    let events = [
        rx_event.recv().await.expect("canonical start"),
        rx_event.recv().await.expect("legacy begin"),
        rx_event.recv().await.expect("canonical completion"),
        rx_event.recv().await.expect("legacy end"),
    ];
    assert!(rx_event.try_recv().is_err());

    let (
        EventMsg::ItemStarted(started),
        EventMsg::ExecCommandBegin(begin),
        EventMsg::ItemCompleted(completed),
        EventMsg::ExecCommandEnd(end),
    ) = (
        &events[0].msg,
        &events[1].msg,
        &events[2].msg,
        &events[3].msg,
    )
    else {
        panic!("unexpected command lifecycle");
    };
    let (TurnItem::CommandExecution(started_item), TurnItem::CommandExecution(completed_item)) =
        (&started.item, &completed.item)
    else {
        panic!("expected command execution items");
    };

    assert_eq!(started_item.id, call_id);
    assert_eq!(started_item.process_id.as_deref(), process_id);
    assert_eq!(started_item.source, source);
    assert_eq!(started_item.interaction_input.as_deref(), interaction_input);
    assert_eq!(started_item.status, CommandExecutionStatus::InProgress);
    let expected_completed_item = CommandExecutionItem {
        status: CommandExecutionStatus::Completed,
        stdout: Some("hello\n".to_string()),
        stderr: Some(String::new()),
        aggregated_output: Some("hello\n".to_string()),
        exit_code: Some(0),
        duration: Some(Duration::from_millis(25)),
        formatted_output: Some("formatted output".to_string()),
        ..started_item.clone()
    };
    assert_eq!(completed_item, &expected_completed_item);
    assert_eq!(started_item.id, completed_item.id);
    assert_eq!(begin.call_id, started_item.id);
    assert_eq!(begin.started_at_ms, started.started_at_ms);
    assert_eq!(end.call_id, completed_item.id);
    assert_eq!(end.completed_at_ms, completed.completed_at_ms);
}
