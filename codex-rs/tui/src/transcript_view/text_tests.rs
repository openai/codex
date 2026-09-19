//! Copy and painting regressions over the shared source-backed text layout.

use super::TextLayout;
use crate::terminal_hyperlinks::HyperlinkLine;
use crate::terminal_hyperlinks::adaptive_wrap_hyperlink_lines;
use crate::terminal_hyperlinks::prefix_hyperlink_lines;
use crate::wrapping::RtOptions;
use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Stylize;
use ratatui::text::Line;

#[test]
fn copying_wrapped_text_preserves_only_hard_newlines() {
    let layout = TextLayout::new(
        vec!["alpha beta gamma".into(), "  next line".into()],
        /*width*/ 10,
    );
    assert_eq!(layout.text(), "alpha beta gamma\n  next line");
    assert_eq!(
        (
            layout.position_at(/*row*/ 1, /*column*/ 0),
            layout.row_for_offset(/*offset*/ 11)
        ),
        (11, 1),
    );
    assert_eq!(
        &layout.text()[layout.line_range(/*offset*/ 12)],
        "alpha beta gamma\n"
    );

    // A commit tick can divide one logical code line across several history cells.
    let mut stream = crate::streaming::controller::StreamController::new(
        Some(7),
        std::path::Path::new("/"),
        crate::history_cell::HistoryRenderMode::Rich,
    );
    stream.push("```text\nalpha   beta\n\n界界界界界\nnext\n```\n");
    let mut cells: Vec<std::sync::Arc<dyn crate::history_cell::HistoryCell>> = Vec::new();
    while let Some(cell) = stream.on_commit_tick_batch(/*max_lines*/ 1).0 {
        cells.push(cell.into());
    }
    let expected = TextLayout::new(
        cells
            .iter()
            .flat_map(|cell| cell.transcript_hyperlink_lines(/*width*/ 9))
            .collect(),
        /*width*/ 9,
    );
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 9, /*height*/ 64,
    );
    let mut view = crate::transcript_view::TranscriptView::default();
    view.render(area, &mut Buffer::empty(area), &cells);
    view.begin_selection(&cells, /*column*/ 2, /*row*/ 0, /*clicks*/ 1);
    view.extend_selection(/*column*/ 8, /*row*/ 63);
    assert_eq!(view.selected_text(&cells).as_deref(), Some(expected.text()));
}

#[test]
fn hit_testing_uses_graphemes_and_does_not_select_padding() {
    let layout = TextLayout::new(vec!["a界e\u{301}👩‍💻".into()], /*width*/ 12);
    assert_eq!(
        (0..12)
            .map(|column| layout.position_at(/*row*/ 0, column))
            .collect::<Vec<_>>(),
        vec![0, 1, 1, 4, 7, 7, 18, 18, 18, 18, 18, 18],
    );
    assert_eq!(&layout.text()[layout.word_range(/*offset*/ 4)], "e\u{301}");
}

#[test]
fn visible_rows_preserve_styles_and_wrapped_hyperlinks() {
    let mut line = HyperlinkLine::new(Line::from("read ".green()));
    line.push_span("documentation".cyan(), Some("https://example.com/docs"));
    let layout = TextLayout::new(vec![line], /*width*/ 8);
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 8, /*height*/ 2,
    );
    let mut buffer = Buffer::empty(area);
    layout.render(area, &mut buffer, /*start_row*/ 1);
    assert_eq!(buffer[(0, 0)].fg, Color::Cyan);
    assert_eq!(
        buffer[(0, 0)].symbol(),
        "\x1b]8;;https://example.com/docs\x07d\x1b]8;;\x07",
    );
    assert_eq!(layout.position_at(/*row*/ 1, /*column*/ 0), 5);
}

#[test]
fn selection_highlights_wide_graphemes_without_trailing_padding() {
    let layout = TextLayout::new(vec!["a界b".into()], /*width*/ 8);
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 8, /*height*/ 1,
    );
    let mut buffer = Buffer::empty(area);
    layout.render(area, &mut buffer, /*start_row*/ 0);
    layout.highlight(1..4, area, &mut buffer, /*start_row*/ 0);
    assert_eq!(
        (0..8)
            .map(|column| buffer[(column, 0)].modifier.contains(Modifier::REVERSED))
            .collect::<Vec<_>>(),
        vec![false, true, true, false, false, false, false, false],
    );
}

#[test]
fn offsets_can_reach_rows_after_u16_limit() {
    let layout = TextLayout::new(vec!["x".repeat(/*n*/ 65_538).into()], /*width*/ 1);
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 1, /*height*/ 1,
    );
    let mut buffer = Buffer::empty(area);
    layout.render(area, &mut buffer, /*start_row*/ 65_537);
    assert_eq!(
        (
            layout.row_count(),
            layout.row_for_offset(/*offset*/ 65_537),
            layout.position_at(/*row*/ 65_537, /*column*/ 0),
            buffer[(0, 0)].symbol(),
        ),
        (65_538, 65_537, 65_537, "x"),
    );
}

#[test]
fn aligned_text_excludes_synthetic_leading_columns() {
    let mut line = HyperlinkLine::new(Line::default().right_aligned());
    line.push_span("wide".into(), Some("https://example.com"));
    let layout = TextLayout::new(vec![line], /*width*/ 8);
    assert_eq!(layout.link_at(/*row*/ 0, /*column*/ 3), None);
    assert_eq!(
        layout.link_at(/*row*/ 0, /*column*/ 4).as_deref(),
        Some("https://example.com")
    );
    assert_eq!(
        (0..8)
            .map(|column| layout.position_at(/*row*/ 0, column))
            .collect::<Vec<_>>(),
        vec![0, 0, 0, 0, 0, 1, 2, 3],
    );
}

#[test]
fn empty_logical_lines_keep_their_hard_breaks() {
    let layout = TextLayout::new(vec!["".into(), "text".into(), "".into()], /*width*/ 10);
    assert_eq!(
        (
            layout.text(),
            layout.row_count(),
            layout.line_range(/*offset*/ 0),
            layout.line_range(/*offset*/ 5)
        ),
        ("\ntext\n", 3, 0..1, 1..6),
    );
}

#[test]
fn source_provenance_preserves_spaces_and_excludes_gutters() {
    let source = "alpha   beta gamma";
    let lines = adaptive_wrap_hyperlink_lines(&[source.into()], RtOptions::new(/*width*/ 10));
    let lines = prefix_hyperlink_lines(lines, "› ".bold(), "  ".into());
    let layout = TextLayout::new(lines, /*width*/ 12);
    assert_eq!(layout.text(), source);
    assert_eq!(layout.position_at(/*row*/ 0, /*column*/ 0), 0);
    assert_eq!(layout.position_at(/*row*/ 0, /*column*/ 2), 0);
    assert_eq!(layout.column_for_offset(/*offset*/ 0), 2);
}

#[test]
fn selected_revision_rewraps_both_directions_with_styles_and_indents() {
    let mut source = HyperlinkLine::new(Line::from("alpha ".green()));
    source.push_span("beta gamma".cyan(), Some("https://example.com"));
    let lines = adaptive_wrap_hyperlink_lines(
        &[source],
        RtOptions::new(/*width*/ 10)
            .initial_indent("› ".into())
            .subsequent_indent("  ".into()),
    );
    let narrow = TextLayout::new(lines, /*width*/ 10);
    let wide = narrow.rewrap(/*width*/ 24);
    let restored = wide.rewrap(/*width*/ 10);
    assert_eq!(
        (
            narrow.text(),
            wide.text(),
            restored.text(),
            wide.row_count()
        ),
        (
            "alpha beta gamma",
            "alpha beta gamma",
            "alpha beta gamma",
            1
        ),
    );
    let narrow_area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 10, /*height*/ 3,
    );
    let mut original_buffer = Buffer::empty(narrow_area);
    let mut restored_buffer = Buffer::empty(narrow_area);
    narrow.render(narrow_area, &mut original_buffer, /*start_row*/ 0);
    restored.render(narrow_area, &mut restored_buffer, /*start_row*/ 0);
    assert_eq!(restored_buffer, original_buffer);
    assert_eq!(
        wide.link_at(/*row*/ 0, /*column*/ 8).as_deref(),
        Some("https://example.com")
    );
}

#[test]
fn partial_stream_fragment_does_not_copy_unseen_source_text() {
    let lines =
        adaptive_wrap_hyperlink_lines(&["alpha beta gamma".into()], RtOptions::new(/*width*/ 7));
    let first = TextLayout::new(vec![lines[0].clone()], /*width*/ 7);
    let last = TextLayout::new(vec![lines[2].clone()], /*width*/ 7);
    assert_eq!((first.text(), last.text()), ("alpha", "gamma"));
}

#[test]
fn narrow_wrapping_keeps_joined_emoji_whole() {
    let layout = TextLayout::new(vec!["👩‍💻x".into()], /*width*/ 2);
    assert_eq!(
        (
            layout.row_count(),
            layout.position_at(/*row*/ 1, /*column*/ 0)
        ),
        (2, 11),
    );
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 2, /*height*/ 2,
    );
    let mut buffer = Buffer::empty(area);
    layout.render(area, &mut buffer, /*start_row*/ 0);
    assert_eq!(buffer[(0, 0)].symbol(), "👩‍💻");
}

#[test]
fn source_backed_markdown_copy_preserves_code_indentation() {
    use crate::history_cell::AgentMarkdownCell;
    use crate::history_cell::HistoryCell;
    let cell = AgentMarkdownCell::new(
        "first **paragraph** with words\n\n```rust\n    let value = 12;\n```".into(),
        std::path::Path::new("."),
    );
    let layout = TextLayout::new(
        cell.display_hyperlink_lines(/*width*/ 14),
        /*width*/ 14,
    );
    assert!(layout.text().contains("first paragraph with words"));
    assert!(layout.text().contains("    let value = 12;"));
    assert_eq!(layout.rewrap(/*width*/ 40).text(), layout.text());
}

#[test]
fn repeated_source_prefix_never_becomes_a_synthetic_indent() {
    let source = "x x x x x";
    let wrapped = adaptive_wrap_hyperlink_lines(
        &[HyperlinkLine::from(source)],
        RtOptions::new(/*width*/ 4)
            .initial_indent("x ".into())
            .subsequent_indent("x ".into()),
    );
    let layout = TextLayout::new(wrapped, /*width*/ 4);
    assert_eq!(layout.text(), source);
    assert_eq!(layout.rewrap(/*width*/ 12).text(), source);
}

#[test]
fn command_and_output_copy_preserve_hard_lines_across_resize() {
    use crate::exec_cell::CommandOutput;
    use crate::exec_cell::ExecCall;
    use crate::exec_cell::ExecCell;
    use crate::history_cell::HistoryCell;
    let command = "printf 'alpha beta gamma delta'";
    let output = "  output alpha beta gamma delta\nnext line";
    let cell = ExecCell::new(
        ExecCall {
            call_id: "copy-test".into(),
            command: vec!["bash".into(), "-lc".into(), command.into()],
            parsed: Vec::new(),
            output: Some(CommandOutput::new(/*exit_code*/ 0, output.into())),
            source: codex_app_server_protocol::CommandExecutionSource::Agent,
            start_time: None,
            duration: None,
            interaction_input: None,
        },
        /*animations_enabled*/ false,
    );
    let rendered = crate::terminal_palette::with_test_default_colors(
        crate::terminal_probe::DefaultColors {
            fg: (220, 220, 220),
            bg: (20, 20, 20),
        },
        || {
            [
                (
                    "compact_command_copy",
                    cell.display_hyperlink_lines(/*width*/ 32),
                ),
                (
                    "detailed_command_copy",
                    cell.transcript_hyperlink_lines(/*width*/ 32),
                ),
            ]
        },
    );
    for (name, lines) in rendered {
        let layout = TextLayout::new(lines, /*width*/ 32);
        assert_eq!(layout.text(), format!("{command}\n{output}"));
        let narrow = layout.rewrap(/*width*/ 24);
        let wide = narrow.rewrap(/*width*/ 64);
        assert_eq!((narrow.text(), wide.text()), (layout.text(), layout.text()));
        let area = Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 24, /*height*/ 8,
        );
        let mut buffer = Buffer::empty(area);
        narrow.render(area, &mut buffer, /*start_row*/ 0);
        insta::assert_snapshot!(name, format!("{buffer:?}"));
    }
}

#[test]
fn background_stdin_copy_preserves_indentation_and_newlines() {
    use crate::history_cell::HistoryCell;
    use crate::history_cell::UnifiedExecInteractionCell;
    let stdin = "    first long line of input\nnext";
    let cell = UnifiedExecInteractionCell::new(/*command_display*/ None, stdin.into());
    let layout = TextLayout::new(
        cell.transcript_hyperlink_lines(/*width*/ 24),
        /*width*/ 24,
    );
    assert!(layout.text().ends_with(stdin));
    assert_eq!(layout.rewrap(/*width*/ 48).text(), layout.text());
}
