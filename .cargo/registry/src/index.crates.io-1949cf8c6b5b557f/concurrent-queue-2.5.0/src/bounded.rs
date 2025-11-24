use alloc::{boxed::Box, vec::Vec};
use core::mem::MaybeUninit;

use crossbeam_utils::CachePadded;

use crate::sync::atomic::{AtomicUsize, Ordering};
use crate::sync::cell::UnsafeCell;
#[allow(unused_imports)]
use crate::sync::prelude::*;
use crate::{busy_wait, ForcePushError, PopError, PushError};

/// A slot in a queue.
struct Slot<T> {
    /// The current stamp.
    stamp: AtomicUsize,

    /// The value in this slot.
    value: UnsafeCell<MaybeUninit<T>>,
}

/// A bounded queue.
pub struct Bounded<T> {
    /// The head of the queue.
    ///
    /// This value is a "stamp" consisting of an index into the buffer, a mark bit, and a lap, but
    /// packed into a single `usize`. The lower bits represent the index, while the upper bits
    /// represent the lap. The mark bit in the head is always zero.
    ///
    /// Values are popped from the head of the queue.
    head: CachePadded<AtomicUsize>,

    /// The tail of the queue.
    ///
    /// This value is a "stamp" consisting of an index into the buffer, a mark bit, and a lap, but
    /// packed into a single `usize`. The lower bits represent the index, while the upper bits
    /// represent the lap. The mark bit indicates that the queue is closed.
    ///
    /// Values are pushed into the tail of the queue.
    tail: CachePadded<AtomicUsize>,

    /// The buffer holding slots.
    buffer: Box<[Slot<T>]>,

    /// A stamp with the value of `{ lap: 1, mark: 0, index: 0 }`.
    one_lap: usize,

    /// If this bit is set in the tail, that means the queue is closed.
    mark_bit: usize,
}

impl<T> Bounded<T> {
    /// Creates a new bounded queue.
    pub fn new(cap: usize) -> Bounded<T> {
        assert!(cap > 0, "capacity must be positive");

        // Head is initialized to `{ lap: 0, mark: 0, index: 0 }`.
        let head = 0;
        // Tail is initialized to `{ lap: 0, mark: 0, index: 0 }`.
        let tail = 0;

        // Allocate a buffer of `cap` slots initialized with stamps.
        let mut buffer = Vec::with_capacity(cap);
        for i in 0..cap {
            // Set the stamp to `{ lap: 0, mark: 0, index: i }`.
            buffer.push(Slot {
                stamp: AtomicUsize::new(i),
                value: UnsafeCell::new(MaybeUninit::uninit()),
            });
        }

        // Compute constants `mark_bit` and `one_lap`.
        let mark_bit = (cap + 1).next_power_of_two();
        let one_lap = mark_bit * 2;

        Bounded {
            buffer: buffer.into(),
            one_lap,
            mark_bit,
            head: CachePadded::new(AtomicUsize::new(head)),
            tail: CachePadded::new(AtomicUsize::new(tail)),
        }
    }

    /// Attempts to push an item into the queue.
    pub fn push(&self, value: T) -> Result<(), PushError<T>> {
        self.push_or_else(value, |value, tail, _, _| {
            let head = self.head.load(Ordering::Relaxed);

            // If the head lags one lap behind the tail as well...
            if head.wrapping_add(self.one_lap) == tail {
                // ...then the queue is full.
                Err(PushError::Full(value))
            } else {
                Ok(value)
            }
        })
    }

    /// Pushes an item into the queue, displacing another item if needed.
    pub fn force_push(&self, value: T) -> Result<Option<T>, ForcePushError<T>> {
        let result = self.push_or_else(value, |value, tail, new_tail, slot| {
            let head = tail.wrapping_sub(self.one_lap);
            let new_head = new_tail.wrapping_sub(self.one_lap);

            // Try to move the head.
            if self
                .head
                .compare_exchange_weak(head, new_head, Ordering::SeqCst, Ordering::Relaxed)
                .is_ok()
            {
                // Move the tail.
                self.tail.store(new_tail, Ordering::SeqCst);

                // Swap out the old value.
                // SAFETY: We know this is initialized, since it's covered by the current queue.
                let old = unsafe {
                    slot.value
                        .with_mut(|slot| slot.replace(MaybeUninit::new(value)).assume_init())
                };

                // Update the stamp.
                slot.stamp.store(tail + 1, Ordering::Release);

                // Return a PushError.
                Err(PushError::Full(old))
            } else {
                Ok(value)
            }
        });

        match result {
            Ok(()) => Ok(None),
            Err(PushError::Full(old_value)) => Ok(Some(old_value)),
            Err(PushError::Closed(value)) => Err(ForcePushError(value)),
        }
    }

    /// Attempts to push an item into the queue, running a closure on failure.
    ///
    /// `fail` is run when there is no more room left in the tail of the queue. The parameters of
    /// this function are as follows:
    ///
    /// - The item that failed to push.
    /// - The value of `self.tail` before the new value would be inserted.
    /// - The value of `self.tail` after the new value would be inserted.
    /// - The slot that we attempted to push into.
    ///
    /// If `fail` returns `Ok(val)`, we will try pushing `val` to the head of the queue. Otherwise,
    /// this function will return the error.
    fn push_or_else<F>(&self, mut value: T, mut fail: F) -> Result<(), PushError<T>>
    where
        F: FnMut(T, usize, usize, &Slot<T>) -> Result<T, PushError<T>>,
    {
        let mut tail = self.tail.load(Ordering::Relaxed);

        loop {
            // Check if the queue is closed.
            if tail & self.mark_bit != 0 {
                return Err(PushError::Closed(value));
            }

            // Deconstruct the tail.
            let index = tail & (self.mark_bit - 1);
            let lap = tail & !(self.one_lap - 1);

            // Calculate the new location of the tail.
            let new_tail = if index + 1 < self.buffer.len() {
                // Same lap, incremented index.
                // Set to `{ lap: lap, mark: 0, index: index + 1 }`.
                tail + 1
            } else {
                // One lap forward, index wraps around to zero.
                // Set to `{ lap: lap.wrapping_add(1), mark: 0, index: 0 }`.
                lap.wrapping_add(self.one_lap)
            };

            // Inspect the corresponding slot.
            let slot = &self.buffer[index];
            let stamp = slot.stamp.load(Ordering::Acquire);

            // If the tail and the stamp match, we may attempt to push.
            if tail == stamp {
                // Try moving the tail.
                match self.tail.compare_exchange_weak(
                    tail,
                    new_tail,
                    Ordering::SeqCst,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => {
                        // Write the value into the slot and update the stamp.
                        slot.value.with_mut(|slot| unsafe {
                            slot.write(MaybeUninit::new(value));
                        });
                        slot.stamp.store(tail + 1, Ordering::Release);
                        return Ok(());
                    }
                    Err(t) => {
                        tail = t;
                    }
                }
            } else if stamp.wrapping_add(self.one_lap) == tail + 1 {
                crate::full_fence();

                // We've failed to push; run our failure closure.
                value = fail(value, tail, new_tail, slot)?;

                // Loom complains if there isn't an explicit busy wait here.
                #[cfg(loom)]
                busy_wait();

                tail = self.tail.load(Ordering::Relaxed);
            } else {
                // Yield because we need to wait for the stamp to get updated.
                busy_wait();
                tail = self.tail.load(Ordering::Relaxed);
            }
        }
    }

    /// Attempts to pop an item from the queue.
    pub fn pop(&self) -> Result<T, PopError> {
        let mut head = self.head.load(Ordering::Relaxed);

        loop {
            // Deconstruct the head.
            let index = head & (self.mark_bit - 1);
            let lap = head & !(self.one_lap - 1);

            // Inspect the corresponding slot.
            let slot = &self.buffer[index];
            let stamp = slot.stamp.load(Ordering::Acquire);

            // If the the stamp is ahead of the head by 1, we may attempt to pop.
            if head + 1 == stamp {
                let new = if index + 1 < self.buffer.len() {
                    // Same lap, incremented index.
                    // Set to `{ lap: lap, mark: 0, index: index + 1 }`.
                    head + 1
                } else {
                    // One lap forward, index wraps around to zero.
                    // Set to `{ lap: lap.wrapping_add(1), mark: 0, index: 0 }`.
                    lap.wrapping_add(self.one_lap)
                };

                // Try moving the head.
                match self.head.compare_exchange_weak(
                    head,
                    new,
                    Ordering::SeqCst,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => {
                        // Read the value from the slot and update the stamp.
                        let value = slot
                            .value
                            .with_mut(|slot| unsafe { slot.read().assume_init() });
                        slot.stamp
                            .store(head.wrapping_add(self.one_lap), Ordering::Release);
                        return Ok(value);
                    }
                    Err(h) => {
                        head = h;
                    }
                }
            } else if stamp == head {
                crate::full_fence();
                let tail = self.tail.load(Ordering::Relaxed);

                // If the tail equals the head, that means the queue is empty.
                if (tail & !self.mark_bit) == head {
                    // Check if the queue is closed.
                    if tail & self.mark_bit != 0 {
                        return Err(PopError::Closed);
                    } else {
                        return Err(PopError::Empty);
                    }
                }

                // Loom complains if there isn't a busy-wait here.
                #[cfg(loom)]
                busy_wait();

                head = self.head.load(Ordering::Relaxed);
            } else {
                // Yield because we need to wait for the stamp to get updated.
                busy_wait();
                head = self.head.load(Ordering::Relaxed);
            }
        }
    }

    /// Returns the number of items in the queue.
    pub fn len(&self) -> usize {
        loop {
            // Load the tail, then load the head.
            let tail = self.tail.load(Ordering::SeqCst);
            let head = self.head.load(Ordering::SeqCst);

            // If the tail didn't change, we've got consistent values to work with.
            if self.tail.load(Ordering::SeqCst) == tail {
                let hix = head & (self.mark_bit - 1);
                let tix = tail & (self.mark_bit - 1);

                return if hix < tix {
                    tix - hix
                } else if hix > tix {
                    self.buffer.len() - hix + tix
                } else if (tail & !self.mark_bit) == head {
                    0
                } else {
                    self.buffer.len()
                };
            }
        }
    }

    /// Returns `true` if the queue is empty.
    pub fn is_empty(&self) -> bool {
        let head = self.head.load(Ordering::SeqCst);
        let tail = self.tail.load(Ordering::SeqCst);

        // Is the tail equal to the head?
        //
        // Note: If the head changes just before we load the tail, that means there was a moment
        // when the queue was not empty, so it is safe to just return `false`.
        (tail & !self.mark_bit) == head
    }

    /// Returns `true` if the queue is full.
    pub fn is_full(&self) -> bool {
        let tail = self.tail.load(Ordering::SeqCst);
        let head = self.head.load(Ordering::SeqCst);

        // Is the head lagging one lap behind tail?
        //
        // Note: If the tail changes just before we load the head, that means there was a moment
        // when the queue was not full, so it is safe to just return `false`.
        head.wrapping_add(self.one_lap) == tail & !self.mark_bit
    }

    /// Returns the capacity of the queue.
    pub fn capacity(&self) -> usize {
        self.buffer.len()
    }

    /// Closes the queue.
    ///
    /// Returns `true` if this call closed the queue.
    pub fn close(&self) -> bool {
        let tail = self.tail.fetch_or(self.mark_bit, Ordering::SeqCst);
        tail & self.mark_bit == 0
    }

    /// Returns `true` if the queue is closed.
    pub fn is_closed(&self) -> bool {
        self.tail.load(Ordering::SeqCst) & self.mark_bit != 0
    }
}

impl<T> Drop for Bounded<T> {
    fn drop(&mut self) {
        // Get the index of the head.
        let Self {
            head,
            tail,
            buffer,
            mark_bit,
            ..
        } = self;

        let mark_bit = *mark_bit;

        head.with_mut(|&mut head| {
            tail.with_mut(|&mut tail| {
                let hix = head & (mark_bit - 1);
                let tix = tail & (mark_bit - 1);

                let len = if hix < tix {
                    tix - hix
                } else if hix > tix {
                    buffer.len() - hix + tix
                } else if (tail & !mark_bit) == head {
                    0
                } else {
                    buffer.len()
                };

                // Loop over all slots that hold a value and drop them.
                for i in 0..len {
                    // Compute the index of the next slot holding a value.
                    let index = if hix + i < buffer.len() {
                        hix + i
                    } else {
                        hix + i - buffer.len()
                    };

                    // Drop the value in the slot.
                    let slot = &buffer[index];
                    slot.value.with_mut(|slot| unsafe {
                        let value = &mut *slot;
                        value.as_mut_ptr().drop_in_place();
                    });
                }
            });
        });
    }
}
