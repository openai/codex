// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::{TinyAsciiStr, UnvalidatedTinyAsciiStr};
#[cfg(feature = "alloc")]
use zerovec::maps::ZeroMapKV;
use zerovec::ule::*;
#[cfg(feature = "alloc")]
use zerovec::{ZeroSlice, ZeroVec};

// Safety (based on the safety checklist on the ULE trait):
//  1. TinyAsciiStr does not include any uninitialized or padding bytes.
//     (achieved by `#[repr(transparent)]` on a type that satisfies this invariant)
//  2. TinyAsciiStr is aligned to 1 byte.
//     (achieved by `#[repr(transparent)]` on a type that satisfies this invariant)
//  3. The impl of validate_bytes() returns an error if any byte is not valid.
//  4. The impl of validate_bytes() returns an error if there are extra bytes.
//  5. The other ULE methods use the default impl.
//  6. TinyAsciiStr byte equality is semantic equality
unsafe impl<const N: usize> ULE for TinyAsciiStr<N> {
    #[inline]
    fn validate_bytes(bytes: &[u8]) -> Result<(), UleError> {
        if bytes.len() % N != 0 {
            return Err(UleError::length::<Self>(bytes.len()));
        }
        // Validate the bytes
        for chunk in bytes.chunks_exact(N) {
            let _ = TinyAsciiStr::<N>::try_from_utf8_inner(chunk, true)
                .map_err(|_| UleError::parse::<Self>())?;
        }
        Ok(())
    }
}

impl<const N: usize> NicheBytes<N> for TinyAsciiStr<N> {
    // AsciiByte is 0..128
    const NICHE_BIT_PATTERN: [u8; N] = [255; N];
}

impl<const N: usize> AsULE for TinyAsciiStr<N> {
    type ULE = Self;

    #[inline]
    fn to_unaligned(self) -> Self::ULE {
        self
    }

    #[inline]
    fn from_unaligned(unaligned: Self::ULE) -> Self {
        unaligned
    }
}

#[cfg(feature = "alloc")]
impl<'a, const N: usize> ZeroMapKV<'a> for TinyAsciiStr<N> {
    type Container = ZeroVec<'a, TinyAsciiStr<N>>;
    type Slice = ZeroSlice<TinyAsciiStr<N>>;
    type GetType = TinyAsciiStr<N>;
    type OwnedType = TinyAsciiStr<N>;
}

// Safety (based on the safety checklist on the ULE trait):
//  1. UnvalidatedTinyAsciiStr does not include any uninitialized or padding bytes.
//     (achieved by `#[repr(transparent)]` on a type that satisfies this invariant)
//  2. UnvalidatedTinyAsciiStr is aligned to 1 byte.
//     (achieved by `#[repr(transparent)]` on a type that satisfies this invariant)
//  3. The impl of validate_bytes() returns an error if any byte is not valid.
//  4. The impl of validate_bytes() returns an error if there are extra bytes.
//  5. The other ULE methods use the default impl.
//  6. UnvalidatedTinyAsciiStr byte equality is semantic equality
unsafe impl<const N: usize> ULE for UnvalidatedTinyAsciiStr<N> {
    #[inline]
    fn validate_bytes(bytes: &[u8]) -> Result<(), UleError> {
        if bytes.len() % N != 0 {
            return Err(UleError::length::<Self>(bytes.len()));
        }
        Ok(())
    }
}

impl<const N: usize> AsULE for UnvalidatedTinyAsciiStr<N> {
    type ULE = Self;

    #[inline]
    fn to_unaligned(self) -> Self::ULE {
        self
    }

    #[inline]
    fn from_unaligned(unaligned: Self::ULE) -> Self {
        unaligned
    }
}

#[cfg(feature = "alloc")]
impl<'a, const N: usize> ZeroMapKV<'a> for UnvalidatedTinyAsciiStr<N> {
    type Container = ZeroVec<'a, UnvalidatedTinyAsciiStr<N>>;
    type Slice = ZeroSlice<UnvalidatedTinyAsciiStr<N>>;
    type GetType = UnvalidatedTinyAsciiStr<N>;
    type OwnedType = UnvalidatedTinyAsciiStr<N>;
}

#[cfg(test)]
mod test {
    use crate::*;
    use zerovec::*;

    #[test]
    fn test_zerovec() {
        let mut vec = ZeroVec::<TinyAsciiStr<7>>::new();

        vec.with_mut(|v| v.push("foobar".parse().unwrap()));
        vec.with_mut(|v| v.push("baz".parse().unwrap()));
        vec.with_mut(|v| v.push("quux".parse().unwrap()));

        let bytes = vec.as_bytes();

        let vec: ZeroVec<TinyAsciiStr<7>> = ZeroVec::parse_bytes(bytes).unwrap();

        assert_eq!(&*vec.get(0).unwrap(), "foobar");
        assert_eq!(&*vec.get(1).unwrap(), "baz");
        assert_eq!(&*vec.get(2).unwrap(), "quux");
    }
}
