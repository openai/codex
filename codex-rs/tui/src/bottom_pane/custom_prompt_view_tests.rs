use super::*;
use crate::key_hint;
use crate::keymap::KeymapContext;
use crate::keymap::RuntimeKeymap;
use pretty_assertions::assert_eq;
use std::sync::mpsc::Receiver;

#[test]
fn paste_burst_newline_does_not_submit_short_first_line() {
    let now = Instant::now();

    for (first_line, second_line) in [("x", "rest"), ("id", "body"), ("foo", "bar")] {
        let (mut view, submitted_rx) = custom_prompt_view();
        let mut ms = 0;

        for ch in first_line.chars() {
            view.handle_key_event_at(KeyEvent::from(KeyCode::Char(ch)), now + elapsed(ms));
            ms += 1;
        }
        view.handle_key_event_at(KeyEvent::from(KeyCode::Enter), now + elapsed(ms));
        ms += 1;
        for ch in second_line.chars() {
            view.handle_key_event_at(KeyEvent::from(KeyCode::Char(ch)), now + elapsed(ms));
            ms += 1;
        }

        assert!(submitted_rx.try_recv().is_err());
        assert!(!view.is_complete());

        view.handle_key_event_at(KeyEvent::from(KeyCode::Enter), now + elapsed(/*ms*/ 200));

        assert_eq!(
            submitted_rx.try_recv(),
            Ok(format!("{first_line}\n{second_line}"))
        );
        assert!(view.is_complete());
    }
}

#[test]
fn paste_burst_newline_after_tab_does_not_submit() {
    let (mut view, submitted_rx) = custom_prompt_view();
    let now = Instant::now();
    let mut ms = 0;

    view.handle_key_event_at(KeyEvent::from(KeyCode::Char('x')), now + elapsed(ms));
    ms += 1;
    view.handle_key_event_at(KeyEvent::from(KeyCode::Tab), now + elapsed(ms));
    ms += 1;
    view.handle_key_event_at(KeyEvent::from(KeyCode::Enter), now + elapsed(ms));
    ms += 1;
    for ch in "rest".chars() {
        view.handle_key_event_at(KeyEvent::from(KeyCode::Char(ch)), now + elapsed(ms));
        ms += 1;
    }

    assert!(submitted_rx.try_recv().is_err());
    assert!(!view.is_complete());

    view.handle_key_event_at(KeyEvent::from(KeyCode::Enter), now + elapsed(/*ms*/ 200));

    assert_eq!(submitted_rx.try_recv(), Ok("x\nrest".to_string()));
    assert!(view.is_complete());
}

#[test]
fn delayed_enter_after_typing_submits() {
    let (mut view, submitted_rx) = custom_prompt_view();
    let now = Instant::now();

    for (idx, ch) in "foo".chars().enumerate() {
        view.handle_key_event_at(KeyEvent::from(KeyCode::Char(ch)), now + elapsed(idx * 20));
    }
    view.handle_key_event_at(KeyEvent::from(KeyCode::Enter), now + elapsed(/*ms*/ 80));

    assert_eq!(submitted_rx.try_recv(), Ok("foo".to_string()));
    assert!(view.is_complete());
}

#[test]
fn vim_insert_escape_clears_paste_burst_before_enter() {
    let (mut view, submitted_rx) = custom_prompt_view();
    let now = Instant::now();
    view.enable_vim_in_insert_mode();

    for (idx, ch) in "abc".chars().enumerate() {
        view.handle_key_event_at(KeyEvent::from(KeyCode::Char(ch)), now + elapsed(idx));
    }
    view.handle_key_event_at(KeyEvent::from(KeyCode::Esc), now + elapsed(/*ms*/ 3));
    view.handle_key_event_at(KeyEvent::from(KeyCode::Enter), now + elapsed(/*ms*/ 4));

    assert_eq!(submitted_rx.try_recv(), Ok("abc".to_string()));
    assert_eq!(view.completion(), Some(ViewCompletion::Accepted));
}

#[test]
fn vim_prompt_hint_tracks_escape_behavior() {
    let (mut view, _submitted_rx) = custom_prompt_view();
    let rendered_hint = |view: &CustomPromptView, width: u16| {
        let area = Rect::new(0, 0, width, 5);
        let mut buf = Buffer::empty(area);
        view.render(area, &mut buf);
        (0..area.width)
            .map(|x| buf[(x, area.height - 1)].symbol())
            .collect::<String>()
            .trim_end()
            .to_string()
    };
    let vim_color = |view: &CustomPromptView| {
        let area = Rect::new(0, 0, 80, 5);
        let mut buf = Buffer::empty(area);
        view.render(area, &mut buf);
        buf[(67, 4)].style().fg
    };

    insta::assert_snapshot!(rendered_hint(&view, /*width*/ 80), @"Press enter to confirm or esc to go back");
    view.enable_vim_in_insert_mode();
    insta::assert_snapshot!(rendered_hint(&view, /*width*/ 80), @"Press enter to confirm or esc to enter normal mode                 Vim: Insert");
    insta::assert_snapshot!(rendered_hint(&view, /*width*/ 60), @"Press enter to confirm or esc to enter norm…   Vim: Insert");
    insta::assert_snapshot!(rendered_hint(&view, /*width*/ 14), @"Press enter to");
    assert_eq!(vim_color(&view), Some(ratatui::style::Color::Green));

    view.handle_key_event(KeyEvent::from(KeyCode::Esc));
    insta::assert_snapshot!(rendered_hint(&view, /*width*/ 80), @"Press enter to confirm or esc to go back                           Vim: Normal");
    assert_eq!(vim_color(&view), Some(ratatui::style::Color::Magenta));
}

#[test]
fn vim_insert_cursor_tracks_mode_and_normal_mode_commands() {
    let (mut view, _submitted_rx) = custom_prompt_view();
    let area = Rect::new(0, 0, 80, 10);
    view.textarea.set_text_clearing_elements("rename");
    view.textarea.set_cursor("rename".len());
    let mut config = codex_config::types::TuiKeymap::default();
    config.editor.move_left = Some(codex_config::types::KeybindingsSpec::One(
        codex_config::types::KeybindingSpec("f2".to_string()),
    ));
    let mut keymap = RuntimeKeymap::from_config(&config).expect("valid editor remap");
    keymap.vim_normal.delete_char = vec![key_hint::plain(KeyCode::F(3))];
    view.set_keymap_bindings(&keymap);
    view.handle_key_event(KeyEvent::from(KeyCode::F(2)));
    assert_eq!(view.textarea.cursor(), "rename".len() - 1);
    view.textarea.set_cursor("rename".len());

    assert!(matches!(
        view.cursor_style(area),
        crossterm::cursor::SetCursorStyle::DefaultUserShape
    ));

    view.enable_vim_in_insert_mode();
    assert!(view.prefer_esc_to_handle_key_event());
    assert!(view.keymap_contexts().contains(KeymapContext::Editor));
    assert!(matches!(
        view.cursor_style(area),
        crossterm::cursor::SetCursorStyle::SteadyBar
    ));

    view.handle_key_event(KeyEvent::from(KeyCode::Esc));
    assert!(!view.is_complete());
    assert!(!view.prefer_esc_to_handle_key_event());
    assert!(view.keymap_contexts().contains(KeymapContext::VimNormal));
    assert!(matches!(
        view.cursor_style(area),
        crossterm::cursor::SetCursorStyle::DefaultUserShape
    ));

    for key_code in [KeyCode::Enter, KeyCode::Esc] {
        view.handle_key_event(KeyEvent::from(KeyCode::Char('d')));
        assert!(view.prefer_esc_to_handle_key_event());
        assert!(view.keymap_contexts().contains(KeymapContext::VimOperator));
        view.handle_key_event(KeyEvent::from(key_code));
        assert!(!view.is_complete());
        assert!(!view.prefer_esc_to_handle_key_event());
        assert_eq!(view.textarea.text(), "rename");
    }

    view.handle_key_event(KeyEvent::from(KeyCode::Char('d')));
    view.handle_key_event(KeyEvent::from(KeyCode::Char('i')));
    assert!(
        view.keymap_contexts()
            .contains(KeymapContext::VimTextObject)
    );
    view.handle_key_event(KeyEvent::from(KeyCode::Esc));

    view.handle_key_event(KeyEvent::from(KeyCode::Char('h')));
    view.handle_key_event(KeyEvent::from(KeyCode::F(3)));
    assert_eq!(view.textarea.text(), "renae");

    view.handle_key_event(KeyEvent::from(KeyCode::Char('i')));
    assert!(matches!(
        view.cursor_style(area),
        crossterm::cursor::SetCursorStyle::SteadyBar
    ));
    view.handle_key_event(KeyEvent::from(KeyCode::Char('m')));
    assert_eq!(view.textarea.text(), "rename");

    view.handle_key_event(KeyEvent::from(KeyCode::Esc));
    view.handle_key_event(KeyEvent::from(KeyCode::Esc));
    assert!(view.is_complete());
    assert_eq!(view.completion(), Some(ViewCompletion::Cancelled));
}

#[test]
fn escape_without_vim_cancels_prompt() {
    let (mut view, _submitted_rx) = custom_prompt_view();

    view.handle_key_event(KeyEvent::from(KeyCode::Esc));

    assert!(view.is_complete());
    assert_eq!(view.completion(), Some(ViewCompletion::Cancelled));
}

fn custom_prompt_view() -> (CustomPromptView, Receiver<String>) {
    let (submitted, submitted_rx) = std::sync::mpsc::channel();
    let view = CustomPromptView::new(
        "Edit goal".to_string(),
        "Type a goal objective and press Enter".to_string(),
        String::new(),
        /*context_label*/ None,
        Box::new(move |text| {
            submitted.send(text).expect("send submitted text");
        }),
    );
    (view, submitted_rx)
}

fn elapsed(ms: usize) -> std::time::Duration {
    std::time::Duration::from_millis(ms as u64)
}
