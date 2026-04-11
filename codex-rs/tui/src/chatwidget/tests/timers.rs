use super::*;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::InjectedMessageEvent;
use codex_protocol::protocol::UserMessageEvent;
use insta::assert_snapshot;

#[tokio::test]
async fn injected_message_renders_content_history() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_codex_event_replay(Event {
        id: "event-1".to_string(),
        msg: EventMsg::InjectedMessage(InjectedMessageEvent {
            content: "Give me a random animal name.".to_string(),
            source: "timer timer-1".to_string(),
        }),
    });

    let cells = drain_insert_history(&mut rx);
    let rendered = lines_to_single_string(&cells[0]);
    assert_snapshot!(rendered, @"› Give me a random animal name.
");
}

#[tokio::test]
async fn user_message_xml_renders_literally() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_codex_event_replay(Event {
        id: "event-1".to_string(),
        msg: EventMsg::UserMessage(UserMessageEvent {
            message: "<codex_message>not hidden</codex_message>".to_string(),
            images: None,
            local_images: Vec::new(),
            text_elements: Vec::new(),
        }),
    });

    let cells = drain_insert_history(&mut rx);
    let rendered = lines_to_single_string(&cells[0]);
    assert_snapshot!(rendered, @"› <codex_message>not hidden</codex_message>
");
}
