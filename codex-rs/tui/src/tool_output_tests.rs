use super::*;
use crate::terminal_hyperlinks::visible_lines;
use pretty_assertions::assert_eq;

#[test]
fn preview_caps_wrapped_output_and_counts_hidden_logical_lines() {
    let lines = [
        "first",
        "https://example.test/a/very/long/path/that/exceeds/the/preview",
        "last",
    ];
    let preview = visible_lines(tool_output_hyperlink_preview(
        lines.into_iter().map(Line::from),
        /*width*/ 16,
        /*total_lines*/ 10,
    ));
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
    let mut preview = ToolOutputPreview::new(/*width*/ 40, /*omitted*/ 0);
    for line in lines.clone() {
        preview.push_line(line);
    }
    assert_eq!(visible_lines(preview.finish_hyperlink_lines()), lines);
}

#[test]
fn preview_preserves_hyperlinks_and_original_logical_source() {
    let line = Line::from("one   two three four five".red());
    let source = LogicalLineSource::from_line(&line);
    let destination = "https://example.test/original".to_owned();
    let mut preview = ToolOutputPreview::new(/*width*/ 6, /*omitted*/ 0);
    preview.push_hyperlink_line(HyperlinkLine {
        line,
        hyperlinks: vec![TerminalHyperlink::web(
            /*columns*/ 0..24,
            destination.clone(),
        )],
        source: Some(source.clone()),
    });
    let preview = preview.finish_hyperlink_lines();
    assert_eq!(
        preview[..PREVIEW_LINES]
            .iter()
            .map(|line| (
                line.line.clone(),
                line.hyperlinks.clone(),
                line.source.clone()
            ))
            .collect::<Vec<_>>(),
        [("one", 0..3), ("two", 6..9), ("three", 10..15)]
            .into_iter()
            .map(|(text, range)| {
                let mut source = source.clone();
                source.range = range;
                (
                    Line::from(text.red()),
                    vec![TerminalHyperlink::web(
                        /*columns*/ 0..text.len(),
                        destination.clone(),
                    )],
                    Some(source),
                )
            })
            .collect::<Vec<_>>(),
    );
    assert_eq!(
        (
            preview[PREVIEW_LINES].source.clone(),
            preview[PREVIEW_LINES].hyperlinks.clone()
        ),
        (None, Vec::new()),
    );
}

#[test]
fn preview_counts_newline_dense_output() {
    let mut rendered = 0;
    let preview = visible_lines(tool_output_hyperlink_preview(
        std::iter::repeat_n(Line::default(), /*n*/ 100_000).inspect(|_| rendered += 1),
        /*width*/ 40,
        /*total_lines*/ 100_000,
    ));
    assert_eq!(rendered, PREVIEW_LINES);
    assert_eq!(
        preview.iter().map(ToString::to_string).collect::<Vec<_>>(),
        ["", "", "", "+99997 lines (ctrl+t to view transcript)"],
    );
}

#[test]
fn preview_caps_combining_text_across_styled_spans() {
    let marks = "\u{301}".repeat(MAX_PREVIEW_LINE_BYTES / 4);
    let preview = tool_output_hyperlink_preview(
        [
            Line::from(vec!["e".red(), marks.clone().dim(), marks.clone().blue()]),
            "later".into(),
        ],
        /*width*/ 80,
        /*total_lines*/ 2,
    );
    let expected_line = Line::from(vec![
        "e".red(),
        marks.clone().dim(),
        marks[..marks.len() - 2].blue(),
    ]);
    assert_eq!(
        preview[0].source,
        Some(LogicalLineSource::from_line(&expected_line)),
    );
    assert_eq!(
        visible_lines(preview),
        vec![
            expected_line,
            "+2 lines (ctrl+t to view transcript)".dim().into()
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
        "history:\n{}\n\ncompact:\n{}\n\ntranscript:\n{}",
        completed
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n"),
        visible_lines(cell.compact_hyperlink_lines(/*width*/ 80))
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
