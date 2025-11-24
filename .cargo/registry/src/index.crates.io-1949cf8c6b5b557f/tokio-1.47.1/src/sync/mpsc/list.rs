//! A concurrent, lock-free, FIFO list.

use crate::loom::sync::atomic::{AtomicPtr, AtomicUsize};
use crate::loom::thread;
use crate::sync::mpsc::block::{self, Block};

use std::fmt;
use std::ptr::NonNull;
use std::sync::atomic::Ordering::{AcqRel, Acquire, Relaxed, Release};

/// List queue transmit handle.
pub(crate) struct Tx<T> {
    /// Tail in the `Block` mpmc list.
    block_tail: AtomicPtr<Block<T>>,

    /// Position to push the next message. This references a block and offset
    /// into the block.
    tail_position: AtomicUsize,
}

/// List queue receive handle
pub(crate) struct Rx<T> {
    /// Pointer to the block being processed.
    head: NonNull<Block<T>>,

    /// Next slot index to process.
    index: usize,

    /// Pointer to the next block pending release.
    free_head: NonNull<Block<T>>,
}

/// Return value of `Rx::try_pop`.
pub(crate) enum TryPopResult<T> {
    /// Successfully popped a value.
    Ok(T),
    /// The channel is empty.
    Empty,
    /// The channel is empty and closed.
    Closed,
    /// The channel is not empty, but the first value is being written.
    Busy,
}

pub(crate) fn channel<T>() -> (Tx<T>, Rx<T>) {
    // Create the initial block shared between the tx and rx halves.
    let initial_block = Block::new(0);
    let initial_block_ptr = Box::into_raw(initial_block);

    let tx = Tx {
        block_tail: AtomicPtr::new(initial_block_ptr),
        tail_position: AtomicUsize::new(0),
    };

    let head = NonNull::new(initial_block_ptr).unwrap();

    let rx = Rx {
        head,
        index: 0,
        free_head: head,
    };

    (tx, rx)
}

impl<T> Tx<T> {
    /// Pushes a value into the list.
    pub(crate) fn push(&self, value: T) {
        // First, claim a slot for the value. `Acquire` is used here to
        // synchronize with the `fetch_add` in `reclaim_blocks`.
        let slot_index = self.tail_position.fetch_add(1, Acquire);

        // Load the current block and write the value
        let block = self.find_block(slot_index);

        unsafe {
            // Write the value to the block
            block.as_ref().write(slot_index, value);
        }
    }

    /// Closes the send half of the list.
    ///
    /// Similar process as pushing a value, but instead of writing the value &
    /// setting the ready flag, the `TX_CLOSED` flag is set on the block.
    pub(crate) fn close(&self) {
        // First, claim a slot for the value. This is the last slot that will be
        // claimed.
        let slot_index = self.tail_position.fetch_add(1, Acquire);

        let block = self.find_block(slot_index);

        unsafe { block.as_ref().tx_close() }
    }

    fn find_block(&self, slot_index: usize) -> NonNull<Block<T>> {
        // The start index of the block that contains `index`.
        let start_index = block::start_index(slot_index);

        // The index offset into the block
        let offset = block::offset(slot_index);

        // Load the current head of the block
        let mut block_ptr = self.block_tail.load(Acquire);

        let block = unsafe { &*block_ptr };

        // Calculate the distance between the tail ptr and the target block
        let distance = block.distance(start_index);

        // Decide if this call to `find_block` should attempt to update the
        // `block_tail` pointer.
        //
        // Updating `block_tail` is not always performed in order to reduce
        // contention.
        //
        // When set, as the routine walks the linked list, it attempts to update
        // `block_tail`. If the update cannot be performed, `try_updating_tail`
        // is unset.
        let mut try_updating_tail = distance > offset;

        // Walk the linked list of blocks until the block with `start_index` is
        // found.
        loop {
            let block = unsafe { &(*block_ptr) };

            if block.is_at_index(start_index) {
                return unsafe { NonNull::new_unchecked(block_ptr) };
            }

            let next_block = block
                .load_next(Acquire)
                // There is no allocated next block, grow the linked list.
                .unwrap_or_else(|| block.grow());

            // If the block is **not** final, then the tail pointer cannot be
            // advanced any more.
            try_updating_tail &= block.is_final();

            if try_updating_tail {
                // Advancing `block_tail` must happen when walking the linked
                // list. `block_tail` may not advance passed any blocks that are
                // not "final". At the point a block is finalized, it is unknown
                // if there are any prior blocks that are unfinalized, which
                // makes it impossible to advance `block_tail`.
                //
                // While walking the linked list, `block_tail` can be advanced
                // as long as finalized blocks are traversed.
                //
                // Release ordering is used to ensure that any subsequent reads
                // are able to see the memory pointed to by `block_tail`.
                //
                // Acquire is not needed as any "actual" value is not accessed.
                // At this point, the linked list is walked to acquire blocks.
                if self
                    .block_tail
                    .compare_exchange(block_ptr, next_block.as_ptr(), Release, Relaxed)
                    .is_ok()
                {
                    // Synchronize with any senders
                    let tail_position = self.tail_position.fetch_add(0, Release);

                    unsafe {
                        block.tx_release(tail_position);
                    }
                } else {
                    // A concurrent sender is also working on advancing
                    // `block_tail` and this thread is falling behind.
                    //
                    // Stop trying to advance the tail pointer
                    try_updating_tail = false;
                }
            }

            block_ptr = next_block.as_ptr();

            thread::yield_now();
        }
    }

    pub(crate) unsafe fn reclaim_block(&self, mut block: NonNull<Block<T>>) {
        // The block has been removed from the linked list and ownership
        // is reclaimed.
        //
        // Before dropping the block, see if it can be reused by
        // inserting it back at the end of the linked list.
        //
        // First, reset the data
        block.as_mut().reclaim();

        let mut reused = false;

        // Attempt to insert the block at the end
        //
        // Walk at most three times
        //
        let curr_ptr = self.block_tail.load(Acquire);

        // The pointer can never be null
        debug_assert!(!curr_ptr.is_null());

        let mut curr = NonNull::new_unchecked(curr_ptr);

        // TODO: Unify this logic with Block::grow
        for _ in 0..3 {
            match curr.as_ref().try_push(&mut block, AcqRel, Acquire) {
                Ok(()) => {
                    reused = true;
                    break;
                }
                Err(next) => {
                    curr = next;
                }
            }
        }

        if !reused {
            let _ = Box::from_raw(block.as_ptr());
        }
    }

    pub(crate) fn is_closed(&self) -> bool {
        let tail = self.block_tail.load(Acquire);

        unsafe {
            let tail_block = &*tail;
            tail_block.is_closed()
        }
    }
}

impl<T> fmt::Debug for Tx<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("Tx")
            .field("block_tail", &self.block_tail.load(Relaxed))
            .field("tail_position", &self.tail_position.load(Relaxed))
            .finish()
    }
}

impl<T> Rx<T> {
    pub(crate) fn is_empty(&self, tx: &Tx<T>) -> bool {
        let block = unsafe { self.head.as_ref() };
        if block.has_value(self.index) {
            return false;
        }

        // It is possible that a block has no value "now" but the list is still not empty.
        // To be sure, it is necessary to check the length of the list.
        self.len(tx) == 0
    }

    pub(crate) fn len(&self, tx: &Tx<T>) -> usize {
        // When all the senders are dropped, there will be a last block in the tail position,
        // but it will be closed
        let tail_position = tx.tail_position.load(Acquire);
        tail_position - self.index - (tx.is_closed() as usize)
    }

    /// Pops the next value off the queue.
    pub(crate) fn pop(&mut self, tx: &Tx<T>) -> Option<block::Read<T>> {
        // Advance `head`, if needed
        if !self.try_advancing_head() {
            return None;
        }

        self.reclaim_blocks(tx);

        unsafe {
            let block = self.head.as_ref();

            let ret = block.read(self.index);

            if let Some(block::Read::Value(..)) = ret {
                self.index = self.index.wrapping_add(1);
            }

            ret
        }
    }

    /// Pops the next value off the queue, detecting whether the block
    /// is busy or empty on failure.
    ///
    /// This function exists because `Rx::pop` can return `None` even if the
    /// channel's queue contains a message that has been completely written.
    /// This can happen if the fully delivered message is behind another message
    /// that is in the middle of being written to the block, since the channel
    /// can't return the messages out of order.
    pub(crate) fn try_pop(&mut self, tx: &Tx<T>) -> TryPopResult<T> {
        let tail_position = tx.tail_position.load(Acquire);
        let result = self.pop(tx);

        match result {
            Some(block::Read::Value(t)) => TryPopResult::Ok(t),
            Some(block::Read::Closed) => TryPopResult::Closed,
            None if tail_position == self.index => TryPopResult::Empty,
            None => TryPopResult::Busy,
        }
    }

    /// Tries advancing the block pointer to the block referenced by `self.index`.
    ///
    /// Returns `true` if successful, `false` if there is no next block to load.
    fn try_advancing_head(&mut self) -> bool {
        let block_index = block::start_index(self.index);

        loop {
            let next_block = {
                let block = unsafe { self.head.as_ref() };

                if block.is_at_index(block_index) {
                    return true;
                }

                block.load_next(Acquire)
            };

            let next_block = match next_block {
                Some(next_block) => next_block,
                None => {
                    return false;
                }
            };

            self.head = next_block;

            thread::yield_now();
        }
    }

    fn reclaim_blocks(&mut self, tx: &Tx<T>) {
        while self.free_head != self.head {
            unsafe {
                // Get a handle to the block that will be freed and update
                // `free_head` to point to the next block.
                let block = self.free_head;

                let observed_tail_position = block.as_ref().observed_tail_position();

                let required_index = match observed_tail_position {
                    Some(i) => i,
                    None => return,
                };

                if required_index > self.index {
                    return;
                }

                // We may read the next pointer with `Relaxed` ordering as it is
                // guaranteed that the `reclaim_blocks` routine trails the `recv`
                // routine. Any memory accessed by `reclaim_blocks` has already
                // been acquired by `recv`.
                let next_block = block.as_ref().load_next(Relaxed);

                // Update the free list head
                self.free_head = next_block.unwrap();

                // Push the emptied block onto the back of the queue, making it
                // available to senders.
                tx.reclaim_block(block);
            }

            thread::yield_now();
        }
    }

    /// Effectively `Drop` all the blocks. Should only be called once, when
    /// the list is dropping.
    pub(super) unsafe fn free_blocks(&mut self) {
        debug_assert_ne!(self.free_head, NonNull::dangling());

        let mut cur = Some(self.free_head);

        #[cfg(debug_assertions)]
        {
            // to trigger the debug assert above so as to catch that we
            // don't call `free_blocks` more than once.
            self.free_head = NonNull::dangling();
            self.head = NonNull::dangling();
        }

        while let Some(block) = cur {
            cur = block.as_ref().load_next(Relaxed);
            drop(Box::from_raw(block.as_ptr()));
        }
    }
}

impl<T> fmt::Debug for Rx<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("Rx")
            .field("head", &self.head)
            .field("index", &self.index)
            .field("free_head", &self.free_head)
            .finish()
    }
}
