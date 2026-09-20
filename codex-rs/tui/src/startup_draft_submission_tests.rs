//! Verify confirmation follows the visible draft and its selected destination.

use super::*;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn startup_enter_confirms_one_visible_draft_and_escape_cancels() {
    let mut pump = tests::quiet_startup_test_pump();
    let mut tui = crate::tui::test_support::make_test_tui().expect("create test terminal");
    pump.bottom_pane.insert_str("keep this startup draft");
    let draft = pump.bottom_pane.composer_draft_snapshot();
    for _ in 0..3 {
        pump.handle_event(&mut tui, TuiEvent::Key(KeyEvent::from(KeyCode::Enter)))
            .expect("confirm startup draft");
    }
    assert!(pump.submission_pending);
    assert_eq!(pump.bottom_pane.composer_draft_snapshot(), draft);
    pump.handle_event(&mut tui, TuiEvent::Key(KeyEvent::from(KeyCode::Esc)))
        .expect("cancel startup submission");
    assert!(!pump.take_submission_intent());
    assert_eq!(pump.bottom_pane.composer_draft_snapshot(), draft);
    pump.handle_event(&mut tui, TuiEvent::Key(KeyEvent::from(KeyCode::Enter)))
        .expect("confirm again");
    pump.handle_event(&mut tui, TuiEvent::Paste(" with more text".into()))
        .expect("edit confirmed startup draft");
    assert!(!pump.take_submission_intent());
    assert_eq!(
        pump.bottom_pane.composer_text(),
        "keep this startup draft with more text"
    );
    let draft = pump.bottom_pane.composer_draft_snapshot();
    pump.session_action = StartupDraftSessionAction::NewFromCommandCenter;
    pump.handle_event(&mut tui, TuiEvent::Key(KeyEvent::from(KeyCode::Enter)))
        .expect("command-center startup stays edit-only");
    assert!(!pump.take_submission_intent());
    assert_eq!(pump.bottom_pane.composer_draft_snapshot(), draft);
    pump.bottom_pane
        .set_composer_text(String::new(), Vec::new(), Vec::new());
    pump.events = Box::pin(futures::stream::iter(['c', 'd'].map(|character| {
        TuiEvent::Key(KeyEvent::new(
            KeyCode::Char(character),
            KeyModifiers::CONTROL,
        ))
    })));
    pump.flush_pending_events(&mut tui)
        .await
        .expect("command-center startup survives cancellation during the final input drain");
}

#[tokio::test]
async fn startup_submission_honors_configured_submit_binding() {
    let mut pump = tests::quiet_startup_test_pump();
    let mut tui = crate::tui::test_support::make_test_tui().expect("create test terminal");
    let config = toml::from_str(
        "[composer]\nsubmit = \"ctrl-x s\"\n[editor]\ninsert_newline = \"enter\"\nmove_line_start = \"ctrl-x a\"\n"
    )
    .unwrap();
    let keymap = crate::keymap::RuntimeKeymap::from_config(&config).unwrap();
    pump.bottom_pane.set_keymap_bindings(&keymap);
    pump.key_chords = keymap.chords;
    for code in [
        KeyCode::Char('a'),
        KeyCode::Char('b'),
        KeyCode::Enter,
        KeyCode::Char('c'),
    ] {
        pump.handle_event(&mut tui, TuiEvent::Key(KeyEvent::from(code)))
            .expect("rebound Enter inserts one newline after a paste burst");
    }
    pump.bottom_pane.flush_composer_paste_burst();
    assert_eq!(pump.bottom_pane.composer_text(), "ab\nc");
    pump.bottom_pane
        .set_composer_text("draft".into(), Vec::new(), Vec::new());
    for key in [
        KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
        KeyEvent::from(KeyCode::Char('a')),
    ] {
        pump.handle_event(&mut tui, TuiEvent::Key(key))
            .expect("configured editor chord");
    }
    assert_eq!(pump.bottom_pane.composer_cursor(), 0);
    pump.handle_event(&mut tui, TuiEvent::Key(KeyEvent::from(KeyCode::Enter)))
        .expect("Enter does not submit");
    assert!(!pump.submission_pending);
    let draft = pump.bottom_pane.composer_draft_snapshot();
    for key in [
        KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
        KeyEvent::from(KeyCode::Char('s')),
    ] {
        pump.handle_event(&mut tui, TuiEvent::Key(key))
            .expect("configured submit");
    }
    assert!(pump.take_submission_intent());
    assert_eq!(pump.into_draft(), draft);
}

#[tokio::test]
async fn startup_submission_accepts_enter_after_paste_ambiguity_expires() {
    let mut pump = tests::quiet_startup_test_pump();
    let mut tui = crate::tui::test_support::make_test_tui().expect("create test terminal");
    pump.bottom_pane.insert_str("pasted draft");
    pump.pending_paste_newline = Some((
        Instant::now() - STARTUP_PASTE_NEWLINE_TIMEOUT - Duration::from_millis(/*millis*/ 1),
        "\n".into(),
    ));
    pump.handle_event(&mut tui, TuiEvent::Key(KeyEvent::from(KeyCode::Enter)))
        .expect("confirm after the ambiguous paste has finished");
    assert!(pump.take_submission_intent());
    assert_eq!(pump.into_draft().text, "pasted draft");
}

#[tokio::test]
async fn startup_submission_redirect_to_agents_preserves_only_editable_text() {
    for selection in [SessionSelection::AgentsOverview, SessionSelection::Exit] {
        let mut pump = tests::quiet_startup_test_pump();
        let mut tui = crate::tui::test_support::make_test_tui().expect("create test terminal");
        pump.update_session_selection(&mut tui, &SessionSelection::StartFresh)
            .expect("resolve initial destination");
        pump.bottom_pane
            .insert_str("keep this draft in its original context");
        let draft = pump.bottom_pane.composer_draft_snapshot();
        pump.handle_event(&mut tui, TuiEvent::Key(KeyEvent::from(KeyCode::Enter)))
            .expect("confirm startup draft");
        pump.update_session_selection(&mut tui, &selection)
            .expect("redirect away from startup destination");
        assert!(!pump.take_submission_intent());
        assert_eq!(pump.into_draft(), draft);
    }
}

#[tokio::test]
async fn startup_submission_thread_change_preserves_only_editable_text() {
    let mut pump = tests::quiet_startup_test_pump();
    let mut tui = crate::tui::test_support::make_test_tui().expect("create test terminal");
    for destination in [
        codex_protocol::ThreadId::new(),
        codex_protocol::ThreadId::new(),
    ] {
        let selection = SessionSelection::Resume(crate::resume_picker::SessionTarget {
            path: None,
            thread_id: destination,
            cwd: None,
            history_mode: None,
        });
        pump.update_session_selection(&mut tui, &selection)
            .expect("select startup destination");
        assert!(!pump.submission_pending);
        if pump.bottom_pane.composer_is_empty() {
            pump.bottom_pane.insert_str("retained draft");
            pump.handle_event(&mut tui, TuiEvent::Key(KeyEvent::from(KeyCode::Enter)))
                .expect("confirm first destination");
            assert!(pump.submission_pending);
        }
    }
    assert!(!pump.take_submission_intent());
    assert_eq!(pump.into_draft().text, "retained draft");
}
