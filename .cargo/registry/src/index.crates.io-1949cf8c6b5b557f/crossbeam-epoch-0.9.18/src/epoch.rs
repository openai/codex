//! The global epoch
//!
//! The last bit in this number is unused and is always zero. Every so often the global epoch is
//! incremented, i.e. we say it "advances". A pinned participant may advance the global epoch only
//! if all currently pinned participants have been pinned in the current epoch.
//!
//! If an object became garbage in some epoch, then we can be sure that after two advancements no
//! participant will hold a reference to it. That is the crux of safe memory reclamation.

use crate::primitive::sync::atomic::{AtomicUsize, Ordering};

/// An epoch that can be marked as pinned or unpinned.
///
/// Internally, the epoch is represented as an integer that wraps around at some unspecified point
/// and a flag that represents whether it is pinned or unpinned.
#[derive(Copy, Clone, Default, Debug, Eq, PartialEq)]
pub(crate) struct Epoch {
    /// The least significant bit is set if pinned. The rest of the bits hold the epoch.
    data: usize,
}

impl Epoch {
    /// Returns the starting epoch in unpinned state.
    #[inline]
    pub(crate) fn starting() -> Self {
        Self::default()
    }

    /// Returns the number of epochs `self` is ahead of `rhs`.
    ///
    /// Internally, epochs are represented as numbers in the range `(isize::MIN / 2) .. (isize::MAX
    /// / 2)`, so the returned distance will be in the same interval.
    pub(crate) fn wrapping_sub(self, rhs: Self) -> isize {
        // The result is the same with `(self.data & !1).wrapping_sub(rhs.data & !1) as isize >> 1`,
        // because the possible difference of LSB in `(self.data & !1).wrapping_sub(rhs.data & !1)`
        // will be ignored in the shift operation.
        self.data.wrapping_sub(rhs.data & !1) as isize >> 1
    }

    /// Returns `true` if the epoch is marked as pinned.
    #[inline]
    pub(crate) fn is_pinned(self) -> bool {
        (self.data & 1) == 1
    }

    /// Returns the same epoch, but marked as pinned.
    #[inline]
    pub(crate) fn pinned(self) -> Epoch {
        Epoch {
            data: self.data | 1,
        }
    }

    /// Returns the same epoch, but marked as unpinned.
    #[inline]
    pub(crate) fn unpinned(self) -> Epoch {
        Epoch {
            data: self.data & !1,
        }
    }

    /// Returns the successor epoch.
    ///
    /// The returned epoch will be marked as pinned only if the previous one was as well.
    #[inline]
    pub(crate) fn successor(self) -> Epoch {
        Epoch {
            data: self.data.wrapping_add(2),
        }
    }
}

/// An atomic value that holds an `Epoch`.
#[derive(Default, Debug)]
pub(crate) struct AtomicEpoch {
    /// Since `Epoch` is just a wrapper around `usize`, an `AtomicEpoch` is similarly represented
    /// using an `AtomicUsize`.
    data: AtomicUsize,
}

impl AtomicEpoch {
    /// Creates a new atomic epoch.
    #[inline]
    pub(crate) fn new(epoch: Epoch) -> Self {
        let data = AtomicUsize::new(epoch.data);
        AtomicEpoch { data }
    }

    /// Loads a value from the atomic epoch.
    #[inline]
    pub(crate) fn load(&self, ord: Ordering) -> Epoch {
        Epoch {
            data: self.data.load(ord),
        }
    }

    /// Stores a value into the atomic epoch.
    #[inline]
    pub(crate) fn store(&self, epoch: Epoch, ord: Ordering) {
        self.data.store(epoch.data, ord);
    }

    /// Stores a value into the atomic epoch if the current value is the same as `current`.
    ///
    /// The return value is a result indicating whether the new value was written and containing
    /// the previous value. On success this value is guaranteed to be equal to `current`.
    ///
    /// This method takes two `Ordering` arguments to describe the memory
    /// ordering of this operation. `success` describes the required ordering for the
    /// read-modify-write operation that takes place if the comparison with `current` succeeds.
    /// `failure` describes the required ordering for the load operation that takes place when
    /// the comparison fails. Using `Acquire` as success ordering makes the store part
    /// of this operation `Relaxed`, and using `Release` makes the successful load
    /// `Relaxed`. The failure ordering can only be `SeqCst`, `Acquire` or `Relaxed`
    /// and must be equivalent to or weaker than the success ordering.
    #[inline]
    pub(crate) fn compare_exchange(
        &self,
        current: Epoch,
        new: Epoch,
        success: Ordering,
        failure: Ordering,
    ) -> Result<Epoch, Epoch> {
        match self
            .data
            .compare_exchange(current.data, new.data, success, failure)
        {
            Ok(data) => Ok(Epoch { data }),
            Err(data) => Err(Epoch { data }),
        }
    }
}
