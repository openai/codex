//! Warning notice layout, shortcut remapping, and input-priority coverage.

use super::*;
use pretty_assertions::assert_eq;
use tokio::sync::mpsc::unbounded_channel;

fn composer() -> ChatComposer {
    let (tx, _) = unbounded_channel();
    let mut composer = ChatComposer::new(
        /*has_input_focus*/ true,
        AppEventSender::new(tx),
        /*enhanced_keys_supported*/ false,
        "Ask Codex".into(),
        /*disable_paste_burst*/ true,
    );
    // Keep layout fixtures independent of platform-specific default shortcuts.
    composer.footer.show_warnings_key = Some(crate::key_hint::plain(KeyCode::F(2)).into());
    composer.set_status_line_enabled(/*enabled*/ true);
    composer.set_status_line(Some(Line::from("MODEL · ~/project")));
    composer
}

fn render(composer: &ChatComposer, width: u16, count: usize) -> Buffer {
    let options = composer.resolve_render_options(ComposerRenderOptions {
        separate_status_line: true,
        warning_count: count,
        command_popup_placement: CommandPopupPlacement::Overlay,
        ..Default::default()
    });
    let area = Rect::new(
        /*x*/ 0,
        /*y*/ 0,
        width,
        composer.desired_height_with_options(width, options),
    );
    let mut buffer = Buffer::empty(area);
    composer.render_with_options(area, &mut buffer, /*mask_char*/ None, options);
    buffer
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
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn warning_notice_styles() {
    let mut composer = composer();
    composer.set_text_content("keep this draft".into(), Vec::new(), Vec::new());
    let styled = crate::terminal_palette::with_test_default_colors(
        crate::terminal_probe::DefaultColors {
            fg: (222, 222, 222),
            bg: (16, 16, 16),
        },
        || render(&composer, /*width*/ 80, /*count*/ 3),
    );
    insta::assert_debug_snapshot!("warning_notice_styles", styled);
}

#[test]
fn light_warning_notice_clears_inherited_dimming() {
    crate::terminal_palette::with_test_default_colors(
        crate::terminal_probe::DefaultColors {
            fg: (30, 30, 30),
            bg: (255, 255, 255),
        },
        || {
            let composer = composer();
            let area = Rect::new(
                /*x*/ 0, /*y*/ 0, /*width*/ 20, /*height*/ 1,
            );
            let mut buffer = Buffer::empty(area);
            buffer.set_style(area, Style::default().dim());
            composer
                .warning_notice(/*count*/ 3, area.width)
                .render(area, &mut buffer);
            for cell in &buffer.content {
                if !cell.symbol().trim().is_empty() {
                    assert!(!cell.modifier.contains(Modifier::DIM));
                }
            }
            assert_eq!(buffer[(2, 0)].symbol(), "3");
            assert_eq!(
                buffer[(2, 0)].fg,
                crate::style::warning_notice_style().fg.unwrap()
            );
        },
    );
}

#[test]
fn warning_notice_respects_shortcuts_and_interactive_hints() {
    let mut composer = composer();
    composer.footer.show_warnings_key = Some(crate::key_hint::plain(KeyCode::F(12)).into());
    assert!(text(&render(&composer, /*width*/ 64, /*count*/ 2)).contains("f12 to view"));
    composer.footer.show_warnings_key = None;
    assert!(text(&render(&composer, /*width*/ 64, /*count*/ 2)).contains("⚠ 2 · /warnings"));
    let interactive = TranscriptFooter {
        text: Line::from("Find: needle").into(),
        cursor_column: Some(6),
        is_interactive: true,
    };
    let options = composer.resolve_render_options(ComposerRenderOptions {
        warning_count: 2,
        footer: Some(&interactive),
        ..Default::default()
    });
    assert!(!composer.show_warning_notice(options));
    let passive = ComposerRenderOptions {
        warning_count: 2,
        ..Default::default()
    };
    composer.begin_history_search();
    assert!(!composer.show_warning_notice(passive));
    composer.history_search = None;
    composer.set_footer_hint_override(Some(vec![("Ctrl+X".into(), "pending chord".into())]));
    assert!(!composer.show_warning_notice(passive));
    composer.set_footer_hint_override(/*items*/ None);
    assert!(composer.show_warning_notice(passive));
    composer.set_text_content("/".into(), Vec::new(), Vec::new());
    composer.sync_popups();
    assert!(!composer.show_warning_notice(passive));
}

#[test]
fn warnings_yield_to_queue_controls_and_complete_navigation_hints() {
    let mut composer = composer();
    composer.set_text_content("queued draft".into(), Vec::new(), Vec::new());
    composer.set_task_running(/*running*/ true);
    assert_eq!(
        render(&composer, /*width*/ 32, /*count*/ 4),
        render(&composer, /*width*/ 32, /*count*/ 0),
    );
    composer.set_task_running(/*running*/ false);
    let footer = TranscriptFooter {
        text: "esc latest".into(),
        cursor_column: None,
        is_interactive: false,
    };
    let options = composer.resolve_render_options(ComposerRenderOptions {
        footer: Some(&footer),
        warning_count: 4,
        separate_status_line: true,
        ..Default::default()
    });
    for width in [12, 20] {
        let area = Rect::new(/*x*/ 0, /*y*/ 0, width, /*height*/ 1);
        let notice = composer.warning_notice_layout(area, options);
        assert_eq!(notice.is_some(), width > 12);
        if let Some((warning, _)) = notice {
            assert!(warning.x >= footer.text.width() as u16 + 2);
            assert_eq!(warning.right(), area.right() - 1);
        }
    }
}
