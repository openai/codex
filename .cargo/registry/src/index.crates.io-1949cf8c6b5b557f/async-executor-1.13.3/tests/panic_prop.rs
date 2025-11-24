use async_executor::Executor;
use futures_lite::{future, prelude::*};

#[test]
fn test_panic_propagation() {
    let ex = Executor::new();
    let task = ex.spawn(async { panic!("should be caught by the task") });

    // Running the executor should not panic.
    assert!(ex.try_tick());

    // Polling the task should.
    assert!(future::block_on(task.catch_unwind()).is_err());
}
