use std::time::Duration;

use super::ShutdownBehavior;
use super::run_until_shutdown_with_signals;

#[tokio::test]
async fn parent_disconnect_still_stops_executor_after_signal_listener_error() {
    let (parent_sender, parent_receiver) = tokio::sync::oneshot::channel();
    let (shutdown_sender, shutdown_receiver) = tokio::sync::oneshot::channel();
    let run = async move {
        let _ = shutdown_receiver.await;
        Ok::<(), std::io::Error>(())
    };
    let task = tokio::spawn(run_until_shutdown_with_signals(
        run,
        async move {
            let _ = parent_receiver.await;
        },
        std::future::ready(Err(std::io::Error::other("signal listener failed"))),
        ShutdownBehavior::Graceful(shutdown_sender),
    ));

    tokio::task::yield_now().await;
    parent_sender.send(()).expect("parent lifetime receiver");

    tokio::time::timeout(Duration::from_secs(1), task)
        .await
        .expect("parent disconnect should stop the executor")
        .expect("shutdown task should not panic")
        .expect("executor should shut down successfully");
}
