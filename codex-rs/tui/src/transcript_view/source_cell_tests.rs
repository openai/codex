//! Logical copy and display wrapping for reasoning, plans, questions, and recaps.

use super::*;
use crate::history_cell::HistoryCell;
use crate::history_cell::ReasoningSummaryCell;
use crate::history_cell::RequestUserInputResultCell;
use crate::history_cell::ThreadRecapHistoryCell;
use codex_app_server_protocol::ToolRequestUserInputAnswer;
use codex_app_server_protocol::ToolRequestUserInputQuestion;
use codex_protocol::plan_tool::PlanItemArg;
use codex_protocol::plan_tool::StepStatus;
use codex_protocol::plan_tool::UpdatePlanArgs;
use pretty_assertions::assert_eq;
use ratatui::style::Stylize;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

#[test]
fn reasoning_copy_joins_soft_wraps_and_retains_the_existing_rendering() {
    let content = "Investigate the failing condition and preserve the current user experience across terminal resize.";
    let cell = ReasoningSummaryCell::new(
        "Thinking".to_owned(),
        content.to_owned(),
        Path::new("/project"),
        /*transcript_only*/ true,
    );
    let width = 36;
    let lines = cell.transcript_hyperlink_lines(width);
    let layout = TextLayout::new(lines.clone(), width);
    assert_eq!(layout.text(), content);
    assert_eq!(layout.rewrap(/*width*/ 19).text(), content);
    assert_eq!(cell.display_hyperlink_lines(width), Vec::new());

    assert_layout_paints_like_lines(lines, width);
}

#[test]
fn plan_copy_retains_checkboxes_but_omits_wrapping_gutters() {
    let note = "Preserve the existing behavior while simplifying the transcript renderer.";
    let step = "Replace the terminal scrollback path with an owned transcript viewport.";
    let cell = crate::history_cell::new_plan_update(UpdatePlanArgs {
        explanation: Some(note.to_owned()),
        plan: vec![PlanItemArg {
            step: step.to_owned(),
            status: StepStatus::Completed,
        }],
    });
    let width = 38;
    let lines = cell.display_hyperlink_lines(width);
    let layout = TextLayout::new(lines.clone(), width);
    let expected_text = format!("• Updated Plan\n{note}\n✔ {step}");
    assert_eq!(layout.text(), expected_text);
    assert_eq!(layout.rewrap(/*width*/ 20).text(), expected_text);

    assert_layout_paints_like_lines(lines, width);
}

#[test]
fn question_copy_retains_answer_labels_masks_secrets_and_keeps_unanswered_suffixes() {
    let question = "Which behavior should remain available when this long conversation is resumed?";
    let answer = "Preserve all of the existing selection and navigation controls across resize.";
    let secret_question = ToolRequestUserInputQuestion {
        id: "secret".to_owned(),
        header: "Token".to_owned(),
        question: "Which token?".to_owned(),
        is_other: false,
        is_secret: true,
        options: None,
    };
    let cell = RequestUserInputResultCell {
        questions: vec![
            ToolRequestUserInputQuestion {
                id: "behavior".to_owned(),
                header: "Behavior".to_owned(),
                question: question.to_owned(),
                is_other: false,
                is_secret: false,
                options: None,
            },
            secret_question.clone(),
            ToolRequestUserInputQuestion {
                id: "pending".to_owned(),
                question: "Where next?".to_owned(),
                is_secret: false,
                ..secret_question
            },
        ],
        answers: HashMap::from([
            (
                "behavior".to_owned(),
                ToolRequestUserInputAnswer {
                    answers: vec![answer.to_owned()],
                },
            ),
            (
                "secret".to_owned(),
                ToolRequestUserInputAnswer {
                    answers: vec!["never expose this credential".to_owned()],
                },
            ),
        ]),
        interrupted: true,
    };
    let width = 48;
    let lines = cell.display_hyperlink_lines(width);
    let layout = TextLayout::new(lines.clone(), width);
    let expected = format!(
        "• Questions 2/3 answered (interrupted)\n• {question}\nanswer: {answer}\n• Which token?\nanswer: ••••••\n• Where next? (unanswered)\n↳ interrupted with 1 unanswered"
    );
    assert_eq!(layout.text(), expected);
    assert_eq!(layout.rewrap(/*width*/ 24).text(), expected);

    assert_layout_paints_like_lines(lines, width);
}

#[test]
fn recap_copy_preserves_hard_lines_and_reflows_only_soft_wraps() {
    let recap = "The café review is paused at https://example.com/review/42. 日本語\n  An indented hard line.";
    let next_action = "Review https://example.com/integration/report before continuing.";
    let cell = ThreadRecapHistoryCell::new(recap.to_owned())
        .with_next_action(Some(next_action.to_owned()));
    let expected_text = format!("{recap}\nNext: {next_action}");
    let retained = TextLayout::new(
        cell.display_hyperlink_lines(/*width*/ 80),
        /*width*/ 80,
    );
    let cells: Vec<Arc<dyn HistoryCell>> = vec![Arc::new(cell)];
    let mut view = crate::transcript_view::TranscriptView::default();

    for width in [80, 32, 12, 80] {
        // Ordinary resize renders the cell again; a selected revision rewraps its saved source.
        assert_eq!(
            retained.rewrap(width).text(),
            expected_text,
            "retained copy at {width} columns"
        );
        view.end_selection(&cells);
        let lines = cells[0].display_hyperlink_lines(width);
        let area = Rect::new(/*x*/ 0, /*y*/ 0, width, /*height*/ 64);
        let mut expected = Buffer::empty(area);
        HyperlinkParagraph::new(&lines, Style::default()).render(area, &mut expected);
        // The owned viewport inherits the line style on padding and hidden wide-character
        // cells as well. The legacy paragraph only paints the visible spans there.
        for (row, line) in lines.iter().enumerate() {
            for column in 0..area.width {
                let cell = &mut expected[(column, row as u16)];
                if cell.symbol() == " " {
                    cell.set_style(line.line.style);
                }
            }
        }
        let mut actual = Buffer::empty(area);
        view.render(area, &mut actual, &cells);
        // The viewport styles entire rows, including padding and hidden wide-character cells.
        // Italic has no visible glyph there; retain every other attribute in the comparison.
        for cell in actual.content.iter_mut().chain(expected.content.iter_mut()) {
            if cell.symbol() == " " {
                cell.modifier.remove(Modifier::ITALIC);
            }
        }
        assert_eq!(actual, expected, "owned recap at {width} columns");
        let layout = view.layout(&cells, /*index*/ 0).expect("rendered recap");

        // Select the body so the very narrow fallback's separate Recap label is not copied.
        let start = layout.text().find("The café").expect("recap body");
        let end = layout.text().len();
        view.begin_selection(
            &cells,
            layout.column_for_offset(start),
            layout.row_for_offset(start) as u16,
            /*clicks*/ 1,
        );
        view.extend_selection(
            layout.column_for_offset(end),
            layout.row_for_offset(end) as u16,
        );
        assert_eq!(
            view.selected_text(&cells),
            Some(expected_text.clone()),
            "selected copy at {width} columns"
        );
    }
}

#[test]
fn recap_tabs_preserve_url_wrapping_and_source_across_resize() {
    let recap = "\thttps://example.com";
    let cell = ThreadRecapHistoryCell::new(recap.to_owned());
    let wide = TextLayout::new(
        cell.display_hyperlink_lines(/*width*/ 80),
        /*width*/ 80,
    );
    let narrow = wide.rewrap(/*width*/ 32);
    assert_eq!(narrow.text(), recap);
    assert_eq!(
        narrow
            .rows
            .iter()
            .map(|row| row.line.line.to_string())
            .collect::<Vec<_>>(),
        ["  ↳ Recap:      ", "           https://example.com"],
    );
    let area = Rect::new(
        /*x*/ 0,
        /*y*/ 0,
        /*width*/ 32,
        narrow.row_count() as u16,
    );
    let mut buffer = Buffer::empty(area);
    narrow.render(area, &mut buffer, /*start_row*/ 0);
    let restored = narrow.rewrap(/*width*/ 80).rewrap(/*width*/ 32);
    let mut restored_buffer = Buffer::empty(area);
    restored.render(area, &mut restored_buffer, /*start_row*/ 0);
    assert_eq!(restored_buffer, buffer);
    let link_start = recap.find("https://").expect("URL in recap");
    assert_eq!(
        narrow.position_at(
            narrow.row_for_offset(link_start),
            narrow.column_for_offset(link_start)
        ),
        link_start,
    );
}

#[test]
fn selected_recap_remains_visible_when_resize_cannot_fit_its_gutter() {
    let mut frames = Vec::new();
    for recap in [
        "The café review is paused at https://example.com/review/42.",
        "\thttps://example.com",
    ] {
        let cells: Vec<Arc<dyn HistoryCell>> =
            vec![Arc::new(ThreadRecapHistoryCell::new(recap.to_owned()))];
        let mut view = crate::transcript_view::TranscriptView::default();
        let wide_area = Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 80, /*height*/ 64,
        );
        view.render(wide_area, &mut Buffer::empty(wide_area), &cells);
        let layout = view.layout(&cells, /*index*/ 0).expect("rendered recap");
        view.begin_selection(
            &cells,
            layout.column_for_offset(/*offset*/ 0),
            /*row*/ 0,
            /*clicks*/ 1,
        );
        view.extend_selection(
            layout.column_for_offset(recap.len()),
            layout.row_for_offset(recap.len()) as u16,
        );
        view.end_drag();
        let mut wide_buffer = Buffer::empty(wide_area);
        view.render(wide_area, &mut wide_buffer, &cells);

        let area = Rect::new(
            /*x*/ 0, /*y*/ 0, /*width*/ 12, /*height*/ 64,
        );
        let mut actual = Buffer::empty(area);
        view.render(area, &mut actual, &cells);
        // A held selection retains its source; the fresh narrow formatter adds a separate label.
        let body_lines = cells[0]
            .display_hyperlink_lines(area.width)
            .into_iter()
            .skip(/*n*/ 1)
            .collect();
        let expected_layout = TextLayout::new(body_lines, area.width);
        let mut expected = Buffer::empty(area);
        expected_layout.render(area, &mut expected, /*start_row*/ 0);
        expected_layout.highlight(0..recap.len(), area, &mut expected, /*start_row*/ 0);
        assert_eq!(actual, expected, "selected recap {recap:?}");
        assert_eq!(view.selected_text(&cells), Some(recap.to_owned()));
        let layout = view
            .layout(&cells, /*index*/ 0)
            .expect("selected narrow recap");
        frames.push(
            layout
                .rows
                .iter()
                .map(|row| row.line.line.to_string())
                .collect::<Vec<_>>()
                .join("\n"),
        );

        let mut restored = Buffer::empty(wide_area);
        view.render(wide_area, &mut restored, &cells);
        assert_eq!(restored, wide_buffer);
        assert_eq!(view.selected_text(&cells), Some(recap.to_owned()));
    }
    insta::assert_snapshot!("owned_recap_selected_narrow", frames.join("\n---\n"));
}

#[test]
fn resize_restores_the_exact_styles_of_whitespace_omitted_by_wrapping() {
    let line = HyperlinkLine::new(Line::from(vec![
        "alpha".bold(),
        "  ".on_red(),
        "beta".italic(),
        " ".dim(),
        "gamma".underlined(),
    ]));
    let wrapped = crate::terminal_hyperlinks::adaptive_wrap_hyperlink_lines(
        std::slice::from_ref(&line),
        RtOptions::new(/*width*/ 6),
    );
    let layout = TextLayout::new(wrapped, /*width*/ 6).rewrap(/*width*/ 24);
    assert_eq!(layout.text(), "alpha  beta gamma");
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 24, /*height*/ 2,
    );
    let mut expected = Buffer::empty(area);
    HyperlinkParagraph::new(&[line], Style::default()).render(area, &mut expected);
    let mut actual = Buffer::empty(area);
    layout.render(area, &mut actual, /*start_row*/ 0);
    assert_eq!(actual, expected);
}

fn assert_layout_paints_like_lines(lines: Vec<HyperlinkLine>, width: u16) {
    let area = Rect::new(/*x*/ 0, /*y*/ 0, width, /*height*/ 30);
    let mut expected = Buffer::empty(area);
    HyperlinkParagraph::new(&lines, Style::default()).render(area, &mut expected);
    let mut actual = Buffer::empty(area);
    TextLayout::new(lines, width).render(area, &mut actual, /*start_row*/ 0);
    assert_eq!(actual, expected);
}
