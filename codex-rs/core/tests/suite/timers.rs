use anyhow::Result;
use anyhow::anyhow;
use codex_core::timers::TIMER_FIRED_BACKGROUND_EVENT_PREFIX;
use codex_core::timers::ThreadTimer;
use codex_core::timers::ThreadTimerTrigger;
use codex_core::timers::TimerDelivery;
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
async fn create_timer_emits_fired_background_event_when_timer_starts() -> Result<()> {
    assert_after_turn_timer_starts_and_emits_fired_event().await
}

#[tokio::test(flavor = "current_thread")]
async fn create_timer_starts_on_current_thread_runtime() -> Result<()> {
    assert_after_turn_timer_starts_and_emits_fired_event().await
}

async fn assert_after_turn_timer_starts_and_emits_fired_event() -> Result<()> {
    let server = start_mock_server().await;
    let mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "timer ran"),
            ev_completed("resp-1"),
        ]),
    )
    .await;

    let mut builder = test_codex().with_config(|config| {
        config
            .features
            .enable(Feature::TimerScheduler)
            .unwrap_or_else(|err| panic!("test config should allow feature update: {err}"));
    });
    let test = builder.build(&server).await?;

    let created = test
        .codex
        .create_timer(
            ThreadTimerTrigger::Delay {
                seconds: 0,
                repeat: None,
            },
            "run timer".to_string(),
            TimerDelivery::AfterTurn,
        )
        .await
        .map_err(|err| anyhow!("{err}"))?;

    let payload = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::BackgroundEvent(event) => event
            .message
            .strip_prefix(TIMER_FIRED_BACKGROUND_EVENT_PREFIX)
            .map(str::to_owned),
        _ => None,
    })
    .await;
    let fired: ThreadTimer = serde_json::from_str(&payload)?;
    assert_eq!(fired, created);

    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let user_messages = mock.single_request().message_input_texts("user");
    let expected_timer_input = format!(
        "<timer_fired>
<id>{}</id>
<trigger>delay 0s</trigger>
<delivery>after-turn</delivery>
<recurring>false</recurring>
<prompt>
run timer
</prompt>
<instructions>
This one-shot timer has already been removed from the schedule, so you do not need to call TimerDelete.
Do not expose scheduler internals unless they matter to the user.
</instructions>
</timer_fired>",
        created.id
    );
    assert_eq!(
        user_messages
            .iter()
            .filter(|message| message.contains("<timer_fired>"))
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec![expected_timer_input.as_str()]
    );

    Ok(())
}
