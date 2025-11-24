use crate::loom::sync::Mutex;
use crate::sync::watch;
#[cfg(all(tokio_unstable, feature = "tracing"))]
use crate::util::trace;

/// A barrier enables multiple tasks to synchronize the beginning of some computation.
///
/// ```
/// # #[tokio::main]
/// # async fn main() {
/// use tokio::sync::Barrier;
/// use std::sync::Arc;
///
/// let mut handles = Vec::with_capacity(10);
/// let barrier = Arc::new(Barrier::new(10));
/// for _ in 0..10 {
///     let c = barrier.clone();
///     // The same messages will be printed together.
///     // You will NOT see any interleaving.
///     handles.push(tokio::spawn(async move {
///         println!("before wait");
///         let wait_result = c.wait().await;
///         println!("after wait");
///         wait_result
///     }));
/// }
///
/// // Will not resolve until all "after wait" messages have been printed
/// let mut num_leaders = 0;
/// for handle in handles {
///     let wait_result = handle.await.unwrap();
///     if wait_result.is_leader() {
///         num_leaders += 1;
///     }
/// }
///
/// // Exactly one barrier will resolve as the "leader"
/// assert_eq!(num_leaders, 1);
/// # }
/// ```
#[derive(Debug)]
pub struct Barrier {
    state: Mutex<BarrierState>,
    wait: watch::Receiver<usize>,
    n: usize,
    #[cfg(all(tokio_unstable, feature = "tracing"))]
    resource_span: tracing::Span,
}

#[derive(Debug)]
struct BarrierState {
    waker: watch::Sender<usize>,
    arrived: usize,
    generation: usize,
}

impl Barrier {
    /// Creates a new barrier that can block a given number of tasks.
    ///
    /// A barrier will block `n`-1 tasks which call [`Barrier::wait`] and then wake up all
    /// tasks at once when the `n`th task calls `wait`.
    #[track_caller]
    pub fn new(mut n: usize) -> Barrier {
        let (waker, wait) = crate::sync::watch::channel(0);

        if n == 0 {
            // if n is 0, it's not clear what behavior the user wants.
            // in std::sync::Barrier, an n of 0 exhibits the same behavior as n == 1, where every
            // .wait() immediately unblocks, so we adopt that here as well.
            n = 1;
        }

        #[cfg(all(tokio_unstable, feature = "tracing"))]
        let resource_span = {
            let location = std::panic::Location::caller();
            let resource_span = tracing::trace_span!(
                parent: None,
                "runtime.resource",
                concrete_type = "Barrier",
                kind = "Sync",
                loc.file = location.file(),
                loc.line = location.line(),
                loc.col = location.column(),
            );

            resource_span.in_scope(|| {
                tracing::trace!(
                    target: "runtime::resource::state_update",
                    size = n,
                );

                tracing::trace!(
                    target: "runtime::resource::state_update",
                    arrived = 0,
                )
            });
            resource_span
        };

        Barrier {
            state: Mutex::new(BarrierState {
                waker,
                arrived: 0,
                generation: 1,
            }),
            n,
            wait,
            #[cfg(all(tokio_unstable, feature = "tracing"))]
            resource_span,
        }
    }

    /// Does not resolve until all tasks have rendezvoused here.
    ///
    /// Barriers are re-usable after all tasks have rendezvoused once, and can
    /// be used continuously.
    ///
    /// A single (arbitrary) future will receive a [`BarrierWaitResult`] that returns `true` from
    /// [`BarrierWaitResult::is_leader`] when returning from this function, and all other tasks
    /// will receive a result that will return `false` from `is_leader`.
    ///
    /// # Cancel safety
    ///
    /// This method is not cancel safe.
    pub async fn wait(&self) -> BarrierWaitResult {
        #[cfg(all(tokio_unstable, feature = "tracing"))]
        return trace::async_op(
            || self.wait_internal(),
            self.resource_span.clone(),
            "Barrier::wait",
            "poll",
            false,
        )
        .await;

        #[cfg(any(not(tokio_unstable), not(feature = "tracing")))]
        return self.wait_internal().await;
    }
    async fn wait_internal(&self) -> BarrierWaitResult {
        crate::trace::async_trace_leaf().await;

        // NOTE: we are taking a _synchronous_ lock here.
        // It is okay to do so because the critical section is fast and never yields, so it cannot
        // deadlock even if another future is concurrently holding the lock.
        // It is _desirable_ to do so as synchronous Mutexes are, at least in theory, faster than
        // the asynchronous counter-parts, so we should use them where possible [citation needed].
        // NOTE: the extra scope here is so that the compiler doesn't think `state` is held across
        // a yield point, and thus marks the returned future as !Send.
        let generation = {
            let mut state = self.state.lock();
            let generation = state.generation;
            state.arrived += 1;
            #[cfg(all(tokio_unstable, feature = "tracing"))]
            tracing::trace!(
                target: "runtime::resource::state_update",
                arrived = 1,
                arrived.op = "add",
            );
            #[cfg(all(tokio_unstable, feature = "tracing"))]
            tracing::trace!(
                target: "runtime::resource::async_op::state_update",
                arrived = true,
            );
            if state.arrived == self.n {
                #[cfg(all(tokio_unstable, feature = "tracing"))]
                tracing::trace!(
                    target: "runtime::resource::async_op::state_update",
                    is_leader = true,
                );
                // we are the leader for this generation
                // wake everyone, increment the generation, and return
                state
                    .waker
                    .send(state.generation)
                    .expect("there is at least one receiver");
                state.arrived = 0;
                state.generation += 1;
                return BarrierWaitResult(true);
            }

            generation
        };

        // we're going to have to wait for the last of the generation to arrive
        let mut wait = self.wait.clone();

        loop {
            let _ = wait.changed().await;

            // note that the first time through the loop, this _will_ yield a generation
            // immediately, since we cloned a receiver that has never seen any values.
            if *wait.borrow() >= generation {
                break;
            }
        }

        BarrierWaitResult(false)
    }
}

/// A `BarrierWaitResult` is returned by `wait` when all tasks in the `Barrier` have rendezvoused.
#[derive(Debug, Clone)]
pub struct BarrierWaitResult(bool);

impl BarrierWaitResult {
    /// Returns `true` if this task from wait is the "leader task".
    ///
    /// Only one task will have `true` returned from their result, all other tasks will have
    /// `false` returned.
    pub fn is_leader(&self) -> bool {
        self.0
    }
}
