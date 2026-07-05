use super::*;
use crate::history_cell::HistoryRenderMode;
use crate::history_cell::SelectionContribution;
use codex_app_server_protocol::CommandExecutionSource;
use codex_app_server_protocol::CommandExecutionStatus;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;

#[test]
fn fallback_command_selection_excludes_gutters_and_preserves_output_whitespace() {
    let item = ThreadItem::CommandExecution {
        id: "command-1".to_string(),
        command: "printf 'x'\nprintf 'y'".to_string(),
        cwd: AbsolutePathBuf::try_from("/workspace")
            .expect("absolute path")
            .into(),
        process_id: None,
        source: CommandExecutionSource::Agent,
        status: CommandExecutionStatus::Completed,
        command_actions: Vec::new(),
        aggregated_output: Some("first  \n\nthird  \n".to_string()),
        exit_code: Some(0),
        duration_ms: Some(1),
    };
    let cell = fallback_transcript_cell(&item).expect("fallback command cell");

    let rendered = cell
        .display_lines(/*width*/ 80)
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    insta::assert_debug_snapshot!(rendered, @r###"
    [
        "$ printf 'x'",
        "  printf 'y'",
        "status: Completed · exit 0",
        "  first  ",
        "  ",
        "  third  ",
    ]
    "###);

    let SelectionContribution::Selectable(projection) =
        cell.selection_contribution(/*width*/ 80, HistoryRenderMode::Rich)
    else {
        panic!("fallback command should be selectable");
    };
    assert_eq!(
        projection.text(),
        "printf 'x'\nprintf 'y'\nstatus: Completed · exit 0\nfirst  \n\nthird  "
    );
}
