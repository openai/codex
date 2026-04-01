use super::*;
use codex_app_server_protocol::ThreadJob;
use codex_app_server_protocol::ThreadJobFiredNotification;
use insta::assert_snapshot;

#[tokio::test]
async fn thread_job_fired_renders_prompt_history() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.handle_server_notification(
        ServerNotification::ThreadJobFired(ThreadJobFiredNotification {
            thread_id: ThreadId::new().to_string(),
            job: ThreadJob {
                id: "job-1".to_string(),
                cron_expression: "@after-turn".to_string(),
                prompt: "Give me a random animal name.".to_string(),
                run_once: false,
                created_at: 0,
                next_run_at: None,
                last_run_at: None,
            },
        }),
        /*replay_kind*/ None,
    );

    let cells = drain_insert_history(&mut rx);
    let rendered = lines_to_single_string(&cells[0]);
    assert_snapshot!(rendered, @"• Give me a random animal name. Running thread job • @after-turn
");
}

#[tokio::test]
async fn thread_jobs_popup_keeps_selected_job_prompt_visible() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.open_thread_jobs_popup(
        ThreadId::new(),
        vec![ThreadJob {
            id: "job-1".to_string(),
            cron_expression: "@after-turn".to_string(),
            prompt: "Give me a random animal name.".to_string(),
            run_once: false,
            created_at: 0,
            next_run_at: None,
            last_run_at: None,
        }],
    );

    let popup = render_bottom_popup(&chat, /*width*/ 80);
    assert_snapshot!("thread_jobs_popup_keeps_selected_job_prompt_visible", popup);
}
