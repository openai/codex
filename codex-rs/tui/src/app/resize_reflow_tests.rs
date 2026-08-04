use super::*;
use crate::app::test_support::make_test_app;
use crate::history_cell::PlainHistoryCell;
use crate::legacy_core::config::TerminalResizeReflowMaxRows;
use pretty_assertions::assert_eq;

fn plain_history_cells(count: usize) -> Vec<Arc<dyn HistoryCell>> {
    (0..count)
        .map(|index| {
            Arc::new(PlainHistoryCell::new(vec![Line::from(format!(
                "cell {index}"
            ))])) as Arc<dyn HistoryCell>
        })
        .collect()
}

fn rendered_line_text(line: &HyperlinkLine) -> String {
    line.line
        .spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect()
}

#[tokio::test]
async fn resize_reflow_preserves_configured_scrollback_beyond_the_visible_viewport() {
    let mut app = make_test_app().await;
    app.config.terminal_resize_reflow.max_rows = TerminalResizeReflowMaxRows::Limit(32);
    app.transcript_cells = plain_history_cells(/*count*/ 64);
    let screen_size = Size::new(/*width*/ 80, /*height*/ 24);
    let chat_height = app.with_chat_widget_frame(screen_size.width, |height, _| height);
    let visible_history_rows = screen_size
        .height
        .saturating_sub(chat_height)
        .max(/*other*/ 1);

    app.update_visible_history_rows(screen_size);
    let rendered = app.render_transcript_lines_for_reflow(screen_size.width);

    assert_eq!(app.resize_reflow_max_rows(), Some(32));
    assert_eq!(rendered.lines.len(), 32);
    assert!(rendered.lines.len() > usize::from(visible_history_rows));
    assert_eq!(
        rendered.lines.last().map(rendered_line_text),
        Some("cell 63".to_string())
    );
}

#[tokio::test]
async fn initial_resume_replay_retains_scrollback_beyond_the_visible_viewport() -> Result<()> {
    let mut app = make_test_app().await;
    app.config.terminal_resize_reflow.max_rows = TerminalResizeReflowMaxRows::Limit(32);
    let screen_size = Size::new(/*width*/ 80, /*height*/ 24);
    app.update_visible_history_rows(screen_size);
    let visible_history_rows = app
        .transcript_reflow
        .visible_history_rows()
        .expect("visible history row budget");

    app.begin_initial_history_replay_buffer();
    let mut tui = crate::tui::test_support::make_test_tui()?;
    for cell in plain_history_cells(/*count*/ 24) {
        app.insert_history_cell_lines_with_initial_replay_buffer(
            &mut tui,
            cell.as_ref(),
            screen_size.width,
        );
    }

    let retained_lines = &app
        .initial_history_replay_buffer
        .as_ref()
        .expect("initial replay buffer should remain active")
        .retained_lines;
    assert_eq!(retained_lines.len(), 32);
    assert!(retained_lines.len() > usize::from(visible_history_rows));
    assert!(
        app.initial_history_replay_buffer
            .as_ref()
            .is_some_and(|buffer| buffer.was_truncated)
    );
    insta::assert_snapshot!(
        retained_lines
            .iter()
            .rev()
            .take(/*n*/ 3)
            .map(rendered_line_text)
            .collect::<Vec<_>>()
            .join("\n"),
        @r"
    cell 23

    cell 22
    "
    );
    Ok(())
}

#[tokio::test]
async fn resize_reflow_preserves_configured_scrollback_when_the_terminal_height_changes() {
    let mut app = make_test_app().await;
    app.config.terminal_resize_reflow.max_rows = TerminalResizeReflowMaxRows::Limit(48);
    app.transcript_cells = plain_history_cells(/*count*/ 64);

    app.update_visible_history_rows(Size::new(/*width*/ 80, /*height*/ 24));
    let smaller = app.render_transcript_lines_for_reflow(/*width*/ 80);
    app.update_visible_history_rows(Size::new(/*width*/ 80, /*height*/ 48));
    let larger = app.render_transcript_lines_for_reflow(/*width*/ 80);

    assert_eq!(smaller.lines.len(), 48);
    assert_eq!(larger.lines.len(), 48);
    assert_eq!(
        smaller
            .lines
            .iter()
            .map(rendered_line_text)
            .collect::<Vec<_>>(),
        larger
            .lines
            .iter()
            .map(rendered_line_text)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        larger.lines.last().map(rendered_line_text),
        Some("cell 63".to_string())
    );
}

#[tokio::test]
async fn resize_reflow_preserves_explicitly_unlimited_history() {
    let mut app = make_test_app().await;
    app.config.terminal_resize_reflow.max_rows = TerminalResizeReflowMaxRows::Disabled;
    app.transcript_cells = plain_history_cells(/*count*/ 20);

    app.update_visible_history_rows(Size::new(/*width*/ 80, /*height*/ 24));
    let rendered = app.render_transcript_lines_for_reflow(/*width*/ 80);

    assert_eq!(app.resize_reflow_max_rows(), None);
    assert_eq!(rendered.lines.len(), 39);
    assert_eq!(
        rendered.lines.first().map(rendered_line_text),
        Some("cell 0".to_string())
    );
    assert_eq!(
        rendered.lines.last().map(rendered_line_text),
        Some("cell 19".to_string())
    );
}

#[tokio::test]
async fn capped_resize_reflow_prepends_transcript_notice_without_changing_transcript() {
    let mut app = make_test_app().await;
    app.config.terminal_resize_reflow.max_rows = TerminalResizeReflowMaxRows::Limit(8);
    app.transcript_cells = plain_history_cells(/*count*/ 12);

    let rendered = app.render_transcript_lines_for_reflow(/*width*/ 80);

    assert_eq!(rendered.lines.len(), 8);
    assert_eq!(app.transcript_cells.len(), 12);
    insta::assert_snapshot!(
        rendered
            .lines
            .iter()
            .map(rendered_line_text)
            .collect::<Vec<_>>()
            .join("\n"),
        @r"
    Earlier messages are available — press ctrl + t to view the full transcript
    cell 8

    cell 9

    cell 10

    cell 11
    "
    );
}

#[tokio::test]
async fn capped_resize_reflow_counts_wrapped_notice_rows() {
    let mut app = make_test_app().await;
    app.config.terminal_resize_reflow.max_rows = TerminalResizeReflowMaxRows::Limit(8);
    app.transcript_cells = plain_history_cells(/*count*/ 12);

    let rendered = app.render_transcript_lines_for_reflow(/*width*/ 28);

    assert_eq!(rendered.lines.len(), 8);
    insta::assert_snapshot!(
        rendered
            .lines
            .iter()
            .map(rendered_line_text)
            .collect::<Vec<_>>()
            .join("\n"),
        @r"
    Earlier messages are
    available — press ctrl + t
    to view the full transcript
    cell 9

    cell 10

    cell 11
    "
    );
}

#[tokio::test]
async fn one_row_history_cap_preserves_conversation_instead_of_notice() {
    let mut app = make_test_app().await;
    app.config.terminal_resize_reflow.max_rows = TerminalResizeReflowMaxRows::Limit(1);
    app.scrollback_has_older_history = true;
    app.transcript_cells = plain_history_cells(/*count*/ 2);

    let rendered = app.render_transcript_lines_for_reflow(/*width*/ 80);

    assert_eq!(rendered.lines.len(), 1);
    insta::assert_snapshot!(
        rendered
            .lines
            .iter()
            .map(rendered_line_text)
            .collect::<Vec<_>>()
            .join("\n"),
        @r"cell 1"
    );
}

#[tokio::test]
async fn paginated_resize_reflow_prepends_transcript_notice_for_unloaded_history() {
    let mut app = make_test_app().await;
    app.config.terminal_resize_reflow.max_rows = TerminalResizeReflowMaxRows::Limit(32);
    app.scrollback_has_older_history = true;
    app.transcript_cells = plain_history_cells(/*count*/ 2);

    let rendered = app.render_transcript_lines_for_reflow(/*width*/ 80);

    insta::assert_snapshot!(
        rendered
            .lines
            .iter()
            .map(rendered_line_text)
            .collect::<Vec<_>>()
            .join("\n"),
        @r"
    Earlier messages are available — press ctrl + t to view the full transcript
    cell 0

    cell 1
    "
    );
}

#[tokio::test]
async fn scrollback_refill_only_loads_older_pages_for_an_underfilled_row_cap() {
    let mut app = make_test_app().await;
    app.config.terminal_resize_reflow.max_rows = TerminalResizeReflowMaxRows::Limit(32);
    app.scrollback_has_older_history = true;

    assert!(app.scrollback_history_needs_top_up(/*rendered_rows*/ 31));
    assert!(!app.scrollback_history_needs_top_up(/*rendered_rows*/ 32));

    app.scrollback_has_older_history = false;
    assert!(!app.scrollback_history_needs_top_up(/*rendered_rows*/ 31));

    app.scrollback_has_older_history = true;
    app.config.terminal_resize_reflow.max_rows = TerminalResizeReflowMaxRows::Disabled;
    assert!(!app.scrollback_history_needs_top_up(/*rendered_rows*/ 31));
}
