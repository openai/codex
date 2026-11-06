//! Completed replay messages preserve order, formatting, and composer state.

use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn completed_replay_preserves_messages_and_draft_across_reconstruction() {
    let mut outputs = Vec::new();
    for _ in 0..2 {
        let (mut chat, mut rx, _ops) = make_chatwidget_manual(/*model_override*/ None).await;
        chat.note_rendered_width(/*width*/ 80);
        chat.handle_paste("Unsent draft".to_string());
        replay_user_message_text(
            &mut chat,
            "user",
            "**Question**",
            ReplayKind::ThreadSnapshot,
        );
        replay_agent_message(
            &mut chat,
            "first",
            "First **answer**",
            ReplayKind::ThreadSnapshot,
        );
        replay_agent_message(
            &mut chat,
            "second",
            "Second `answer`",
            ReplayKind::ThreadSnapshot,
        );
        let cells = drain_insert_history(&mut rx);
        outputs.push(cells.into_iter().flatten().collect::<Vec<_>>());
        assert_eq!(chat.composer_text_with_pending(), "Unsent draft");
    }
    assert_eq!(outputs[0], outputs[1]);
    insta::assert_snapshot!(outputs[0].iter().map(ToString::to_string).collect::<Vec<_>>().join("\n").trim(), @"
    › Question


    • First answer

    • Second answer
    ");
}
