use async_executor::Executor;
use futures_lite::{future, pin};

#[test]
fn two_queues() {
    future::block_on(async {
        // Create an executor with two runners.
        let ex = Executor::new();
        let (run1, run2) = (
            ex.run(future::pending::<()>()),
            ex.run(future::pending::<()>()),
        );
        let mut run1 = Box::pin(run1);
        pin!(run2);

        // Poll them both.
        assert!(future::poll_once(run1.as_mut()).await.is_none());
        assert!(future::poll_once(run2.as_mut()).await.is_none());

        // Drop the first one, which should leave the local queue in the `None` state.
        drop(run1);
        assert!(future::poll_once(run2.as_mut()).await.is_none());
    });
}
