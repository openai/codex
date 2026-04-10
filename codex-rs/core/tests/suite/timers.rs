use anyhow::Result;
use anyhow::anyhow;
use chrono::Utc;
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
        config
            .features
            .enable(Feature::Sqlite)
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn create_timer_persists_source_and_client_metadata() -> Result<()> {
    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config
            .features
            .enable(Feature::TimerScheduler)
            .unwrap_or_else(|err| panic!("test config should allow feature update: {err}"));
        config
            .features
            .enable(Feature::Sqlite)
            .unwrap_or_else(|err| panic!("test config should allow feature update: {err}"));
    });
    let test = builder.build(&server).await?;

    let created = test
        .codex
        .create_timer(
            ThreadTimerTrigger::Delay {
                seconds: 60,
                repeat: Some(true),
            },
            "run timer".to_string(),
            TimerDelivery::AfterTurn,
        )
        .await
        .map_err(|err| anyhow!("{err}"))?;

    let db = test.codex.state_db().expect("state db enabled");
    let timers = db
        .list_thread_timers(&test.session_configured.session_id.to_string())
        .await?;

    assert_eq!(timers.len(), 1);
    assert_eq!(timers[0].id, created.id);
    assert_eq!(timers[0].source, "agent");
    assert_eq!(timers[0].client_id, "codex-cli");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn create_timer_lazily_opens_sqlite_for_ephemeral_thread() -> Result<()> {
    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.ephemeral = true;
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
                seconds: 60,
                repeat: Some(true),
            },
            "run timer".to_string(),
            TimerDelivery::AfterTurn,
        )
        .await
        .map_err(|err| anyhow!("{err}"))?;

    assert_eq!(
        test.codex.list_timers().await,
        vec![created],
        "ephemeral threads should still open sqlite timer storage lazily"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn list_timers_discovers_externally_inserted_timer() -> Result<()> {
    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config
            .features
            .enable(Feature::TimerScheduler)
            .unwrap_or_else(|err| panic!("test config should allow feature update: {err}"));
        config
            .features
            .enable(Feature::Sqlite)
            .unwrap_or_else(|err| panic!("test config should allow feature update: {err}"));
    });
    let test = builder.build(&server).await?;
    let db = test.codex.state_db().expect("state db enabled");
    let created_at = Utc::now().timestamp();

    db.create_thread_timer(&codex_state::ThreadTimerCreateParams {
        id: "external-timer".to_string(),
        thread_id: test.session_configured.session_id.to_string(),
        source: "client".to_string(),
        client_id: "external-client".to_string(),
        trigger_json: r#"{"kind":"delay","seconds":60,"repeat":true}"#.to_string(),
        prompt: "external timer".to_string(),
        delivery: "after-turn".to_string(),
        created_at,
        next_run_at: Some(created_at + 60),
        last_run_at: None,
        pending_run: false,
    })
    .await?;

    let timers = test.codex.list_timers().await;

    assert_eq!(timers.len(), 1);
    assert_eq!(timers[0].id, "external-timer");
    assert_eq!(
        timers[0].trigger,
        ThreadTimerTrigger::Delay {
            seconds: 60,
            repeat: Some(true),
        }
    );
    assert_eq!(timers[0].prompt, "external timer");
    assert_eq!(timers[0].delivery, TimerDelivery::AfterTurn);
    assert_eq!(timers[0].created_at, created_at);
    assert_eq!(timers[0].last_run_at, None);

    Ok(())
}
