//! Footer replacement keeps the existing layout and the focused editor's cursor together.

use super::*;
use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::ChatComposer;
use crate::bottom_pane::footer::inset_footer_hint_area;
use pretty_assertions::assert_eq;
use tokio::sync::mpsc::unbounded_channel;

fn composer() -> ChatComposer {
    let (tx, _rx) = unbounded_channel::<AppEvent>();
    let mut composer = ChatComposer::new(
        /*has_input_focus*/ true,
        AppEventSender::new(tx),
        /*enhanced_keys_supported*/ false,
        "Ask Codex".to_string(),
        /*disable_paste_burst*/ true,
    );
    composer.set_status_line_enabled(/*enabled*/ true);
    composer.set_status_line(Some(Line::from("MODEL STATUS")));
    composer
}

fn render(
    composer: &ChatComposer,
    footer: Option<&TranscriptFooter>,
) -> (Buffer, Option<(u16, u16)>) {
    let options = composer.resolve_render_options(ComposerRenderOptions {
        footer,
        ..ComposerRenderOptions::default()
    });
    let width = 60;
    let area = Rect::new(
        /*x*/ 0,
        /*y*/ 0,
        width,
        composer.desired_height_with_options(width, options),
    );
    let mut buffer = Buffer::empty(area);
    composer.render_with_options(area, &mut buffer, /*mask_char*/ None, options);
    (buffer, composer.cursor_pos_with_options(area, options))
}

fn text(buffer: &Buffer) -> String {
    buffer
        .content
        .chunks(usize::from(buffer.area.width))
        .map(|row| {
            row.iter()
                .map(ratatui::buffer::Cell::symbol)
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn retained_command_suggestions_yield_paint_and_cursor_to_find() {
    let mut composer = composer();
    composer.draft.textarea.insert_str("/");
    composer.sync_popups();
    let original = render(&composer, /*footer*/ None);
    let footer = TranscriptFooter {
        text: vec![
            Line::from("Find: needle"),
            Line::from("Enter next · Esc close"),
        ]
        .into(),
        cursor_column: Some(8),
        is_interactive: true,
    };
    let options = composer.resolve_render_options(ComposerRenderOptions {
        footer: Some(&footer),
        command_popup_placement: CommandPopupPlacement::Hidden,
        ..ComposerRenderOptions::default()
    });
    let mut frames = Vec::new();
    for (width, height) in [(60, 8), (24, 8), (12, 3)] {
        let area = Rect::new(/*x*/ 0, /*y*/ 0, width, height);
        let mut buffer = Buffer::empty(area);
        composer.render_with_options(area, &mut buffer, /*mask_char*/ None, options);
        let cursor = composer.cursor_pos_with_options(area, options);
        let footer_area =
            inset_footer_hint_area(composer.layout_with_options(area, options).footer);
        assert_eq!(
            cursor,
            (footer_area.width > 8 && footer_area.height > 0)
                .then_some((footer_area.x + 8, footer_area.y))
        );
        assert!(matches!(composer.popups.active, ActivePopup::Command(_)));
        assert_eq!(composer.draft.textarea.text(), "/");
        frames.push(format!(
            "{width}x{height}, cursor {cursor:?}\n{}",
            text(&buffer)
        ));
    }
    insta::assert_snapshot!(
        "find_hides_retained_command_suggestions",
        frames.join("\n\n")
    );
    assert_eq!(render(&composer, /*footer*/ None), original);
}

#[test]
fn copying_from_find_preserves_the_query_above_selection_feedback() {
    let mut composer = composer();
    let footer = TranscriptFooter {
        text: vec![
            Line::from("Find: needle"),
            Line::from("Ctrl+C copy · Esc clear selection"),
        ]
        .into(),
        cursor_column: None,
        is_interactive: true,
    };
    composer.show_footer_flash(
        "Copied selection to clipboard".into(),
        Duration::from_secs(/*secs*/ 3),
    );
    let (buffer, cursor) = render(&composer, Some(&footer));
    assert_eq!(cursor, None);
    insta::assert_snapshot!("transcript_find_copy_footer", text(&buffer));
}

#[test]
fn interactive_query_keeps_focus_over_underlying_hints_but_empty_override_wins() {
    let mut composer = composer();
    let footer = TranscriptFooter {
        text: vec![
            Line::from("Find: needle"),
            Line::from("Enter next · Esc close"),
        ]
        .into(),
        cursor_column: Some(12),
        is_interactive: true,
    };
    let expected = render(&composer, Some(&footer));
    composer.set_footer_hint_override(Some(vec![("ctrl+c".to_string(), "quit".to_string())]));
    assert_eq!(render(&composer, Some(&footer)), expected);
    composer.set_footer_hint_override(Some(Vec::new()));
    assert_eq!(
        render(&composer, Some(&footer)),
        render(&composer, /*footer*/ None)
    );
}
