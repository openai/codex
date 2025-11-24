//! A function that runs a future to completion on a dedicated thread.

use std::future::Future;
use std::sync::Arc;
use std::thread;

use async_task::Task;
use smol::future;

/// Spawns a future on a new dedicated thread.
///
/// The returned task can be used to await the output of the future.
fn spawn_on_thread<F, T>(future: F) -> Task<T>
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    // Create a channel that holds the task when it is scheduled for running.
    let (sender, receiver) = flume::unbounded();
    let sender = Arc::new(sender);
    let s = Arc::downgrade(&sender);

    // Wrap the future into one that disconnects the channel on completion.
    let future = async move {
        // When the inner future completes, the sender gets dropped and disconnects the channel.
        let _sender = sender;
        future.await
    };

    // Create a task that is scheduled by sending it into the channel.
    let schedule = move |runnable| s.upgrade().unwrap().send(runnable).unwrap();
    let (runnable, task) = async_task::spawn(future, schedule);

    // Schedule the task by sending it into the channel.
    runnable.schedule();

    // Spawn a thread running the task to completion.
    thread::spawn(move || {
        // Keep taking the task from the channel and running it until completion.
        for runnable in receiver {
            runnable.run();
        }
    });

    task
}

fn main() {
    // Spawn a future on a dedicated thread.
    future::block_on(spawn_on_thread(async {
        println!("Hello, world!");
    }));
}
