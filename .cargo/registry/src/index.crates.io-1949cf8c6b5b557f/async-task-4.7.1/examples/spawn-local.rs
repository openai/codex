//! A simple single-threaded executor that can spawn non-`Send` futures.

use std::cell::Cell;
use std::future::Future;
use std::rc::Rc;

use async_task::{Runnable, Task};

thread_local! {
    // A queue that holds scheduled tasks.
    static QUEUE: (flume::Sender<Runnable>, flume::Receiver<Runnable>) = flume::unbounded();
}

/// Spawns a future on the executor.
fn spawn<F, T>(future: F) -> Task<T>
where
    F: Future<Output = T> + 'static,
    T: 'static,
{
    // Create a task that is scheduled by pushing itself into the queue.
    let schedule = |runnable| QUEUE.with(|(s, _)| s.send(runnable).unwrap());
    let (runnable, task) = async_task::spawn_local(future, schedule);

    // Schedule the task by pushing it into the queue.
    runnable.schedule();

    task
}

/// Runs a future to completion.
fn run<F, T>(future: F) -> T
where
    F: Future<Output = T> + 'static,
    T: 'static,
{
    // Spawn a task that sends its result through a channel.
    let (s, r) = flume::unbounded();
    spawn(async move { drop(s.send(future.await)) }).detach();

    loop {
        // If the original task has completed, return its result.
        if let Ok(val) = r.try_recv() {
            return val;
        }

        // Otherwise, take a task from the queue and run it.
        QUEUE.with(|(_, r)| r.recv().unwrap().run());
    }
}

fn main() {
    let val = Rc::new(Cell::new(0));

    // Run a future that increments a non-`Send` value.
    run({
        let val = val.clone();
        async move {
            // Spawn a future that increments the value.
            let task = spawn({
                let val = val.clone();
                async move {
                    val.set(dbg!(val.get()) + 1);
                }
            });

            val.set(dbg!(val.get()) + 1);
            task.await;
        }
    });

    // The value should be 2 at the end of the program.
    dbg!(val.get());
}
