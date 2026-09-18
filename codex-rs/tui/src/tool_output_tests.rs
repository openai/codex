use super::*;
use pretty_assertions::assert_eq;

#[test]
fn preview_caps_wrapped_output_and_counts_hidden_logical_lines() {
    let lines = [
        "first",
        "https://example.test/a/very/long/path/that/exceeds/the/preview",
        "last",
    ];
    let preview = tool_output_preview(
        lines.into_iter().map(Line::from),
        /*width*/ 16,
        /*total_lines*/ 10,
    );
    assert_eq!(preview.len(), PREVIEW_LINES + 1);
    assert!(
        preview[..PREVIEW_LINES]
            .iter()
            .all(|line| line.width() <= 16)
    );
    insta::assert_snapshot!(
        preview
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn preview_preserves_short_output_and_blank_lines() {
    let lines = vec!["first".red().into(), Line::default(), "last".dim().into()];
    assert_eq!(
        tool_output_preview(lines.clone(), /*width*/ 40, lines.len()),
        lines,
    );
}

#[test]
fn preview_counts_newline_dense_output() {
    let mut rendered = 0;
    let preview = tool_output_preview(
        std::iter::repeat_n(Line::default(), 100_000).inspect(|_| rendered += 1),
        /*width*/ 40,
        /*total_lines*/ 100_000,
    );
    assert_eq!(rendered, PREVIEW_LINES);
    assert_eq!(
        preview.iter().map(ToString::to_string).collect::<Vec<_>>(),
        ["", "", "", "+99997 lines (ctrl + t to view transcri…"],
    );
}

#[test]
fn preview_caps_combining_text_across_styled_spans() {
    let marks = "\u{301}".repeat(MAX_PREVIEW_LINE_BYTES / 4);
    let preview = tool_output_preview(
        [
            Line::from(vec!["e".red(), marks.clone().dim(), marks.clone().blue()]),
            "later".into(),
        ],
        /*width*/ 80,
        /*total_lines*/ 2,
    );
    assert_eq!(
        preview,
        vec![
            Line::from(vec![
                "e".red(),
                marks.clone().dim(),
                marks[..marks.len() - 2].blue(),
            ]),
            "+2 lines (ctrl + t to view transcript)".dim().into(),
        ],
    );
}

#[test]
fn command_preview_matches_streamed_and_completed_output() {
    use crate::exec_cell::CommandOutput;
    use crate::exec_cell::new_active_exec_command;
    use crate::history_cell::HistoryCell;
    use codex_app_server_protocol::CommandExecutionSource;
    use std::time::Duration;

    let mut cell = new_active_exec_command(
        "call-preview".into(),
        vec!["bash".into(), "-lc".into(), "echo output".into()],
        Vec::new(),
        CommandExecutionSource::Agent,
        /*interaction_input*/ None,
        /*animations_enabled*/ false,
    );
    let output = "first\nsecond\nthird\nfourth\nfifth\n";
    cell.append_output("call-preview", output);
    let live = cell.display_lines(/*width*/ 80);
    cell.complete_call(
        "call-preview",
        CommandOutput::new(/*exit_code*/ 0, output.into()),
        Duration::ZERO,
    );
    let completed = cell.display_lines(/*width*/ 80);
    assert_eq!(&live[1..], &completed[1..]);
    insta::assert_snapshot!(format!(
        "history:\n{}\n\ntranscript:\n{}",
        completed
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n"),
        cell.transcript_lines(/*width*/ 80)
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n"),
    ));
}
