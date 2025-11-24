use alloc::boxed::Box;
use alloc::vec::Vec;
use core::mem;
#[cfg(feature = "std")]
use std::sync::{RwLock, RwLockReadGuard};

use pki_types::UnixTime;

use crate::lock::{Mutex, MutexGuard};
use crate::server::ProducesTickets;
#[cfg(not(feature = "std"))]
use crate::time_provider::TimeProvider;
use crate::{Error, rand};

#[derive(Debug)]
pub(crate) struct TicketSwitcherState {
    next: Option<Box<dyn ProducesTickets>>,
    current: Box<dyn ProducesTickets>,
    previous: Option<Box<dyn ProducesTickets>>,
    next_switch_time: u64,
}

/// A ticketer that has a 'current' sub-ticketer and a single
/// 'previous' ticketer.  It creates a new ticketer every so
/// often, demoting the current ticketer.
#[cfg_attr(feature = "std", derive(Debug))]
pub struct TicketSwitcher {
    pub(crate) generator: fn() -> Result<Box<dyn ProducesTickets>, rand::GetRandomFailed>,
    lifetime: u32,
    state: Mutex<TicketSwitcherState>,
    #[cfg(not(feature = "std"))]
    time_provider: &'static dyn TimeProvider,
}

impl TicketSwitcher {
    /// Creates a new `TicketSwitcher`, which rotates through sub-ticketers
    /// based on the passage of time.
    ///
    /// `lifetime` is in seconds, and is how long the current ticketer
    /// is used to generate new tickets.  Tickets are accepted for no
    /// longer than twice this duration.  `generator` produces a new
    /// `ProducesTickets` implementation.
    #[cfg(feature = "std")]
    #[deprecated(note = "use TicketRotator instead")]
    pub fn new(
        lifetime: u32,
        generator: fn() -> Result<Box<dyn ProducesTickets>, rand::GetRandomFailed>,
    ) -> Result<Self, Error> {
        Ok(Self {
            generator,
            lifetime,
            state: Mutex::new(TicketSwitcherState {
                next: Some(generator()?),
                current: generator()?,
                previous: None,
                next_switch_time: UnixTime::now()
                    .as_secs()
                    .saturating_add(u64::from(lifetime)),
            }),
        })
    }

    /// Creates a new `TicketSwitcher`, which rotates through sub-ticketers
    /// based on the passage of time.
    ///
    /// `lifetime` is in seconds, and is how long the current ticketer
    /// is used to generate new tickets.  Tickets are accepted for no
    /// longer than twice this duration.  `generator` produces a new
    /// `ProducesTickets` implementation.
    #[cfg(not(feature = "std"))]
    pub fn new<M: crate::lock::MakeMutex>(
        lifetime: u32,
        generator: fn() -> Result<Box<dyn ProducesTickets>, rand::GetRandomFailed>,
        time_provider: &'static dyn TimeProvider,
    ) -> Result<Self, Error> {
        Ok(Self {
            generator,
            lifetime,
            state: Mutex::new::<M>(TicketSwitcherState {
                next: Some(generator()?),
                current: generator()?,
                previous: None,
                next_switch_time: time_provider
                    .current_time()
                    .unwrap()
                    .as_secs()
                    .saturating_add(u64::from(lifetime)),
            }),
            time_provider,
        })
    }

    /// If it's time, demote the `current` ticketer to `previous` (so it
    /// does no new encryptions but can do decryption) and use next for a
    /// new `current` ticketer.
    ///
    /// Calling this regularly will ensure timely key erasure.  Otherwise,
    /// key erasure will be delayed until the next encrypt/decrypt call.
    ///
    /// For efficiency, this is also responsible for locking the state mutex
    /// and returning the mutexguard.
    pub(crate) fn maybe_roll(&self, now: UnixTime) -> Option<MutexGuard<'_, TicketSwitcherState>> {
        // The code below aims to make switching as efficient as possible
        // in the common case that the generator never fails. To achieve this
        // we run the following steps:
        //  1. If no switch is necessary, just return the mutexguard
        //  2. Shift over all of the ticketers (so current becomes previous,
        //     and next becomes current). After this, other threads can
        //     start using the new current ticketer.
        //  3. unlock mutex and generate new ticketer.
        //  4. Place new ticketer in next and return current
        //
        // There are a few things to note here. First, we don't check whether
        // a new switch might be needed in step 4, even though, due to locking
        // and entropy collection, significant amounts of time may have passed.
        // This is to guarantee that the thread doing the switch will eventually
        // make progress.
        //
        // Second, because next may be None, step 2 can fail. In that case
        // we enter a recovery mode where we generate 2 new ticketers, one for
        // next and one for the current ticketer. We then take the mutex a
        // second time and redo the time check to see if a switch is still
        // necessary.
        //
        // This somewhat convoluted approach ensures good availability of the
        // mutex, by ensuring that the state is usable and the mutex not held
        // during generation. It also ensures that, so long as the inner
        // ticketer never generates panics during encryption/decryption,
        // we are guaranteed to never panic when holding the mutex.

        let now = now.as_secs();
        let mut are_recovering = false; // Are we recovering from previous failure?
        {
            // Scope the mutex so we only take it for as long as needed
            let mut state = self.state.lock()?;

            // Fast path in case we do not need to switch to the next ticketer yet
            if now <= state.next_switch_time {
                return Some(state);
            }

            // Make the switch, or mark for recovery if not possible
            match state.next.take() {
                Some(next) => {
                    state.previous = Some(mem::replace(&mut state.current, next));
                    state.next_switch_time = now.saturating_add(u64::from(self.lifetime));
                }
                _ => are_recovering = true,
            }
        }

        // We always need a next, so generate it now
        let next = (self.generator)().ok()?;
        if !are_recovering {
            // Normal path, generate new next and place it in the state
            let mut state = self.state.lock()?;
            state.next = Some(next);
            Some(state)
        } else {
            // Recovering, generate also a new current ticketer, and modify state
            // as needed. (we need to redo the time check, otherwise this might
            // result in very rapid switching of ticketers)
            let new_current = (self.generator)().ok()?;
            let mut state = self.state.lock()?;
            state.next = Some(next);
            if now > state.next_switch_time {
                state.previous = Some(mem::replace(&mut state.current, new_current));
                state.next_switch_time = now.saturating_add(u64::from(self.lifetime));
            }
            Some(state)
        }
    }
}

impl ProducesTickets for TicketSwitcher {
    fn lifetime(&self) -> u32 {
        self.lifetime * 2
    }

    fn enabled(&self) -> bool {
        true
    }

    fn encrypt(&self, message: &[u8]) -> Option<Vec<u8>> {
        #[cfg(feature = "std")]
        let now = UnixTime::now();
        #[cfg(not(feature = "std"))]
        let now = self
            .time_provider
            .current_time()
            .unwrap();

        self.maybe_roll(now)?
            .current
            .encrypt(message)
    }

    fn decrypt(&self, ciphertext: &[u8]) -> Option<Vec<u8>> {
        #[cfg(feature = "std")]
        let now = UnixTime::now();
        #[cfg(not(feature = "std"))]
        let now = self
            .time_provider
            .current_time()
            .unwrap();

        let state = self.maybe_roll(now)?;

        // Decrypt with the current key; if that fails, try with the previous.
        state
            .current
            .decrypt(ciphertext)
            .or_else(|| {
                state
                    .previous
                    .as_ref()
                    .and_then(|previous| previous.decrypt(ciphertext))
            })
    }
}

#[cfg(not(feature = "std"))]
impl core::fmt::Debug for TicketSwitcher {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TicketSwitcher")
            .field("generator", &self.generator)
            .field("lifetime", &self.lifetime)
            .field("state", &**self.state.lock().unwrap())
            .finish()
    }
}

#[cfg(feature = "std")]
#[derive(Debug)]
pub(crate) struct TicketRotatorState {
    current: Box<dyn ProducesTickets>,
    previous: Option<Box<dyn ProducesTickets>>,
    next_switch_time: u64,
}

/// A ticketer that has a 'current' sub-ticketer and a single
/// 'previous' ticketer.  It creates a new ticketer every so
/// often, demoting the current ticketer.
#[cfg(feature = "std")]
pub struct TicketRotator {
    pub(crate) generator: fn() -> Result<Box<dyn ProducesTickets>, rand::GetRandomFailed>,
    lifetime: u32,
    state: RwLock<TicketRotatorState>,
}

#[cfg(feature = "std")]
impl TicketRotator {
    /// Creates a new `TicketRotator`, which rotates through sub-ticketers
    /// based on the passage of time.
    ///
    /// `lifetime` is in seconds, and is how long the current ticketer
    /// is used to generate new tickets.  Tickets are accepted for no
    /// longer than twice this duration.  `generator` produces a new
    /// `ProducesTickets` implementation.
    pub fn new(
        lifetime: u32,
        generator: fn() -> Result<Box<dyn ProducesTickets>, rand::GetRandomFailed>,
    ) -> Result<Self, Error> {
        Ok(Self {
            generator,
            lifetime,
            state: RwLock::new(TicketRotatorState {
                current: generator()?,
                previous: None,
                next_switch_time: UnixTime::now()
                    .as_secs()
                    .saturating_add(u64::from(lifetime)),
            }),
        })
    }

    /// If it's time, demote the `current` ticketer to `previous` (so it
    /// does no new encryptions but can do decryption) and replace it
    /// with a new one.
    ///
    /// Calling this regularly will ensure timely key erasure.  Otherwise,
    /// key erasure will be delayed until the next encrypt/decrypt call.
    ///
    /// For efficiency, this is also responsible for locking the state rwlock
    /// and returning it for read.
    pub(crate) fn maybe_roll(
        &self,
        now: UnixTime,
    ) -> Option<RwLockReadGuard<'_, TicketRotatorState>> {
        let now = now.as_secs();

        // Fast, common, & read-only path in case we do not need to switch
        // to the next ticketer yet
        {
            let read = self.state.read().ok()?;

            if now <= read.next_switch_time {
                return Some(read);
            }
        }

        // We need to switch ticketers, and make a new one.
        // Generate a potential "next" ticketer outside the lock.
        let next = (self.generator)().ok()?;

        let mut write = self.state.write().ok()?;

        if now <= write.next_switch_time {
            // Another thread beat us to it.  Nothing to do.
            drop(write);

            return self.state.read().ok();
        }

        // Now we have:
        // - confirmed we need rotation
        // - confirmed we are the thread that will do it
        // - successfully made the replacement ticketer
        write.previous = Some(mem::replace(&mut write.current, next));
        write.next_switch_time = now.saturating_add(u64::from(self.lifetime));
        drop(write);

        self.state.read().ok()
    }
}

#[cfg(feature = "std")]
impl ProducesTickets for TicketRotator {
    fn lifetime(&self) -> u32 {
        self.lifetime * 2
    }

    fn enabled(&self) -> bool {
        true
    }

    fn encrypt(&self, message: &[u8]) -> Option<Vec<u8>> {
        self.maybe_roll(UnixTime::now())?
            .current
            .encrypt(message)
    }

    fn decrypt(&self, ciphertext: &[u8]) -> Option<Vec<u8>> {
        let state = self.maybe_roll(UnixTime::now())?;

        // Decrypt with the current key; if that fails, try with the previous.
        state
            .current
            .decrypt(ciphertext)
            .or_else(|| {
                state
                    .previous
                    .as_ref()
                    .and_then(|previous| previous.decrypt(ciphertext))
            })
    }
}

#[cfg(feature = "std")]
impl core::fmt::Debug for TicketRotator {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TicketRotator")
            .finish_non_exhaustive()
    }
}
