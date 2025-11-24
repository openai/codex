mod level;
pub(crate) use self::level::Expiration;
use self::level::Level;

mod stack;
pub(crate) use self::stack::Stack;

use std::borrow::Borrow;
use std::fmt::Debug;

/// Timing wheel implementation.
///
/// This type provides the hashed timing wheel implementation that backs `Timer`
/// and `DelayQueue`.
///
/// The structure is generic over `T: Stack`. This allows handling timeout data
/// being stored on the heap or in a slab. In order to support the latter case,
/// the slab must be passed into each function allowing the implementation to
/// lookup timer entries.
///
/// See `Timer` documentation for some implementation notes.
#[derive(Debug)]
pub(crate) struct Wheel<T> {
    /// The number of milliseconds elapsed since the wheel started.
    elapsed: u64,

    /// Timer wheel.
    ///
    /// Levels:
    ///
    /// * 1 ms slots / 64 ms range
    /// * 64 ms slots / ~ 4 sec range
    /// * ~ 4 sec slots / ~ 4 min range
    /// * ~ 4 min slots / ~ 4 hr range
    /// * ~ 4 hr slots / ~ 12 day range
    /// * ~ 12 day slots / ~ 2 yr range
    levels: Box<[Level<T>]>,
}

/// Number of levels. Each level has 64 slots. By using 6 levels with 64 slots
/// each, the timer is able to track time up to 2 years into the future with a
/// precision of 1 millisecond.
const NUM_LEVELS: usize = 6;

/// The maximum duration of a delay
const MAX_DURATION: u64 = (1 << (6 * NUM_LEVELS)) - 1;

#[derive(Debug)]
pub(crate) enum InsertError {
    Elapsed,
    Invalid,
}

impl<T> Wheel<T>
where
    T: Stack,
{
    /// Create a new timing wheel
    pub(crate) fn new() -> Wheel<T> {
        let levels = (0..NUM_LEVELS).map(Level::new).collect();

        Wheel { elapsed: 0, levels }
    }

    /// Return the number of milliseconds that have elapsed since the timing
    /// wheel's creation.
    pub(crate) fn elapsed(&self) -> u64 {
        self.elapsed
    }

    /// Insert an entry into the timing wheel.
    ///
    /// # Arguments
    ///
    /// * `when`: is the instant at which the entry should be fired. It is
    ///   represented as the number of milliseconds since the creation
    ///   of the timing wheel.
    ///
    /// * `item`: The item to insert into the wheel.
    ///
    /// * `store`: The slab or `()` when using heap storage.
    ///
    /// # Return
    ///
    /// Returns `Ok` when the item is successfully inserted, `Err` otherwise.
    ///
    /// `Err(Elapsed)` indicates that `when` represents an instant that has
    /// already passed. In this case, the caller should fire the timeout
    /// immediately.
    ///
    /// `Err(Invalid)` indicates an invalid `when` argument as been supplied.
    pub(crate) fn insert(
        &mut self,
        when: u64,
        item: T::Owned,
        store: &mut T::Store,
    ) -> Result<(), (T::Owned, InsertError)> {
        if when <= self.elapsed {
            return Err((item, InsertError::Elapsed));
        } else if when - self.elapsed > MAX_DURATION {
            return Err((item, InsertError::Invalid));
        }

        // Get the level at which the entry should be stored
        let level = self.level_for(when);

        self.levels[level].add_entry(when, item, store);

        debug_assert!({
            self.levels[level]
                .next_expiration(self.elapsed)
                .map(|e| e.deadline >= self.elapsed)
                .unwrap_or(true)
        });

        Ok(())
    }

    /// Remove `item` from the timing wheel.
    #[track_caller]
    pub(crate) fn remove(&mut self, item: &T::Borrowed, store: &mut T::Store) {
        let when = T::when(item, store);

        assert!(
            self.elapsed <= when,
            "elapsed={}; when={}",
            self.elapsed,
            when
        );

        let level = self.level_for(when);

        self.levels[level].remove_entry(when, item, store);
    }

    /// Instant at which to poll
    pub(crate) fn poll_at(&self) -> Option<u64> {
        self.next_expiration().map(|expiration| expiration.deadline)
    }

    /// Next key that will expire
    pub(crate) fn peek(&self) -> Option<T::Owned> {
        self.next_expiration()
            .and_then(|expiration| self.peek_entry(&expiration))
    }

    /// Advances the timer up to the instant represented by `now`.
    pub(crate) fn poll(&mut self, now: u64, store: &mut T::Store) -> Option<T::Owned> {
        loop {
            let expiration = self.next_expiration().and_then(|expiration| {
                if expiration.deadline > now {
                    None
                } else {
                    Some(expiration)
                }
            });

            match expiration {
                Some(ref expiration) => {
                    if let Some(item) = self.poll_expiration(expiration, store) {
                        return Some(item);
                    }

                    self.set_elapsed(expiration.deadline);
                }
                None => {
                    // in this case the poll did not indicate an expiration
                    // _and_ we were not able to find a next expiration in
                    // the current list of timers.  advance to the poll's
                    // current time and do nothing else.
                    self.set_elapsed(now);
                    return None;
                }
            }
        }
    }

    /// Returns the instant at which the next timeout expires.
    fn next_expiration(&self) -> Option<Expiration> {
        // Check all levels
        for level in 0..NUM_LEVELS {
            if let Some(expiration) = self.levels[level].next_expiration(self.elapsed) {
                // There cannot be any expirations at a higher level that happen
                // before this one.
                debug_assert!(self.no_expirations_before(level + 1, expiration.deadline));

                return Some(expiration);
            }
        }

        None
    }

    /// Used for debug assertions
    fn no_expirations_before(&self, start_level: usize, before: u64) -> bool {
        let mut res = true;

        for l2 in start_level..NUM_LEVELS {
            if let Some(e2) = self.levels[l2].next_expiration(self.elapsed) {
                if e2.deadline < before {
                    res = false;
                }
            }
        }

        res
    }

    /// iteratively find entries that are between the wheel's current
    /// time and the expiration time.  for each in that population either
    /// return it for notification (in the case of the last level) or tier
    /// it down to the next level (in all other cases).
    pub(crate) fn poll_expiration(
        &mut self,
        expiration: &Expiration,
        store: &mut T::Store,
    ) -> Option<T::Owned> {
        while let Some(item) = self.pop_entry(expiration, store) {
            if expiration.level == 0 {
                debug_assert_eq!(T::when(item.borrow(), store), expiration.deadline);

                return Some(item);
            } else {
                let when = T::when(item.borrow(), store);

                let next_level = expiration.level - 1;

                self.levels[next_level].add_entry(when, item, store);
            }
        }

        None
    }

    fn set_elapsed(&mut self, when: u64) {
        assert!(
            self.elapsed <= when,
            "elapsed={:?}; when={:?}",
            self.elapsed,
            when
        );

        if when > self.elapsed {
            self.elapsed = when;
        }
    }

    fn pop_entry(&mut self, expiration: &Expiration, store: &mut T::Store) -> Option<T::Owned> {
        self.levels[expiration.level].pop_entry_slot(expiration.slot, store)
    }

    fn peek_entry(&self, expiration: &Expiration) -> Option<T::Owned> {
        self.levels[expiration.level].peek_entry_slot(expiration.slot)
    }

    fn level_for(&self, when: u64) -> usize {
        level_for(self.elapsed, when)
    }
}

fn level_for(elapsed: u64, when: u64) -> usize {
    const SLOT_MASK: u64 = (1 << 6) - 1;

    // Mask in the trailing bits ignored by the level calculation in order to cap
    // the possible leading zeros
    let mut masked = elapsed ^ when | SLOT_MASK;
    if masked >= MAX_DURATION {
        // Fudge the timer into the top level
        masked = MAX_DURATION - 1;
    }
    let leading_zeros = masked.leading_zeros() as usize;
    let significant = 63 - leading_zeros;
    significant / 6
}

#[cfg(all(test, not(loom)))]
mod test {
    use super::*;

    #[test]
    fn test_level_for() {
        for pos in 0..64 {
            assert_eq!(0, level_for(0, pos), "level_for({pos}) -- binary = {pos:b}");
        }

        for level in 1..5 {
            for pos in level..64 {
                let a = pos * 64_usize.pow(level as u32);
                assert_eq!(
                    level,
                    level_for(0, a as u64),
                    "level_for({a}) -- binary = {a:b}"
                );

                if pos > level {
                    let a = a - 1;
                    assert_eq!(
                        level,
                        level_for(0, a as u64),
                        "level_for({a}) -- binary = {a:b}"
                    );
                }

                if pos < 64 {
                    let a = a + 1;
                    assert_eq!(
                        level,
                        level_for(0, a as u64),
                        "level_for({a}) -- binary = {a:b}"
                    );
                }
            }
        }
    }
}
