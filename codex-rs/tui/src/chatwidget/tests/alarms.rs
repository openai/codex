use super::*;
use codex_app_server_protocol::AlarmDelivery;
use codex_app_server_protocol::AlarmTrigger;
use codex_app_server_protocol::ThreadAlarm;
use codex_app_server_protocol::ThreadAlarmFiredNotification;
use insta::assert_snapshot;

#[tokio::test]
async fn thread_alarm_fired_renders_prompt_history() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_server_notification(
        ServerNotification::ThreadAlarmFired(ThreadAlarmFiredNotification {
            thread_id: ThreadId::new().to_string(),
            alarm: ThreadAlarm {
                id: "alarm-1".to_string(),
                trigger: AlarmTrigger::Delay {
                    seconds: 0,
                    repeat: None,
                },
                prompt: "Give me a random animal name.".to_string(),
                delivery: AlarmDelivery::AfterTurn,
                created_at: 0,
                next_run_at: None,
                last_run_at: None,
            },
        }),
        /*replay_kind*/ None,
    );

    let cells = drain_insert_history(&mut rx);
    let rendered = lines_to_single_string(&cells[0]);
    assert_snapshot!(rendered, @"• Give me a random animal name. Running thread alarm • delay 0s • one-shot • after-turn
");
}

#[tokio::test]
async fn thread_alarms_popup_keeps_selected_alarm_prompt_visible() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.open_thread_alarms_popup(
        ThreadId::new(),
        vec![ThreadAlarm {
            id: "alarm-1".to_string(),
            trigger: AlarmTrigger::Delay {
                seconds: 0,
                repeat: None,
            },
            prompt: "Give me a random animal name.".to_string(),
            delivery: AlarmDelivery::AfterTurn,
            created_at: 0,
            next_run_at: None,
            last_run_at: None,
        }],
    );

    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert_snapshot!(
        "thread_alarms_popup_keeps_selected_alarm_prompt_visible",
        popup
    );
}

#[tokio::test]
async fn thread_alarms_popup_renders_schedule_triggers_readably() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.open_thread_alarms_popup(
        ThreadId::new(),
        vec![
            ThreadAlarm {
                id: "alarm-1".to_string(),
                trigger: AlarmTrigger::Schedule {
                    dtstart: Some("2026-04-07T10:57:00".to_string()),
                    rrule: None,
                },
                prompt: "tell me to take a piss".to_string(),
                delivery: AlarmDelivery::AfterTurn,
                created_at: 0,
                next_run_at: None,
                last_run_at: None,
            },
            ThreadAlarm {
                id: "alarm-2".to_string(),
                trigger: AlarmTrigger::Schedule {
                    dtstart: Some("2026-04-07T17:00:00".to_string()),
                    rrule: Some(
                        "FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR;BYHOUR=17;BYMINUTE=0;BYSECOND=0"
                            .to_string(),
                    ),
                },
                prompt: "wrap up for the day".to_string(),
                delivery: AlarmDelivery::AfterTurn,
                created_at: 0,
                next_run_at: None,
                last_run_at: None,
            },
        ],
    );

    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert_snapshot!(
        "thread_alarms_popup_renders_schedule_triggers_readably",
        popup
    );
}
