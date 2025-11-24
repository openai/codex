/* Copyright (c) 2010-2011 Dmitry Vyukov. All rights reserved.
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions are met:
 *
 *    1. Redistributions of source code must retain the above copyright notice,
 *       this list of conditions and the following disclaimer.
 *
 *    2. Redistributions in binary form must reproduce the above copyright
 *       notice, this list of conditions and the following disclaimer in the
 *       documentation and/or other materials provided with the distribution.
 *
 * THIS SOFTWARE IS PROVIDED BY DMITRY VYUKOV "AS IS" AND ANY EXPRESS OR IMPLIED
 * WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF
 * MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT
 * SHALL DMITRY VYUKOV OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT,
 * INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
 * LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR
 * PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF
 * LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE
 * OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF
 * ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
 *
 * The views and conclusions contained in the software and documentation are
 * those of the authors and should not be interpreted as representing official
 * policies, either expressed or implied, of Dmitry Vyukov.
 */

//! A mostly lock-free multi-producer, single consumer queue for sending
//! messages between asynchronous tasks.
//!
//! The queue implementation is essentially the same one used for mpsc channels
//! in the standard library.
//!
//! Note that the current implementation of this queue has a caveat of the `pop`
//! method, and see the method for more information about it. Due to this
//! caveat, this queue may not be appropriate for all use-cases.

// http://www.1024cores.net/home/lock-free-algorithms
//                         /queues/non-intrusive-mpsc-node-based-queue

// NOTE: this implementation is lifted from the standard library and only
//       slightly modified

pub(super) use self::PopResult::*;

use std::boxed::Box;
use std::cell::UnsafeCell;
use std::ptr;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::thread;

/// A result of the `pop` function.
pub(super) enum PopResult<T> {
    /// Some data has been popped
    Data(T),
    /// The queue is empty
    Empty,
    /// The queue is in an inconsistent state. Popping data should succeed, but
    /// some pushers have yet to make enough progress in order allow a pop to
    /// succeed. It is recommended that a pop() occur "in the near future" in
    /// order to see if the sender has made progress or not
    Inconsistent,
}

struct Node<T> {
    next: AtomicPtr<Self>,
    value: Option<T>,
}

/// The multi-producer single-consumer structure. This is not cloneable, but it
/// may be safely shared so long as it is guaranteed that there is only one
/// popper at a time (many pushers are allowed).
pub(super) struct Queue<T> {
    head: AtomicPtr<Node<T>>,
    tail: UnsafeCell<*mut Node<T>>,
}

unsafe impl<T: Send> Send for Queue<T> {}
unsafe impl<T: Send> Sync for Queue<T> {}

impl<T> Node<T> {
    unsafe fn new(v: Option<T>) -> *mut Self {
        Box::into_raw(Box::new(Self { next: AtomicPtr::new(ptr::null_mut()), value: v }))
    }
}

impl<T> Queue<T> {
    /// Creates a new queue that is safe to share among multiple producers and
    /// one consumer.
    pub(super) fn new() -> Self {
        let stub = unsafe { Node::new(None) };
        Self { head: AtomicPtr::new(stub), tail: UnsafeCell::new(stub) }
    }

    /// Pushes a new value onto this queue.
    pub(super) fn push(&self, t: T) {
        unsafe {
            let n = Node::new(Some(t));
            let prev = self.head.swap(n, Ordering::AcqRel);
            (*prev).next.store(n, Ordering::Release);
        }
    }

    /// Pops some data from this queue.
    ///
    /// Note that the current implementation means that this function cannot
    /// return `Option<T>`. It is possible for this queue to be in an
    /// inconsistent state where many pushes have succeeded and completely
    /// finished, but pops cannot return `Some(t)`. This inconsistent state
    /// happens when a pusher is preempted at an inopportune moment.
    ///
    /// This inconsistent state means that this queue does indeed have data, but
    /// it does not currently have access to it at this time.
    ///
    /// This function is unsafe because only one thread can call it at a time.
    pub(super) unsafe fn pop(&self) -> PopResult<T> {
        unsafe {
            let tail = *self.tail.get();
            let next = (*tail).next.load(Ordering::Acquire);

            if !next.is_null() {
                *self.tail.get() = next;
                assert!((*tail).value.is_none());
                assert!((*next).value.is_some());
                let ret = (*next).value.take().unwrap();
                drop(Box::from_raw(tail));
                return Data(ret);
            }

            if self.head.load(Ordering::Acquire) == tail {
                Empty
            } else {
                Inconsistent
            }
        }
    }

    /// Pop an element similarly to `pop` function, but spin-wait on inconsistent
    /// queue state instead of returning `Inconsistent`.
    ///
    /// This function is unsafe because only one thread can call it at a time.
    pub(super) unsafe fn pop_spin(&self) -> Option<T> {
        loop {
            match unsafe { self.pop() } {
                Empty => return None,
                Data(t) => return Some(t),
                // Inconsistent means that there will be a message to pop
                // in a short time. This branch can only be reached if
                // values are being produced from another thread, so there
                // are a few ways that we can deal with this:
                //
                // 1) Spin
                // 2) thread::yield_now()
                // 3) task::current().unwrap() & return Pending
                //
                // For now, thread::yield_now() is used, but it would
                // probably be better to spin a few times then yield.
                Inconsistent => {
                    thread::yield_now();
                }
            }
        }
    }
}

impl<T> Drop for Queue<T> {
    fn drop(&mut self) {
        unsafe {
            let mut cur = *self.tail.get();
            while !cur.is_null() {
                let next = (*cur).next.load(Ordering::Relaxed);
                drop(Box::from_raw(cur));
                cur = next;
            }
        }
    }
}
