// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use core::{marker::Copy, mem::size_of};

#[cfg(feature = "alloc")]
use crate::map::ZeroMapKV;
#[cfg(feature = "alloc")]
use crate::{ZeroSlice, ZeroVec};

use super::{AsULE, ULE};

/// The [`ULE`] types implementing this trait guarantee that [`NicheBytes::NICHE_BIT_PATTERN`]
/// can never occur as a valid byte representation of the type.
///
/// Guarantees for a valid implementation.
/// 1. N must be equal to `core::mem::sizeo_of::<Self>()` or else it will
///    cause panics.
/// 2. The bit pattern [`NicheBytes::NICHE_BIT_PATTERN`] must not be incorrect as it would lead to
///    weird behaviour.
/// 3. The abstractions built on top of this trait must panic on an invalid N.
/// 4. The abstractions built on this trait that use type punning must ensure that type being
///    punned is [`ULE`].
pub trait NicheBytes<const N: usize> {
    const NICHE_BIT_PATTERN: [u8; N];
}

/// [`ULE`] type for [`NichedOption<U,N>`] where U implements [`NicheBytes`].
/// The invalid bit pattern is used as the niche.
///
/// This uses 1 byte less than [`crate::ule::OptionULE<U>`] to represent [`NichedOption<U,N>`].
///
/// # Example
///
/// ```
/// use core::num::NonZeroI8;
/// use zerovec::ule::NichedOption;
/// use zerovec::ZeroVec;
///
/// let bytes = &[0x00, 0x01, 0x02, 0x00];
/// let zv_no: ZeroVec<NichedOption<NonZeroI8, 1>> =
///     ZeroVec::parse_bytes(bytes).expect("Unable to parse as NichedOption.");
///
/// assert_eq!(zv_no.get(0).map(|e| e.0), Some(None));
/// assert_eq!(zv_no.get(1).map(|e| e.0), Some(NonZeroI8::new(1)));
/// assert_eq!(zv_no.get(2).map(|e| e.0), Some(NonZeroI8::new(2)));
/// assert_eq!(zv_no.get(3).map(|e| e.0), Some(None));
/// ```
// Invariants:
// The union stores [`NicheBytes::NICHE_BIT_PATTERN`] when None.
// Any other bit pattern is a valid.
#[repr(C)]
pub union NichedOptionULE<U: NicheBytes<N> + ULE, const N: usize> {
    /// Invariant: The value is `niche` only if the bytes equal NICHE_BIT_PATTERN.
    niche: [u8; N],
    /// Invariant: The value is `valid` if the `niche` field does not match NICHE_BIT_PATTERN.
    valid: U,
}

impl<U: NicheBytes<N> + ULE + core::fmt::Debug, const N: usize> core::fmt::Debug
    for NichedOptionULE<U, N>
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.get().fmt(f)
    }
}

impl<U: NicheBytes<N> + ULE, const N: usize> NichedOptionULE<U, N> {
    /// New `NichedOptionULE<U, N>` from `Option<U>`
    pub fn new(opt: Option<U>) -> Self {
        assert!(N == core::mem::size_of::<U>());
        match opt {
            Some(u) => Self { valid: u },
            None => Self {
                niche: <U as NicheBytes<N>>::NICHE_BIT_PATTERN,
            },
        }
    }

    /// Convert to an `Option<U>`
    pub fn get(self) -> Option<U> {
        // Safety: The union stores NICHE_BIT_PATTERN when None otherwise a valid U
        unsafe {
            if self.niche == <U as NicheBytes<N>>::NICHE_BIT_PATTERN {
                None
            } else {
                Some(self.valid)
            }
        }
    }

    /// Borrows as an `Option<&U>`.
    pub fn as_ref(&self) -> Option<&U> {
        // Safety: The union stores NICHE_BIT_PATTERN when None otherwise a valid U
        unsafe {
            if self.niche == <U as NicheBytes<N>>::NICHE_BIT_PATTERN {
                None
            } else {
                Some(&self.valid)
            }
        }
    }
}

impl<U: NicheBytes<N> + ULE, const N: usize> Copy for NichedOptionULE<U, N> {}

impl<U: NicheBytes<N> + ULE, const N: usize> Clone for NichedOptionULE<U, N> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<U: NicheBytes<N> + ULE + PartialEq, const N: usize> PartialEq for NichedOptionULE<U, N> {
    fn eq(&self, other: &Self) -> bool {
        self.get().eq(&other.get())
    }
}

impl<U: NicheBytes<N> + ULE + Eq, const N: usize> Eq for NichedOptionULE<U, N> {}

/// Safety for ULE trait
/// 1. NichedOptionULE does not have any padding bytes due to `#[repr(C)]` on a struct
///    containing only ULE fields.
///    NichedOptionULE either contains NICHE_BIT_PATTERN or valid U byte sequences.
///    In both cases the data is initialized.
/// 2. NichedOptionULE is aligned to 1 byte due to `#[repr(C, packed)]` on a struct containing only
///    ULE fields.
/// 3. validate_bytes impl returns an error if invalid bytes are encountered.
/// 4. validate_bytes impl returns an error there are extra bytes.
/// 5. The other ULE methods are left to their default impl.
/// 6. NichedOptionULE equality is based on ULE equality of the subfield, assuming that NicheBytes
///    has been implemented correctly (this is a correctness but not a safety guarantee).
unsafe impl<U: NicheBytes<N> + ULE, const N: usize> ULE for NichedOptionULE<U, N> {
    fn validate_bytes(bytes: &[u8]) -> Result<(), crate::ule::UleError> {
        let size = size_of::<Self>();
        // The implemention is only correct if NICHE_BIT_PATTERN has same number of bytes as the
        // type.
        debug_assert!(N == core::mem::size_of::<U>());

        // The bytes should fully transmute to a collection of Self
        if bytes.len() % size != 0 {
            return Err(crate::ule::UleError::length::<Self>(bytes.len()));
        }
        bytes.chunks(size).try_for_each(|chunk| {
            // Associated const cannot be referenced in a pattern
            // https://doc.rust-lang.org/error-index.html#E0158
            if chunk == <U as NicheBytes<N>>::NICHE_BIT_PATTERN {
                Ok(())
            } else {
                U::validate_bytes(chunk)
            }
        })
    }
}

/// Optional type which uses [`NichedOptionULE<U,N>`] as ULE type.
///
/// The implementors guarantee that `N == core::mem::size_of::<Self>()`
/// `#[repr(transparent)]` guarantees that the layout is same as [`Option<U>`]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
#[allow(clippy::exhaustive_structs)] // newtype
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NichedOption<U, const N: usize>(pub Option<U>);

impl<U, const N: usize> Default for NichedOption<U, N> {
    fn default() -> Self {
        Self(None)
    }
}

impl<U: AsULE, const N: usize> AsULE for NichedOption<U, N>
where
    U::ULE: NicheBytes<N>,
{
    type ULE = NichedOptionULE<U::ULE, N>;

    fn to_unaligned(self) -> Self::ULE {
        NichedOptionULE::new(self.0.map(U::to_unaligned))
    }

    fn from_unaligned(unaligned: Self::ULE) -> Self {
        Self(unaligned.get().map(U::from_unaligned))
    }
}

#[cfg(feature = "alloc")]
impl<'a, T: AsULE + 'static, const N: usize> ZeroMapKV<'a> for NichedOption<T, N>
where
    T::ULE: NicheBytes<N>,
{
    type Container = ZeroVec<'a, NichedOption<T, N>>;
    type Slice = ZeroSlice<NichedOption<T, N>>;
    type GetType = <NichedOption<T, N> as AsULE>::ULE;
    type OwnedType = Self;
}

impl<T, const N: usize> IntoIterator for NichedOption<T, N> {
    type IntoIter = <Option<T> as IntoIterator>::IntoIter;
    type Item = T;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}
