//! Verify unsent startup text survives edits, buffered pastes, and handoff.

use super::*;
use codex_protocol::user_input::ByteRange;
use codex_protocol::user_input::TextElement;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use pretty_assertions::assert_eq;

fn snapshot(text: &str) -> ComposerDraftSnapshot {
    ComposerDraftSnapshot {
        text: text.to_string(),
        cursor: text.len(),
        text_elements: Vec::new(),
        local_images: Vec::new(),
        remote_image_urls: Vec::new(),
        mention_bindings: Vec::new(),
        pending_pastes: Vec::new(),
        startup_local_history: Vec::new(),
        last_composer_activity_at: None,
        sparkle_draft: Default::default(),
    }
}

#[tokio::test]
async fn recovery_keeps_edits_and_pastes_after_chatwidget_handoff() {
    for destination in ["", "existing draft\n"] {
        scope(async {
            let (mut chat, _sender, _rx, _op_rx) =
                crate::chatwidget::tests::make_chatwidget_manual_with_sender().await;
            let draft = snapshot("startup draft");
            remember(draft.clone());
            chat.handle_paste(destination.trim_end().to_string());
            let mut pending = Some(draft);
            let mut confirmed = true;
            chat.restore_startup_input_when_ready(&mut pending, &mut confirmed);
            assert!(pending.is_none());
            chat.handle_key_event(KeyEvent::from(KeyCode::Backspace));
            let pasted = format!(" edited {}", "x".repeat(/*n*/ 1100));
            chat.handle_paste(pasted.clone());
            // This is the exact expanded text printed if startup fails before session readiness.
            assert_eq!(
                take_unsent_text(),
                Some(format!("{destination}startup draf{pasted}")),
            );
        })
        .await;
    }
}

#[tokio::test]
async fn recovery_refresh_stops_after_submission_or_startup_completion() {
    scope(async {
        remember(snapshot("before handoff"));
        refresh_after_handoff(|| panic!("protected input must not replace the startup draft"));
        handed_off(|| snapshot("confirmed submission"));
        bind_submission("confirmed submission", "initial-prompt");
        acknowledged("initial-prompt");
        submitted(&InputResult::Submitted {
            text: "confirmed submission".to_string(),
            text_elements: Vec::new(),
        });
        bind_submission("other message", "unrelated");
        bind_submission("confirmed submission", "recovered");
        bind_submission("confirmed submission", "later-identical-message");
        acknowledged("initial-prompt");
        acknowledged("later-identical-message");
        refresh_after_handoff(|| panic!("new edits must not replace a submitted draft"));
        assert_eq!(take_unsent_text(), Some("confirmed submission".to_string()));

        remember(snapshot("editable draft"));
        handed_off(|| snapshot("merged draft"));
        submitted(&InputResult::Submitted {
            text: "merged draft".into(),
            text_elements: Vec::new(),
        });
        bind_submission("merged draft", "accepted");
        acknowledged("accepted");
        assert_eq!(take_unsent_text(), None);
        for submission in [
            InputResult::CommandWithArgs(
                crate::slash_command::SlashCommand::Plan,
                "make a plan".into(),
                Vec::new(),
            ),
            InputResult::Queued {
                text: "/plan make a plan".into(),
                text_elements: Vec::new(),
                action: crate::bottom_pane::QueuedInputAction::ParseSlash,
                pending_pastes: Vec::new(),
            },
        ] {
            remember(snapshot("/plan make a plan"));
            handed_off(|| snapshot("/plan make a plan"));
            submitted(&submission);
            bind_submission("make a plan", "plan-request");
            acknowledged("plan-request");
            assert_eq!(take_unsent_text(), None);
        }
        handed_off(|| panic!("completed startup must not retain input"));
        refresh_after_handoff(|| panic!("completed startup must not copy edits"));
        assert_eq!(take_unsent_text(), None);
    })
    .await;
}

#[tokio::test]
async fn recovery_keeps_buffered_input_on_exit_or_queued_submission() {
    for queue in [false, true] {
        scope(async {
            let (mut chat, _sender, _rx, _op_rx) =
                crate::chatwidget::tests::make_chatwidget_manual_with_sender().await;
            let draft = snapshot("startup draft");
            remember(draft.clone());
            let mut pending = Some(draft);
            let mut confirmed = false;
            chat.restore_startup_input_when_ready(&mut pending, &mut confirmed);
            chat.set_queue_submissions_until_session_configured(queue);
            // Rapid raw keys remain buffered outside the textarea until the burst ends.
            chat.handle_key_event(KeyEvent::from(KeyCode::Char('x')));
            chat.handle_key_event(KeyEvent::from(KeyCode::Char('y')));
            assert_eq!(chat.composer_text_with_pending(), "startup draft");
            if !queue {
                // A fatal exit must include keys still waiting for the paste timer.
                assert_eq!(take_unsent_text(), Some("startup draftxy".to_string()));
                return;
            }
            // Queueing must capture the buffer without waiting for a draw/tick flush.
            chat.handle_key_event(KeyEvent::from(KeyCode::Enter));
            assert_eq!(
                chat.queued_user_message_texts(),
                vec!["startup draftxy".to_string()]
            );
            assert_eq!(take_unsent_text(), Some("startup draftxy".to_string()));
        })
        .await;
    }
}

#[tokio::test]
async fn recovery_uses_pending_pastes_from_the_actual_queued_result() {
    scope(async {
        remember(snapshot("before burst"));
        handed_off(|| snapshot("before burst"));
        let placeholder = "[Pasted text]";
        submitted(&InputResult::Queued {
            text: placeholder.to_string(),
            text_elements: vec![TextElement::new(
                ByteRange {
                    start: 0,
                    end: placeholder.len(),
                },
                Some(placeholder.to_string()),
            )],
            action: crate::bottom_pane::QueuedInputAction::Literal,
            pending_pastes: vec![(
                placeholder.to_string(),
                "complete queued paste\n\twith control\u{1b}[31m".to_string(),
            )],
        });
        assert_eq!(
            take_unsent_text(),
            Some("complete queued paste\n\twith control\\u{1b}[31m".to_string())
        );
        assert_eq!(take_unsent_text(), None);
    })
    .await;
}
