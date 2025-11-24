//! Waking mechanism for threads blocked on channel operations.

use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::thread::{self, ThreadId};
use std::vec::Vec;

use crate::context::Context;
use crate::select::{Operation, Selected};

/// Represents a thread blocked on a specific channel operation.
pub(crate) struct Entry {
    /// The operation.
    pub(crate) oper: Operation,

    /// Optional packet.
    pub(crate) packet: *mut (),

    /// Context associated with the thread owning this operation.
    pub(crate) cx: Context,
}

/// A queue of threads blocked on channel operations.
///
/// This data structure is used by threads to register blocking operations and get woken up once
/// an operation becomes ready.
pub(crate) struct Waker {
    /// A list of select operations.
    selectors: Vec<Entry>,

    /// A list of operations waiting to be ready.
    observers: Vec<Entry>,
}

impl Waker {
    /// Creates a new `Waker`.
    #[inline]
    pub(crate) fn new() -> Self {
        Waker {
            selectors: Vec::new(),
            observers: Vec::new(),
        }
    }

    /// Registers a select operation.
    #[inline]
    pub(crate) fn register(&mut self, oper: Operation, cx: &Context) {
        self.register_with_packet(oper, ptr::null_mut(), cx);
    }

    /// Registers a select operation and a packet.
    #[inline]
    pub(crate) fn register_with_packet(&mut self, oper: Operation, packet: *mut (), cx: &Context) {
        self.selectors.push(Entry {
            oper,
            packet,
            cx: cx.clone(),
        });
    }

    /// Unregisters a select operation.
    #[inline]
    pub(crate) fn unregister(&mut self, oper: Operation) -> Option<Entry> {
        if let Some((i, _)) = self
            .selectors
            .iter()
            .enumerate()
            .find(|&(_, entry)| entry.oper == oper)
        {
            let entry = self.selectors.remove(i);
            Some(entry)
        } else {
            None
        }
    }

    /// Attempts to find another thread's entry, select the operation, and wake it up.
    #[inline]
    pub(crate) fn try_select(&mut self) -> Option<Entry> {
        if self.selectors.is_empty() {
            None
        } else {
            let thread_id = current_thread_id();

            self.selectors
                .iter()
                .position(|selector| {
                    // Does the entry belong to a different thread?
                    selector.cx.thread_id() != thread_id
                        && selector // Try selecting this operation.
                            .cx
                            .try_select(Selected::Operation(selector.oper))
                            .is_ok()
                        && {
                            // Provide the packet.
                            selector.cx.store_packet(selector.packet);
                            // Wake the thread up.
                            selector.cx.unpark();
                            true
                        }
                })
                // Remove the entry from the queue to keep it clean and improve
                // performance.
                .map(|pos| self.selectors.remove(pos))
        }
    }

    /// Returns `true` if there is an entry which can be selected by the current thread.
    #[inline]
    pub(crate) fn can_select(&self) -> bool {
        if self.selectors.is_empty() {
            false
        } else {
            let thread_id = current_thread_id();

            self.selectors.iter().any(|entry| {
                entry.cx.thread_id() != thread_id && entry.cx.selected() == Selected::Waiting
            })
        }
    }

    /// Registers an operation waiting to be ready.
    #[inline]
    pub(crate) fn watch(&mut self, oper: Operation, cx: &Context) {
        self.observers.push(Entry {
            oper,
            packet: ptr::null_mut(),
            cx: cx.clone(),
        });
    }

    /// Unregisters an operation waiting to be ready.
    #[inline]
    pub(crate) fn unwatch(&mut self, oper: Operation) {
        self.observers.retain(|e| e.oper != oper);
    }

    /// Notifies all operations waiting to be ready.
    #[inline]
    pub(crate) fn notify(&mut self) {
        for entry in self.observers.drain(..) {
            if entry.cx.try_select(Selected::Operation(entry.oper)).is_ok() {
                entry.cx.unpark();
            }
        }
    }

    /// Notifies all registered operations that the channel is disconnected.
    #[inline]
    pub(crate) fn disconnect(&mut self) {
        for entry in self.selectors.iter() {
            if entry.cx.try_select(Selected::Disconnected).is_ok() {
                // Wake the thread up.
                //
                // Here we don't remove the entry from the queue. Registered threads must
                // unregister from the waker by themselves. They might also want to recover the
                // packet value and destroy it, if necessary.
                entry.cx.unpark();
            }
        }

        self.notify();
    }
}

impl Drop for Waker {
    #[inline]
    fn drop(&mut self) {
        debug_assert_eq!(self.selectors.len(), 0);
        debug_assert_eq!(self.observers.len(), 0);
    }
}

/// A waker that can be shared among threads without locking.
///
/// This is a simple wrapper around `Waker` that internally uses a mutex for synchronization.
pub(crate) struct SyncWaker {
    /// The inner `Waker`.
    inner: Mutex<Waker>,

    /// `true` if the waker is empty.
    is_empty: AtomicBool,
}

impl SyncWaker {
    /// Creates a new `SyncWaker`.
    #[inline]
    pub(crate) fn new() -> Self {
        SyncWaker {
            inner: Mutex::new(Waker::new()),
            is_empty: AtomicBool::new(true),
        }
    }

    /// Registers the current thread with an operation.
    #[inline]
    pub(crate) fn register(&self, oper: Operation, cx: &Context) {
        let mut inner = self.inner.lock().unwrap();
        inner.register(oper, cx);
        self.is_empty.store(
            inner.selectors.is_empty() && inner.observers.is_empty(),
            Ordering::SeqCst,
        );
    }

    /// Unregisters an operation previously registered by the current thread.
    #[inline]
    pub(crate) fn unregister(&self, oper: Operation) -> Option<Entry> {
        let mut inner = self.inner.lock().unwrap();
        let entry = inner.unregister(oper);
        self.is_empty.store(
            inner.selectors.is_empty() && inner.observers.is_empty(),
            Ordering::SeqCst,
        );
        entry
    }

    /// Attempts to find one thread (not the current one), select its operation, and wake it up.
    #[inline]
    pub(crate) fn notify(&self) {
        if !self.is_empty.load(Ordering::SeqCst) {
            let mut inner = self.inner.lock().unwrap();
            if !self.is_empty.load(Ordering::SeqCst) {
                inner.try_select();
                inner.notify();
                self.is_empty.store(
                    inner.selectors.is_empty() && inner.observers.is_empty(),
                    Ordering::SeqCst,
                );
            }
        }
    }

    /// Registers an operation waiting to be ready.
    #[inline]
    pub(crate) fn watch(&self, oper: Operation, cx: &Context) {
        let mut inner = self.inner.lock().unwrap();
        inner.watch(oper, cx);
        self.is_empty.store(
            inner.selectors.is_empty() && inner.observers.is_empty(),
            Ordering::SeqCst,
        );
    }

    /// Unregisters an operation waiting to be ready.
    #[inline]
    pub(crate) fn unwatch(&self, oper: Operation) {
        let mut inner = self.inner.lock().unwrap();
        inner.unwatch(oper);
        self.is_empty.store(
            inner.selectors.is_empty() && inner.observers.is_empty(),
            Ordering::SeqCst,
        );
    }

    /// Notifies all threads that the channel is disconnected.
    #[inline]
    pub(crate) fn disconnect(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.disconnect();
        self.is_empty.store(
            inner.selectors.is_empty() && inner.observers.is_empty(),
            Ordering::SeqCst,
        );
    }
}

impl Drop for SyncWaker {
    #[inline]
    fn drop(&mut self) {
        debug_assert!(self.is_empty.load(Ordering::SeqCst));
    }
}

/// Returns the id of the current thread.
#[inline]
fn current_thread_id() -> ThreadId {
    std::thread_local! {
        /// Cached thread-local id.
        static THREAD_ID: ThreadId = thread::current().id();
    }

    THREAD_ID
        .try_with(|id| *id)
        .unwrap_or_else(|_| thread::current().id())
}
