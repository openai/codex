//! Exercise question drafts, selection, and the constrained terminal viewport.
use super::*;
use crate::render::renderable::Renderable;
use crossterm::event::KeyCode;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

fn editor() -> AsyncQuestions {
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let mut editor = AsyncQuestions::new(
        AppEventSender::new(tx),
        /*has_input_focus*/ true,
        /*enhanced_keys_supported*/ true,
        /*disable_paste_burst*/ true,
        RuntimeKeymap::defaults(),
    );
    editor.next_hint = Some(crate::key_hint::alt(KeyCode::Up).into());
    editor.append(
        "message",
        &[
            question("First", /*options*/ None),
            question("Second", Some(vec!["Named".into()])),
        ],
    );
    editor
}

#[test]
fn arrival_preserves_vim_undo_and_navigation_flushes_buffered_input() {
    let mut editor = editor();
    editor.set_expanded(/*expanded*/ true);
    editor.set_vim_enabled(/*enabled*/ true);
    editor.handle_key_event(KeyEvent::from(KeyCode::Char('i')));
    editor.handle_paste("draft".into());
    editor.handle_key_event(KeyEvent::from(KeyCode::Esc));
    editor.append("new", &[question("New", /*options*/ None)]);
    editor.handle_key_event(KeyEvent::from(KeyCode::Char('u')));
    assert_eq!(editor.composer.current_text(), "");
    editor.handle_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
    assert_eq!(editor.composer.current_text(), "draft");
    editor.set_vim_enabled(/*enabled*/ false);
    editor.composer.set_disable_paste_burst(/*disabled*/ false);
    editor.composer.move_cursor_to_end();
    for ch in "buffered".chars() {
        editor.handle_key_event(KeyEvent::from(KeyCode::Char(ch)));
    }
    assert!(editor.composer.is_in_paste_burst());
    editor.navigate(/*forward*/ true);
    editor.navigate(/*forward*/ false);
    assert_eq!(editor.composer.current_text_with_pending(), "draftbuffered");
    for ch in "skipped".chars() {
        editor.handle_key_event(KeyEvent::from(KeyCode::Char(ch)));
    }
    assert!(editor.composer.is_in_paste_burst());
    editor.handle_key_event(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::CONTROL));
    editor.composer.flush_pending_input();
    assert_eq!(
        (
            editor.current_question().unwrap().title.as_str(),
            editor.composer.current_text()
        ),
        ("Second", String::new())
    );
}

#[test]
fn long_prompt_keeps_the_active_input_visible() {
    let mut editor = editor();
    editor.state.pending[0].question.title = "A lengthy prompt. ".repeat(30);
    editor.handle_paste("visible answer".into());
    let buffer = render_editor(&editor, /*width*/ 40, /*height*/ 8);
    let text = buffer_text(&buffer);
    assert!(text.contains("visible answer"));
    insta::assert_snapshot!("long_prompt_active_input", text);
}

fn buffer_text(buffer: &ratatui::buffer::Buffer) -> String {
    (buffer.area.y..buffer.area.bottom())
        .map(|y| {
            (buffer.area.x..buffer.area.right())
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn question_wrapped_options_and_other_share_the_text_indent() {
    let mut editor = editor();
    editor.state.pending.remove(0);
    editor.state.pending[0].question.options = Some(vec![
        "A suggested answer that is long enough to wrap across multiple rows".into(),
    ]);
    editor.restore_current_draft();
    let buffer = render_editor(&editor, /*width*/ 36, /*height*/ 16);
    insta::assert_snapshot!("question_wrapped_named_option", buffer_text(&buffer));
    let options = editor.state.pending[0].question.options.as_mut().unwrap();
    options.insert(
        0,
        "Another suggested answer that wraps over several rows before the selected option".into(),
    );
    editor.select_option(/*index*/ 1);
    let clipped = render_editor(&editor, /*width*/ 36, /*height*/ 10);
    insta::assert_snapshot!("question_wrapped_selected_option", buffer_text(&clipped));
    render_editor(&editor, /*width*/ 0, /*height*/ 0);
    editor.go_next_or_submit();
    assert!(editor.submission.is_none());
    render_editor(&editor, /*width*/ 36, /*height*/ 6);
    editor.go_next_or_submit();
    assert!(editor.submission.is_none());
    insta::assert_snapshot!(
        "question_clipped_choice_rejected",
        buffer_text(&render_editor(
            &editor, /*width*/ 50, /*height*/ 8
        ))
    );
    render_editor(&editor, /*width*/ 80, /*height*/ 20);
    editor.go_next_or_submit();
    assert!(editor.submission.take().is_some());
    editor.append(
        "many",
        &[question("Bounded", Some(vec!["x".repeat(41); 1000]))],
    );
    editor.navigate(/*forward*/ true);
    assert_eq!(editor.options().len(), 32);
    render_editor(&editor, /*width*/ 50, /*height*/ 10);
    let height = editor.desired_height(/*width*/ 50);
    assert_eq!(editor.visible_options.get().1, 2);
    editor.handle_key_event(KeyEvent::from(KeyCode::End));
    assert_eq!(editor.desired_height(/*width*/ 50), height);
    assert_eq!(
        editor.current_answer().unwrap().options_state,
        ScrollState {
            selected_idx: Some(31),
            scroll_top: 32 - editor.visible_options.get().1,
        }
    );
    render_editor(&editor, /*width*/ 80, /*height*/ 80);
    assert_eq!(editor.visible_options.get(), (0, 32));
    editor.state.pending[editor.state.current_idx]
        .question
        .title = "Long question ".repeat(3 * 65_536);
    assert_eq!(editor.desired_height(/*width*/ 50), u16::MAX);
    let clipped = render_editor(&editor, /*width*/ 50, /*height*/ 3);
    editor.go_next_or_submit();
    assert!(editor.submission.is_none());
    insta::assert_snapshot!("question_clipped_prompt", buffer_text(&clipped));
    editor.state.pending.last_mut().unwrap().question.title = "Question".into();
    for action in ["move_left", "move_right", "cancel"] {
        let config = toml::from_str(&format!("[list]\n{action} = '1'")).unwrap();
        editor.set_keymap(&RuntimeKeymap::from_config(&config).unwrap());
        editor.set_expanded(/*expanded*/ true);
        render_editor(&editor, /*width*/ 80, /*height*/ 80);
        for modifiers in [KeyModifiers::CONTROL, KeyModifiers::ALT] {
            editor.handle_key_event(KeyEvent::new(KeyCode::Char('1'), modifiers));
            assert!(editor.submission.is_none());
        }
        editor.handle_key_event(KeyEvent::from(KeyCode::Char('1')));
        assert_eq!(editor.expanded, action != "cancel");
        assert!(editor.submission.is_none());
    }
}

fn question(title: &str, options: Option<Vec<String>>) -> AsyncUserInputQuestion {
    AsyncUserInputQuestion {
        title: title.into(),
        options,
    }
}

fn render_editor(editor: &AsyncQuestions, width: u16, height: u16) -> Buffer {
    let area = Rect::new(/*x*/ 0, /*y*/ 0, width, height);
    let mut buffer = Buffer::empty(area);
    editor.render(area, &mut buffer);
    buffer
}

#[test]
fn existing_cross_context_keymaps_load_without_misleading_submit_hints() {
    for action in ["interrupt_turn", "edit_queued_message"] {
        let config =
            toml::from_str(&format!("[chat]\n{action} = 'f12'\n[list]\naccept = 'f12'")).unwrap();
        let keymap = RuntimeKeymap::from_config(&config).unwrap();
        let mut editor = editor();
        editor.navigate(/*forward*/ true);
        editor.set_keymap(&keymap);
        insta::allow_duplicates! { insta::assert_snapshot!(editor.footer_lines(/*width*/ 100, /*option_tip*/ None)[0].to_string(), @"ctrl + ] skip   ⌥ + ↓ prev question"); }
        editor.handle_key_event(KeyEvent::from(KeyCode::F(12)));
        assert!(editor.submission.is_none());
    }
}
