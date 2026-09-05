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
