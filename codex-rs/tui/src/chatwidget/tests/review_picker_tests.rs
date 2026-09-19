//! Production review picker rendering, filtered activation, and parent/draft restoration.

use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn review_picker_filtered_accept_and_cancel_preserve_parent_and_draft() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let mut keymap = crate::keymap::RuntimeKeymap::defaults();
    keymap.list.accept = vec![key_hint::plain(KeyCode::F(/*n*/ 3))];
    keymap.list.cancel = vec![key_hint::plain(KeyCode::F(/*n*/ 2))];
    chat.bottom_pane.set_keymap_bindings(&keymap);
    chat.bottom_pane
        .set_composer_text("Keep this draft".into(), Vec::new(), Vec::new());
    chat.open_review_popup();
    for _ in 0..2 {
        chat.handle_key_event(KeyEvent::from(KeyCode::Down));
    }
    chat.handle_key_event(KeyEvent::from(KeyCode::F(/*n*/ 3)));
    assert_matches!(rx.try_recv(), Ok(AppEvent::OpenReviewCommitPicker(_)));
    let entries = vec![
        CommitLogEntry {
            sha: "1111111".into(),
            timestamp: 0,
            subject: "First commit".into(),
        },
        CommitLogEntry {
            sha: "abcdef2".into(),
            timestamp: 0,
            subject: "Selected commit".into(),
        },
    ];
    chat.show_review_commits(entries.clone());
    chat.handle_key_event(KeyEvent::from(KeyCode::Char('a')));
    chat.handle_key_event(KeyEvent::from(KeyCode::F(/*n*/ 2)));
    let parent = render_bottom_popup(&chat, /*width*/ 80);
    assert!(parent.contains("› 3. Review a commit"), "{parent}");
    assert!(parent.contains("f3 select · f2 back"), "{parent}");
    chat.handle_key_event(KeyEvent::from(KeyCode::F(/*n*/ 3)));
    assert_matches!(rx.try_recv(), Ok(AppEvent::OpenReviewCommitPicker(_)));
    chat.show_review_commits(entries);
    for ch in "abcdef2".chars() {
        chat.handle_key_event(KeyEvent::from(KeyCode::Char(ch)));
    }
    chat.handle_key_event(KeyEvent::from(KeyCode::F(/*n*/ 3)));
    let AppEvent::CodexOp(Op::Review { target }) = rx.try_recv().unwrap() else {
        panic!("expected the filtered review target");
    };
    assert_eq!(
        target,
        ReviewTarget::Commit {
            sha: "abcdef2".into(),
            title: Some("Selected commit".into())
        }
    );
    assert!(chat.no_modal_or_popup_active());
    assert_eq!(chat.bottom_pane.composer_text(), "Keep this draft");
}
