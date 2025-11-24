use futures::channel::mpsc;
use futures::executor::block_on;
use futures::future::Future;
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use futures::task::{Context, Poll};
use std::pin::Pin;
use std::sync::{Arc, Weak};
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn smoke() {
    let (mut sender, receiver) = mpsc::channel(1);

    let t = thread::spawn(move || while let Ok(()) = block_on(sender.send(42)) {});

    // `receiver` needs to be dropped for `sender` to stop sending and therefore before the join.
    block_on(receiver.take(3).for_each(|_| futures::future::ready(())));

    t.join().unwrap()
}

#[test]
fn multiple_senders_disconnect() {
    {
        let (mut tx1, mut rx) = mpsc::channel(1);
        let (tx2, mut tx3, mut tx4) = (tx1.clone(), tx1.clone(), tx1.clone());

        // disconnect, dropping and Sink::poll_close should all close this sender but leave the
        // channel open for other senders
        tx1.disconnect();
        drop(tx2);
        block_on(tx3.close()).unwrap();

        assert!(tx1.is_closed());
        assert!(tx3.is_closed());
        assert!(!tx4.is_closed());

        block_on(tx4.send(5)).unwrap();
        assert_eq!(block_on(rx.next()), Some(5));

        // dropping the final sender will close the channel
        drop(tx4);
        assert_eq!(block_on(rx.next()), None);
    }

    {
        let (mut tx1, mut rx) = mpsc::unbounded();
        let (tx2, mut tx3, mut tx4) = (tx1.clone(), tx1.clone(), tx1.clone());

        // disconnect, dropping and Sink::poll_close should all close this sender but leave the
        // channel open for other senders
        tx1.disconnect();
        drop(tx2);
        block_on(tx3.close()).unwrap();

        assert!(tx1.is_closed());
        assert!(tx3.is_closed());
        assert!(!tx4.is_closed());

        block_on(tx4.send(5)).unwrap();
        assert_eq!(block_on(rx.next()), Some(5));

        // dropping the final sender will close the channel
        drop(tx4);
        assert_eq!(block_on(rx.next()), None);
    }
}

#[test]
fn multiple_senders_close_channel() {
    {
        let (mut tx1, mut rx) = mpsc::channel(1);
        let mut tx2 = tx1.clone();

        // close_channel should shut down the whole channel
        tx1.close_channel();

        assert!(tx1.is_closed());
        assert!(tx2.is_closed());

        let err = block_on(tx2.send(5)).unwrap_err();
        assert!(err.is_disconnected());

        assert_eq!(block_on(rx.next()), None);
    }

    {
        let (tx1, mut rx) = mpsc::unbounded();
        let mut tx2 = tx1.clone();

        // close_channel should shut down the whole channel
        tx1.close_channel();

        assert!(tx1.is_closed());
        assert!(tx2.is_closed());

        let err = block_on(tx2.send(5)).unwrap_err();
        assert!(err.is_disconnected());

        assert_eq!(block_on(rx.next()), None);
    }
}

#[test]
fn single_receiver_drop_closes_channel_and_drains() {
    {
        let ref_count = Arc::new(0);
        let weak_ref = Arc::downgrade(&ref_count);

        let (sender, receiver) = mpsc::unbounded();
        sender.unbounded_send(ref_count).expect("failed to send");

        // Verify that the sent message is still live.
        assert!(weak_ref.upgrade().is_some());

        drop(receiver);

        // The sender should know the channel is closed.
        assert!(sender.is_closed());

        // Verify that the sent message has been dropped.
        assert!(weak_ref.upgrade().is_none());
    }

    {
        let ref_count = Arc::new(0);
        let weak_ref = Arc::downgrade(&ref_count);

        let (mut sender, receiver) = mpsc::channel(1);
        sender.try_send(ref_count).expect("failed to send");

        // Verify that the sent message is still live.
        assert!(weak_ref.upgrade().is_some());

        drop(receiver);

        // The sender should know the channel is closed.
        assert!(sender.is_closed());

        // Verify that the sent message has been dropped.
        assert!(weak_ref.upgrade().is_none());
        assert!(sender.is_closed());
    }
}

// Stress test that `try_send()`s occurring concurrently with receiver
// close/drops don't appear as successful sends.
#[cfg_attr(miri, ignore)] // Miri is too slow
#[test]
fn stress_try_send_as_receiver_closes() {
    const AMT: usize = 10000;
    // To provide variable timing characteristics (in the hopes of
    // reproducing the collision that leads to a race), we busy-re-poll
    // the test MPSC receiver a variable number of times before actually
    // stopping.  We vary this countdown between 1 and the following
    // value.
    const MAX_COUNTDOWN: usize = 20;
    // When we detect that a successfully sent item is still in the
    // queue after a disconnect, we spin for up to 100ms to confirm that
    // it is a persistent condition and not a concurrency illusion.
    const SPIN_TIMEOUT_S: u64 = 10;
    const SPIN_SLEEP_MS: u64 = 10;
    struct TestRx {
        rx: mpsc::Receiver<Arc<()>>,
        // The number of times to query `rx` before dropping it.
        poll_count: usize,
    }
    struct TestTask {
        command_rx: mpsc::Receiver<TestRx>,
        test_rx: Option<mpsc::Receiver<Arc<()>>>,
        countdown: usize,
    }
    impl TestTask {
        /// Create a new TestTask
        fn new() -> (Self, mpsc::Sender<TestRx>) {
            let (command_tx, command_rx) = mpsc::channel::<TestRx>(0);
            (
                Self {
                    command_rx,
                    test_rx: None,
                    countdown: 0, // 0 means no countdown is in progress.
                },
                command_tx,
            )
        }
    }
    impl Future for TestTask {
        type Output = ();

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            // Poll the test channel, if one is present.
            if let Some(rx) = &mut self.test_rx {
                if let Poll::Ready(v) = rx.poll_next_unpin(cx) {
                    let _ = v.expect("test finished unexpectedly!");
                }
                self.countdown -= 1;
                // Busy-poll until the countdown is finished.
                cx.waker().wake_by_ref();
            }
            // Accept any newly submitted MPSC channels for testing.
            match self.command_rx.poll_next_unpin(cx) {
                Poll::Ready(Some(TestRx { rx, poll_count })) => {
                    self.test_rx = Some(rx);
                    self.countdown = poll_count;
                    cx.waker().wake_by_ref();
                }
                Poll::Ready(None) => return Poll::Ready(()),
                Poll::Pending => {}
            }
            if self.countdown == 0 {
                // Countdown complete -- drop the Receiver.
                self.test_rx = None;
            }
            Poll::Pending
        }
    }
    let (f, mut cmd_tx) = TestTask::new();
    let bg = thread::spawn(move || block_on(f));
    for i in 0..AMT {
        let (mut test_tx, rx) = mpsc::channel(0);
        let poll_count = i % MAX_COUNTDOWN;
        cmd_tx.try_send(TestRx { rx, poll_count }).unwrap();
        let mut prev_weak: Option<Weak<()>> = None;
        let mut attempted_sends = 0;
        let mut successful_sends = 0;
        loop {
            // Create a test item.
            let item = Arc::new(());
            let weak = Arc::downgrade(&item);
            match test_tx.try_send(item) {
                Ok(_) => {
                    prev_weak = Some(weak);
                    successful_sends += 1;
                }
                Err(ref e) if e.is_full() => {}
                Err(ref e) if e.is_disconnected() => {
                    // Test for evidence of the race condition.
                    if let Some(prev_weak) = prev_weak {
                        if prev_weak.upgrade().is_some() {
                            // The previously sent item is still allocated.
                            // However, there appears to be some aspect of the
                            // concurrency that can legitimately cause the Arc
                            // to be momentarily valid.  Spin for up to 100ms
                            // waiting for the previously sent item to be
                            // dropped.
                            let t0 = Instant::now();
                            let mut spins = 0;
                            loop {
                                if prev_weak.upgrade().is_none() {
                                    break;
                                }
                                assert!(
                                    t0.elapsed() < Duration::from_secs(SPIN_TIMEOUT_S),
                                    "item not dropped on iteration {} after \
                                     {} sends ({} successful). spin=({})",
                                    i,
                                    attempted_sends,
                                    successful_sends,
                                    spins
                                );
                                spins += 1;
                                thread::sleep(Duration::from_millis(SPIN_SLEEP_MS));
                            }
                        }
                    }
                    break;
                }
                Err(ref e) => panic!("unexpected error: {}", e),
            }
            attempted_sends += 1;
        }
    }
    drop(cmd_tx);
    bg.join().expect("background thread join");
}

#[test]
fn unbounded_try_next_after_none() {
    let (tx, mut rx) = mpsc::unbounded::<String>();
    // Drop the sender, close the channel.
    drop(tx);
    // Receive the end of channel.
    assert_eq!(Ok(None), rx.try_next().map_err(|_| ()));
    // None received, check we can call `try_next` again.
    assert_eq!(Ok(None), rx.try_next().map_err(|_| ()));
}

#[test]
fn bounded_try_next_after_none() {
    let (tx, mut rx) = mpsc::channel::<String>(17);
    // Drop the sender, close the channel.
    drop(tx);
    // Receive the end of channel.
    assert_eq!(Ok(None), rx.try_next().map_err(|_| ()));
    // None received, check we can call `try_next` again.
    assert_eq!(Ok(None), rx.try_next().map_err(|_| ()));
}
