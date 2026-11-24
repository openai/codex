use std::sync::Arc;
use std::time::Duration;

use codex_history::RetainedContextEntry;
use codex_history::RetainedUserMessage;
use pretty_assertions::assert_eq;
use test_case::test_case;

use super::thread_settings;
use crate::guardian_review::PendingReviewWrites;
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
async fn guardian_waits_for_detached_confirmed_delivery() {
    let (mut session, _) = make_session_and_context().await;
    attach_thread_persistence(&mut session).await;
    let writes = Arc::new(PendingReviewWrites::default());
    session.pending_review_writes = Some(Arc::clone(&writes));
    let session = Arc::new(session);
    let checkpoint = thread_settings::acquire_persistence_lock(&session).await;
    let (recording, _) = session.record_delivered_assistant_message(delivered_message());
    let waiting = writes.wait();
    tokio::pin!(waiting);
    assert!(futures::poll!(waiting.as_mut()).is_pending());
    drop(checkpoint);
    waiting.await.expect("confirmed delivery appended");
    recording.await.expect("delivery task finished");
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
