//! A simple single-threaded executor.

use std::future::Future;
use std::panic::catch_unwind;
use std::thread;

use async_task::{Runnable, Task};
use once_cell::sync::Lazy;
use smol::future;

/// Spawns a future on the executor.
fn spawn<F, T>(future: F) -> Task<T>
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    // A queue that holds scheduled tasks.
    static QUEUE: Lazy<flume::Sender<Runnable>> = Lazy::new(|| {
        let (sender, receiver) = flume::unbounded::<Runnable>();

        // Start the executor thread.
        thread::spawn(|| {
            for runnable in receiver {
                // Ignore panics inside futures.
                let _ignore_panic = catch_unwind(|| runnable.run());
            }
        });

        sender
    });

    // Create a task that is scheduled by pushing it into the queue.
    let schedule = |runnable| QUEUE.send(runnable).unwrap();
    let (runnable, task) = async_task::spawn(future, schedule);

    // Schedule the task by pushing it into the queue.
    runnable.schedule();

    task
}

fn main() {
    // Spawn a future and await its result.
    let task = spawn(async {
        println!("Hello, world!");
    });
    future::block_on(task);
}
