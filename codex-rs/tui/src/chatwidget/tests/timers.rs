use super::*;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::UserMessageEvent;
use insta::assert_snapshot;

#[tokio::test]
async fn codex_message_renders_content_history() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_codex_event_replay(Event {
        id: "event-1".to_string(),
        msg: EventMsg::UserMessage(UserMessageEvent {
            message: "<codex_message>
<source>timer timer-1</source>
<queued_at>100</queued_at>
<content>
Give me a random animal name.
</content>
<instructions>
This one-shot timer has already been removed from the schedule, so you do not need to call delete_timer.
Do not expose scheduler internals unless they matter to the user.
</instructions>
<meta />
</codex_message>
"
            .to_string(),
            images: None,
            local_images: Vec::new(),
            text_elements: Vec::new(),
        }),
    });

    let cells = drain_insert_history(&mut rx);
    let rendered = lines_to_single_string(&cells[0]);
    assert_snapshot!(rendered, @"› Give me a random animal name.
");
}
