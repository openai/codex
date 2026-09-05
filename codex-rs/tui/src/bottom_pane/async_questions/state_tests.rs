//! Exercise question drafts, selection, and the constrained terminal viewport.
use super::*;
use crate::render::renderable::Renderable;
use crossterm::event::KeyCode;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;

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
fn replay_preserves_drafts_and_does_not_reopen_handled_questions() {
    let mut original = editor();
    original.set_expanded(/*expanded*/ true);
    original.accept_answer();
    original.handle_key_event(KeyEvent::from(KeyCode::Char('2')));
    let saved = original.capture();
    for replay_first in [false, true] {
        let mut restored = editor();
        if !replay_first {
            restored.state = QuestionState::default();
        }
        restored.restore(saved.clone());
        restored.append("message", &[question("First", /*options*/ None)]);
        assert_eq!(restored.capture(), saved);
        restored.accept_answer();
        restored.append("message", &[question("First", /*options*/ None)]);
        assert_eq!(restored.unanswered_count(), 0);
    }
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

#[test]
fn selected_other_renders_as_a_dim_placeholder() {
    let mut editor = editor();
    editor.navigate(/*forward*/ true);
    editor.handle_key_event(KeyEvent::from(KeyCode::Char('2')));
    let mut repeat = KeyEvent::from(KeyCode::Char('2'));
    repeat.kind = KeyEventKind::Repeat;
    editor.handle_key_event(repeat);
    assert!(editor.composer.is_empty());
    editor.handle_paste("saved".into());
    editor.handle_key_event(KeyEvent::from(KeyCode::Up));
    editor.handle_key_event(KeyEvent::from(KeyCode::Char('2')));
    editor.handle_key_event(repeat);
    assert_eq!(editor.composer.current_text(), "saved");
    editor.handle_key_event(KeyEvent::from(KeyCode::Char('2')));
    editor.handle_key_event(repeat);
    assert_eq!(editor.composer.current_text(), "saved22");
    editor.on_ctrl_c();
    editor.state.pending[1].question.title = "Long question ".repeat(40);
    let buffer = render_editor(&editor, /*width*/ 50, /*height*/ 10);
    let area = buffer.area;
    assert!(editor.cursor_pos(area).is_some());
    let input = editor.other_input_area(
        editor
            .layout_sections(crate::bottom_pane::selection_popup_common::menu_surface_inset(area))
            .options_area,
    );
    for offset in 0..5 {
        assert_eq!(buffer[(input.x + offset, input.y)].modifier, Modifier::DIM);
    }
    insta::assert_snapshot!("selected_other_placeholder", buffer_text(&buffer));
    editor.state.pending[1].question.title = "Second".into();
    render_editor(&editor, /*width*/ 50, /*height*/ 10);
    editor.set_vim_enabled(/*enabled*/ true);
    editor.handle_key_event(KeyEvent::from(KeyCode::Char('i')));
    editor.handle_paste("draft".into());
    editor.handle_key_event(KeyEvent::from(KeyCode::Esc));
    editor.handle_key_event(KeyEvent::from(KeyCode::Up));
    assert_eq!(editor.selected_option_index(), Some(0));
    editor.handle_key_event(KeyEvent::from(KeyCode::Enter));
    assert!(editor.submission.take().is_some());
    editor.handle_key_event(KeyEvent::from(KeyCode::Char('x')));
    editor.handle_key_event(KeyEvent::from(KeyCode::Esc));
    for _ in 0..2 {
        editor.handle_key_event(KeyEvent::from(KeyCode::Char('u')));
    }
    assert_eq!(editor.composer.current_text(), "");
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
fn question_choice_remaps_paging_and_inline_feedback() {
    use codex_config::types::KeybindingSpec;
    use codex_config::types::KeybindingsSpec;
    use codex_config::types::TuiKeymap;
    let mut editor = editor();
    editor.navigate(/*forward*/ true);
    editor.state.pending[1].question.options = Some(
        (1..=12)
            .map(|i| format!("Option {i} is long enough to wrap across rows"))
            .collect(),
    );
    let mut config = TuiKeymap::default();
    config.list.move_down = Some(KeybindingsSpec::One(KeybindingSpec("x".into())));
    config.list.accept = Some(KeybindingsSpec::One(KeybindingSpec("f9".into())));
    config.list.jump_top = Some(KeybindingsSpec::One(KeybindingSpec("7".into())));
    editor.set_keymap(&RuntimeKeymap::from_config(&config).unwrap());
    editor.handle_key_event(KeyEvent::from(KeyCode::Down));
    assert_eq!(editor.selected_option_index(), Some(0));
    editor.handle_key_event(KeyEvent::from(KeyCode::Char('x')));
    assert_eq!(editor.selected_option_index(), Some(1));
    let mut buffer = render_editor(&editor, /*width*/ 50, /*height*/ 12);
    let area = buffer.area;
    let visible = editor.visible_options.get().1;
    assert_eq!(visible, 2);
    editor.handle_key_event(KeyEvent::from(KeyCode::PageDown));
    assert_eq!(editor.selected_option_index(), Some(1 + visible));
    editor.handle_key_event(KeyEvent::from(KeyCode::Char('7')));
    assert_eq!(editor.selected_option_index(), Some(0));
    editor.handle_key_event(KeyEvent::from(KeyCode::Char('7')));
    assert_eq!(editor.selected_option_index(), Some(0));
    editor.render(area, &mut buffer);
    insta::assert_snapshot!("question_remapped_choice_footer", buffer_text(&buffer));
}

#[test]
fn question_inline_history_search() {
    let mut editor = editor();
    editor.navigate(/*forward*/ true);
    editor.state.pending[1].question.options =
        Some((1..=12).map(|i| format!("Option {i}")).collect());
    editor.select_option(/*index*/ 12);
    editor
        .composer
        .record_replayed_user_message_history(crate::bottom_pane::HistoryEntry::new(
            "earlier answer".into(),
        ));
    editor.handle_paste("original answer".into());
    let saved = editor.capture();
    editor.handle_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
    editor.handle_key_event(KeyEvent::from(KeyCode::Char('e')));
    let buffer = render_editor(&editor, /*width*/ 50, /*height*/ 12);
    assert!(
        buffer
            .content
            .iter()
            .any(|cell| cell.modifier.contains(Modifier::REVERSED))
    );
    insta::assert_snapshot!("question_inline_history_search", buffer_text(&buffer));
    assert_eq!(editor.capture(), saved);
}

#[test]
fn question_other_remains_visible_at_nine_columns_and_distinguishes_named_other() {
    let mut editor = editor();
    editor.navigate(/*forward*/ true);
    editor.state.pending[1].question.options = Some(vec!["Other".into()]);
    editor.handle_paste("other".into());
    editor.on_ctrl_c();
    editor.handle_key_event(KeyEvent::from(KeyCode::Up));
    let buffer = render_editor(&editor, /*width*/ 50, /*height*/ 12);
    insta::assert_snapshot!("question_named_other", buffer_text(&buffer));
    editor.select_option(/*index*/ 1);
    editor.handle_paste("x".into());
    let buffer = render_editor(&editor, /*width*/ 9, /*height*/ 8);
    let narrow = buffer.area;
    assert!(editor.cursor_pos(narrow).is_some());
    assert!(buffer.content.iter().any(|cell| cell.symbol() == "x"));
    insta::assert_snapshot!("question_narrow_other", buffer_text(&buffer));
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
            selected_idx: Some(editor.options_len() - 1),
            scroll_top: editor.options_len() - editor.visible_options.get().1,
        }
    );
    editor.handle_key_event(KeyEvent::from(KeyCode::Up));
    render_editor(&editor, /*width*/ 80, /*height*/ 80);
    assert_eq!(editor.visible_options.get(), (0, editor.options_len()));
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

#[test]
fn question_wrapped_other_shares_the_text_indent() {
    let mut editor = editor();
    editor.state.pending.remove(0);
    editor.state.pending[0].question.options = Some(vec![
        "A suggested answer that is long enough to wrap across multiple rows".into(),
    ]);
    editor.select_option(/*index*/ 1);
    editor.handle_paste("An alternative answer that wraps across several option rows".into());
    let buffer = render_editor(&editor, /*width*/ 36, /*height*/ 16);
    insta::assert_snapshot!("question_wrapped_other", buffer_text(&buffer));
    editor.composer.clear_for_ctrl_c();
    editor.handle_paste(" \n ".into());
    editor.select_option(/*index*/ 0);
    assert_eq!(editor.other_label(), "Other");
    editor.select_option(/*index*/ 1);
    editor
        .composer
        .set_text_content("long draft\n".repeat(1000), Vec::new(), Vec::new());
    editor.composer.move_cursor_to_end();
    let buffer = render_editor(&editor, /*width*/ 50, /*height*/ 18);
    assert_eq!(editor.cursor_pos(buffer.area), Some((7, 12)));
    insta::assert_snapshot!("question_capped_other", buffer_text(&buffer));
    editor.select_option(/*index*/ 0);
    assert!(editor.other_label().len() <= 128);
    editor.state.pending[0].draft.text = format!("a{}", "\u{301}".repeat(10_000));
    assert!(editor.other_label().len() <= 512);
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

#[test]
fn question_navigation_resets_history_recall() {
    let mut editor = editor();
    for text in ["older", "newer"] {
        editor.composer.record_replayed_user_message_history(
            crate::bottom_pane::HistoryEntry::new(text.into()),
        );
    }
    editor.handle_key_event(KeyCode::Up.into());
    assert_eq!(editor.composer.current_text(), "newer");
    editor.navigate(/*forward*/ true);
    editor.navigate(/*forward*/ false);
    editor.handle_key_event(KeyCode::Up.into());
    assert_eq!(editor.composer.current_text(), "newer");
}
