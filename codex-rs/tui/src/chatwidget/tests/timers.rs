use super::*;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::UserMessageEvent;
use insta::assert_snapshot;

#[tokio::test]
async fn thread_timer_fired_renders_prompt_history() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_codex_event_replay(Event {
        id: "event-1".to_string(),
        msg: EventMsg::UserMessage(UserMessageEvent {
            message: "<timer_fired>
<id>timer-1</id>
<trigger>delay 0s</trigger>
<delivery>after-turn</delivery>
<recurring>false</recurring>
<prompt>
Give me a random animal name.
</prompt>
<instructions>
This one-shot timer has already been removed from the schedule, so you do not need to call TimerDelete.
Do not expose scheduler internals unless they matter to the user.
</instructions>
</timer_fired>
"
            .to_string(),
            images: None,
            local_images: Vec::new(),
            text_elements: Vec::new(),
        }),
    });

    let cells = drain_insert_history(&mut rx);
    let rendered = lines_to_single_string(&cells[0]);
    assert_snapshot!(rendered, @"• Give me a random animal name. Running thread timer • delay 0s • one-shot • after-turn
");
}
