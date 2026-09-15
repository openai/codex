//! Loading transitions and cancellation when a view replaces pending work.

use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn completed_loads_publish_data_unavailability_and_errors() {
    for (result, expected) in [
        (Ok(Some(7)), (Some(7), None)),
        (Ok(None), (None, Some("No history has been reported."))),
        (
            Err("temporary failure".to_string()),
            (None, Some("temporary failure")),
        ),
    ] {
        let mut load = Load::start(async move { result }, FrameRequester::test_dummy());
        let Load::Loading(pending) = &mut load else {
            unreachable!()
        };
        (&mut pending.task).await.unwrap();
        load.poll();
        assert_eq!((load.ready().copied(), load.message()), expected);
    }
}

#[tokio::test]
async fn replacing_a_pending_load_cancels_its_request() {
    let (cancelled_tx, cancelled_rx) = oneshot::channel::<()>();
    let load = Load::<()>::start(
        async move {
            let _cancelled = cancelled_tx;
            std::future::pending().await
        },
        FrameRequester::test_dummy(),
    );
    tokio::task::yield_now().await;
    drop(load);
    assert!(cancelled_rx.await.is_err());
}

#[tokio::test]
async fn panicking_loads_request_a_frame_and_report_interruption() {
    let (draw, mut frames) = tokio::sync::broadcast::channel(/*capacity*/ 1);
    let frame = FrameRequester::new(draw);
    let mut load = Load::<()>::start(async { panic!("report task failed") }, frame.clone());
    let Load::Loading(pending) = &mut load else {
        unreachable!()
    };
    let _ = (&mut pending.task).await;
    tokio::time::timeout(std::time::Duration::from_secs(/*secs*/ 1), frames.recv())
        .await
        .unwrap()
        .unwrap();
    load.poll();
    assert_eq!(
        load.message(),
        Some("Request interrupted. Press R to retry.")
    );
}
