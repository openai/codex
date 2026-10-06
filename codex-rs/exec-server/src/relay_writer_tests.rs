//! Exercise heartbeat deadlines while a single physical write is stalled.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use pretty_assertions::assert_eq;
use tokio::sync::Notify;
use tokio::sync::mpsc;
use tokio_util::task::AbortOnDropHandle;

use super::*;

#[tokio::test(start_paused = true)]
async fn pong_during_write_clears_only_the_pong_deadline() {
    for release_write in [true, false] {
        let (outgoing_tx, outgoing_rx) = mpsc::channel(1);
        let (pong_tx, pong_rx) = mpsc::channel(1);
        let (observed_tx, mut observed_rx) = mpsc::unbounded_channel();
        let release = Arc::new(Notify::new());
        let completed = Arc::new(Notify::new());
        let sink = futures::sink::unfold(
            (observed_tx, Arc::clone(&release), Arc::clone(&completed)),
            |(observed_tx, release, completed), message| async move {
                let is_data = matches!(message, Message::Binary(_));
                observed_tx.send(message).unwrap();
                if is_data {
                    release.notified().await;
                    completed.notify_one();
                }
                Ok::<_, Infallible>((observed_tx, release, completed))
            },
        );
        let task = AbortOnDropHandle::new(tokio::spawn(run(Box::pin(sink), outgoing_rx, pong_rx)));
        assert!(matches!(observed_rx.recv().await, Some(Message::Ping(_))));
        tokio::time::advance(WEBSOCKET_PONG_TIMEOUT * 3 / 4).await;
        outgoing_tx.send(vec![42]).await.unwrap();
        assert_eq!(
            observed_rx.recv().await,
            Some(Message::Binary(vec![42].into()))
        );
        pong_tx.send(()).await.unwrap();

        // Let the old pong deadline expire while the write remains blocked.
        tokio::time::advance(WEBSOCKET_PONG_TIMEOUT / 2).await;
        tokio::task::yield_now().await;
        assert!(!task.is_finished());
        if release_write {
            release.notify_one();
            completed.notified().await;
            drop(outgoing_tx);
            drop(pong_tx);
            assert_eq!(
                task.await.unwrap(),
                RendezvousDisconnectReason::LocalShutdown
            );
        } else {
            // Receiving a pong must not extend the independent write budget.
            tokio::time::advance(WEBSOCKET_PONG_TIMEOUT).await;
            assert_eq!(task.await.unwrap(), RendezvousDisconnectReason::WriteError);
        }
    }
}

#[tokio::test(start_paused = true)]
async fn missing_pong_times_out_during_a_blocked_write() {
    let (outgoing_tx, outgoing_rx) = mpsc::channel(1);
    let (_pong_tx, pong_rx) = mpsc::channel(1);
    let (observed_tx, mut observed_rx) = mpsc::unbounded_channel();
    let sink = futures::sink::unfold(observed_tx, |observed_tx, message| async move {
        let is_data = matches!(message, Message::Binary(_));
        observed_tx.send(message).unwrap();
        if is_data {
            std::future::pending::<()>().await;
        }
        Ok::<_, Infallible>(observed_tx)
    });
    let task = AbortOnDropHandle::new(tokio::spawn(run(Box::pin(sink), outgoing_rx, pong_rx)));
    assert!(matches!(observed_rx.recv().await, Some(Message::Ping(_))));
    tokio::time::advance(WEBSOCKET_PONG_TIMEOUT * 3 / 4).await;
    outgoing_tx.send(vec![42]).await.unwrap();
    assert_eq!(
        observed_rx.recv().await,
        Some(Message::Binary(vec![42].into()))
    );
    assert_eq!(task.await.unwrap(), RendezvousDisconnectReason::PongTimeout);
}

#[tokio::test(start_paused = true)]
async fn queued_pong_wins_over_an_expired_deadline() {
    let (pong_tx, mut pong_rx) = mpsc::channel(1);
    let mut watchdog = WebSocketPongWatchdog::new(Duration::from_secs(1));
    watchdog.ping_sent(Instant::now());
    tokio::time::advance(Duration::from_secs(1)).await;
    pong_tx.send(()).await.unwrap();
    assert_eq!(receive_pong(&mut watchdog, &mut pong_rx).await, Ok(()));
    assert_eq!(watchdog.deadline(), None);
}
