//! An exfiltrator providing the raw [`siginfo_t`].

// Note on unsafety in this module:
// * Implementing an unsafe trait, that one needs to ensure at least store is async-signal-safe.
//   That's done by delegating to the Channel (and reading an atomic pointer, but that one is
//   primitive op).
// * A bit of juggling with atomic and raw pointers. In effect, that is just late lazy
//   initialization, the Slot is in line with Option would be, except that it is set atomically
//   during the init. Lifetime is ensured by not dropping until the Drop of the whole slot and that
//   is checked by taking `&mut self`.

use std::sync::atomic::{AtomicPtr, Ordering};

use libc::{c_int, siginfo_t};

use super::sealed::Exfiltrator;
use crate::low_level::channel::Channel;

#[doc(hidden)]
#[derive(Default, Debug)]
pub struct Slot(AtomicPtr<Channel<siginfo_t>>);

impl Drop for Slot {
    fn drop(&mut self) {
        let ptr = self.0.load(Ordering::Acquire);
        if !ptr.is_null() {
            drop(unsafe { Box::from_raw(ptr) });
        }
    }
}

/// The [`Exfiltrator`][crate::iterator::exfiltrator::Exfiltrator] that produces the raw
/// [`libc::siginfo_t`]. Note that it might look differently on different OSes and its API is a
/// little bit more limited than its C counterpart.
///
/// You might prefer the [`WithOrigin`][super::WithOrigin] if you simply need information about the
/// origin of the signal.
///
/// # Examples
///
/// ```rust
/// # use signal_hook::consts::SIGUSR1;
/// # use signal_hook::iterator::SignalsInfo;
/// # use signal_hook::iterator::exfiltrator::WithRawSiginfo;
/// #
/// # fn main() -> Result<(), std::io::Error> {
/// // Subscribe to SIGUSR1, with information about the process.
/// let mut signals = SignalsInfo::<WithRawSiginfo>::new(&[SIGUSR1])?;
///
/// // Send ourselves a signal.
/// signal_hook::low_level::raise(SIGUSR1)?;
///
/// // Grab the signal and look into the details.
/// let received = signals.wait().next().unwrap();
///
/// // Not much else is useful in a cross-platform way :-(
/// assert_eq!(SIGUSR1, received.si_signo);
/// # Ok(()) }
/// ```
#[derive(Copy, Clone, Debug, Default)]
pub struct WithRawSiginfo;

unsafe impl Exfiltrator for WithRawSiginfo {
    type Storage = Slot;
    type Output = siginfo_t;

    fn supports_signal(&self, _: c_int) -> bool {
        true
    }

    fn store(&self, slot: &Slot, _: c_int, info: &siginfo_t) {
        let info = *info;
        // Condition just not to crash if someone forgot to call init.
        //
        // Lifetime is from init to our own drop, and drop needs &mut self.
        if let Some(slot) = unsafe { slot.0.load(Ordering::Acquire).as_ref() } {
            slot.send(info);
        }
    }

    fn load(&self, slot: &Slot, _: libc::c_int) -> Option<siginfo_t> {
        let slot = unsafe { slot.0.load(Ordering::Acquire).as_ref() };
        // Condition just not to crash if someone forgot to call init.
        slot.and_then(|s| s.recv())
    }

    fn init(&self, slot: &Self::Storage, _: c_int) {
        let new = Box::default();
        let old = slot.0.swap(Box::into_raw(new), Ordering::Release);
        // We leak the pointer on purpose here. This is invalid state anyway and must not happen,
        // but if it still does, we can't drop that while some other thread might still be having
        // the raw pointer.
        assert!(old.is_null(), "Init called multiple times");
    }
}
