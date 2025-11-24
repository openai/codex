#![doc(html_root_url = "https://docs.rs/want/0.3.1")]
#![deny(warnings)]
#![deny(missing_docs)]
#![deny(missing_debug_implementations)]

//! A Futures channel-like utility to signal when a value is wanted.
//!
//! Futures are supposed to be lazy, and only starting work if `Future::poll`
//! is called. The same is true of `Stream`s, but when using a channel as
//! a `Stream`, it can be hard to know if the receiver is ready for the next
//! value.
//!
//! Put another way, given a `(tx, rx)` from `futures::sync::mpsc::channel()`,
//! how can the sender (`tx`) know when the receiver (`rx`) actually wants more
//! work to be produced? Just because there is room in the channel buffer
//! doesn't mean the work would be used by the receiver.
//!
//! This is where something like `want` comes in. Added to a channel, you can
//! make sure that the `tx` only creates the message and sends it when the `rx`
//! has `poll()` for it, and the buffer was empty.
//!
//! # Example
//!
//! ```nightly
//! # //#![feature(async_await)]
//! extern crate want;
//!
//! # fn spawn<T>(_t: T) {}
//! # fn we_still_want_message() -> bool { true }
//! # fn mpsc_channel() -> (Tx, Rx) { (Tx, Rx) }
//! # struct Tx;
//! # impl Tx { fn send<T>(&mut self, _: T) {} }
//! # struct Rx;
//! # impl Rx { async fn recv(&mut self) -> Option<Expensive> { Some(Expensive) } }
//!
//! // Some message that is expensive to produce.
//! struct Expensive;
//!
//! // Some futures-aware MPSC channel...
//! let (mut tx, mut rx) = mpsc_channel();
//!
//! // And our `want` channel!
//! let (mut gv, mut tk) = want::new();
//!
//!
//! // Our receiving task...
//! spawn(async move {
//!     // Maybe something comes up that prevents us from ever
//!     // using the expensive message.
//!     //
//!     // Without `want`, the "send" task may have started to
//!     // produce the expensive message even though we wouldn't
//!     // be able to use it.
//!     if !we_still_want_message() {
//!         return;
//!     }
//!
//!     // But we can use it! So tell the `want` channel.
//!     tk.want();
//!
//!     match rx.recv().await {
//!         Some(_msg) => println!("got a message"),
//!         None => println!("DONE"),
//!     }
//! });
//!
//! // Our sending task
//! spawn(async move {
//!     // It's expensive to create a new message, so we wait until the
//!     // receiving end truly *wants* the message.
//!     if let Err(_closed) = gv.want().await {
//!         // Looks like they will never want it...
//!         return;
//!     }
//!
//!     // They want it, let's go!
//!     tx.send(Expensive);
//! });
//!
//! # fn main() {}
//! ```

use std::fmt;
use std::future::Future;
use std::mem;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
// SeqCst is the only ordering used to ensure accessing the state and
// TryLock are never re-ordered.
use std::sync::atomic::Ordering::SeqCst;
use std::task::{self, Poll, Waker};


use try_lock::TryLock;

/// Create a new `want` channel.
pub fn new() -> (Giver, Taker) {
    let inner = Arc::new(Inner {
        state: AtomicUsize::new(State::Idle.into()),
        task: TryLock::new(None),
    });
    let inner2 = inner.clone();
    (
        Giver {
            inner,
        },
        Taker {
            inner: inner2,
        },
    )
}

/// An entity that gives a value when wanted.
pub struct Giver {
    inner: Arc<Inner>,
}

/// An entity that wants a value.
pub struct Taker {
    inner: Arc<Inner>,
}

/// A cloneable `Giver`.
///
/// It differs from `Giver` in that you cannot poll for `want`. It's only
/// usable as a cancellation watcher.
#[derive(Clone)]
pub struct SharedGiver {
    inner: Arc<Inner>,
}

/// The `Taker` has canceled its interest in a value.
pub struct Closed {
    _inner: (),
}

#[derive(Clone, Copy, Debug)]
enum State {
    Idle,
    Want,
    Give,
    Closed,
}

impl From<State> for usize {
    fn from(s: State) -> usize {
        match s {
            State::Idle => 0,
            State::Want => 1,
            State::Give => 2,
            State::Closed => 3,
        }
    }
}

impl From<usize> for State {
    fn from(num: usize) -> State {
        match num {
            0 => State::Idle,
            1 => State::Want,
            2 => State::Give,
            3 => State::Closed,
            _ => unreachable!("unknown state: {}", num),
        }
    }
}

struct Inner {
    state: AtomicUsize,
    task: TryLock<Option<Waker>>,
}

// ===== impl Giver ======

impl Giver {
    /// Returns a `Future` that fulfills when the `Taker` has done some action.
    pub fn want(&mut self) -> impl Future<Output = Result<(), Closed>> + '_ {
        Want(self)
    }

    /// Poll whether the `Taker` has registered interest in another value.
    ///
    /// - If the `Taker` has called `want()`, this returns `Async::Ready(())`.
    /// - If the `Taker` has not called `want()` since last poll, this
    ///   returns `Async::NotReady`, and parks the current task to be notified
    ///   when the `Taker` does call `want()`.
    /// - If the `Taker` has canceled (or dropped), this returns `Closed`.
    ///
    /// After knowing that the Taker is wanting, the state can be reset by
    /// calling [`give`](Giver::give).
    pub fn poll_want(&mut self, cx: &mut task::Context<'_>) -> Poll<Result<(), Closed>> {
        loop {
            let state = self.inner.state.load(SeqCst).into();
            match state {
                State::Want => {
                    return Poll::Ready(Ok(()));
                },
                State::Closed => {
                    return Poll::Ready(Err(Closed { _inner: () }));
                },
                State::Idle | State::Give => {
                    // Taker doesn't want anything yet, so park.
                    if let Some(mut locked) = self.inner.task.try_lock_explicit(SeqCst, SeqCst) {

                        // While we have the lock, try to set to GIVE.
                        let old = self.inner.state.compare_exchange(
                            state.into(),
                            State::Give.into(),
                            SeqCst,
                            SeqCst,
                        );
                        // If it's still the first state (Idle or Give), park current task.
                        if old == Ok(state.into()) {
                            let park = locked.as_ref()
                                .map(|w| !w.will_wake(cx.waker()))
                                .unwrap_or(true);
                            if park {
                                let old = mem::replace(&mut *locked, Some(cx.waker().clone()));
                                drop(locked);
                                if let Some(prev_task) = old {
                                    // there was an old task parked here.
                                    // it might be waiting to be notified,
                                    // so poke it before dropping.
                                    prev_task.wake();
                                };
                            }
                            return Poll::Pending;
                        }
                        // Otherwise, something happened! Go around the loop again.
                    } else {
                        // if we couldn't take the lock, then a Taker has it.
                        // The *ONLY* reason is because it is in the process of notifying us
                        // of its want.
                        //
                        // We need to loop again to see what state it was changed to.
                    }
                },
            }
        }
    }

    /// Mark the state as idle, if the Taker currently is wanting.
    ///
    /// Returns true if Taker was wanting, false otherwise.
    #[inline]
    pub fn give(&self) -> bool {
        // only set to IDLE if it is still Want
        let old = self.inner.state.compare_exchange(
            State::Want.into(),
            State::Idle.into(),
            SeqCst,
            SeqCst);
        old == Ok(State::Want.into())
    }

    /// Check if the `Taker` has called `want()` without parking a task.
    ///
    /// This is safe to call outside of a futures task context, but other
    /// means of being notified is left to the user.
    #[inline]
    pub fn is_wanting(&self) -> bool {
        self.inner.state.load(SeqCst) == State::Want.into()
    }


    /// Check if the `Taker` has canceled interest without parking a task.
    #[inline]
    pub fn is_canceled(&self) -> bool {
        self.inner.state.load(SeqCst) == State::Closed.into()
    }

    /// Converts this into a `SharedGiver`.
    #[inline]
    pub fn shared(self) -> SharedGiver {
        SharedGiver {
            inner: self.inner,
        }
    }
}

impl fmt::Debug for Giver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Giver")
            .field("state", &self.inner.state())
            .finish()
    }
}

// ===== impl SharedGiver ======

impl SharedGiver {
    /// Check if the `Taker` has called `want()` without parking a task.
    ///
    /// This is safe to call outside of a futures task context, but other
    /// means of being notified is left to the user.
    #[inline]
    pub fn is_wanting(&self) -> bool {
        self.inner.state.load(SeqCst) == State::Want.into()
    }


    /// Check if the `Taker` has canceled interest without parking a task.
    #[inline]
    pub fn is_canceled(&self) -> bool {
        self.inner.state.load(SeqCst) == State::Closed.into()
    }
}

impl fmt::Debug for SharedGiver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SharedGiver")
            .field("state", &self.inner.state())
            .finish()
    }
}

// ===== impl Taker ======

impl Taker {
    /// Signal to the `Giver` that the want is canceled.
    ///
    /// This is useful to tell that the channel is closed if you cannot
    /// drop the value yet.
    #[inline]
    pub fn cancel(&mut self) {
        self.signal(State::Closed)
    }

    /// Signal to the `Giver` that a value is wanted.
    #[inline]
    pub fn want(&mut self) {
        debug_assert!(
            self.inner.state.load(SeqCst) != State::Closed.into(),
            "want called after cancel"
        );
        self.signal(State::Want)
    }

    #[inline]
    fn signal(&mut self, state: State) {
        let old_state = self.inner.state.swap(state.into(), SeqCst).into();
        match old_state {
            State::Idle | State::Want | State::Closed => (),
            State::Give => {
                loop {
                    if let Some(mut locked) = self.inner.task.try_lock_explicit(SeqCst, SeqCst) {
                        if let Some(task) = locked.take() {
                            drop(locked);
                            task.wake();
                        }
                        return;
                    } else {
                        // if we couldn't take the lock, then a Giver has it.
                        // The *ONLY* reason is because it is in the process of parking.
                        //
                        // We need to loop and take the lock so we can notify this task.
                    }
                }
            },
        }
    }
}

impl Drop for Taker {
    #[inline]
    fn drop(&mut self) {
        self.signal(State::Closed);
    }
}

impl fmt::Debug for Taker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Taker")
            .field("state", &self.inner.state())
            .finish()
    }
}

// ===== impl Closed ======

impl fmt::Debug for Closed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Closed")
            .finish()
    }
}

// ===== impl Inner ======

impl Inner {
    #[inline]
    fn state(&self) -> State {
        self.state.load(SeqCst).into()
    }
}

// ===== impl PollFn ======

struct Want<'a>(&'a mut Giver);


impl Future for Want<'_> {
    type Output = Result<(), Closed>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
        self.0.poll_want(cx)
    }
}

#[cfg(test)]
mod tests {
    use std::thread;
    use tokio_sync::oneshot;
    use super::*;

    fn block_on<F: Future>(f: F) -> F::Output {
        tokio_executor::enter()
            .expect("block_on enter")
            .block_on(f)
    }

    #[test]
    fn want_ready() {
        let (mut gv, mut tk) = new();

        tk.want();

        block_on(gv.want()).unwrap();
    }

    #[test]
    fn want_notify_0() {
        let (mut gv, mut tk) = new();
        let (tx, rx) = oneshot::channel();

        thread::spawn(move || {
            tk.want();
            // use a oneshot to keep this thread alive
            // until other thread was notified of want
            block_on(rx).expect("rx");
        });

        block_on(gv.want()).expect("want");

        assert!(gv.is_wanting(), "still wanting after poll_want success");
        assert!(gv.give(), "give is true when wanting");

        assert!(!gv.is_wanting(), "no longer wanting after give");
        assert!(!gv.is_canceled(), "give doesn't cancel");

        assert!(!gv.give(), "give is false if not wanting");

        tx.send(()).expect("tx");
    }

    /*
    /// This tests that if the Giver moves tasks after parking,
    /// it will still wake up the correct task.
    #[test]
    fn want_notify_moving_tasks() {
        use std::sync::Arc;
        use futures::executor::{spawn, Notify, NotifyHandle};

        struct WantNotify;

        impl Notify for WantNotify {
            fn notify(&self, _id: usize) {
            }
        }

        fn n() -> NotifyHandle {
            Arc::new(WantNotify).into()
        }

        let (mut gv, mut tk) = new();

        let mut s = spawn(poll_fn(move || {
            gv.poll_want()
        }));

        // Register with t1 as the task::current()
        let t1 = n();
        assert!(s.poll_future_notify(&t1, 1).unwrap().is_not_ready());

        thread::spawn(move || {
            thread::sleep(::std::time::Duration::from_millis(100));
            tk.want();
        });

        // And now, move to a ThreadNotify task.
        s.into_inner().wait().expect("poll_want");
    }
    */

    #[test]
    fn cancel() {
        // explicit
        let (mut gv, mut tk) = new();

        assert!(!gv.is_canceled());

        tk.cancel();

        assert!(gv.is_canceled());
        block_on(gv.want()).unwrap_err();

        // implicit
        let (mut gv, tk) = new();

        assert!(!gv.is_canceled());

        drop(tk);

        assert!(gv.is_canceled());
        block_on(gv.want()).unwrap_err();

        // notifies
        let (mut gv, tk) = new();

        thread::spawn(move || {
            let _tk = tk;
            // and dropped
        });

        block_on(gv.want()).unwrap_err();
    }

    /*
    #[test]
    fn stress() {
        let nthreads = 5;
        let nwants = 100;

        for _ in 0..nthreads {
            let (mut gv, mut tk) = new();
            let (mut tx, mut rx) = mpsc::channel(0);

            // rx thread
            thread::spawn(move || {
                let mut cnt = 0;
                poll_fn(move || {
                    while cnt < nwants {
                        let n = match rx.poll().expect("rx poll") {
                            Async::Ready(n) => n.expect("rx opt"),
                            Async::NotReady => {
                                tk.want();
                                return Ok(Async::NotReady);
                            },
                        };
                        assert_eq!(cnt, n);
                        cnt += 1;
                    }
                    Ok::<_, ()>(Async::Ready(()))
                }).wait().expect("rx wait");
            });

            // tx thread
            thread::spawn(move || {
                let mut cnt = 0;
                let nsent = poll_fn(move || {
                    loop {
                        while let Ok(()) = tx.try_send(cnt) {
                            cnt += 1;
                        }
                        match gv.poll_want() {
                            Ok(Async::Ready(_)) => (),
                            Ok(Async::NotReady) => return Ok::<_, ()>(Async::NotReady),
                            Err(_) => return Ok(Async::Ready(cnt)),
                        }
                    }
                }).wait().expect("tx wait");

                assert_eq!(nsent, nwants);
            }).join().expect("thread join");
        }
    }
    */
}
