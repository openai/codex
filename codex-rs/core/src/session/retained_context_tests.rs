use std::sync::Arc;
use std::time::Duration;

use codex_history::RetainedContextEntry;
use codex_history::RetainedContextEvent;
use codex_history::RetainedUserMessage;
use codex_history::RolloutItem;
use codex_protocol::protocol::EventMsg;
use futures::FutureExt;
use pretty_assertions::assert_eq;
use serde_json::json;
use test_case::test_case;

use super::thread_settings;
use crate::rollout::recorder::RolloutRecorder;
use crate::session::tests::attach_thread_persistence;
use crate::session::tests::make_session_and_context;

fn delivered_message() -> RetainedUserMessage {
    RetainedUserMessage {
        origin: codex_history::UserInputOrigin::User,
        turn_id: "turn".to_owned(),
        message_id: Some("send".to_owned()),
        text: "May I publish?".to_owned(),
        complete: true,
        phase: None,
    }
}

#[tokio::test]
async fn guardian_retains_an_accepted_answer_when_the_tool_is_cancelled() -> anyhow::Result<()> {
    let (mut session, turn) = make_session_and_context().await;
    session.preserve_review_writes = true;
    let path = attach_thread_persistence(&mut session).await;
    *session.active_turn.lock().await = Some(Default::default());
    let (tx, events) = async_channel::unbounded();
    session.tx_event = tx;
    let args = serde_json::from_value(json!({
        "questions": [{"id": "publish", "header": "Release", "question": "May I publish?"}],
        "isBlocking": true,
    }))?;
    let mut request =
        Box::pin(session.prompt_user(&turn, "ask".into(), args, /*verify*/ true));
    tokio::select! {
        _ = &mut request => panic!("tool completed before the answer"),
        event = events.recv() => assert!(matches!(event.unwrap().msg, EventMsg::RequestUserInput(_))),
    }
    let response = serde_json::from_value(json!({"answers": {"publish": {"answers": ["No"]}}}))?;
    // Hold the writer. Accepting the answer must leave the session free to process Stop.
    let flush = tokio::task::unconstrained(session.live_thread().unwrap().flush());
    tokio::pin!(flush);
    assert!(futures::poll!(&mut flush).is_pending());
    tokio::task::unconstrained(session.notify_user_input_response(&turn.sub_id, response))
        .now_or_never()
        .expect("answer handling blocked on the writer");
    drop(request);

    // Queue the interrupt before the detached task gets a turn on this runtime.
    let interrupt = [RolloutItem::EventMsg(EventMsg::TurnAborted(
        serde_json::from_value(json!({"turn_id": turn.sub_id, "reason": "interrupted"}))?,
    ))];
    let append = tokio::task::unconstrained(session.persist_rollout_items(&interrupt));
    let (appended, flushed) = tokio::join!(biased; append, flush);
    flushed?;
    assert!(appended);
    session.drain_code_mode_messages().await;

    let history = RolloutRecorder::get_rollout_history(&path).await?;
    let events = history
        .get_rollout_items()
        .iter()
        .filter_map(|item| match item {
            RolloutItem::RetainedContext(RetainedContextEvent::VerifiedAnswer {
                answer, ..
            }) => Some(answer.questions[0].answer.as_str()),
            RolloutItem::EventMsg {
                event: EventMsg::TurnAborted(_),
                ..
            } => Some("interrupted"),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(events, ["No", "interrupted"]);
    Ok(())
}

#[tokio::test]
async fn shutdown_retains_an_admitted_delivery_until_persistence_finishes() {
    let (session, _) = make_session_and_context().await;
    let session = Arc::new(session);
    let checkpoint = thread_settings::acquire_persistence_lock(&session).await;
    let dispatch = session.track_code_mode_message().expect("open dispatch");
    let mut shutdown = Box::pin(session.drain_code_mode_messages());
    assert!(futures::poll!(shutdown.as_mut()).is_pending());
    assert!(session.track_code_mode_message().is_none());

    let message = delivered_message();
    let (admitted, receiver) = tokio::sync::oneshot::channel();
    let outer_session = Arc::clone(&session);
    let outer_message = message.clone();
    let outer = tokio::spawn(async move {
        let (_, completion) = outer_session.record_delivered_assistant_message(outer_message);
        admitted
            .send(completion)
            .expect("receive recording admission");
        futures::future::pending::<()>().await;
    });
    let mut completion = receiver.await.expect("recording admitted");
    outer.abort();
    assert!(
        outer
            .await
            .expect_err("outer call interrupted")
            .is_cancelled()
    );
    drop(dispatch);
    let live_messages = tokio::time::timeout(Duration::from_secs(/*secs*/ 5), async {
        loop {
            let history = session.clone_history().await;
            let messages = history
                .retained_context()
                .ordered_entries()
                .filter_map(|(_, entry)| match entry {
                    RetainedContextEntry::AssistantMessage(message) => Some(message.clone()),
                    RetainedContextEntry::UserMessage(_)
                    | RetainedContextEntry::VerifiedAnswer(_) => None,
                })
                .collect::<Vec<_>>();
            if !messages.is_empty() {
                break messages;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("live admission does not wait for the checkpoint");
    assert_eq!(live_messages, vec![message]);
    assert!(futures::poll!(shutdown.as_mut()).is_pending());
    assert!(!completion.has_changed().expect("recording remains open"));

    drop(checkpoint);
    shutdown.await;
    assert!(completion.changed().await.is_err());
}

#[test_case(false; "free state")]
#[test_case(true; "busy state")]
#[tokio::test]
async fn confirmed_delivery_reserves_order_and_persists_before_a_fast_user_reply(busy: bool) {
    let (session, _) = make_session_and_context().await;
    let session = Arc::new(session);
    let checkpoint = thread_settings::acquire_persistence_lock(&session).await;
    let state = if busy {
        Some(session.state.lock().await)
    } else {
        None
    };
    let (recording, _) = session.record_delivered_assistant_message(delivered_message());
    let reply_session = Arc::clone(&session);
    let mut reply = tokio::spawn(async move { reply_session.reserve_user_input_order().await });
    drop(state);
    assert!(
        tokio::time::timeout(Duration::from_millis(/*millis*/ 100), &mut reply)
            .await
            .is_err()
    );
    drop(checkpoint);
    let reply_order = reply
        .await
        .expect("reply accepted after delivery persistence");
    recording.await.expect("confirmed delivery recorded");
    let assistant_order = session
        .clone_history()
        .await
        .retained_context()
        .ordered_entries()
        .find_map(|(order, entry)| {
            matches!(entry, RetainedContextEntry::AssistantMessage(_)).then_some(order)
        })
        .expect("confirmed delivery order");
    assert!(assistant_order < codex_history::RetainedContextOrder::Local(reply_order));
}
