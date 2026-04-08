use super::*;
use codex_app_server_protocol::ThreadTimer;
use codex_app_server_protocol::TimerDelivery;
use codex_app_server_protocol::TimerTrigger;
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

#[tokio::test]
async fn thread_timers_popup_keeps_selected_timer_prompt_visible() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.open_thread_timers_popup(
        ThreadId::new(),
        vec![ThreadTimer {
            id: "timer-1".to_string(),
            trigger: TimerTrigger::Delay {
                seconds: 0,
                repeat: None,
            },
            prompt: "Give me a random animal name.".to_string(),
            delivery: TimerDelivery::AfterTurn,
            created_at: 0,
            next_run_at: None,
            last_run_at: None,
        }],
    );

    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert_snapshot!(
        "thread_timers_popup_keeps_selected_timer_prompt_visible",
        popup
    );
}

#[tokio::test]
async fn thread_timers_popup_renders_schedule_triggers_readably() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.open_thread_timers_popup(
        ThreadId::new(),
        vec![
            ThreadTimer {
                id: "timer-1".to_string(),
                trigger: TimerTrigger::Schedule {
                    dtstart: Some("2026-04-07T10:57:00".to_string()),
                    rrule: None,
                },
                prompt: "tell me to take a piss".to_string(),
                delivery: TimerDelivery::AfterTurn,
                created_at: 0,
                next_run_at: None,
                last_run_at: None,
            },
            ThreadTimer {
                id: "timer-2".to_string(),
                trigger: TimerTrigger::Schedule {
                    dtstart: Some("2026-04-07T17:00:00".to_string()),
                    rrule: Some(
                        "FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR;BYHOUR=17;BYMINUTE=0;BYSECOND=0"
                            .to_string(),
                    ),
                },
                prompt: "wrap up for the day".to_string(),
                delivery: TimerDelivery::AfterTurn,
                created_at: 0,
                next_run_at: None,
                last_run_at: None,
            },
        ],
    );

    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert_snapshot!(
        "thread_timers_popup_renders_schedule_triggers_readably",
        popup
    );
}
