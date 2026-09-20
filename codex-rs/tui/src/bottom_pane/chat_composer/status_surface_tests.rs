//! Persistent status survives transient footer modes and suggestion overlays.

use super::*;
use pretty_assertions::assert_eq;
use tokio::sync::mpsc::unbounded_channel;

fn composer() -> ChatComposer {
    let (tx, _rx) = unbounded_channel();
    let mut composer = ChatComposer::new(
        /*has_input_focus*/ true,
        AppEventSender::new(tx),
        /*enhanced_keys_supported*/ false,
        "Ask Codex".to_string(),
        /*disable_paste_burst*/ true,
    );
    composer.set_status_line_enabled(/*enabled*/ true);
    composer.set_status_line(Some(Line::from("MODEL · ~/project · Context 20% used")));
    composer
}

fn render(
    composer: &ChatComposer,
    width: u16,
    footer: Option<&TranscriptFooter>,
) -> (String, Option<(u16, u16)>) {
    render_with_height(composer, width, footer, /*height*/ None)
}

fn render_with_height(
    composer: &ChatComposer,
    width: u16,
    footer: Option<&TranscriptFooter>,
    height: Option<u16>,
) -> (String, Option<(u16, u16)>) {
    let options = composer.resolve_render_options(ComposerRenderOptions {
        separate_status_line: true,
        command_popup_placement: CommandPopupPlacement::Overlay,
        footer,
        ..ComposerRenderOptions::default()
    });
    let height = height.unwrap_or_else(|| composer.desired_height_with_options(width, options));
    let area = Rect::new(/*x*/ 0, /*y*/ 8, width, height);
    let mut buf = Buffer::empty(Rect::new(/*x*/ 0, /*y*/ 0, width, area.bottom()));
    composer.render_with_options(area, &mut buf, /*mask_char*/ None, options);
    let text = buf
        .content
        .chunks(usize::from(width))
        .map(|row| {
            row.iter()
                .map(ratatui::buffer::Cell::symbol)
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n");
    (text, composer.cursor_pos_with_options(area, options))
}

#[test]
fn passive_activity_keeps_shortcuts_only_when_the_complete_hint_fits() {
    let mut composer = composer();
    let mut line = Line::from(key_hint::key_label_spans("ctrl+o t/f4"));
    line.push_span(" inspect activity".dim());
    let mut footer = TranscriptFooter {
        text: line.into(),
        cursor_column: None,
        is_interactive: false,
    };
    for (width, expected) in [
        (80, "ctrl+o t/f4 inspect activity · ? shortcuts"),
        (40, "ctrl+o t/f4 inspect activity"),
    ] {
        let (text, _) = render(&composer, width, Some(&footer));
        assert_eq!(text.lines().last().map(str::trim), Some(expected));
    }
    footer.is_interactive = true;
    let (text, _) = render(&composer, /*width*/ 80, Some(&footer));
    assert_eq!(
        text.lines().last().map(str::trim),
        Some("ctrl+o t/f4 inspect activity")
    );
    footer.is_interactive = false;
    composer.footer.toggle_shortcuts_key = None;
    let (text, _) = render(&composer, /*width*/ 80, Some(&footer));
    assert_eq!(
        text.lines().last().map(str::trim),
        Some("ctrl+o t/f4 inspect activity")
    );
}

#[test]
fn shortcut_help_stays_above_the_prompt_and_bottom_status() {
    let mut composer = composer();
    composer.handle_key_event(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
    for (width, height, snapshot) in [
        (130, None, Some("shortcut_help_above_wide")),
        (80, None, None),
        (40, None, Some("shortcut_help_above_narrow")),
        (80, Some(12), None),
    ] {
        let (text, cursor) = render_with_height(&composer, width, /*footer*/ None, height);
        let rows: Vec<_> = text.lines().collect();
        let help_row = rows
            .iter()
            .position(|line| line.contains("Keyboard shortcuts"))
            .unwrap();
        let prompt_row = rows
            .iter()
            .position(|line| line.contains("Ask Codex"))
            .unwrap();
        assert!(help_row < prompt_row);
        assert!(
            rows[..prompt_row]
                .iter()
                .any(|line| line.trim() == "/keymap customize")
        );
        assert_eq!(text.matches("? / esc close").count(), 1);
        assert_eq!(
            (
                rows.last().map(|line| line.trim()),
                rows[rows.len() - 2].trim(),
                cursor,
            ),
            (
                Some("? / esc close"),
                "MODEL · ~/project · Context 20% used",
                Some((2, prompt_row as u16))
            ),
        );
        assert_eq!(prompt_row + 3, rows.len() - 1);
        if let Some(name) = snapshot {
            insta::assert_snapshot!(name, text);
        }
    }
}

#[test]
fn shortcut_help_keeps_bottom_anchored_composer_status_and_cursor_stable() {
    let (width, available_height) = (40, 12);
    let mut composer = composer();
    let mut geometry = Vec::new();
    for event in [
        None,
        Some(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE)),
        Some(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
    ] {
        if let Some(event) = event {
            composer.handle_key_event(event);
        }
        let options = composer.resolve_render_options(ComposerRenderOptions {
            separate_status_line: true,
            ..ComposerRenderOptions::default()
        });
        let height = composer
            .desired_height_with_options(width, options)
            .min(available_height);
        let area = Rect::new(/*x*/ 3, 80 - height, width, height);
        let ComposerLayout {
            composer: surface,
            footer,
            status,
            ..
        } = composer.layout_with_options(area, options);
        geometry.push((
            surface,
            status,
            footer,
            composer.cursor_pos_with_options(area, options),
        ));
    }
    assert_eq!(geometry, vec![geometry[0]; 3]);
}

#[test]
fn shortcut_help_keeps_input_and_close_hint_visible_when_height_is_tiny() {
    for status_enabled in [true, false] {
        let mut composer = composer();
        composer.set_status_line_enabled(status_enabled);
        composer.handle_key_event(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
        let options = composer.resolve_render_options(ComposerRenderOptions {
            separate_status_line: true,
            ..ComposerRenderOptions::default()
        });
        for height in 0..=7 {
            let area = Rect::new(/*x*/ 3, /*y*/ 5, /*width*/ 40, height);
            let mut buf = Buffer::empty(area);
            composer.render_with_options(area, &mut buf, /*mask_char*/ None, options);
            if let Some((x, y)) = composer.cursor_pos_with_options(area, options) {
                assert!(x >= area.x && x < area.right() && y >= area.y && y < area.bottom());
            }
            if height >= 4 {
                let ComposerLayout {
                    composer: surface,
                    footer,
                    ..
                } = composer.layout_with_options(area, options);
                assert_eq!(
                    (surface.height, footer.height, footer.bottom()),
                    (3, 1, area.bottom())
                );
                let last_row = (area.x..area.right())
                    .map(|x| buf[(x, area.bottom() - 1)].symbol())
                    .collect::<String>();
                assert_eq!(last_row.trim(), "? / esc close");
            }
        }
    }
}

#[test]
fn shortcut_help_close_row_uses_runtime_binding_and_narrow_fallback() {
    let chord = Some(key_hint::ShortcutHint::Chord {
        prefix: key_hint::ctrl(KeyCode::Char('x')),
        completion: key_hint::ctrl(KeyCode::Char('h')),
    });
    for (width, binding, expected) in [
        (80, chord, "ctrl+x ctrl+h / esc close"),
        (22, chord, "esc close"),
        (80, None, "esc close"),
    ] {
        let mut composer = composer();
        composer.handle_key_event(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
        composer.footer.toggle_shortcuts_key = binding;
        let (text, _) = render_with_height(
            &composer,
            width,
            /*footer*/ None,
            /*height*/ Some(8),
        );
        assert_eq!(text.lines().last().map(str::trim), Some(expected));
        assert_eq!(text.matches("close").count(), 1);
        assert_eq!(text.matches("/keymap customize").count(), 1);
    }
}

#[test]
fn status_survives_hints_feedback_and_queue() {
    let mut composer = composer();
    composer.set_collaboration_mode_indicator(Some(CollaborationModeIndicator::Plan));
    let mut states = Vec::new();
    states.push(render(&composer, /*width*/ 64, /*footer*/ None).0);
    composer.footer.mode = FooterMode::ShortcutOverlay;
    composer.show_footer_flash("Copied selection".into(), Duration::from_secs(/*secs*/ 3));
    states.push(render(&composer, /*width*/ 64, /*footer*/ None).0);
    composer.footer.flash = None;
    composer.footer.mode = FooterMode::ComposerEmpty;
    composer.set_footer_hint_override(Some(vec![("Ctrl+X".into(), "waiting for chord".into())]));
    states.push(render(&composer, /*width*/ 64, /*footer*/ None).0);
    composer.set_footer_hint_override(/*items*/ None);
    composer.set_text_content("Continue the task".into(), Vec::new(), Vec::new());
    composer.set_task_running(/*running*/ true);
    states.push(render(&composer, /*width*/ 64, /*footer*/ None).0);
    for popup in [
        ActivePopup::File(FileSearchPopup::new()),
        ActivePopup::Skill(SkillPopup::new(Vec::new())),
    ] {
        composer.popups.active = popup;
        let (text, _) = render(&composer, /*width*/ 64, /*footer*/ None);
        assert!(text.contains("MODEL · ~/project · Context 20% used"));
        assert_eq!(text.matches("Plan mode").count(), 1);
    }
    for state in &states {
        assert_eq!(state.matches("Plan mode").count(), 1);
        assert!(state.contains("MODEL · ~/project · Context 20% used"));
    }
    insta::assert_snapshot!("persistent_status_and_messages", states.join("\n---\n"));
}

#[test]
fn transcript_search_keeps_status_and_query_cursor() {
    let composer = composer();
    let footer = TranscriptFooter {
        text: vec![
            Line::from("Find: needle"),
            Line::from("Enter next · Esc close"),
        ]
        .into(),
        cursor_column: Some(8),
        is_interactive: true,
    };
    let (text, cursor) = render(&composer, /*width*/ 48, Some(&footer));
    assert_eq!(cursor, Some((10, text.lines().count() as u16 - 2)));
    assert!(text.contains("MODEL · ~/project · Context 20% used"));
}

#[test]
fn input_history_search_uses_the_final_row() {
    let mut composer = composer();
    composer.begin_history_search();
    let (text, cursor) = render(&composer, /*width*/ 48, /*footer*/ None);
    assert_eq!(
        cursor.map(|(_, y)| y),
        Some(text.lines().count() as u16 - 1)
    );
}

#[test]
fn slash_popup_preserves_footer_and_selection_style() {
    let (tx, _rx) = unbounded_channel::<AppEvent>();
    let mut composer = ChatComposer::new(
        /*has_input_focus*/ true,
        AppEventSender::new(tx),
        /*enhanced_keys_supported*/ false,
        "Ask Codex to do anything".to_string(),
        /*disable_paste_burst*/ true,
    );
    composer.set_status_line_enabled(/*enabled*/ true);
    composer.set_status_line(Some("model · high · fast".into()));
    for ch in ['/', 'm'] {
        composer.handle_key_event(KeyEvent::from(KeyCode::Char(ch)));
    }
    composer.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));

    for (width, clipped) in [(80, false), (32, false), (80, true)] {
        let height = if clipped {
            6
        } else {
            composer.desired_height(width)
        };
        let area = Rect::new(/*x*/ 0, /*y*/ 0, width, height);
        let mut buf = Buffer::empty(area);
        composer.render(area, &mut buf);
        let ComposerLayout {
            composer: surface,
            textarea,
            popup,
            footer,
            ..
        } = composer.layout_with_options(area, ComposerRenderOptions::default());
        assert_eq!(popup.bottom(), surface.y);
        assert_eq!(surface.bottom(), footer.y);
        assert_eq!(composer.cursor_pos(area), Some((4, textarea.y)));
        let footer_text: String = (0..width).map(|x| buf[(x, height - 1)].symbol()).collect();
        assert!(footer_text.contains("model · high · fast"));
        if !clipped && width == 80 {
            insta::assert_snapshot!("slash_popup_footer_wide", format!("{buf:?}"));
        }
    }

    let footer = TranscriptFooter {
        text: "transcript status".into(),
        cursor_column: None,
        is_interactive: false,
    };
    let options = composer.resolve_render_options(ComposerRenderOptions {
        footer: Some(&footer),
        ..Default::default()
    });
    assert!(options.footer.is_some());
    let width = 80;
    let area = Rect::new(
        /*x*/ 0,
        /*y*/ 0,
        width,
        composer.desired_height_with_options(width, options),
    );
    let mut buf = Buffer::empty(area);
    composer.render_with_options(area, &mut buf, /*mask_char*/ None, options);
    let footer_text: String = (0..width)
        .map(|x| buf[(x, area.bottom() - 1)].symbol())
        .collect();
    assert!(footer_text.contains("transcript status"));
}
