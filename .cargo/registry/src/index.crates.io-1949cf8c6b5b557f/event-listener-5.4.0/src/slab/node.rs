//! An operation that can be delayed.

//! The node that makes up queues.

use crate::notify::{GenericNotify, Internal, NotificationPrivate, TagProducer};
use crate::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
use crate::sync::Arc;
use crate::sys::ListenerSlab;
use crate::{State, Task};

use alloc::boxed::Box;

use core::fmt;
use core::marker::PhantomData;
use core::mem;
use core::num::NonZeroUsize;
use core::ptr;

pub(crate) struct NothingProducer<T>(PhantomData<T>);

impl<T> Default for NothingProducer<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<T> fmt::Debug for NothingProducer<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NothingProducer").finish()
    }
}

impl<T> TagProducer for NothingProducer<T> {
    type Tag = T;

    fn next_tag(&mut self) -> Self::Tag {
        // This has to be a zero-sized type with no drop handler.
        assert_eq!(mem::size_of::<Self::Tag>(), 0);
        assert!(!mem::needs_drop::<Self::Tag>());

        // SAFETY: As this is a ZST without a drop handler, zero is valid.
        unsafe { mem::zeroed() }
    }
}

/// A node in the backup queue.
pub(crate) enum Node<T> {
    /// This node is requesting to add a listener.
    // For some reason, the MSRV build says this variant is never constructed.
    #[allow(dead_code)]
    AddListener {
        /// The state of the listener that wants to be added.
        task_waiting: Arc<TaskWaiting>,
    },

    /// This node is notifying a listener.
    Notify(GenericNotify<NothingProducer<T>>),

    /// This node is removing a listener.
    RemoveListener {
        /// The ID of the listener to remove.
        listener: NonZeroUsize,

        /// Whether to propagate notifications to the next listener.
        propagate: bool,
    },

    /// We are waiting for the mutex to lock, so they can manipulate it.
    Waiting(Task),
}

impl<T> fmt::Debug for Node<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AddListener { .. } => f.write_str("AddListener"),
            Self::Notify(notify) => f
                .debug_struct("Notify")
                .field("count", &notify.count(Internal::new()))
                .field("is_additional", &notify.is_additional(Internal::new()))
                .finish(),
            Self::RemoveListener {
                listener,
                propagate,
            } => f
                .debug_struct("RemoveListener")
                .field("listener", listener)
                .field("propagate", propagate)
                .finish(),
            Self::Waiting(_) => f.write_str("Waiting"),
        }
    }
}

#[derive(Debug)]
pub(crate) struct TaskWaiting {
    /// The task that is being waited on.
    task: AtomicCell<Task>,

    /// The ID of the new entry.
    ///
    /// This is set to zero when the task is still queued, or usize::MAX when the node should not
    /// be added at all.
    entry_id: AtomicUsize,
}

impl<T> Node<T> {
    pub(crate) fn listener() -> (Self, Arc<TaskWaiting>) {
        // Create a new `TaskWaiting` structure.
        let task_waiting = Arc::new(TaskWaiting {
            task: AtomicCell::new(),
            entry_id: AtomicUsize::new(0),
        });

        (
            Self::AddListener {
                task_waiting: task_waiting.clone(),
            },
            task_waiting,
        )
    }

    /// Apply the node to the list.
    pub(super) fn apply(self, list: &mut ListenerSlab<T>) -> Option<Task> {
        match self {
            Node::AddListener { task_waiting } => {
                // If we're cancelled, do nothing.
                if task_waiting.entry_id.load(Ordering::Relaxed) == usize::MAX {
                    return task_waiting.task.take().map(|t| *t);
                }

                // Add a new entry to the list.
                let key = list.insert(State::Created);
                assert!(key.get() != usize::MAX);

                // Send the new key to the listener and wake it if necessary.
                let old_value = task_waiting.entry_id.swap(key.get(), Ordering::Release);

                // If we're cancelled, remove ourselves from the list.
                if old_value == usize::MAX {
                    list.remove(key, false);
                }

                return task_waiting.task.take().map(|t| *t);
            }
            Node::Notify(notify) => {
                // Notify the next `count` listeners.
                list.notify(notify);
            }
            Node::RemoveListener {
                listener,
                propagate,
            } => {
                // Remove the listener from the list.
                list.remove(listener, propagate);
            }
            Node::Waiting(task) => {
                return Some(task);
            }
        }

        None
    }
}

impl TaskWaiting {
    /// Determine if we are still queued.
    ///
    /// Returns `Some` with the entry ID if we are no longer queued.
    pub(crate) fn status(&self) -> Option<NonZeroUsize> {
        NonZeroUsize::new(self.entry_id.load(Ordering::Acquire))
    }

    /// Register a listener.
    pub(crate) fn register(&self, task: Task) {
        // Set the task.
        if let Some(task) = self.task.replace(Some(Box::new(task))) {
            task.wake();
        }

        // If the entry ID is non-zero, then we are no longer queued.
        if self.status().is_some() {
            // Wake the task.
            if let Some(task) = self.task.take() {
                task.wake();
            }
        }
    }

    /// Mark this listener as cancelled, indicating that it should not be inserted into the list.
    ///
    /// If this listener was already inserted into the list, returns the entry ID. Otherwise returns
    /// `None`.
    pub(crate) fn cancel(&self) -> Option<NonZeroUsize> {
        // Set the entry ID to usize::MAX.
        let id = self.entry_id.swap(usize::MAX, Ordering::Release);

        // Wake the task.
        if let Some(task) = self.task.take() {
            task.wake();
        }

        // Return the entry ID if we were queued.
        NonZeroUsize::new(id)
    }
}

/// A shared pointer to a value.
///
/// The inner value is a `Box<T>`.
#[derive(Debug)]
struct AtomicCell<T>(AtomicPtr<T>);

impl<T> AtomicCell<T> {
    /// Create a new `AtomicCell`.
    fn new() -> Self {
        Self(AtomicPtr::new(ptr::null_mut()))
    }

    /// Swap the value out.
    fn replace(&self, value: Option<Box<T>>) -> Option<Box<T>> {
        let old_value = match value {
            Some(value) => self.0.swap(Box::into_raw(value), Ordering::AcqRel),
            // Acquire is needed to synchronize with the store of a non-null ptr, but since a null ptr
            // will never be dereferenced, there is no need to synchronize the store of a null ptr.
            None => self.0.swap(ptr::null_mut(), Ordering::Acquire),
        };

        if old_value.is_null() {
            None
        } else {
            // SAFETY:
            // - AcqRel/Acquire ensures that it does not read a pointer to potentially invalid memory.
            // - We've checked that old_value is not null.
            // - We do not store invalid pointers other than null in self.0.
            Some(unsafe { Box::from_raw(old_value) })
        }
    }

    /// Take the value out.
    fn take(&self) -> Option<Box<T>> {
        self.replace(None)
    }
}

impl<T> Drop for AtomicCell<T> {
    fn drop(&mut self) {
        self.take();
    }
}
