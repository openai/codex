use alloc::boxed::Box;
use core::mem::MaybeUninit;
use core::ptr;

use crossbeam_utils::CachePadded;

use crate::const_fn;
use crate::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
use crate::sync::cell::UnsafeCell;
#[allow(unused_imports)]
use crate::sync::prelude::*;
use crate::{busy_wait, PopError, PushError};

// Bits indicating the state of a slot:
// * If a value has been written into the slot, `WRITE` is set.
// * If a value has been read from the slot, `READ` is set.
// * If the block is being destroyed, `DESTROY` is set.
const WRITE: usize = 1;
const READ: usize = 2;
const DESTROY: usize = 4;

// Each block covers one "lap" of indices.
const LAP: usize = 32;
// The maximum number of items a block can hold.
const BLOCK_CAP: usize = LAP - 1;
// How many lower bits are reserved for metadata.
const SHIFT: usize = 1;
// Has two different purposes:
// * If set in head, indicates that the block is not the last one.
// * If set in tail, indicates that the queue is closed.
const MARK_BIT: usize = 1;

/// A slot in a block.
struct Slot<T> {
    /// The value.
    value: UnsafeCell<MaybeUninit<T>>,

    /// The state of the slot.
    state: AtomicUsize,
}

impl<T> Slot<T> {
    #[cfg(not(loom))]
    const UNINIT: Slot<T> = Slot {
        value: UnsafeCell::new(MaybeUninit::uninit()),
        state: AtomicUsize::new(0),
    };

    #[cfg(not(loom))]
    fn uninit_block() -> [Slot<T>; BLOCK_CAP] {
        [Self::UNINIT; BLOCK_CAP]
    }

    #[cfg(loom)]
    fn uninit_block() -> [Slot<T>; BLOCK_CAP] {
        // Repeat this expression 31 times.
        // Update if we change BLOCK_CAP
        macro_rules! repeat_31 {
            ($e: expr) => {
                [
                    $e, $e, $e, $e, $e, $e, $e, $e, $e, $e, $e, $e, $e, $e, $e, $e, $e, $e, $e, $e,
                    $e, $e, $e, $e, $e, $e, $e, $e, $e, $e, $e,
                ]
            };
        }

        repeat_31!(Slot {
            value: UnsafeCell::new(MaybeUninit::uninit()),
            state: AtomicUsize::new(0),
        })
    }

    /// Waits until a value is written into the slot.
    fn wait_write(&self) {
        while self.state.load(Ordering::Acquire) & WRITE == 0 {
            busy_wait();
        }
    }
}

/// A block in a linked list.
///
/// Each block in the list can hold up to `BLOCK_CAP` values.
struct Block<T> {
    /// The next block in the linked list.
    next: AtomicPtr<Block<T>>,

    /// Slots for values.
    slots: [Slot<T>; BLOCK_CAP],
}

impl<T> Block<T> {
    /// Creates an empty block.
    fn new() -> Block<T> {
        Block {
            next: AtomicPtr::new(ptr::null_mut()),
            slots: Slot::uninit_block(),
        }
    }

    /// Waits until the next pointer is set.
    fn wait_next(&self) -> *mut Block<T> {
        loop {
            let next = self.next.load(Ordering::Acquire);
            if !next.is_null() {
                return next;
            }
            busy_wait();
        }
    }

    /// Sets the `DESTROY` bit in slots starting from `start` and destroys the block.
    unsafe fn destroy(this: *mut Block<T>, start: usize) {
        // It is not necessary to set the `DESTROY` bit in the last slot because that slot has
        // begun destruction of the block.
        for i in start..BLOCK_CAP - 1 {
            let slot = (*this).slots.get_unchecked(i);

            // Mark the `DESTROY` bit if a thread is still using the slot.
            if slot.state.load(Ordering::Acquire) & READ == 0
                && slot.state.fetch_or(DESTROY, Ordering::AcqRel) & READ == 0
            {
                // If a thread is still using the slot, it will continue destruction of the block.
                return;
            }
        }

        // No thread is using the block, now it is safe to destroy it.
        drop(Box::from_raw(this));
    }
}

/// A position in a queue.
struct Position<T> {
    /// The index in the queue.
    index: AtomicUsize,

    /// The block in the linked list.
    block: AtomicPtr<Block<T>>,
}

/// An unbounded queue.
pub struct Unbounded<T> {
    /// The head of the queue.
    head: CachePadded<Position<T>>,

    /// The tail of the queue.
    tail: CachePadded<Position<T>>,
}

impl<T> Unbounded<T> {
    const_fn!(
        const_if: #[cfg(not(loom))];
        /// Creates a new unbounded queue.
        pub const fn new() -> Unbounded<T> {
            Unbounded {
                head: CachePadded::new(Position {
                    block: AtomicPtr::new(ptr::null_mut()),
                    index: AtomicUsize::new(0),
                }),
                tail: CachePadded::new(Position {
                    block: AtomicPtr::new(ptr::null_mut()),
                    index: AtomicUsize::new(0),
                }),
            }
        }
    );

    /// Pushes an item into the queue.
    pub fn push(&self, value: T) -> Result<(), PushError<T>> {
        let mut tail = self.tail.index.load(Ordering::Acquire);
        let mut block = self.tail.block.load(Ordering::Acquire);
        let mut next_block = None;

        loop {
            // Check if the queue is closed.
            if tail & MARK_BIT != 0 {
                return Err(PushError::Closed(value));
            }

            // Calculate the offset of the index into the block.
            let offset = (tail >> SHIFT) % LAP;

            // If we reached the end of the block, wait until the next one is installed.
            if offset == BLOCK_CAP {
                busy_wait();
                tail = self.tail.index.load(Ordering::Acquire);
                block = self.tail.block.load(Ordering::Acquire);
                continue;
            }

            // If we're going to have to install the next block, allocate it in advance in order to
            // make the wait for other threads as short as possible.
            if offset + 1 == BLOCK_CAP && next_block.is_none() {
                next_block = Some(Box::new(Block::<T>::new()));
            }

            // If this is the first value to be pushed into the queue, we need to allocate the
            // first block and install it.
            if block.is_null() {
                let new = Box::into_raw(Box::new(Block::<T>::new()));

                if self
                    .tail
                    .block
                    .compare_exchange(block, new, Ordering::Release, Ordering::Relaxed)
                    .is_ok()
                {
                    self.head.block.store(new, Ordering::Release);
                    block = new;
                } else {
                    next_block = unsafe { Some(Box::from_raw(new)) };
                    tail = self.tail.index.load(Ordering::Acquire);
                    block = self.tail.block.load(Ordering::Acquire);
                    continue;
                }
            }

            let new_tail = tail + (1 << SHIFT);

            // Try advancing the tail forward.
            match self.tail.index.compare_exchange_weak(
                tail,
                new_tail,
                Ordering::SeqCst,
                Ordering::Acquire,
            ) {
                Ok(_) => unsafe {
                    // If we've reached the end of the block, install the next one.
                    if offset + 1 == BLOCK_CAP {
                        let next_block = Box::into_raw(next_block.unwrap());
                        self.tail.block.store(next_block, Ordering::Release);
                        self.tail.index.fetch_add(1 << SHIFT, Ordering::Release);
                        (*block).next.store(next_block, Ordering::Release);
                    }

                    // Write the value into the slot.
                    let slot = (*block).slots.get_unchecked(offset);
                    slot.value.with_mut(|slot| {
                        slot.write(MaybeUninit::new(value));
                    });
                    slot.state.fetch_or(WRITE, Ordering::Release);
                    return Ok(());
                },
                Err(t) => {
                    tail = t;
                    block = self.tail.block.load(Ordering::Acquire);
                }
            }
        }
    }

    /// Pops an item from the queue.
    pub fn pop(&self) -> Result<T, PopError> {
        let mut head = self.head.index.load(Ordering::Acquire);
        let mut block = self.head.block.load(Ordering::Acquire);

        loop {
            // Calculate the offset of the index into the block.
            let offset = (head >> SHIFT) % LAP;

            // If we reached the end of the block, wait until the next one is installed.
            if offset == BLOCK_CAP {
                busy_wait();
                head = self.head.index.load(Ordering::Acquire);
                block = self.head.block.load(Ordering::Acquire);
                continue;
            }

            let mut new_head = head + (1 << SHIFT);

            if new_head & MARK_BIT == 0 {
                crate::full_fence();
                let tail = self.tail.index.load(Ordering::Relaxed);

                // If the tail equals the head, that means the queue is empty.
                if head >> SHIFT == tail >> SHIFT {
                    // Check if the queue is closed.
                    if tail & MARK_BIT != 0 {
                        return Err(PopError::Closed);
                    } else {
                        return Err(PopError::Empty);
                    }
                }

                // If head and tail are not in the same block, set `MARK_BIT` in head.
                if (head >> SHIFT) / LAP != (tail >> SHIFT) / LAP {
                    new_head |= MARK_BIT;
                }
            }

            // The block can be null here only if the first push operation is in progress.
            if block.is_null() {
                busy_wait();
                head = self.head.index.load(Ordering::Acquire);
                block = self.head.block.load(Ordering::Acquire);
                continue;
            }

            // Try moving the head index forward.
            match self.head.index.compare_exchange_weak(
                head,
                new_head,
                Ordering::SeqCst,
                Ordering::Acquire,
            ) {
                Ok(_) => unsafe {
                    // If we've reached the end of the block, move to the next one.
                    if offset + 1 == BLOCK_CAP {
                        let next = (*block).wait_next();
                        let mut next_index = (new_head & !MARK_BIT).wrapping_add(1 << SHIFT);
                        if !(*next).next.load(Ordering::Relaxed).is_null() {
                            next_index |= MARK_BIT;
                        }

                        self.head.block.store(next, Ordering::Release);
                        self.head.index.store(next_index, Ordering::Release);
                    }

                    // Read the value.
                    let slot = (*block).slots.get_unchecked(offset);
                    slot.wait_write();
                    let value = slot.value.with_mut(|slot| slot.read().assume_init());

                    // Destroy the block if we've reached the end, or if another thread wanted to
                    // destroy but couldn't because we were busy reading from the slot.
                    if offset + 1 == BLOCK_CAP {
                        Block::destroy(block, 0);
                    } else if slot.state.fetch_or(READ, Ordering::AcqRel) & DESTROY != 0 {
                        Block::destroy(block, offset + 1);
                    }

                    return Ok(value);
                },
                Err(h) => {
                    head = h;
                    block = self.head.block.load(Ordering::Acquire);
                }
            }
        }
    }

    /// Returns the number of items in the queue.
    pub fn len(&self) -> usize {
        loop {
            // Load the tail index, then load the head index.
            let mut tail = self.tail.index.load(Ordering::SeqCst);
            let mut head = self.head.index.load(Ordering::SeqCst);

            // If the tail index didn't change, we've got consistent indices to work with.
            if self.tail.index.load(Ordering::SeqCst) == tail {
                // Erase the lower bits.
                tail &= !((1 << SHIFT) - 1);
                head &= !((1 << SHIFT) - 1);

                // Fix up indices if they fall onto block ends.
                if (tail >> SHIFT) & (LAP - 1) == LAP - 1 {
                    tail = tail.wrapping_add(1 << SHIFT);
                }
                if (head >> SHIFT) & (LAP - 1) == LAP - 1 {
                    head = head.wrapping_add(1 << SHIFT);
                }

                // Rotate indices so that head falls into the first block.
                let lap = (head >> SHIFT) / LAP;
                tail = tail.wrapping_sub((lap * LAP) << SHIFT);
                head = head.wrapping_sub((lap * LAP) << SHIFT);

                // Remove the lower bits.
                tail >>= SHIFT;
                head >>= SHIFT;

                // Return the difference minus the number of blocks between tail and head.
                return tail - head - tail / LAP;
            }
        }
    }

    /// Returns `true` if the queue is empty.
    pub fn is_empty(&self) -> bool {
        let head = self.head.index.load(Ordering::SeqCst);
        let tail = self.tail.index.load(Ordering::SeqCst);
        head >> SHIFT == tail >> SHIFT
    }

    /// Returns `true` if the queue is full.
    pub fn is_full(&self) -> bool {
        false
    }

    /// Closes the queue.
    ///
    /// Returns `true` if this call closed the queue.
    pub fn close(&self) -> bool {
        let tail = self.tail.index.fetch_or(MARK_BIT, Ordering::SeqCst);
        tail & MARK_BIT == 0
    }

    /// Returns `true` if the queue is closed.
    pub fn is_closed(&self) -> bool {
        self.tail.index.load(Ordering::SeqCst) & MARK_BIT != 0
    }
}

impl<T> Drop for Unbounded<T> {
    fn drop(&mut self) {
        let Self { head, tail } = self;
        let Position { index: head, block } = &mut **head;

        head.with_mut(|&mut mut head| {
            tail.index.with_mut(|&mut mut tail| {
                // Erase the lower bits.
                head &= !((1 << SHIFT) - 1);
                tail &= !((1 << SHIFT) - 1);

                unsafe {
                    // Drop all values between `head` and `tail` and deallocate the heap-allocated blocks.
                    while head != tail {
                        let offset = (head >> SHIFT) % LAP;

                        if offset < BLOCK_CAP {
                            // Drop the value in the slot.
                            block.with_mut(|block| {
                                let slot = (**block).slots.get_unchecked(offset);
                                slot.value.with_mut(|slot| {
                                    let value = &mut *slot;
                                    value.as_mut_ptr().drop_in_place();
                                });
                            });
                        } else {
                            // Deallocate the block and move to the next one.
                            block.with_mut(|block| {
                                let next_block = (**block).next.with_mut(|next| *next);
                                drop(Box::from_raw(*block));
                                *block = next_block;
                            });
                        }

                        head = head.wrapping_add(1 << SHIFT);
                    }

                    // Deallocate the last remaining block.
                    block.with_mut(|block| {
                        if !block.is_null() {
                            drop(Box::from_raw(*block));
                        }
                    });
                }
            });
        });
    }
}
