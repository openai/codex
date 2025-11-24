use event_listener::{Event, EventListener};
use event_listener_strategy::{easy_wrapper, EventListenerFuture, Strategy};

use core::fmt;
use core::pin::Pin;
use core::task::Poll;

use crate::futures::Lock;
use crate::Mutex;

/// A counter to synchronize multiple tasks at the same time.
#[derive(Debug)]
pub struct Barrier {
    n: usize,
    state: Mutex<State>,
    event: Event,
}

#[derive(Debug)]
struct State {
    count: usize,
    generation_id: u64,
}

impl Barrier {
    const_fn! {
        const_if: #[cfg(not(loom))];
        /// Creates a barrier that can block the given number of tasks.
        ///
        /// A barrier will block `n`-1 tasks which call [`wait()`] and then wake up all tasks
        /// at once when the `n`th task calls [`wait()`].
        ///
        /// [`wait()`]: `Barrier::wait()`
        ///
        /// # Examples
        ///
        /// ```
        /// use async_lock::Barrier;
        ///
        /// let barrier = Barrier::new(5);
        /// ```
        pub const fn new(n: usize) -> Barrier {
            Barrier {
                n,
                state: Mutex::new(State {
                    count: 0,
                    generation_id: 0,
                }),
                event: Event::new(),
            }
        }
    }

    /// Blocks the current task until all tasks reach this point.
    ///
    /// Barriers are reusable after all tasks have synchronized, and can be used continuously.
    ///
    /// Returns a [`BarrierWaitResult`] indicating whether this task is the "leader", meaning the
    /// last task to call this method.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_lock::Barrier;
    /// use futures_lite::future;
    /// use std::sync::Arc;
    /// use std::thread;
    ///
    /// let barrier = Arc::new(Barrier::new(5));
    ///
    /// for _ in 0..5 {
    ///     let b = barrier.clone();
    ///     thread::spawn(move || {
    ///         future::block_on(async {
    ///             // The same messages will be printed together.
    ///             // There will NOT be interleaving of "before" and "after".
    ///             println!("before wait");
    ///             b.wait().await;
    ///             println!("after wait");
    ///         });
    ///     });
    /// }
    /// ```
    pub fn wait(&self) -> BarrierWait<'_> {
        BarrierWait::_new(BarrierWaitInner {
            barrier: self,
            lock: Some(self.state.lock()),
            evl: None,
            state: WaitState::Initial,
        })
    }

    /// Blocks the current thread until all tasks reach this point.
    ///
    /// Barriers are reusable after all tasks have synchronized, and can be used continuously.
    ///
    /// Returns a [`BarrierWaitResult`] indicating whether this task is the "leader", meaning the
    /// last task to call this method.
    ///
    /// # Blocking
    ///
    /// Rather than using asynchronous waiting, like the [`wait`][`Barrier::wait`] method,
    /// this method will block the current thread until the wait is complete.
    ///
    /// This method should not be used in an asynchronous context. It is intended to be
    /// used in a way that a barrier can be used in both asynchronous and synchronous contexts.
    /// Calling this method in an asynchronous context may result in a deadlock.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_lock::Barrier;
    /// use futures_lite::future;
    /// use std::sync::Arc;
    /// use std::thread;
    ///
    /// let barrier = Arc::new(Barrier::new(5));
    ///
    /// for _ in 0..5 {
    ///     let b = barrier.clone();
    ///     thread::spawn(move || {
    ///         // The same messages will be printed together.
    ///         // There will NOT be interleaving of "before" and "after".
    ///         println!("before wait");
    ///         b.wait_blocking();
    ///         println!("after wait");
    ///     });
    /// }
    /// # // Wait for threads to stop.
    /// # std::thread::sleep(std::time::Duration::from_secs(1));
    /// ```
    #[cfg(all(feature = "std", not(target_family = "wasm")))]
    pub fn wait_blocking(&self) -> BarrierWaitResult {
        self.wait().wait()
    }
}

easy_wrapper! {
    /// The future returned by [`Barrier::wait()`].
    pub struct BarrierWait<'a>(BarrierWaitInner<'a> => BarrierWaitResult);
    #[cfg(all(feature = "std", not(target_family = "wasm")))]
    pub(crate) wait();
}

pin_project_lite::pin_project! {
    /// The future returned by [`Barrier::wait()`].
    struct BarrierWaitInner<'a> {
        // The barrier to wait on.
        barrier: &'a Barrier,

        // The ongoing mutex lock operation we are blocking on.
        #[pin]
        lock: Option<Lock<'a, State>>,

        // An event listener for the `barrier.event` event.
        evl: Option<EventListener>,

        // The current state of the future.
        state: WaitState,
    }
}

impl fmt::Debug for BarrierWait<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BarrierWait { .. }")
    }
}

enum WaitState {
    /// We are getting the original values of the state.
    Initial,

    /// We are waiting for the listener to complete.
    Waiting { local_gen: u64 },

    /// Waiting to re-acquire the lock to check the state again.
    Reacquiring { local_gen: u64 },
}

impl EventListenerFuture for BarrierWaitInner<'_> {
    type Output = BarrierWaitResult;

    fn poll_with_strategy<'a, S: Strategy<'a>>(
        self: Pin<&mut Self>,
        strategy: &mut S,
        cx: &mut S::Context,
    ) -> Poll<Self::Output> {
        let mut this = self.project();

        loop {
            match this.state {
                WaitState::Initial => {
                    // See if the lock is ready yet.
                    let mut state = ready!(this
                        .lock
                        .as_mut()
                        .as_pin_mut()
                        .unwrap()
                        .poll_with_strategy(strategy, cx));
                    this.lock.as_mut().set(None);

                    let local_gen = state.generation_id;
                    state.count += 1;

                    if state.count < this.barrier.n {
                        // We need to wait for the event.
                        *this.evl = Some(this.barrier.event.listen());
                        *this.state = WaitState::Waiting { local_gen };
                    } else {
                        // We are the last one.
                        state.count = 0;
                        state.generation_id = state.generation_id.wrapping_add(1);
                        this.barrier.event.notify(usize::MAX);
                        return Poll::Ready(BarrierWaitResult { is_leader: true });
                    }
                }

                WaitState::Waiting { local_gen } => {
                    ready!(strategy.poll(this.evl, cx));

                    // We are now re-acquiring the mutex.
                    this.lock.as_mut().set(Some(this.barrier.state.lock()));
                    *this.state = WaitState::Reacquiring {
                        local_gen: *local_gen,
                    };
                }

                WaitState::Reacquiring { local_gen } => {
                    // Acquire the local state again.
                    let state = ready!(this
                        .lock
                        .as_mut()
                        .as_pin_mut()
                        .unwrap()
                        .poll_with_strategy(strategy, cx));
                    this.lock.set(None);

                    if *local_gen == state.generation_id && state.count < this.barrier.n {
                        // We need to wait for the event again.
                        *this.evl = Some(this.barrier.event.listen());
                        *this.state = WaitState::Waiting {
                            local_gen: *local_gen,
                        };
                    } else {
                        // We are ready, but not the leader.
                        return Poll::Ready(BarrierWaitResult { is_leader: false });
                    }
                }
            }
        }
    }
}

/// Returned by [`Barrier::wait()`] when all tasks have called it.
///
/// # Examples
///
/// ```
/// # futures_lite::future::block_on(async {
/// use async_lock::Barrier;
///
/// let barrier = Barrier::new(1);
/// let barrier_wait_result = barrier.wait().await;
/// # });
/// ```
#[derive(Debug, Clone)]
pub struct BarrierWaitResult {
    is_leader: bool,
}

impl BarrierWaitResult {
    /// Returns `true` if this task was the last to call to [`Barrier::wait()`].
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use async_lock::Barrier;
    /// use futures_lite::future;
    ///
    /// let barrier = Barrier::new(2);
    /// let (a, b) = future::zip(barrier.wait(), barrier.wait()).await;
    /// assert_eq!(a.is_leader(), false);
    /// assert_eq!(b.is_leader(), true);
    /// # });
    /// ```
    pub fn is_leader(&self) -> bool {
        self.is_leader
    }
}
