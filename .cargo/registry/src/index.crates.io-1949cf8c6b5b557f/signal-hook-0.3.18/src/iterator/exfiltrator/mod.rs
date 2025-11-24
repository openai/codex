//! An abstraction over exfiltrating information out of signal handlers.
//!
//! The [`Exfiltrator`] trait provides a way to abstract the information extracted from a signal
//! handler and the way it is extracted out of it.
//!
//! The implementations can be used to parametrize the
//! [`SignalsInfo`][crate::iterator::SignalsInfo] to specify what results are returned.
//!
//! # Sealed
//!
//! Currently, the trait is sealed and all methods hidden. This is likely temporary, until some
//! experience with them is gained.

#[cfg(feature = "extended-siginfo")]
#[cfg_attr(docsrs, doc(cfg(feature = "extended-siginfo")))]
pub mod origin;
pub mod raw;

#[cfg(feature = "extended-siginfo")]
pub use origin::WithOrigin;
pub use raw::WithRawSiginfo;

use std::sync::atomic::{AtomicBool, Ordering};

use libc::{c_int, siginfo_t};

mod sealed {
    use std::fmt::Debug;

    use libc::{c_int, siginfo_t};

    /// The actual implementation of the [`Exfiltrator`][super::Exfiltrator].
    ///
    /// For now, this is hidden from the public API, but the intention is to move it to a public
    /// place so users can implement it eventually, once we verify that it works well.
    ///
    /// # Safety
    ///
    /// The trait is unsafe as the [`Exfiltrator::store`] is called inside the signal handler and
    /// must be async-signal-safe. Implementing this correctly may be difficult, therefore care
    /// needs to be taken. One method known to work is encoding the data into an atomic variable.
    /// Other, less limiting approaches, will be eventually explored.
    pub unsafe trait Exfiltrator: Debug + Send + Sync + 'static {
        /// One slot for storing the data.
        ///
        /// Each signal will get its one slot of this type, independent of other signals. It can
        /// store the information in there inside the signal handler and will be loaded from it in
        /// load.
        ///
        /// Each slot is initialized to the [`Default`] value. It is expected this value represents
        /// „no signal delivered“ state.
        type Storage: Debug + Default + Send + Sync + 'static;

        /// The type returned to the user.
        type Output;

        /// If the given signal is supported by this specific exfiltrator.
        ///
        /// Not all information is available to all signals, therefore not all exfiltrators must
        /// support all signals. If `false` is returned, the user is prevented for registering such
        /// signal number with the given exfiltrator.
        fn supports_signal(&self, sig: c_int) -> bool;

        /// Puts the signal information inside the slot.
        ///
        /// It needs to somehow store the relevant information and the fact that a signal happened.
        ///
        /// # Warning
        ///
        /// This will be called inside the signal handler. It needs to be async-signal-safe. In
        /// particular, very small amount of operations are allowed in there. This namely does
        /// *not* include any locking nor allocation.
        ///
        /// It is also possible that multiple store methods are called concurrently; it is up to
        /// the implementor to deal with that.
        fn store(&self, slot: &Self::Storage, signal: c_int, info: &siginfo_t);

        /// Loads the signal information from the given slot.
        ///
        /// The method shall check if the signal happened (it may be possible to be called without
        /// the signal previously being delivered; it is up to the implementer to recognize it). It
        /// is assumed the [`Default`] value is recognized as no signal delivered.
        ///
        /// If it was delivered, the method shall extract the relevant information *and reset the
        /// slot* to the no signal delivered state.
        ///
        /// It shall return `Some(value)` if the signal was successfully received and `None` in
        /// case no signal was delivered.
        ///
        /// No blocking shall happen inside this method. It may be called concurrently with
        /// [`store`][Exfiltrator::store] (due to how signals work, concurrently even inside the
        /// same thread ‒ a `store` may „interrupt“ a call to `load`). It is up to the implementer
        /// to deal with that.
        fn load(&self, slot: &Self::Storage, signal: c_int) -> Option<Self::Output>;

        /// Initialize the given slot for the given signal before the first use.
        ///
        /// This is called before the first use of the given slot (and it is annotated with the
        /// corresponding signal). The default does nothing, this is just an opportunity to
        /// allocate data lazily (this is called outside of the signal handler, so it doesn't have
        /// to be async-signal-safe). It will be called at most once for each slot.
        ///
        /// Note that you can rely on this being called for correctness, but not for safety (this
        /// crate calls it before the first use, but a user abusing the trait might not and in such
        /// case it is OK to eg. lose signals, but not segfault).
        fn init(&self, slot: &Self::Storage, signal: c_int) {
            // Suppress unused variable warning without putting the underscores into public
            // signature.
            let _ = slot;
            let _ = signal;
        }
    }
}

/// A trait describing what and how is extracted from signal handlers.
///
/// By choosing a specific implementor as the type parameter for
/// [`SignalsInfo`][crate::iterator::SignalsInfo], one can pick how much and what information is
/// returned from the iterator.
pub trait Exfiltrator: sealed::Exfiltrator {}

impl<E: sealed::Exfiltrator> Exfiltrator for E {}

/// An [`Exfiltrator`] providing just the signal numbers.
///
/// This is the basic exfiltrator for most needs. For that reason, there's the
/// [`crate::iterator::Signals`] type alias, to simplify the type names for usual needs.
#[derive(Clone, Copy, Debug, Default)]
pub struct SignalOnly;

unsafe impl sealed::Exfiltrator for SignalOnly {
    type Storage = AtomicBool;
    fn supports_signal(&self, _: c_int) -> bool {
        true
    }
    type Output = c_int;

    fn store(&self, slot: &Self::Storage, _: c_int, _: &siginfo_t) {
        slot.store(true, Ordering::SeqCst);
    }

    fn load(&self, slot: &Self::Storage, signal: c_int) -> Option<Self::Output> {
        if slot
            .compare_exchange(true, false, Ordering::SeqCst, Ordering::Relaxed)
            .is_ok()
        {
            Some(signal)
        } else {
            None
        }
    }
}
