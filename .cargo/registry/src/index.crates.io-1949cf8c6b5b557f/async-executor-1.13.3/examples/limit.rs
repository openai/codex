//! An executor where you can only push a limited number of tasks.

use async_executor::{Executor, Task};
use async_lock::Semaphore;
use std::{future::Future, sync::Arc, time::Duration};

/// An executor where you can only push a limited number of tasks.
struct LimitedExecutor {
    /// Inner running executor.
    executor: Executor<'static>,

    /// Semaphore limiting the number of tasks.
    semaphore: Arc<Semaphore>,
}

impl LimitedExecutor {
    fn new(max: usize) -> Self {
        Self {
            executor: Executor::new(),
            semaphore: Semaphore::new(max).into(),
        }
    }

    /// Spawn a task, waiting until there is a slot available.
    async fn spawn<F: Future + Send + 'static>(&self, future: F) -> Task<F::Output>
    where
        F::Output: Send + 'static,
    {
        // Wait for a semaphore permit.
        let permit = self.semaphore.acquire_arc().await;

        // Wrap it into a new future.
        let future = async move {
            let result = future.await;
            drop(permit);
            result
        };

        // Spawn the task.
        self.executor.spawn(future)
    }

    /// Run a future to completion.
    async fn run<F: Future>(&self, future: F) -> F::Output {
        self.executor.run(future).await
    }
}

fn main() {
    futures_lite::future::block_on(async {
        let ex = Arc::new(LimitedExecutor::new(10));
        ex.run({
            let ex = ex.clone();
            async move {
                // Spawn a bunch of tasks that wait for a while.
                for i in 0..15 {
                    ex.spawn(async move {
                        async_io::Timer::after(Duration::from_millis(fastrand::u64(1..3))).await;
                        println!("Waiting task #{i} finished!");
                    })
                    .await
                    .detach();
                }

                let (start_tx, start_rx) = async_channel::bounded::<()>(1);
                let mut current_rx = start_rx;

                // Send the first message.
                start_tx.send(()).await.unwrap();

                // Spawn a bunch of channel tasks that wake eachother up.
                for i in 0..25 {
                    let (next_tx, next_rx) = async_channel::bounded::<()>(1);

                    ex.spawn(async move {
                        current_rx.recv().await.unwrap();
                        println!("Channel task {i} woken up!");
                        next_tx.send(()).await.unwrap();
                        println!("Channel task {i} finished!");
                    })
                    .await
                    .detach();

                    current_rx = next_rx;
                }

                // Wait for the last task to finish.
                current_rx.recv().await.unwrap();

                println!("All tasks finished!");
            }
        })
        .await;
    });
}
