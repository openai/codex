use super::*;
use codex_app_server_protocol::AlarmDelivery;
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
                cron_expression: "@after-turn".to_string(),
                prompt: "Give me a random animal name.".to_string(),
                run_once: false,
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
    assert_snapshot!(rendered, @"• Give me a random animal name. Running thread alarm • @after-turn • after-turn
");
}

#[tokio::test]
async fn thread_alarms_popup_keeps_selected_alarm_prompt_visible() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.open_thread_alarms_popup(
        ThreadId::new(),
        vec![ThreadAlarm {
            id: "alarm-1".to_string(),
            cron_expression: "@after-turn".to_string(),
            prompt: "Give me a random animal name.".to_string(),
            run_once: false,
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
