//! Pasted text edits the active history query while preserving the original draft.

use super::super::super::chat_composer_history::HistoryEntry;
use super::super::ChatComposer;
use super::super::InputResult;
use super::super::LARGE_PASTE_CHAR_THRESHOLD;
use crate::app_event_sender::AppEventSender;
use crate::render::renderable::Renderable;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use tokio::sync::mpsc::unbounded_channel;

fn composer_with_history() -> ChatComposer {
    let (tx, _rx) = unbounded_channel();
    let mut composer = ChatComposer::new(
        /*has_input_focus*/ true,
        AppEventSender::new(tx),
        /*enhanced_keys_supported*/ false,
        "Ask Codex to do anything".to_string(),
        /*disable_paste_burst*/ false,
    );
    for entry in ["git status", "git log"] {
        composer
            .history
            .record_local_submission(HistoryEntry::new(entry.to_string()));
    }
    composer.set_text_content("draft".to_string(), Vec::new(), Vec::new());
    composer
}

fn render_composer(composer: &ChatComposer, width: u16) -> Terminal<TestBackend> {
    let mut terminal = Terminal::new(TestBackend::new(width, /*height*/ 5)).unwrap();
    terminal
        .draw(|frame| composer.render(frame.area(), frame.buffer_mut()))
        .unwrap();
    terminal
}

#[test]
fn history_search_paste_appends_query_and_accepts_match() {
    let mut composer = composer_with_history();
    composer.handle_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
    assert!(composer.handle_paste("git".to_string()));
    assert_eq!(composer.draft.textarea.text(), "git log");

    composer.handle_key_event(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
    assert!(composer.handle_paste("status".to_string()));
    assert_eq!(
        composer.history_search.as_ref().unwrap().query,
        "git status"
    );
    assert_eq!(composer.draft.textarea.text(), "git status");

    let terminal = render_composer(&composer, /*width*/ 70);
    insta::assert_snapshot!("history_search_pasted_query", terminal.backend());

    let (result, _) = composer.handle_key_event(KeyCode::Enter.into());
    assert!(matches!(result, InputResult::None));
    assert!(!composer.history_search_active());
    assert_eq!(composer.draft.textarea.text(), "git status");
}

#[test]
fn history_search_large_paste_clamps_cursor() {
    let mut composer = composer_with_history();
    composer.handle_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
    composer.handle_paste("x".repeat(usize::from(u16::MAX) + 1));

    let terminal = render_composer(&composer, /*width*/ 80);
    let cursor = composer.history_search_cursor_pos(terminal.size().unwrap().into());
    assert_eq!(cursor, Some((79, 4)));
    insta::assert_snapshot!(
        "history_search_large_paste_cursor",
        format!("{}\nCursor: {cursor:?}", terminal.backend())
    );
}

#[test]
fn history_search_empty_paste_preserves_selected_match() {
    let mut composer = composer_with_history();
    let reverse_search = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL);
    composer.handle_key_event(reverse_search);
    composer.handle_paste("git".to_string());
    assert_eq!(composer.draft.textarea.text(), "git log");
    composer.handle_key_event(reverse_search);
    assert_eq!(composer.draft.textarea.text(), "git status");
    let selected_draft = composer.snapshot_draft();

    for paste in ["", "\x1b[31m\x1b[0m"] {
        composer.handle_paste(paste.to_string());
        assert_eq!(composer.snapshot_draft(), selected_draft);
    }

    let terminal = render_composer(&composer, /*width*/ 70);
    insta::assert_snapshot!("history_search_empty_paste", terminal.backend());
}

#[test]
fn history_search_paste_shows_separators_and_matches_original_query() {
    let mut composer = composer_with_history();
    for entry in ["foobarbaz", "foo↵bar⇥baz"] {
        composer
            .history
            .record_local_submission(HistoryEntry::new(entry.to_string()));
    }
    let query = "foo\nbar\tbaz";
    let reverse_search = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL);
    composer.handle_key_event(reverse_search);
    composer.handle_paste(query.to_string());
    assert_eq!(composer.history_search.as_ref().unwrap().query, query);
    assert_eq!(composer.draft.textarea.text(), "draft");

    let terminal = render_composer(&composer, /*width*/ 70);
    let cursor = composer.history_search_cursor_pos(terminal.size().unwrap().into());
    assert_eq!(cursor, Some((31, 4)));
    insta::assert_snapshot!(
        "history_search_pasted_separators",
        format!("{}\nCursor: {cursor:?}", terminal.backend())
    );

    composer.handle_key_event(KeyCode::Esc.into());
    composer
        .history
        .record_local_submission(HistoryEntry::new(query.to_string()));
    composer.handle_key_event(reverse_search);
    composer.handle_paste(query.to_string());
    composer.handle_key_event(KeyCode::Enter.into());
    assert!(!composer.history_search_active());
    assert_eq!(composer.draft.textarea.text(), query);
}

#[test]
fn history_search_paste_preserves_original_draft_on_miss_and_cancel() {
    let mut composer = composer_with_history();
    composer.handle_paste("x".repeat(LARGE_PASTE_CHAR_THRESHOLD + 1));
    composer.draft.textarea.set_cursor(/*pos*/ 2);
    let original_draft = composer.snapshot_draft();

    for cancel_key in [
        KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
    ] {
        composer.handle_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
        composer.handle_paste("git".to_string());
        assert_eq!(composer.draft.textarea.text(), "git log");

        composer.handle_paste(" missing".to_string());
        assert_eq!(composer.snapshot_draft(), original_draft);
        assert_eq!(
            composer.history_search.as_ref().unwrap().query,
            "git missing"
        );

        composer.handle_key_event(cancel_key);
        assert!(!composer.history_search_active());
        assert_eq!(composer.snapshot_draft(), original_draft);
    }
}

#[test]
fn history_search_paste_uses_full_sanitized_text() {
    let temp = tempfile::tempdir().unwrap();
    let image_path = temp.path().join("history.png");
    image::RgbaImage::new(/*width*/ 1, /*height*/ 1)
        .save(&image_path)
        .unwrap();
    let image_path = image_path.to_string_lossy().into_owned();
    let large_paste = "x".repeat(LARGE_PASTE_CHAR_THRESHOLD + 1);

    for (pasted, query) in [
        (image_path.clone(), image_path),
        (large_paste.clone(), large_paste),
        (
            "é\r\n中\r\x1b[31mtext\x1b[0m".to_string(),
            "é\n中\ntext".to_string(),
        ),
    ] {
        let mut composer = composer_with_history();
        composer
            .history
            .record_local_submission(HistoryEntry::new(query.clone()));
        composer.handle_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
        composer.handle_paste(pasted);

        assert_eq!(composer.history_search.as_ref().unwrap().query, query);
        assert_eq!(composer.draft.textarea.text(), query);
        assert!(composer.attachments.local_image_paths().is_empty());
        assert!(composer.draft.pending_pastes.is_empty());
        assert!(composer.current_text_elements().is_empty());
    }
}
