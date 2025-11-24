// Allow `unreachable_pub` warnings when sync is not enabled
// due to the usage of `Notify` within the `rt` feature set.
// When this module is compiled with `sync` enabled we will warn on
// this lint. When `rt` is enabled we use `pub(crate)` which
// triggers this warning but it is safe to ignore in this case.
#![cfg_attr(not(feature = "sync"), allow(unreachable_pub, dead_code))]

use crate::loom::cell::UnsafeCell;
use crate::loom::sync::atomic::AtomicUsize;
use crate::loom::sync::Mutex;
use crate::util::linked_list::{self, GuardedLinkedList, LinkedList};
use crate::util::WakeList;

use std::future::Future;
use std::marker::PhantomPinned;
use std::panic::{RefUnwindSafe, UnwindSafe};
use std::pin::Pin;
use std::ptr::NonNull;
use std::sync::atomic::Ordering::{self, Acquire, Relaxed, Release, SeqCst};
use std::sync::Arc;
use std::task::{Context, Poll, Waker};

type WaitList = LinkedList<Waiter, <Waiter as linked_list::Link>::Target>;
type GuardedWaitList = GuardedLinkedList<Waiter, <Waiter as linked_list::Link>::Target>;

/// Notifies a single task to wake up.
///
/// `Notify` provides a basic mechanism to notify a single task of an event.
/// `Notify` itself does not carry any data. Instead, it is to be used to signal
/// another task to perform an operation.
///
/// A `Notify` can be thought of as a [`Semaphore`] starting with 0 permits. The
/// [`notified().await`] method waits for a permit to become available, and
/// [`notify_one()`] sets a permit **if there currently are no available
/// permits**.
///
/// The synchronization details of `Notify` are similar to
/// [`thread::park`][park] and [`Thread::unpark`][unpark] from std. A [`Notify`]
/// value contains a single permit. [`notified().await`] waits for the permit to
/// be made available, consumes the permit, and resumes.  [`notify_one()`] sets
/// the permit, waking a pending task if there is one.
///
/// If `notify_one()` is called **before** `notified().await`, then the next
/// call to `notified().await` will complete immediately, consuming the permit.
/// Any subsequent calls to `notified().await` will wait for a new permit.
///
/// If `notify_one()` is called **multiple** times before `notified().await`,
/// only a **single** permit is stored. The next call to `notified().await` will
/// complete immediately, but the one after will wait for a new permit.
///
/// # Examples
///
/// Basic usage.
///
/// ```
/// use tokio::sync::Notify;
/// use std::sync::Arc;
///
/// #[tokio::main]
/// async fn main() {
///     let notify = Arc::new(Notify::new());
///     let notify2 = notify.clone();
///
///     let handle = tokio::spawn(async move {
///         notify2.notified().await;
///         println!("received notification");
///     });
///
///     println!("sending notification");
///     notify.notify_one();
///
///     // Wait for task to receive notification.
///     handle.await.unwrap();
/// }
/// ```
///
/// Unbound multi-producer single-consumer (mpsc) channel.
///
/// No wakeups can be lost when using this channel because the call to
/// `notify_one()` will store a permit in the `Notify`, which the following call
/// to `notified()` will consume.
///
/// ```
/// use tokio::sync::Notify;
///
/// use std::collections::VecDeque;
/// use std::sync::Mutex;
///
/// struct Channel<T> {
///     values: Mutex<VecDeque<T>>,
///     notify: Notify,
/// }
///
/// impl<T> Channel<T> {
///     pub fn send(&self, value: T) {
///         self.values.lock().unwrap()
///             .push_back(value);
///
///         // Notify the consumer a value is available
///         self.notify.notify_one();
///     }
///
///     // This is a single-consumer channel, so several concurrent calls to
///     // `recv` are not allowed.
///     pub async fn recv(&self) -> T {
///         loop {
///             // Drain values
///             if let Some(value) = self.values.lock().unwrap().pop_front() {
///                 return value;
///             }
///
///             // Wait for values to be available
///             self.notify.notified().await;
///         }
///     }
/// }
/// ```
///
/// Unbound multi-producer multi-consumer (mpmc) channel.
///
/// The call to [`enable`] is important because otherwise if you have two
/// calls to `recv` and two calls to `send` in parallel, the following could
/// happen:
///
///  1. Both calls to `try_recv` return `None`.
///  2. Both new elements are added to the vector.
///  3. The `notify_one` method is called twice, adding only a single
///     permit to the `Notify`.
///  4. Both calls to `recv` reach the `Notified` future. One of them
///     consumes the permit, and the other sleeps forever.
///
/// By adding the `Notified` futures to the list by calling `enable` before
/// `try_recv`, the `notify_one` calls in step three would remove the
/// futures from the list and mark them notified instead of adding a permit
/// to the `Notify`. This ensures that both futures are woken.
///
/// Notice that this failure can only happen if there are two concurrent calls
/// to `recv`. This is why the mpsc example above does not require a call to
/// `enable`.
///
/// ```
/// use tokio::sync::Notify;
///
/// use std::collections::VecDeque;
/// use std::sync::Mutex;
///
/// struct Channel<T> {
///     messages: Mutex<VecDeque<T>>,
///     notify_on_sent: Notify,
/// }
///
/// impl<T> Channel<T> {
///     pub fn send(&self, msg: T) {
///         let mut locked_queue = self.messages.lock().unwrap();
///         locked_queue.push_back(msg);
///         drop(locked_queue);
///
///         // Send a notification to one of the calls currently
///         // waiting in a call to `recv`.
///         self.notify_on_sent.notify_one();
///     }
///
///     pub fn try_recv(&self) -> Option<T> {
///         let mut locked_queue = self.messages.lock().unwrap();
///         locked_queue.pop_front()
///     }
///
///     pub async fn recv(&self) -> T {
///         let future = self.notify_on_sent.notified();
///         tokio::pin!(future);
///
///         loop {
///             // Make sure that no wakeup is lost if we get
///             // `None` from `try_recv`.
///             future.as_mut().enable();
///
///             if let Some(msg) = self.try_recv() {
///                 return msg;
///             }
///
///             // Wait for a call to `notify_one`.
///             //
///             // This uses `.as_mut()` to avoid consuming the future,
///             // which lets us call `Pin::set` below.
///             future.as_mut().await;
///
///             // Reset the future in case another call to
///             // `try_recv` got the message before us.
///             future.set(self.notify_on_sent.notified());
///         }
///     }
/// }
/// ```
///
/// [park]: std::thread::park
/// [unpark]: std::thread::Thread::unpark
/// [`notified().await`]: Notify::notified()
/// [`notify_one()`]: Notify::notify_one()
/// [`enable`]: Notified::enable()
/// [`Semaphore`]: crate::sync::Semaphore
#[derive(Debug)]
pub struct Notify {
    // `state` uses 2 bits to store one of `EMPTY`,
    // `WAITING` or `NOTIFIED`. The rest of the bits
    // are used to store the number of times `notify_waiters`
    // was called.
    //
    // Throughout the code there are two assumptions:
    // - state can be transitioned *from* `WAITING` only if
    //   `waiters` lock is held
    // - number of times `notify_waiters` was called can
    //   be modified only if `waiters` lock is held
    state: AtomicUsize,
    waiters: Mutex<WaitList>,
}

#[derive(Debug)]
struct Waiter {
    /// Intrusive linked-list pointers.
    pointers: linked_list::Pointers<Waiter>,

    /// Waiting task's waker. Depending on the value of `notification`,
    /// this field is either protected by the `waiters` lock in
    /// `Notify`, or it is exclusively owned by the enclosing `Waiter`.
    waker: UnsafeCell<Option<Waker>>,

    /// Notification for this waiter. Uses 2 bits to store if and how was
    /// notified, 1 bit for storing if it was woken up using FIFO or LIFO, and
    /// the rest of it is unused.
    /// * if it's `None`, then `waker` is protected by the `waiters` lock.
    /// * if it's `Some`, then `waker` is exclusively owned by the
    ///   enclosing `Waiter` and can be accessed without locking.
    notification: AtomicNotification,

    /// Should not be `Unpin`.
    _p: PhantomPinned,
}

impl Waiter {
    fn new() -> Waiter {
        Waiter {
            pointers: linked_list::Pointers::new(),
            waker: UnsafeCell::new(None),
            notification: AtomicNotification::none(),
            _p: PhantomPinned,
        }
    }
}

generate_addr_of_methods! {
    impl<> Waiter {
        unsafe fn addr_of_pointers(self: NonNull<Self>) -> NonNull<linked_list::Pointers<Waiter>> {
            &self.pointers
        }
    }
}

// No notification.
const NOTIFICATION_NONE: usize = 0b000;

// Notification type used by `notify_one`.
const NOTIFICATION_ONE: usize = 0b001;

// Notification type used by `notify_last`.
const NOTIFICATION_LAST: usize = 0b101;

// Notification type used by `notify_waiters`.
const NOTIFICATION_ALL: usize = 0b010;

/// Notification for a `Waiter`.
/// This struct is equivalent to `Option<Notification>`, but uses
/// `AtomicUsize` inside for atomic operations.
#[derive(Debug)]
struct AtomicNotification(AtomicUsize);

impl AtomicNotification {
    fn none() -> Self {
        AtomicNotification(AtomicUsize::new(NOTIFICATION_NONE))
    }

    /// Store-release a notification.
    /// This method should be called exactly once.
    fn store_release(&self, notification: Notification) {
        let data: usize = match notification {
            Notification::All => NOTIFICATION_ALL,
            Notification::One(NotifyOneStrategy::Fifo) => NOTIFICATION_ONE,
            Notification::One(NotifyOneStrategy::Lifo) => NOTIFICATION_LAST,
        };
        self.0.store(data, Release);
    }

    fn load(&self, ordering: Ordering) -> Option<Notification> {
        let data = self.0.load(ordering);
        match data {
            NOTIFICATION_NONE => None,
            NOTIFICATION_ONE => Some(Notification::One(NotifyOneStrategy::Fifo)),
            NOTIFICATION_LAST => Some(Notification::One(NotifyOneStrategy::Lifo)),
            NOTIFICATION_ALL => Some(Notification::All),
            _ => unreachable!(),
        }
    }

    /// Clears the notification.
    /// This method is used by a `Notified` future to consume the
    /// notification. It uses relaxed ordering and should be only
    /// used once the atomic notification is no longer shared.
    fn clear(&self) {
        self.0.store(NOTIFICATION_NONE, Relaxed);
    }
}

#[derive(Debug, PartialEq, Eq)]
#[repr(usize)]
enum NotifyOneStrategy {
    Fifo,
    Lifo,
}

#[derive(Debug, PartialEq, Eq)]
#[repr(usize)]
enum Notification {
    One(NotifyOneStrategy),
    All,
}

/// List used in `Notify::notify_waiters`. It wraps a guarded linked list
/// and gates the access to it on `notify.waiters` mutex. It also empties
/// the list on drop.
struct NotifyWaitersList<'a> {
    list: GuardedWaitList,
    is_empty: bool,
    notify: &'a Notify,
}

impl<'a> NotifyWaitersList<'a> {
    fn new(
        unguarded_list: WaitList,
        guard: Pin<&'a Waiter>,
        notify: &'a Notify,
    ) -> NotifyWaitersList<'a> {
        let guard_ptr = NonNull::from(guard.get_ref());
        let list = unguarded_list.into_guarded(guard_ptr);
        NotifyWaitersList {
            list,
            is_empty: false,
            notify,
        }
    }

    /// Removes the last element from the guarded list. Modifying this list
    /// requires an exclusive access to the main list in `Notify`.
    fn pop_back_locked(&mut self, _waiters: &mut WaitList) -> Option<NonNull<Waiter>> {
        let result = self.list.pop_back();
        if result.is_none() {
            // Save information about emptiness to avoid waiting for lock
            // in the destructor.
            self.is_empty = true;
        }
        result
    }
}

impl Drop for NotifyWaitersList<'_> {
    fn drop(&mut self) {
        // If the list is not empty, we unlink all waiters from it.
        // We do not wake the waiters to avoid double panics.
        if !self.is_empty {
            let _lock_guard = self.notify.waiters.lock();
            while let Some(waiter) = self.list.pop_back() {
                // Safety: we never make mutable references to waiters.
                let waiter = unsafe { waiter.as_ref() };
                waiter.notification.store_release(Notification::All);
            }
        }
    }
}

/// Future returned from [`Notify::notified()`].
///
/// This future is fused, so once it has completed, any future calls to poll
/// will immediately return `Poll::Ready`.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Notified<'a> {
    /// The `Notify` being received on.
    notify: &'a Notify,

    /// The current state of the receiving process.
    state: State,

    /// Number of calls to `notify_waiters` at the time of creation.
    notify_waiters_calls: usize,

    /// Entry in the waiter `LinkedList`.
    waiter: Waiter,
}

unsafe impl<'a> Send for Notified<'a> {}
unsafe impl<'a> Sync for Notified<'a> {}

/// Future returned from [`Notify::notified_owned()`].
///
/// This future is fused, so once it has completed, any future calls to poll
/// will immediately return `Poll::Ready`.
#[derive(Debug)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct OwnedNotified {
    /// The `Notify` being received on.
    notify: Arc<Notify>,

    /// The current state of the receiving process.
    state: State,

    /// Number of calls to `notify_waiters` at the time of creation.
    notify_waiters_calls: usize,

    /// Entry in the waiter `LinkedList`.
    waiter: Waiter,
}

unsafe impl Sync for OwnedNotified {}

/// A custom `project` implementation is used in place of `pin-project-lite`
/// as a custom drop for [`Notified`] and [`OwnedNotified`] implementation
/// is needed.
struct NotifiedProject<'a> {
    notify: &'a Notify,
    state: &'a mut State,
    notify_waiters_calls: &'a usize,
    waiter: &'a Waiter,
}

#[derive(Debug)]
enum State {
    Init,
    Waiting,
    Done,
}

const NOTIFY_WAITERS_SHIFT: usize = 2;
const STATE_MASK: usize = (1 << NOTIFY_WAITERS_SHIFT) - 1;
const NOTIFY_WAITERS_CALLS_MASK: usize = !STATE_MASK;

/// Initial "idle" state.
const EMPTY: usize = 0;

/// One or more threads are currently waiting to be notified.
const WAITING: usize = 1;

/// Pending notification.
const NOTIFIED: usize = 2;

fn set_state(data: usize, state: usize) -> usize {
    (data & NOTIFY_WAITERS_CALLS_MASK) | (state & STATE_MASK)
}

fn get_state(data: usize) -> usize {
    data & STATE_MASK
}

fn get_num_notify_waiters_calls(data: usize) -> usize {
    (data & NOTIFY_WAITERS_CALLS_MASK) >> NOTIFY_WAITERS_SHIFT
}

fn inc_num_notify_waiters_calls(data: usize) -> usize {
    data + (1 << NOTIFY_WAITERS_SHIFT)
}

fn atomic_inc_num_notify_waiters_calls(data: &AtomicUsize) {
    data.fetch_add(1 << NOTIFY_WAITERS_SHIFT, SeqCst);
}

impl Notify {
    /// Create a new `Notify`, initialized without a permit.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::Notify;
    ///
    /// let notify = Notify::new();
    /// ```
    pub fn new() -> Notify {
        Notify {
            state: AtomicUsize::new(0),
            waiters: Mutex::new(LinkedList::new()),
        }
    }

    /// Create a new `Notify`, initialized without a permit.
    ///
    /// When using the `tracing` [unstable feature], a `Notify` created with
    /// `const_new` will not be instrumented. As such, it will not be visible
    /// in [`tokio-console`]. Instead, [`Notify::new`] should be used to create
    /// an instrumented object if that is needed.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::Notify;
    ///
    /// static NOTIFY: Notify = Notify::const_new();
    /// ```
    ///
    /// [`tokio-console`]: https://github.com/tokio-rs/console
    /// [unstable feature]: crate#unstable-features
    #[cfg(not(all(loom, test)))]
    pub const fn const_new() -> Notify {
        Notify {
            state: AtomicUsize::new(0),
            waiters: Mutex::const_new(LinkedList::new()),
        }
    }

    /// Wait for a notification.
    ///
    /// Equivalent to:
    ///
    /// ```ignore
    /// async fn notified(&self);
    /// ```
    ///
    /// Each `Notify` value holds a single permit. If a permit is available from
    /// an earlier call to [`notify_one()`], then `notified().await` will complete
    /// immediately, consuming that permit. Otherwise, `notified().await` waits
    /// for a permit to be made available by the next call to `notify_one()`.
    ///
    /// The `Notified` future is not guaranteed to receive wakeups from calls to
    /// `notify_one()` if it has not yet been polled. See the documentation for
    /// [`Notified::enable()`] for more details.
    ///
    /// The `Notified` future is guaranteed to receive wakeups from
    /// `notify_waiters()` as soon as it has been created, even if it has not
    /// yet been polled.
    ///
    /// [`notify_one()`]: Notify::notify_one
    /// [`Notified::enable()`]: Notified::enable
    ///
    /// # Cancel safety
    ///
    /// This method uses a queue to fairly distribute notifications in the order
    /// they were requested. Cancelling a call to `notified` makes you lose your
    /// place in the queue.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::Notify;
    /// use std::sync::Arc;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let notify = Arc::new(Notify::new());
    ///     let notify2 = notify.clone();
    ///
    ///     tokio::spawn(async move {
    ///         notify2.notified().await;
    ///         println!("received notification");
    ///     });
    ///
    ///     println!("sending notification");
    ///     notify.notify_one();
    /// }
    /// ```
    pub fn notified(&self) -> Notified<'_> {
        // we load the number of times notify_waiters
        // was called and store that in the future.
        let state = self.state.load(SeqCst);
        Notified {
            notify: self,
            state: State::Init,
            notify_waiters_calls: get_num_notify_waiters_calls(state),
            waiter: Waiter::new(),
        }
    }

    /// Wait for a notification with an owned `Future`.
    ///
    /// Unlike [`Self::notified`] which returns a future tied to the `Notify`'s
    /// lifetime, `notified_owned` creates a self-contained future that owns its
    /// notification state, making it safe to move between threads.
    ///
    /// See [`Self::notified`] for more details.
    ///
    /// # Cancel safety
    ///
    /// This method uses a queue to fairly distribute notifications in the order
    /// they were requested. Cancelling a call to `notified_owned` makes you lose your
    /// place in the queue.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use tokio::sync::Notify;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let notify = Arc::new(Notify::new());
    ///
    ///     for _ in 0..10 {
    ///         let notified = notify.clone().notified_owned();
    ///         tokio::spawn(async move {
    ///             notified.await;
    ///             println!("received notification");
    ///         });
    ///     }
    ///
    ///     println!("sending notification");
    ///     notify.notify_waiters();
    /// }
    /// ```
    pub fn notified_owned(self: Arc<Self>) -> OwnedNotified {
        // we load the number of times notify_waiters
        // was called and store that in the future.
        let state = self.state.load(SeqCst);
        OwnedNotified {
            notify: self,
            state: State::Init,
            notify_waiters_calls: get_num_notify_waiters_calls(state),
            waiter: Waiter::new(),
        }
    }
    /// Notifies the first waiting task.
    ///
    /// If a task is currently waiting, that task is notified. Otherwise, a
    /// permit is stored in this `Notify` value and the **next** call to
    /// [`notified().await`] will complete immediately consuming the permit made
    /// available by this call to `notify_one()`.
    ///
    /// At most one permit may be stored by `Notify`. Many sequential calls to
    /// `notify_one` will result in a single permit being stored. The next call to
    /// `notified().await` will complete immediately, but the one after that
    /// will wait.
    ///
    /// [`notified().await`]: Notify::notified()
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::Notify;
    /// use std::sync::Arc;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let notify = Arc::new(Notify::new());
    ///     let notify2 = notify.clone();
    ///
    ///     tokio::spawn(async move {
    ///         notify2.notified().await;
    ///         println!("received notification");
    ///     });
    ///
    ///     println!("sending notification");
    ///     notify.notify_one();
    /// }
    /// ```
    // Alias for old name in 0.x
    #[cfg_attr(docsrs, doc(alias = "notify"))]
    pub fn notify_one(&self) {
        self.notify_with_strategy(NotifyOneStrategy::Fifo);
    }

    /// Notifies the last waiting task.
    ///
    /// This function behaves similar to `notify_one`. The only difference is that it wakes
    /// the most recently added waiter instead of the oldest waiter.
    ///
    /// Check the [`notify_one()`] documentation for more info and
    /// examples.
    ///
    /// [`notify_one()`]: Notify::notify_one
    pub fn notify_last(&self) {
        self.notify_with_strategy(NotifyOneStrategy::Lifo);
    }

    fn notify_with_strategy(&self, strategy: NotifyOneStrategy) {
        // Load the current state
        let mut curr = self.state.load(SeqCst);

        // If the state is `EMPTY`, transition to `NOTIFIED` and return.
        while let EMPTY | NOTIFIED = get_state(curr) {
            // The compare-exchange from `NOTIFIED` -> `NOTIFIED` is intended. A
            // happens-before synchronization must happen between this atomic
            // operation and a task calling `notified().await`.
            let new = set_state(curr, NOTIFIED);
            let res = self.state.compare_exchange(curr, new, SeqCst, SeqCst);

            match res {
                // No waiters, no further work to do
                Ok(_) => return,
                Err(actual) => {
                    curr = actual;
                }
            }
        }

        // There are waiters, the lock must be acquired to notify.
        let mut waiters = self.waiters.lock();

        // The state must be reloaded while the lock is held. The state may only
        // transition out of WAITING while the lock is held.
        curr = self.state.load(SeqCst);

        if let Some(waker) = notify_locked(&mut waiters, &self.state, curr, strategy) {
            drop(waiters);
            waker.wake();
        }
    }

    /// Notifies all waiting tasks.
    ///
    /// If a task is currently waiting, that task is notified. Unlike with
    /// `notify_one()`, no permit is stored to be used by the next call to
    /// `notified().await`. The purpose of this method is to notify all
    /// already registered waiters. Registering for notification is done by
    /// acquiring an instance of the `Notified` future via calling `notified()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::Notify;
    /// use std::sync::Arc;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let notify = Arc::new(Notify::new());
    ///     let notify2 = notify.clone();
    ///
    ///     let notified1 = notify.notified();
    ///     let notified2 = notify.notified();
    ///
    ///     let handle = tokio::spawn(async move {
    ///         println!("sending notifications");
    ///         notify2.notify_waiters();
    ///     });
    ///
    ///     notified1.await;
    ///     notified2.await;
    ///     println!("received notifications");
    /// }
    /// ```
    pub fn notify_waiters(&self) {
        let mut waiters = self.waiters.lock();

        // The state must be loaded while the lock is held. The state may only
        // transition out of WAITING while the lock is held.
        let curr = self.state.load(SeqCst);

        if matches!(get_state(curr), EMPTY | NOTIFIED) {
            // There are no waiting tasks. All we need to do is increment the
            // number of times this method was called.
            atomic_inc_num_notify_waiters_calls(&self.state);
            return;
        }

        // Increment the number of times this method was called
        // and transition to empty.
        let new_state = set_state(inc_num_notify_waiters_calls(curr), EMPTY);
        self.state.store(new_state, SeqCst);

        // It is critical for `GuardedLinkedList` safety that the guard node is
        // pinned in memory and is not dropped until the guarded list is dropped.
        let guard = Waiter::new();
        pin!(guard);

        // We move all waiters to a secondary list. It uses a `GuardedLinkedList`
        // underneath to allow every waiter to safely remove itself from it.
        //
        // * This list will be still guarded by the `waiters` lock.
        //   `NotifyWaitersList` wrapper makes sure we hold the lock to modify it.
        // * This wrapper will empty the list on drop. It is critical for safety
        //   that we will not leave any list entry with a pointer to the local
        //   guard node after this function returns / panics.
        let mut list = NotifyWaitersList::new(std::mem::take(&mut *waiters), guard.as_ref(), self);

        let mut wakers = WakeList::new();
        'outer: loop {
            while wakers.can_push() {
                match list.pop_back_locked(&mut waiters) {
                    Some(waiter) => {
                        // Safety: we never make mutable references to waiters.
                        let waiter = unsafe { waiter.as_ref() };

                        // Safety: we hold the lock, so we can access the waker.
                        if let Some(waker) =
                            unsafe { waiter.waker.with_mut(|waker| (*waker).take()) }
                        {
                            wakers.push(waker);
                        }

                        // This waiter is unlinked and will not be shared ever again, release it.
                        waiter.notification.store_release(Notification::All);
                    }
                    None => {
                        break 'outer;
                    }
                }
            }

            // Release the lock before notifying.
            drop(waiters);

            // One of the wakers may panic, but the remaining waiters will still
            // be unlinked from the list in `NotifyWaitersList` destructor.
            wakers.wake_all();

            // Acquire the lock again.
            waiters = self.waiters.lock();
        }

        // Release the lock before notifying
        drop(waiters);

        wakers.wake_all();
    }
}

impl Default for Notify {
    fn default() -> Notify {
        Notify::new()
    }
}

impl UnwindSafe for Notify {}
impl RefUnwindSafe for Notify {}

fn notify_locked(
    waiters: &mut WaitList,
    state: &AtomicUsize,
    curr: usize,
    strategy: NotifyOneStrategy,
) -> Option<Waker> {
    match get_state(curr) {
        EMPTY | NOTIFIED => {
            let res = state.compare_exchange(curr, set_state(curr, NOTIFIED), SeqCst, SeqCst);

            match res {
                Ok(_) => None,
                Err(actual) => {
                    let actual_state = get_state(actual);
                    assert!(actual_state == EMPTY || actual_state == NOTIFIED);
                    state.store(set_state(actual, NOTIFIED), SeqCst);
                    None
                }
            }
        }
        WAITING => {
            // At this point, it is guaranteed that the state will not
            // concurrently change as holding the lock is required to
            // transition **out** of `WAITING`.
            //
            // Get a pending waiter using one of the available dequeue strategies.
            let waiter = match strategy {
                NotifyOneStrategy::Fifo => waiters.pop_back().unwrap(),
                NotifyOneStrategy::Lifo => waiters.pop_front().unwrap(),
            };

            // Safety: we never make mutable references to waiters.
            let waiter = unsafe { waiter.as_ref() };

            // Safety: we hold the lock, so we can access the waker.
            let waker = unsafe { waiter.waker.with_mut(|waker| (*waker).take()) };

            // This waiter is unlinked and will not be shared ever again, release it.
            waiter
                .notification
                .store_release(Notification::One(strategy));

            if waiters.is_empty() {
                // As this the **final** waiter in the list, the state
                // must be transitioned to `EMPTY`. As transitioning
                // **from** `WAITING` requires the lock to be held, a
                // `store` is sufficient.
                state.store(set_state(curr, EMPTY), SeqCst);
            }
            waker
        }
        _ => unreachable!(),
    }
}

// ===== impl Notified =====

impl Notified<'_> {
    /// Adds this future to the list of futures that are ready to receive
    /// wakeups from calls to [`notify_one`].
    ///
    /// Polling the future also adds it to the list, so this method should only
    /// be used if you want to add the future to the list before the first call
    /// to `poll`. (In fact, this method is equivalent to calling `poll` except
    /// that no `Waker` is registered.)
    ///
    /// This has no effect on notifications sent using [`notify_waiters`], which
    /// are received as long as they happen after the creation of the `Notified`
    /// regardless of whether `enable` or `poll` has been called.
    ///
    /// This method returns true if the `Notified` is ready. This happens in the
    /// following situations:
    ///
    ///  1. The `notify_waiters` method was called between the creation of the
    ///     `Notified` and the call to this method.
    ///  2. This is the first call to `enable` or `poll` on this future, and the
    ///     `Notify` was holding a permit from a previous call to `notify_one`.
    ///     The call consumes the permit in that case.
    ///  3. The future has previously been enabled or polled, and it has since
    ///     then been marked ready by either consuming a permit from the
    ///     `Notify`, or by a call to `notify_one` or `notify_waiters` that
    ///     removed it from the list of futures ready to receive wakeups.
    ///
    /// If this method returns true, any future calls to poll on the same future
    /// will immediately return `Poll::Ready`.
    ///
    /// # Examples
    ///
    /// Unbound multi-producer multi-consumer (mpmc) channel.
    ///
    /// The call to `enable` is important because otherwise if you have two
    /// calls to `recv` and two calls to `send` in parallel, the following could
    /// happen:
    ///
    ///  1. Both calls to `try_recv` return `None`.
    ///  2. Both new elements are added to the vector.
    ///  3. The `notify_one` method is called twice, adding only a single
    ///     permit to the `Notify`.
    ///  4. Both calls to `recv` reach the `Notified` future. One of them
    ///     consumes the permit, and the other sleeps forever.
    ///
    /// By adding the `Notified` futures to the list by calling `enable` before
    /// `try_recv`, the `notify_one` calls in step three would remove the
    /// futures from the list and mark them notified instead of adding a permit
    /// to the `Notify`. This ensures that both futures are woken.
    ///
    /// ```
    /// use tokio::sync::Notify;
    ///
    /// use std::collections::VecDeque;
    /// use std::sync::Mutex;
    ///
    /// struct Channel<T> {
    ///     messages: Mutex<VecDeque<T>>,
    ///     notify_on_sent: Notify,
    /// }
    ///
    /// impl<T> Channel<T> {
    ///     pub fn send(&self, msg: T) {
    ///         let mut locked_queue = self.messages.lock().unwrap();
    ///         locked_queue.push_back(msg);
    ///         drop(locked_queue);
    ///
    ///         // Send a notification to one of the calls currently
    ///         // waiting in a call to `recv`.
    ///         self.notify_on_sent.notify_one();
    ///     }
    ///
    ///     pub fn try_recv(&self) -> Option<T> {
    ///         let mut locked_queue = self.messages.lock().unwrap();
    ///         locked_queue.pop_front()
    ///     }
    ///
    ///     pub async fn recv(&self) -> T {
    ///         let future = self.notify_on_sent.notified();
    ///         tokio::pin!(future);
    ///
    ///         loop {
    ///             // Make sure that no wakeup is lost if we get
    ///             // `None` from `try_recv`.
    ///             future.as_mut().enable();
    ///
    ///             if let Some(msg) = self.try_recv() {
    ///                 return msg;
    ///             }
    ///
    ///             // Wait for a call to `notify_one`.
    ///             //
    ///             // This uses `.as_mut()` to avoid consuming the future,
    ///             // which lets us call `Pin::set` below.
    ///             future.as_mut().await;
    ///
    ///             // Reset the future in case another call to
    ///             // `try_recv` got the message before us.
    ///             future.set(self.notify_on_sent.notified());
    ///         }
    ///     }
    /// }
    /// ```
    ///
    /// [`notify_one`]: Notify::notify_one()
    /// [`notify_waiters`]: Notify::notify_waiters()
    pub fn enable(self: Pin<&mut Self>) -> bool {
        self.poll_notified(None).is_ready()
    }

    fn project(self: Pin<&mut Self>) -> NotifiedProject<'_> {
        unsafe {
            // Safety: `notify`, `state` and `notify_waiters_calls` are `Unpin`.

            is_unpin::<&Notify>();
            is_unpin::<State>();
            is_unpin::<usize>();

            let me = self.get_unchecked_mut();
            NotifiedProject {
                notify: me.notify,
                state: &mut me.state,
                notify_waiters_calls: &me.notify_waiters_calls,
                waiter: &me.waiter,
            }
        }
    }

    fn poll_notified(self: Pin<&mut Self>, waker: Option<&Waker>) -> Poll<()> {
        self.project().poll_notified(waker)
    }
}

impl Future for Notified<'_> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        self.poll_notified(Some(cx.waker()))
    }
}

impl Drop for Notified<'_> {
    fn drop(&mut self) {
        // Safety: The type only transitions to a "Waiting" state when pinned.
        unsafe { Pin::new_unchecked(self) }
            .project()
            .drop_notified();
    }
}

// ===== impl OwnedNotified =====

impl OwnedNotified {
    /// Adds this future to the list of futures that are ready to receive
    /// wakeups from calls to [`notify_one`].
    ///
    /// See [`Notified::enable`] for more details.
    ///
    /// [`notify_one`]: Notify::notify_one()
    pub fn enable(self: Pin<&mut Self>) -> bool {
        self.poll_notified(None).is_ready()
    }

    /// A custom `project` implementation is used in place of `pin-project-lite`
    /// as a custom drop implementation is needed.
    fn project(self: Pin<&mut Self>) -> NotifiedProject<'_> {
        unsafe {
            // Safety: `notify`, `state` and `notify_waiters_calls` are `Unpin`.

            is_unpin::<&Notify>();
            is_unpin::<State>();
            is_unpin::<usize>();

            let me = self.get_unchecked_mut();
            NotifiedProject {
                notify: &me.notify,
                state: &mut me.state,
                notify_waiters_calls: &me.notify_waiters_calls,
                waiter: &me.waiter,
            }
        }
    }

    fn poll_notified(self: Pin<&mut Self>, waker: Option<&Waker>) -> Poll<()> {
        self.project().poll_notified(waker)
    }
}

impl Future for OwnedNotified {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        self.poll_notified(Some(cx.waker()))
    }
}

impl Drop for OwnedNotified {
    fn drop(&mut self) {
        // Safety: The type only transitions to a "Waiting" state when pinned.
        unsafe { Pin::new_unchecked(self) }
            .project()
            .drop_notified();
    }
}

// ===== impl NotifiedProject =====

impl NotifiedProject<'_> {
    fn poll_notified(self, waker: Option<&Waker>) -> Poll<()> {
        let NotifiedProject {
            notify,
            state,
            notify_waiters_calls,
            waiter,
        } = self;

        'outer_loop: loop {
            match *state {
                State::Init => {
                    let curr = notify.state.load(SeqCst);

                    // Optimistically try acquiring a pending notification
                    let res = notify.state.compare_exchange(
                        set_state(curr, NOTIFIED),
                        set_state(curr, EMPTY),
                        SeqCst,
                        SeqCst,
                    );

                    if res.is_ok() {
                        // Acquired the notification
                        *state = State::Done;
                        continue 'outer_loop;
                    }

                    // Clone the waker before locking, a waker clone can be
                    // triggering arbitrary code.
                    let waker = waker.cloned();

                    // Acquire the lock and attempt to transition to the waiting
                    // state.
                    let mut waiters = notify.waiters.lock();

                    // Reload the state with the lock held
                    let mut curr = notify.state.load(SeqCst);

                    // if notify_waiters has been called after the future
                    // was created, then we are done
                    if get_num_notify_waiters_calls(curr) != *notify_waiters_calls {
                        *state = State::Done;
                        continue 'outer_loop;
                    }

                    // Transition the state to WAITING.
                    loop {
                        match get_state(curr) {
                            EMPTY => {
                                // Transition to WAITING
                                let res = notify.state.compare_exchange(
                                    set_state(curr, EMPTY),
                                    set_state(curr, WAITING),
                                    SeqCst,
                                    SeqCst,
                                );

                                if let Err(actual) = res {
                                    assert_eq!(get_state(actual), NOTIFIED);
                                    curr = actual;
                                } else {
                                    break;
                                }
                            }
                            WAITING => break,
                            NOTIFIED => {
                                // Try consuming the notification
                                let res = notify.state.compare_exchange(
                                    set_state(curr, NOTIFIED),
                                    set_state(curr, EMPTY),
                                    SeqCst,
                                    SeqCst,
                                );

                                match res {
                                    Ok(_) => {
                                        // Acquired the notification
                                        *state = State::Done;
                                        continue 'outer_loop;
                                    }
                                    Err(actual) => {
                                        assert_eq!(get_state(actual), EMPTY);
                                        curr = actual;
                                    }
                                }
                            }
                            _ => unreachable!(),
                        }
                    }

                    let mut old_waker = None;
                    if waker.is_some() {
                        // Safety: called while locked.
                        //
                        // The use of `old_waiter` here is not necessary, as the field is always
                        // None when we reach this line.
                        unsafe {
                            old_waker =
                                waiter.waker.with_mut(|v| std::mem::replace(&mut *v, waker));
                        }
                    }

                    // Insert the waiter into the linked list
                    waiters.push_front(NonNull::from(waiter));

                    *state = State::Waiting;

                    drop(waiters);
                    drop(old_waker);

                    return Poll::Pending;
                }
                State::Waiting => {
                    #[cfg(tokio_taskdump)]
                    if let Some(waker) = waker {
                        let mut ctx = Context::from_waker(waker);
                        std::task::ready!(crate::trace::trace_leaf(&mut ctx));
                    }

                    if waiter.notification.load(Acquire).is_some() {
                        // Safety: waiter is already unlinked and will not be shared again,
                        // so we have an exclusive access to `waker`.
                        drop(unsafe { waiter.waker.with_mut(|waker| (*waker).take()) });

                        waiter.notification.clear();
                        *state = State::Done;
                        return Poll::Ready(());
                    }

                    // Our waiter was not notified, implying it is still stored in a waiter
                    // list (guarded by `notify.waiters`). In order to access the waker
                    // fields, we must acquire the lock.

                    let mut old_waker = None;
                    let mut waiters = notify.waiters.lock();

                    // We hold the lock and notifications are set only with the lock held,
                    // so this can be relaxed, because the happens-before relationship is
                    // established through the mutex.
                    if waiter.notification.load(Relaxed).is_some() {
                        // Safety: waiter is already unlinked and will not be shared again,
                        // so we have an exclusive access to `waker`.
                        old_waker = unsafe { waiter.waker.with_mut(|waker| (*waker).take()) };

                        waiter.notification.clear();

                        // Drop the old waker after releasing the lock.
                        drop(waiters);
                        drop(old_waker);

                        *state = State::Done;
                        return Poll::Ready(());
                    }

                    // Load the state with the lock held.
                    let curr = notify.state.load(SeqCst);

                    if get_num_notify_waiters_calls(curr) != *notify_waiters_calls {
                        // Before we add a waiter to the list we check if these numbers are
                        // different while holding the lock. If these numbers are different now,
                        // it means that there is a call to `notify_waiters` in progress and this
                        // waiter must be contained by a guarded list used in `notify_waiters`.
                        // We can treat the waiter as notified and remove it from the list, as
                        // it would have been notified in the `notify_waiters` call anyways.

                        // Safety: we hold the lock, so we can modify the waker.
                        old_waker = unsafe { waiter.waker.with_mut(|waker| (*waker).take()) };

                        // Safety: we hold the lock, so we have an exclusive access to the list.
                        // The list is used in `notify_waiters`, so it must be guarded.
                        unsafe { waiters.remove(NonNull::from(waiter)) };

                        *state = State::Done;
                    } else {
                        // Safety: we hold the lock, so we can modify the waker.
                        unsafe {
                            waiter.waker.with_mut(|v| {
                                if let Some(waker) = waker {
                                    let should_update = match &*v {
                                        Some(current_waker) => !current_waker.will_wake(waker),
                                        None => true,
                                    };
                                    if should_update {
                                        old_waker = (*v).replace(waker.clone());
                                    }
                                }
                            });
                        }

                        // Drop the old waker after releasing the lock.
                        drop(waiters);
                        drop(old_waker);

                        return Poll::Pending;
                    }

                    // Explicit drop of the lock to indicate the scope that the
                    // lock is held. Because holding the lock is required to
                    // ensure safe access to fields not held within the lock, it
                    // is helpful to visualize the scope of the critical
                    // section.
                    drop(waiters);

                    // Drop the old waker after releasing the lock.
                    drop(old_waker);
                }
                State::Done => {
                    #[cfg(tokio_taskdump)]
                    if let Some(waker) = waker {
                        let mut ctx = Context::from_waker(waker);
                        std::task::ready!(crate::trace::trace_leaf(&mut ctx));
                    }
                    return Poll::Ready(());
                }
            }
        }
    }

    fn drop_notified(self) {
        let NotifiedProject {
            notify,
            state,
            waiter,
            ..
        } = self;

        // This is where we ensure safety. The `Notified` value is being
        // dropped, which means we must ensure that the waiter entry is no
        // longer stored in the linked list.
        if matches!(*state, State::Waiting) {
            let mut waiters = notify.waiters.lock();
            let mut notify_state = notify.state.load(SeqCst);

            // We hold the lock, so this field is not concurrently accessed by
            // `notify_*` functions and we can use the relaxed ordering.
            let notification = waiter.notification.load(Relaxed);

            // remove the entry from the list (if not already removed)
            //
            // Safety: we hold the lock, so we have an exclusive access to every list the
            // waiter may be contained in. If the node is not contained in the `waiters`
            // list, then it is contained by a guarded list used by `notify_waiters`.
            unsafe { waiters.remove(NonNull::from(waiter)) };

            if waiters.is_empty() && get_state(notify_state) == WAITING {
                notify_state = set_state(notify_state, EMPTY);
                notify.state.store(notify_state, SeqCst);
            }

            // See if the node was notified but not received. In this case, if
            // the notification was triggered via `notify_one`, it must be sent
            // to the next waiter.
            if let Some(Notification::One(strategy)) = notification {
                if let Some(waker) =
                    notify_locked(&mut waiters, &notify.state, notify_state, strategy)
                {
                    drop(waiters);
                    waker.wake();
                }
            }
        }
    }
}

/// # Safety
///
/// `Waiter` is forced to be !Unpin.
unsafe impl linked_list::Link for Waiter {
    type Handle = NonNull<Waiter>;
    type Target = Waiter;

    fn as_raw(handle: &NonNull<Waiter>) -> NonNull<Waiter> {
        *handle
    }

    unsafe fn from_raw(ptr: NonNull<Waiter>) -> NonNull<Waiter> {
        ptr
    }

    unsafe fn pointers(target: NonNull<Waiter>) -> NonNull<linked_list::Pointers<Waiter>> {
        Waiter::addr_of_pointers(target)
    }
}

fn is_unpin<T: Unpin>() {}
