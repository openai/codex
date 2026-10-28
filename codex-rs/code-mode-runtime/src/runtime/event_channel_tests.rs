//! Raw emissions and control events share one bounded FIFO before collection.

use pretty_assertions::assert_eq;
use tokio::sync::oneshot;

use super::*;
use crate::session_runtime::OutputItem;
use codex_code_mode_protocol::host::BOUNDED_OUTPUT_MAX_BYTES as MAX_OUTPUT_BYTES;

#[tokio::test]
async fn a_blocked_send_uses_the_released_slot_before_the_receiver_takes_the_item() {
    let (tx, mut rx) = runtime_event_channel(CodeModeOutputTruncation::Server);
    let RuntimeEventSender::Bounded(raw) = tx.clone() else {
        panic!("expected bounded sender");
    };
    let mut permits = raw.reserve_many(EVENT_CAPACITY).await.unwrap();
    let (done_tx, mut done_rx) = oneshot::channel();
    let text = "😀".repeat(MAX_OUTPUT_BYTES);
    let producer_text = text.clone();
    let producer = std::thread::spawn(move || {
        let result = tx.blocking_send(RuntimeEvent::ContentItem(OutputItem::InputText {
            text: producer_text,
        }));
        let _ = done_tx.send(result);
    });
    // Queue a competing waiter before releasing the slot. A blocking producer can
    // win the slot in FIFO order; a try_send can never find an unreserved slot.
    let sent = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        let mut slot = permits.next().unwrap();
        loop {
            let mut next = std::pin::pin!(raw.reserve());
            assert!(futures::poll!(next.as_mut()).is_pending());
            drop(slot);
            tokio::select! {
                result = &mut done_rx => break result.unwrap(),
                permit = &mut next => slot = permit.unwrap(),
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the producer must eventually acquire the released slot");
    assert_eq!(sent, Ok(()));
    // Close the test's sender so a missing event fails the receive immediately.
    drop(permits);
    drop(raw);
    let event = rx.recv().await.unwrap();
    let RuntimeEvent::ContentItem(item) = event else {
        panic!("expected raw content");
    };
    assert_eq!(item, OutputItem::InputText { text });
    producer.join().unwrap();
}

#[tokio::test]
async fn content_and_control_are_received_in_send_order() {
    let (tx, mut rx) = runtime_event_channel(CodeModeOutputTruncation::Server);
    let producer = std::thread::spawn(move || {
        tx.blocking_send(RuntimeEvent::ContentItem(OutputItem::InputImage {
            image_url: "image".to_owned(),
            detail: None,
        }))
        .unwrap();
        tx.blocking_send(RuntimeEvent::YieldRequested).unwrap();
        tx.blocking_send(RuntimeEvent::ContentItem(OutputItem::InputText {
            text: "after yield".to_owned(),
        }))
        .unwrap();
    });
    assert!(matches!(
        rx.recv().await,
        Some(RuntimeEvent::ContentItem(OutputItem::InputImage { .. }))
    ));
    assert!(matches!(
        rx.recv().await,
        Some(RuntimeEvent::YieldRequested)
    ));
    assert!(matches!(
        rx.recv().await,
        Some(RuntimeEvent::ContentItem(OutputItem::InputText { text }))
        if text == "after yield"
    ));
    producer.join().unwrap();
}

#[tokio::test]
async fn full_channel_blocks_the_producer_and_receiver_drop_releases_it() {
    let (tx, rx) = runtime_event_channel(CodeModeOutputTruncation::Server);
    let RuntimeEventSender::Bounded(tx) = tx else {
        panic!("expected bounded sender");
    };
    for _ in 0..EVENT_CAPACITY {
        tx.try_send(RuntimeEvent::Started).unwrap();
    }

    let (ready_tx, ready_rx) = oneshot::channel();
    let producer = tokio::spawn(async move {
        // Tokio's blocking_send runs this same send future. Polling it first
        // proves the full channel has registered this producer as a waiter.
        let mut send = std::pin::pin!(tx.send(RuntimeEvent::YieldRequested));
        assert!(futures::poll!(send.as_mut()).is_pending());
        ready_tx.send(()).unwrap();
        send.await
    });
    // This current-thread runtime cannot resume us until the producer yields.
    // Closing must wake that already-waiting task, not just reject a new send.
    ready_rx.await.unwrap();
    drop(rx);
    let result = tokio::time::timeout(std::time::Duration::from_secs(1), producer)
        .await
        .expect("receiver drop must wake the blocked producer")
        .expect("producer task");
    assert!(matches!(
        result,
        Err(mpsc::error::SendError(RuntimeEvent::YieldRequested))
    ));
}

#[tokio::test]
async fn sending_after_receiver_drop_rejects_content() {
    let (tx, rx) = runtime_event_channel(CodeModeOutputTruncation::Server);
    drop(rx);
    assert_eq!(
        std::thread::spawn(move || tx.blocking_send(RuntimeEvent::ContentItem(
            OutputItem::InputAudio {
                audio_url: "large audio".repeat(MAX_OUTPUT_BYTES)
            }
        )))
        .join()
        .unwrap(),
        Err(())
    );
}
