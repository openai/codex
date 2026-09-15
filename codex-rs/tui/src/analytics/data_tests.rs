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

#[test]
fn rpc_errors_do_not_expose_server_diagnostics() {
    let error = codex_app_server_client::TypedRequestError::Server {
        method: "thread/list".into(),
        source: codex_app_server_protocol::JSONRPCErrorError {
            code: -32603,
            message: "Failed opening /private/workspace: token=secret".into(),
            data: Some(serde_json::json!({"credential": "secret"})),
        },
    };
    assert_eq!(
        super::error(error),
        "Couldn't load analytics. Press R to retry."
    );
}

#[tokio::test(start_paused = true)]
async fn extended_chat_load_budget_still_times_out_and_can_be_cancelled() {
    let (cancelled_tx, cancelled_rx) = oneshot::channel::<()>();
    let mut load = Load::<()>::start_with_timeout(
        async move {
            let _cancelled = cancelled_tx;
            std::future::pending().await
        },
        FrameRequester::test_dummy(),
        std::time::Duration::from_secs(/*secs*/ 90),
    );
    tokio::task::yield_now().await;
    tokio::time::advance(std::time::Duration::from_secs(/*secs*/ 31)).await;
    load.poll();
    assert!(matches!(load, Load::Loading(_)));
    tokio::time::advance(std::time::Duration::from_secs(/*secs*/ 60)).await;
    assert!(cancelled_rx.await.is_err());
    load.poll();
    assert_eq!(load.message(), Some("Request timed out. Press R to retry."));
}

#[test]
fn compact_counts_round_promote_units_and_preserve_small_adjustments() {
    let values = [
        0.0,
        959.0,
        1_000.0,
        9_749.0,
        2_901.0,
        12_280_365_226.0,
        142_300_000_000.0,
        999_999.0,
        999_999_999.0,
        -1_250.0,
        -0.000004,
        1_200_000_000_000.0,
    ];
    assert_eq!(
        values.map(compact_amount),
        [
            "0",
            "959",
            "1K",
            "9.7K",
            "2.9K",
            "12.3B",
            "142.3B",
            "1M",
            "1B",
            "-1.3K",
            "-0.000004",
            "1.2T"
        ]
        .map(str::to_string)
    );
}
