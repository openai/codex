use async_executor::LocalExecutor;
use futures_lite::future::{block_on, pending, poll_once};
use futures_lite::pin;
use std::cell::Cell;

#[test]
fn shared_queue_slot() {
    block_on(async {
        let was_polled = Cell::new(false);
        let future = async {
            was_polled.set(true);
            pending::<()>().await;
        };

        let ex1 = LocalExecutor::new();
        let ex2 = LocalExecutor::new();

        // Start the futures for running forever.
        let (run1, run2) = (ex1.run(pending::<()>()), ex2.run(pending::<()>()));
        pin!(run1);
        pin!(run2);
        assert!(poll_once(run1.as_mut()).await.is_none());
        assert!(poll_once(run2.as_mut()).await.is_none());

        // Spawn the future on executor one and then poll executor two.
        ex1.spawn(future).detach();
        assert!(poll_once(run2).await.is_none());
        assert!(!was_polled.get());

        // Poll the first one.
        assert!(poll_once(run1).await.is_none());
        assert!(was_polled.get());
    });
}
