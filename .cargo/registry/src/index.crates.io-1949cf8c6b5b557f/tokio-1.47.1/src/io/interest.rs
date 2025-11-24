#![cfg_attr(not(feature = "net"), allow(dead_code, unreachable_pub))]

use crate::io::ready::Ready;

use std::fmt;
use std::ops;

// These must be unique.
// same as mio
const READABLE: usize = 0b0001;
const WRITABLE: usize = 0b0010;
// The following are not available on all platforms.
#[cfg(target_os = "freebsd")]
const AIO: usize = 0b0100;
#[cfg(target_os = "freebsd")]
const LIO: usize = 0b1000;
#[cfg(any(target_os = "linux", target_os = "android"))]
const PRIORITY: usize = 0b0001_0000;
// error is available on all platforms, but behavior is platform-specific
// mio does not have this interest
const ERROR: usize = 0b0010_0000;

/// Readiness event interest.
///
/// Specifies the readiness events the caller is interested in when awaiting on
/// I/O resource readiness states.
#[cfg_attr(docsrs, doc(cfg(feature = "net")))]
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct Interest(usize);

impl Interest {
    // The non-FreeBSD definitions in this block are active only when
    // building documentation.
    cfg_aio! {
        /// Interest for POSIX AIO.
        #[cfg(target_os = "freebsd")]
        pub const AIO: Interest = Interest(AIO);

        /// Interest for POSIX AIO.
        #[cfg(not(target_os = "freebsd"))]
        pub const AIO: Interest = Interest(READABLE);

        /// Interest for POSIX AIO `lio_listio` events.
        #[cfg(target_os = "freebsd")]
        pub const LIO: Interest = Interest(LIO);

        /// Interest for POSIX AIO `lio_listio` events.
        #[cfg(not(target_os = "freebsd"))]
        pub const LIO: Interest = Interest(READABLE);
    }

    /// Interest in all readable events.
    ///
    /// Readable interest includes read-closed events.
    pub const READABLE: Interest = Interest(READABLE);

    /// Interest in all writable events.
    ///
    /// Writable interest includes write-closed events.
    pub const WRITABLE: Interest = Interest(WRITABLE);

    /// Interest in error events.
    ///
    /// Passes error interest to the underlying OS selector.
    /// Behavior is platform-specific, read your platform's documentation.
    pub const ERROR: Interest = Interest(ERROR);

    /// Returns a `Interest` set representing priority completion interests.
    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[cfg_attr(docsrs, doc(cfg(any(target_os = "linux", target_os = "android"))))]
    pub const PRIORITY: Interest = Interest(PRIORITY);

    /// Returns true if the value includes readable interest.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::io::Interest;
    ///
    /// assert!(Interest::READABLE.is_readable());
    /// assert!(!Interest::WRITABLE.is_readable());
    ///
    /// let both = Interest::READABLE | Interest::WRITABLE;
    /// assert!(both.is_readable());
    /// ```
    pub const fn is_readable(self) -> bool {
        self.0 & READABLE != 0
    }

    /// Returns true if the value includes writable interest.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::io::Interest;
    ///
    /// assert!(!Interest::READABLE.is_writable());
    /// assert!(Interest::WRITABLE.is_writable());
    ///
    /// let both = Interest::READABLE | Interest::WRITABLE;
    /// assert!(both.is_writable());
    /// ```
    pub const fn is_writable(self) -> bool {
        self.0 & WRITABLE != 0
    }

    /// Returns true if the value includes error interest.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::io::Interest;
    ///
    /// assert!(Interest::ERROR.is_error());
    /// assert!(!Interest::WRITABLE.is_error());
    ///
    /// let combined = Interest::READABLE | Interest::ERROR;
    /// assert!(combined.is_error());
    /// ```
    pub const fn is_error(self) -> bool {
        self.0 & ERROR != 0
    }

    #[cfg(target_os = "freebsd")]
    const fn is_aio(self) -> bool {
        self.0 & AIO != 0
    }

    #[cfg(target_os = "freebsd")]
    const fn is_lio(self) -> bool {
        self.0 & LIO != 0
    }

    /// Returns true if the value includes priority interest.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::io::Interest;
    ///
    /// assert!(!Interest::READABLE.is_priority());
    /// assert!(Interest::PRIORITY.is_priority());
    ///
    /// let both = Interest::READABLE | Interest::PRIORITY;
    /// assert!(both.is_priority());
    /// ```
    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[cfg_attr(docsrs, doc(cfg(any(target_os = "linux", target_os = "android"))))]
    pub const fn is_priority(self) -> bool {
        self.0 & PRIORITY != 0
    }

    /// Add together two `Interest` values.
    ///
    /// This function works from a `const` context.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::io::Interest;
    ///
    /// const BOTH: Interest = Interest::READABLE.add(Interest::WRITABLE);
    ///
    /// assert!(BOTH.is_readable());
    /// assert!(BOTH.is_writable());
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub const fn add(self, other: Interest) -> Interest {
        Self(self.0 | other.0)
    }

    /// Remove `Interest` from `self`.
    ///
    /// Interests present in `other` but *not* in `self` are ignored.
    ///
    /// Returns `None` if the set would be empty after removing `Interest`.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::io::Interest;
    ///
    /// const RW_INTEREST: Interest = Interest::READABLE.add(Interest::WRITABLE);
    ///
    /// let w_interest = RW_INTEREST.remove(Interest::READABLE).unwrap();
    /// assert!(!w_interest.is_readable());
    /// assert!(w_interest.is_writable());
    ///
    /// // Removing all interests from the set returns `None`.
    /// assert_eq!(w_interest.remove(Interest::WRITABLE), None);
    ///
    /// // Remove all interests at once.
    /// assert_eq!(RW_INTEREST.remove(RW_INTEREST), None);
    /// ```
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub fn remove(self, other: Interest) -> Option<Interest> {
        let value = self.0 & !other.0;

        if value != 0 {
            Some(Self(value))
        } else {
            None
        }
    }

    // This function must be crate-private to avoid exposing a `mio` dependency.
    pub(crate) fn to_mio(self) -> mio::Interest {
        fn mio_add(wrapped: &mut Option<mio::Interest>, add: mio::Interest) {
            match wrapped {
                Some(inner) => *inner |= add,
                None => *wrapped = Some(add),
            }
        }

        // mio does not allow and empty interest, so use None for empty
        let mut mio = None;

        if self.is_readable() {
            mio_add(&mut mio, mio::Interest::READABLE);
        }

        if self.is_writable() {
            mio_add(&mut mio, mio::Interest::WRITABLE);
        }

        #[cfg(any(target_os = "linux", target_os = "android"))]
        if self.is_priority() {
            mio_add(&mut mio, mio::Interest::PRIORITY);
        }

        #[cfg(target_os = "freebsd")]
        if self.is_aio() {
            mio_add(&mut mio, mio::Interest::AIO);
        }

        #[cfg(target_os = "freebsd")]
        if self.is_lio() {
            mio_add(&mut mio, mio::Interest::LIO);
        }

        if self.is_error() {
            // There is no error interest in mio, because error events are always reported.
            // But mio interests cannot be empty and an interest is needed just for the registration.
            //
            // read readiness is filtered out in `Interest::mask` or `Ready::from_interest` if
            // the read interest was not specified by the user.
            mio_add(&mut mio, mio::Interest::READABLE);
        }

        // the default `mio::Interest::READABLE` should never be used in practice. Either
        //
        // - at least one tokio interest with a mio counterpart was used
        // - only the error tokio interest was specified
        //
        // in both cases, `mio` is Some already
        mio.unwrap_or(mio::Interest::READABLE)
    }

    pub(crate) fn mask(self) -> Ready {
        match self {
            Interest::READABLE => Ready::READABLE | Ready::READ_CLOSED,
            Interest::WRITABLE => Ready::WRITABLE | Ready::WRITE_CLOSED,
            #[cfg(any(target_os = "linux", target_os = "android"))]
            Interest::PRIORITY => Ready::PRIORITY | Ready::READ_CLOSED,
            Interest::ERROR => Ready::ERROR,
            _ => Ready::EMPTY,
        }
    }
}

impl ops::BitOr for Interest {
    type Output = Self;

    #[inline]
    fn bitor(self, other: Self) -> Self {
        self.add(other)
    }
}

impl ops::BitOrAssign for Interest {
    #[inline]
    fn bitor_assign(&mut self, other: Self) {
        *self = *self | other;
    }
}

impl fmt::Debug for Interest {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut separator = false;

        if self.is_readable() {
            if separator {
                write!(fmt, " | ")?;
            }
            write!(fmt, "READABLE")?;
            separator = true;
        }

        if self.is_writable() {
            if separator {
                write!(fmt, " | ")?;
            }
            write!(fmt, "WRITABLE")?;
            separator = true;
        }

        #[cfg(any(target_os = "linux", target_os = "android"))]
        if self.is_priority() {
            if separator {
                write!(fmt, " | ")?;
            }
            write!(fmt, "PRIORITY")?;
            separator = true;
        }

        #[cfg(target_os = "freebsd")]
        if self.is_aio() {
            if separator {
                write!(fmt, " | ")?;
            }
            write!(fmt, "AIO")?;
            separator = true;
        }

        #[cfg(target_os = "freebsd")]
        if self.is_lio() {
            if separator {
                write!(fmt, " | ")?;
            }
            write!(fmt, "LIO")?;
            separator = true;
        }

        if self.is_error() {
            if separator {
                write!(fmt, " | ")?;
            }
            write!(fmt, "ERROR")?;
            separator = true;
        }

        let _ = separator;

        Ok(())
    }
}
