//! A single threaded executor that uses shortest-job-first scheduling.

use std::cell::RefCell;
use std::collections::BinaryHeap;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::thread;
use std::time::{Duration, Instant};
use std::{cell::Cell, future::Future};

use async_task::{Builder, Runnable, Task};
use pin_project_lite::pin_project;
use smol::{channel, future};

struct ByDuration(Runnable<DurationMetadata>);

impl ByDuration {
    fn duration(&self) -> Duration {
        self.0.metadata().inner.get()
    }
}

impl PartialEq for ByDuration {
    fn eq(&self, other: &Self) -> bool {
        self.duration() == other.duration()
    }
}

impl Eq for ByDuration {}

impl PartialOrd for ByDuration {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ByDuration {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.duration().cmp(&other.duration()).reverse()
    }
}

pin_project! {
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    struct MeasureRuntime<'a, F> {
        #[pin]
        f: F,
        duration: &'a Cell<Duration>
    }
}

impl<'a, F: Future> Future for MeasureRuntime<'a, F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let duration_cell: &Cell<Duration> = this.duration;
        let start = Instant::now();
        let res = F::poll(this.f, cx);
        let new_duration = Instant::now() - start;
        duration_cell.set(duration_cell.get() / 2 + new_duration / 2);
        res
    }
}

pub struct DurationMetadata {
    inner: Cell<Duration>,
}

thread_local! {
    // A queue that holds scheduled tasks.
    static QUEUE: RefCell<BinaryHeap<ByDuration>> = RefCell::new(BinaryHeap::new());
}

fn make_future_fn<'a, F>(
    future: F,
) -> impl (FnOnce(&'a DurationMetadata) -> MeasureRuntime<'a, F>) {
    move |duration_meta| MeasureRuntime {
        f: future,
        duration: &duration_meta.inner,
    }
}

fn ensure_safe_schedule<F: Send + Sync + 'static>(f: F) -> F {
    f
}

/// Spawns a future on the executor.
pub fn spawn<F, T>(future: F) -> Task<T, DurationMetadata>
where
    F: Future<Output = T> + 'static,
    T: 'static,
{
    let spawn_thread_id = thread::current().id();
    // Create a task that is scheduled by pushing it into the queue.
    let schedule = ensure_safe_schedule(move |runnable| {
        if thread::current().id() != spawn_thread_id {
            panic!("Task would be run on a different thread than spawned on.");
        }
        QUEUE.with(move |queue| queue.borrow_mut().push(ByDuration(runnable)));
    });
    let future_fn = make_future_fn(future);
    let (runnable, task) = unsafe {
        Builder::new()
            .metadata(DurationMetadata {
                inner: Cell::new(Duration::default()),
            })
            .spawn_unchecked(future_fn, schedule)
    };

    // Schedule the task by pushing it into the queue.
    runnable.schedule();

    task
}

pub fn block_on<F>(future: F)
where
    F: Future<Output = ()> + 'static,
{
    let task = spawn(future);
    while !task.is_finished() {
        let Some(runnable) = QUEUE.with(|queue| queue.borrow_mut().pop()) else {
            thread::yield_now();
            continue;
        };
        runnable.0.run();
    }
}

fn main() {
    // Spawn a future and await its result.
    block_on(async {
        let (sender, receiver) = channel::bounded(1);
        let world = spawn(async move {
            receiver.recv().await.unwrap();
            println!("world.")
        });
        let hello = spawn(async move {
            sender.send(()).await.unwrap();
            print!("Hello, ")
        });
        future::zip(hello, world).await;
    });
}
