//! The [`KernelSigSet`] type.

#![allow(unsafe_code)]
#![allow(non_camel_case_types)]

use crate::backend::c;
use crate::signal::Signal;
use core::fmt;
use linux_raw_sys::general::{kernel_sigset_t, _NSIG};

/// `kernel_sigset_t`—A set of signal numbers, as used by some syscalls.
///
/// This is similar to `libc::sigset_t`, but with only enough space for the
/// signals currently known to be used by the kernel. libc implementations
/// reserve extra space so that if Linux defines new signals in the future
/// they can add support without breaking their dynamic linking ABI. Rustix
/// doesn't support a dynamic linking ABI, so if we need to increase the
/// size of `KernelSigSet` in the future, we can do so.
///
/// It's also the case that the last time Linux changed the size of its
/// `kernel_sigset_t` was when it added support for POSIX.1b signals in 1999.
///
/// `KernelSigSet` is guaranteed to have a subset of the layout of
/// `libc::sigset_t`.
///
/// libc implementations typically also reserve some signal values for internal
/// use. In a process that contains a libc, some unsafe functions invoke
/// undefined behavior if passed a `KernelSigSet` that contains one of the
/// signals that the libc reserves.
#[repr(transparent)]
#[derive(Clone)]
pub struct KernelSigSet(kernel_sigset_t);

impl KernelSigSet {
    /// Create a new empty `KernelSigSet`.
    pub const fn empty() -> Self {
        const fn zeros<const N: usize>() -> [c::c_ulong; N] {
            [0; N]
        }
        Self(kernel_sigset_t { sig: zeros() })
    }

    /// Create a new `KernelSigSet` with all signals set.
    ///
    /// This includes signals which are typically reserved for libc.
    pub const fn all() -> Self {
        const fn ones<const N: usize>() -> [c::c_ulong; N] {
            [!0; N]
        }
        Self(kernel_sigset_t { sig: ones() })
    }

    /// Remove all signals.
    pub fn clear(&mut self) {
        *self = Self(kernel_sigset_t {
            sig: Default::default(),
        });
    }

    /// Insert a signal.
    pub fn insert(&mut self, sig: Signal) {
        let sigs_per_elt = core::mem::size_of_val(&self.0.sig[0]) * 8;

        let raw = (sig.as_raw().wrapping_sub(1)) as usize;
        self.0.sig[raw / sigs_per_elt] |= 1 << (raw % sigs_per_elt);
    }

    /// Insert all signals.
    pub fn insert_all(&mut self) {
        self.0.sig.fill(!0);
    }

    /// Remove a signal.
    pub fn remove(&mut self, sig: Signal) {
        let sigs_per_elt = core::mem::size_of_val(&self.0.sig[0]) * 8;

        let raw = (sig.as_raw().wrapping_sub(1)) as usize;
        self.0.sig[raw / sigs_per_elt] &= !(1 << (raw % sigs_per_elt));
    }

    /// Test whether a given signal is present.
    pub fn contains(&self, sig: Signal) -> bool {
        let sigs_per_elt = core::mem::size_of_val(&self.0.sig[0]) * 8;

        let raw = (sig.as_raw().wrapping_sub(1)) as usize;
        (self.0.sig[raw / sigs_per_elt] & (1 << (raw % sigs_per_elt))) != 0
    }
}

impl Default for KernelSigSet {
    #[inline]
    fn default() -> Self {
        Self::empty()
    }
}

impl fmt::Debug for KernelSigSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut d = f.debug_set();

        // Surprisingly, `_NSIG` is inclusive.
        for i in 1..=_NSIG {
            // SAFETY: This value is non-zero, in range, and only used for
            // debug output.
            let sig = unsafe { Signal::from_raw_unchecked(i as _) };

            if self.contains(sig) {
                d.entry(&sig);
            }
        }

        d.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(linux_raw)]
    use crate::runtime::{KERNEL_SIGRTMAX, KERNEL_SIGRTMIN};
    use core::mem::{align_of, size_of};

    #[test]
    fn test_assumptions() {
        #[cfg(linux_raw)]
        assert!(KERNEL_SIGRTMAX as usize - 1 < size_of::<KernelSigSet>() * 8);
    }

    #[test]
    fn test_layouts() {
        assert!(size_of::<KernelSigSet>() <= size_of::<libc::sigset_t>());
        assert!(align_of::<KernelSigSet>() <= align_of::<libc::sigset_t>());
    }

    /// A bunch of signals for testing.
    fn sigs() -> Vec<Signal> {
        #[allow(unused_mut)]
        let mut sigs = vec![
            Signal::HUP,
            Signal::INT,
            Signal::QUIT,
            Signal::ILL,
            Signal::TRAP,
            Signal::ABORT,
            Signal::BUS,
            Signal::FPE,
            Signal::KILL,
            Signal::USR1,
            Signal::SEGV,
            Signal::USR2,
            Signal::PIPE,
            Signal::ALARM,
            Signal::TERM,
            Signal::CHILD,
            Signal::CONT,
            Signal::STOP,
            Signal::TSTP,
            Signal::TTIN,
            Signal::TTOU,
            Signal::URG,
            Signal::XCPU,
            Signal::XFSZ,
            Signal::VTALARM,
            Signal::PROF,
            Signal::WINCH,
            Signal::SYS,
            unsafe { Signal::from_raw_unchecked(libc::SIGRTMIN()) },
            unsafe { Signal::from_raw_unchecked(libc::SIGRTMIN() + 7) },
            unsafe { Signal::from_raw_unchecked(libc::SIGRTMAX()) },
        ];

        #[cfg(linux_raw)]
        {
            sigs.push(unsafe { Signal::from_raw_unchecked(KERNEL_SIGRTMIN) });
            sigs.push(unsafe { Signal::from_raw_unchecked(KERNEL_SIGRTMIN + 7) });
            sigs.push(unsafe { Signal::from_raw_unchecked(KERNEL_SIGRTMAX) });
        }

        sigs
    }

    /// A bunch of non-reserved signals for testing.
    fn libc_sigs() -> [Signal; 31] {
        [
            Signal::HUP,
            Signal::INT,
            Signal::QUIT,
            Signal::ILL,
            Signal::TRAP,
            Signal::ABORT,
            Signal::BUS,
            Signal::FPE,
            Signal::KILL,
            Signal::USR1,
            Signal::SEGV,
            Signal::USR2,
            Signal::PIPE,
            Signal::ALARM,
            Signal::TERM,
            Signal::CHILD,
            Signal::CONT,
            Signal::STOP,
            Signal::TSTP,
            Signal::TTIN,
            Signal::TTOU,
            Signal::URG,
            Signal::XCPU,
            Signal::XFSZ,
            Signal::VTALARM,
            Signal::PROF,
            Signal::WINCH,
            Signal::SYS,
            unsafe { Signal::from_raw_unchecked(libc::SIGRTMIN()) },
            unsafe { Signal::from_raw_unchecked(libc::SIGRTMIN() + 7) },
            unsafe { Signal::from_raw_unchecked(libc::SIGRTMAX()) },
        ]
    }

    #[test]
    fn test_ops_plain() {
        for sig in sigs() {
            let mut set = KernelSigSet::empty();
            for sig in sigs() {
                assert!(!set.contains(sig));
            }

            set.insert(sig);
            assert!(set.contains(sig));
            for sig in sigs().iter().filter(|s| **s != sig) {
                assert!(!set.contains(*sig));
            }

            set.remove(sig);
            for sig in sigs() {
                assert!(!set.contains(sig));
            }
        }
    }

    #[test]
    fn test_clear() {
        let mut set = KernelSigSet::empty();
        for sig in sigs() {
            set.insert(sig);
        }

        set.clear();

        for sig in sigs() {
            assert!(!set.contains(sig));
        }
    }

    // io_uring libraries assume that libc's `sigset_t` matches the layout
    // of the Linux kernel's `kernel_sigset_t`. Test that rustix's layout
    // matches as well.
    #[test]
    fn test_libc_layout_compatibility() {
        use crate::utils::as_ptr;

        let mut lc = unsafe { core::mem::zeroed::<libc::sigset_t>() };
        let mut ru = KernelSigSet::empty();
        let r = unsafe { libc::sigemptyset(&mut lc) };

        assert_eq!(r, 0);
        assert_eq!(
            unsafe {
                libc::memcmp(
                    as_ptr(&lc).cast(),
                    as_ptr(&ru).cast(),
                    core::mem::size_of::<KernelSigSet>(),
                )
            },
            0
        );

        for sig in libc_sigs() {
            ru.insert(sig);
            assert_ne!(
                unsafe {
                    libc::memcmp(
                        as_ptr(&lc).cast(),
                        as_ptr(&ru).cast(),
                        core::mem::size_of::<KernelSigSet>(),
                    )
                },
                0
            );
            let r = unsafe { libc::sigaddset(&mut lc, sig.as_raw()) };
            assert_eq!(r, 0);
            assert_eq!(
                unsafe {
                    libc::memcmp(
                        as_ptr(&lc).cast(),
                        as_ptr(&ru).cast(),
                        core::mem::size_of::<KernelSigSet>(),
                    )
                },
                0
            );
            ru.remove(sig);
            assert_ne!(
                unsafe {
                    libc::memcmp(
                        as_ptr(&lc).cast(),
                        as_ptr(&ru).cast(),
                        core::mem::size_of::<KernelSigSet>(),
                    )
                },
                0
            );
            let r = unsafe { libc::sigdelset(&mut lc, sig.as_raw()) };
            assert_eq!(r, 0);
            assert_eq!(
                unsafe {
                    libc::memcmp(
                        as_ptr(&lc).cast(),
                        as_ptr(&ru).cast(),
                        core::mem::size_of::<KernelSigSet>(),
                    )
                },
                0
            );
        }
    }
}
