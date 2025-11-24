//! A restricted channel to pass data from signal handler.
//!
//! When trying to communicate data from signal handler to the outside world, one can use an atomic
//! variable (as it doesn't lock, so it can be made async-signal-safe). But this won't work for
//! larger data.
//!
//! This module provides a channel that can be used for that purpose. It is used by certain
//! [exfiltrators][crate::iterator::exfiltrator], but can be used as building block for custom
//! actions. In general, this is not a ready-made end-user API.
//!
//! # How does it work
//!
//! Each channel has a fixed number of slots and two queues (one for empty slots, one for full
//! slots). A signal handler takes a slot out of the empty one, fills it and passes it into the
//! full one. Outside of signal handler, it can take the value out of the full queue and return the
//! slot to the empty queue.
//!
//! The queues are implemented as bit-encoded indexes of the slots in the storage. The bits are
//! stored in an atomic variable.
//!
//! Note that the algorithm allows for a slot to be in neither queue (when it is being emptied or
//! filled).
//!
//! # Fallible allocation of a slot
//!
//! It is apparent that allocation of a new slot can fail (there's nothing in the empty slot). In
//! such case, there's no way to send the new value out of the handler (there's no way to safely
//! wait for a slot to appear, because the handler can be blocking the thread that is responsible
//! for emptying them). But that's considered acceptable ‒ even the kernel collates the same kinds
//! of signals together if they are not consumed by application fast enough and there are no free
//! slots exactly because some are being filled, emptied or are full ‒ in particular, the whole
//! system will yield a signal.
//!
//! This assumes that separate signals don't share the same buffer and that there's only one reader
//! (using multiple readers is still safe, but it is possible that all slots would be inside the
//! readers, but already empty, so the above argument would not hold).

// TODO: Other sizes? Does anyone need more than 5 slots?

use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicU16, Ordering};

const SLOTS: usize = 5;
const BITS: u16 = 3;
const MASK: u16 = 0b111;

fn get(n: u16, idx: u16) -> u16 {
    (n >> (BITS * idx)) & MASK
}

fn set(n: u16, idx: u16, v: u16) -> u16 {
    let v = v << (BITS * idx);
    let mask = MASK << (BITS * idx);
    (n & !mask) | v
}

fn enqueue(q: &AtomicU16, val: u16) {
    let mut current = q.load(Ordering::Relaxed);
    loop {
        let empty = (0..SLOTS as u16)
            .find(|i| get(current, *i) == 0)
            .expect("No empty slot available");
        let modified = set(current, empty, val);
        match q.compare_exchange_weak(current, modified, Ordering::Release, Ordering::Relaxed) {
            Ok(_) => break,
            Err(changed) => current = changed, // And retry with the changed value
        }
    }
}

fn dequeue(q: &AtomicU16) -> Option<u16> {
    let mut current = q.load(Ordering::Relaxed);
    loop {
        let val = current & MASK;
        // It's completely empty
        if val == 0 {
            break None;
        }
        let modified = current >> BITS;
        match q.compare_exchange_weak(current, modified, Ordering::Acquire, Ordering::Relaxed) {
            Ok(_) => break Some(val),
            Err(changed) => current = changed,
        }
    }
}

/// A restricted async-signal-safe channel
///
/// This is a bit like the usual channel used for inter-thread communication, but with several
/// restrictions:
///
/// * There's a limited number of slots (currently 5).
/// * There's no way to wait for a place in it or for a value. If value is not available, `None` is
///   returned. If there's no space for a value, the value is silently dropped.
///
/// In exchange for that, all the operations on that channel are async-signal-safe. That means it
/// is possible to use it to communicate between a signal handler and the rest of the world with it
/// (specifically, it's designed to send information from the handler to the rest of the
/// application). The throwing out of values when full is in line with collating of the same type
/// in kernel (you should not use the same channel for multiple different signals).
///
/// Technically, this is a MPMC queue which preserves order, but it is expected to be used in MPSC
/// mode mostly (in theory, multiple threads can be executing a signal handler for the same signal
/// at the same time). The channel is not responsible for wakeups.
///
/// While the channel is async-signal-safe, you still need to make sure *creating* of the values is
/// too (it should not contain anything that allocates, for example ‒ so no `String`s inside, etc).
///
/// The code was *not* tuned for performance (signals are not expected to happen often).
pub struct Channel<T> {
    storage: [UnsafeCell<Option<T>>; SLOTS],
    empty: AtomicU16,
    full: AtomicU16,
}

impl<T> Channel<T> {
    /// Creates a new channel with nothing in it.
    pub fn new() -> Self {
        let storage = Default::default();
        let me = Self {
            storage,
            empty: AtomicU16::new(0),
            full: AtomicU16::new(0),
        };

        for i in 1..SLOTS + 1 {
            enqueue(&me.empty, i as u16);
        }

        me
    }

    /// Inserts a value into the channel.
    ///
    /// If the value doesn't fit, it is silently dropped. Never blocks.
    pub fn send(&self, val: T) {
        if let Some(empty_idx) = dequeue(&self.empty) {
            unsafe { *self.storage[empty_idx as usize - 1].get() = Some(val) };
            enqueue(&self.full, empty_idx);
        }
    }

    /// Takes a value from the channel.
    ///
    /// Or returns `None` if the channel is empty. Never blocks.
    pub fn recv(&self) -> Option<T> {
        dequeue(&self.full).map(|idx| {
            let result = unsafe { &mut *self.storage[idx as usize - 1].get() }
                .take()
                .expect("Full slot with nothing in it");
            enqueue(&self.empty, idx);
            result
        })
    }
}

impl<T> Default for Channel<T> {
    fn default() -> Self {
        Self::new()
    }
}

unsafe impl<T: Send> Send for Channel<T> {}

// Yes, really Send -> Sync. Having a reference to Channel allows Sending Ts, but not having refs
// on them.
unsafe impl<T: Send> Sync for Channel<T> {}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::thread;

    use super::*;

    #[test]
    fn new_empty() {
        let channel = Channel::<usize>::new();
        assert!(channel.recv().is_none());
        assert!(channel.recv().is_none());
    }

    #[test]
    fn pass_value() {
        let channel = Channel::new();
        channel.send(42);
        assert_eq!(42, channel.recv().unwrap());
        assert!(channel.recv().is_none());
    }

    #[test]
    fn multiple() {
        let channel = Channel::new();
        for i in 0..1000 {
            channel.send(i);
            assert_eq!(i, channel.recv().unwrap());
            assert!(channel.recv().is_none());
        }
    }

    #[test]
    fn overflow() {
        let channel = Channel::new();
        for i in 0..10 {
            channel.send(i);
        }
        for i in 0..5 {
            assert_eq!(i, channel.recv().unwrap());
        }
        assert!(channel.recv().is_none());
    }

    #[test]
    fn multi_thread() {
        let channel = Arc::new(Channel::<usize>::new());

        let sender = thread::spawn({
            let channel = Arc::clone(&channel);
            move || {
                for i in 0..4 {
                    channel.send(i);
                }
            }
        });

        let mut results = Vec::new();
        while results.len() < 4 {
            results.extend(channel.recv());
        }

        assert_eq!(vec![0, 1, 2, 3], results);

        sender.join().unwrap();
    }
}
