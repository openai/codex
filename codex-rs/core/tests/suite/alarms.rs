use anyhow::Result;
use anyhow::anyhow;
use codex_core::alarms::AFTER_TURN_CRON_EXPRESSION;
use codex_core::alarms::ALARM_FIRED_BACKGROUND_EVENT_PREFIX;
use codex_core::alarms::AlarmDelivery;
use codex_core::alarms::ThreadAlarm;
use codex_features::Feature;
use codex_protocol::protocol::EventMsg;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn create_alarm_emits_fired_background_event_when_alarm_starts() -> Result<()> {
    assert_after_turn_alarm_starts_and_emits_fired_event().await
}

#[tokio::test(flavor = "current_thread")]
async fn create_alarm_starts_on_current_thread_runtime() -> Result<()> {
    assert_after_turn_alarm_starts_and_emits_fired_event().await
}

async fn assert_after_turn_alarm_starts_and_emits_fired_event() -> Result<()> {
    let server = start_mock_server().await;
    let _mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "alarm ran"),
            ev_completed("resp-1"),
        ]),
    )
    .await;

    let mut builder = test_codex().with_config(|config| {
        config
            .features
            .enable(Feature::AlarmScheduler)
            .unwrap_or_else(|err| panic!("test config should allow feature update: {err}"));
    });
    let test = builder.build(&server).await?;

    let created = test
        .codex
        .create_alarm(
            AFTER_TURN_CRON_EXPRESSION.to_string(),
            "run alarm".to_string(),
            /*run_once*/ false,
            AlarmDelivery::AfterTurn,
        )
        .await
        .map_err(|err| anyhow!("{err}"))?;

    let payload = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::BackgroundEvent(event) => event
            .message
            .strip_prefix(ALARM_FIRED_BACKGROUND_EVENT_PREFIX)
            .map(str::to_owned),
        _ => None,
    })
    .await;
    let fired: ThreadAlarm = serde_json::from_str(&payload)?;
    let expected = ThreadAlarm {
        last_run_at: Some(
            fired
                .last_run_at
                .ok_or_else(|| anyhow!("claimed alarm should record last_run_at"))?,
        ),
        ..created
    };
    assert_eq!(fired, expected);

    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    Ok(())
}
