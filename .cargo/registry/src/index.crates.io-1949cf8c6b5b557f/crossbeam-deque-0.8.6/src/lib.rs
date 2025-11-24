//! Concurrent work-stealing deques.
//!
//! These data structures are most commonly used in work-stealing schedulers. The typical setup
//! involves a number of threads, each having its own FIFO or LIFO queue (*worker*). There is also
//! one global FIFO queue (*injector*) and a list of references to *worker* queues that are able to
//! steal tasks (*stealers*).
//!
//! We spawn a new task onto the scheduler by pushing it into the *injector* queue. Each worker
//! thread waits in a loop until it finds the next task to run and then runs it. To find a task, it
//! first looks into its local *worker* queue, and then into the *injector* and *stealers*.
//!
//! # Queues
//!
//! [`Injector`] is a FIFO queue, where tasks are pushed and stolen from opposite ends. It is
//! shared among threads and is usually the entry point for new tasks.
//!
//! [`Worker`] has two constructors:
//!
//! * [`new_fifo()`] - Creates a FIFO queue, in which tasks are pushed and popped from opposite
//!   ends.
//! * [`new_lifo()`] - Creates a LIFO queue, in which tasks are pushed and popped from the same
//!   end.
//!
//! Each [`Worker`] is owned by a single thread and supports only push and pop operations.
//!
//! Method [`stealer()`] creates a [`Stealer`] that may be shared among threads and can only steal
//! tasks from its [`Worker`]. Tasks are stolen from the end opposite to where they get pushed.
//!
//! # Stealing
//!
//! Steal operations come in three flavors:
//!
//! 1. [`steal()`] - Steals one task.
//! 2. [`steal_batch()`] - Steals a batch of tasks and moves them into another worker.
//! 3. [`steal_batch_and_pop()`] - Steals a batch of tasks, moves them into another queue, and pops
//!    one task from that worker.
//!
//! In contrast to push and pop operations, stealing can spuriously fail with [`Steal::Retry`], in
//! which case the steal operation needs to be retried.
//!
//! # Examples
//!
//! Suppose a thread in a work-stealing scheduler is idle and looking for the next task to run. To
//! find an available task, it might do the following:
//!
//! 1. Try popping one task from the local worker queue.
//! 2. Try stealing a batch of tasks from the global injector queue.
//! 3. Try stealing one task from another thread using the stealer list.
//!
//! An implementation of this work-stealing strategy:
//!
//! ```
//! use crossbeam_deque::{Injector, Stealer, Worker};
//! use std::iter;
//!
//! fn find_task<T>(
//!     local: &Worker<T>,
//!     global: &Injector<T>,
//!     stealers: &[Stealer<T>],
//! ) -> Option<T> {
//!     // Pop a task from the local queue, if not empty.
//!     local.pop().or_else(|| {
//!         // Otherwise, we need to look for a task elsewhere.
//!         iter::repeat_with(|| {
//!             // Try stealing a batch of tasks from the global queue.
//!             global.steal_batch_and_pop(local)
//!                 // Or try stealing a task from one of the other threads.
//!                 .or_else(|| stealers.iter().map(|s| s.steal()).collect())
//!         })
//!         // Loop while no task was stolen and any steal operation needs to be retried.
//!         .find(|s| !s.is_retry())
//!         // Extract the stolen task, if there is one.
//!         .and_then(|s| s.success())
//!     })
//! }
//! ```
//!
//! [`new_fifo()`]: Worker::new_fifo
//! [`new_lifo()`]: Worker::new_lifo
//! [`stealer()`]: Worker::stealer
//! [`steal()`]: Stealer::steal
//! [`steal_batch()`]: Stealer::steal_batch
//! [`steal_batch_and_pop()`]: Stealer::steal_batch_and_pop

#![no_std]
#![doc(test(
    no_crate_inject,
    attr(
        deny(warnings, rust_2018_idioms),
        allow(dead_code, unused_assignments, unused_variables)
    )
))]
#![warn(
    missing_docs,
    missing_debug_implementations,
    rust_2018_idioms,
    unreachable_pub
)]

#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "std")]
mod deque;
#[cfg(feature = "std")]
pub use crate::deque::{Injector, Steal, Stealer, Worker};
