//! An exfiltrator providing the process that caused the signal.
//!
//! The [`WithOrigin`] is an [`Exfiltrator`][crate::iterator::exfiltrator::Exfiltrator] that
//! provides the information about sending process in addition to the signal number, through the
//! [`Origin`] type.
//!
//! See the [`WithOrigin`] example.

use libc::{c_int, siginfo_t};

pub use super::raw::Slot;
use super::sealed::Exfiltrator;
use super::WithRawSiginfo;
pub use crate::low_level::siginfo::{Origin, Process};

/// The [`Exfiltrator`][crate::iterator::exfiltrator::Exfiltrator] that produces [`Origin`] of
/// signals.
///
/// # Examples
///
/// ```rust
/// # use signal_hook::consts::SIGUSR1;
/// # use signal_hook::iterator::SignalsInfo;
/// # use signal_hook::iterator::exfiltrator::WithOrigin;
/// #
/// # fn main() -> Result<(), std::io::Error> {
/// // Subscribe to SIGUSR1, with information about the process.
/// let mut signals = SignalsInfo::<WithOrigin>::new(&[SIGUSR1])?;
///
/// // Send a signal to ourselves.
/// let my_pid = unsafe { libc::getpid() };
/// unsafe { libc::kill(my_pid, SIGUSR1) };
///
/// // Grab the signal and look into the details.
/// let received = signals.wait().next().unwrap();
///
/// assert_eq!(SIGUSR1, received.signal);
/// assert_eq!(my_pid, received.process.unwrap().pid);
/// # Ok(()) }
/// ```
#[derive(Copy, Clone, Debug, Default)]
pub struct WithOrigin(WithRawSiginfo);

// Safety: We need to be async-signal-safe. We delegate to other Exfiltrator, which already is and
// call a function that promises to be (Origin::extract)
unsafe impl Exfiltrator for WithOrigin {
    type Storage = Slot;
    type Output = Origin;
    fn supports_signal(&self, signal: c_int) -> bool {
        self.0.supports_signal(signal)
    }

    fn store(&self, slot: &Slot, signal: c_int, info: &siginfo_t) {
        self.0.store(slot, signal, info)
    }

    fn load(&self, slot: &Self::Storage, signal: c_int) -> Option<Origin> {
        self.0
            .load(slot, signal)
            .map(|info| unsafe { Origin::extract(&info) })
    }

    fn init(&self, slot: &Self::Storage, signal: c_int) {
        self.0.init(slot, signal)
    }
}
