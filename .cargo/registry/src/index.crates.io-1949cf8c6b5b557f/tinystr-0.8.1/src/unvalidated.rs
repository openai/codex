// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::ParseError;
use crate::TinyAsciiStr;
use core::fmt;

/// A fixed-length bytes array that is expected to be an ASCII string but does not enforce that invariant.
///
/// Use this type instead of `TinyAsciiStr` if you don't need to enforce ASCII during deserialization. For
/// example, strings that are keys of a map don't need to ever be reified as `TinyAsciiStr`s.
///
/// The main advantage of this type over `[u8; N]` is that it serializes as a string in
/// human-readable formats like JSON.
#[derive(PartialEq, PartialOrd, Eq, Ord, Clone, Copy)]
pub struct UnvalidatedTinyAsciiStr<const N: usize>(pub(crate) [u8; N]);

impl<const N: usize> fmt::Debug for UnvalidatedTinyAsciiStr<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Debug as a string if possible
        match self.try_into_tinystr() {
            Ok(s) => fmt::Debug::fmt(&s, f),
            Err(_) => fmt::Debug::fmt(&self.0, f),
        }
    }
}

impl<const N: usize> UnvalidatedTinyAsciiStr<N> {
    #[inline]
    /// Converts into a [`TinyAsciiStr`]. Fails if the bytes are not valid ASCII.
    pub fn try_into_tinystr(self) -> Result<TinyAsciiStr<N>, ParseError> {
        TinyAsciiStr::try_from_raw(self.0)
    }

    #[inline]
    /// Unsafely converts into a [`TinyAsciiStr`].
    pub const fn from_utf8_unchecked(bytes: [u8; N]) -> Self {
        Self(bytes)
    }
}

impl<const N: usize> TinyAsciiStr<N> {
    #[inline]
    // Converts into a [`UnvalidatedTinyAsciiStr`]
    pub const fn to_unvalidated(self) -> UnvalidatedTinyAsciiStr<N> {
        UnvalidatedTinyAsciiStr(*self.all_bytes())
    }
}

impl<const N: usize> From<TinyAsciiStr<N>> for UnvalidatedTinyAsciiStr<N> {
    fn from(other: TinyAsciiStr<N>) -> Self {
        other.to_unvalidated()
    }
}

#[cfg(feature = "serde")]
impl<const N: usize> serde::Serialize for UnvalidatedTinyAsciiStr<N> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::Error;
        self.try_into_tinystr()
            .map_err(|_| S::Error::custom("invalid ascii in UnvalidatedTinyAsciiStr"))?
            .serialize(serializer)
    }
}

macro_rules! deserialize {
    ($size:literal) => {
        #[cfg(feature = "serde")]
        impl<'de, 'a> serde::Deserialize<'de> for UnvalidatedTinyAsciiStr<$size>
        where
            'de: 'a,
        {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                if deserializer.is_human_readable() {
                    Ok(TinyAsciiStr::deserialize(deserializer)?.to_unvalidated())
                } else {
                    Ok(Self(<[u8; $size]>::deserialize(deserializer)?))
                }
            }
        }
    };
}

deserialize!(1);
deserialize!(2);
deserialize!(3);
deserialize!(4);
deserialize!(5);
deserialize!(6);
deserialize!(7);
deserialize!(8);
deserialize!(9);
deserialize!(10);
deserialize!(11);
deserialize!(12);
deserialize!(13);
deserialize!(14);
deserialize!(15);
deserialize!(16);
deserialize!(17);
deserialize!(18);
deserialize!(19);
deserialize!(20);
deserialize!(21);
deserialize!(22);
deserialize!(23);
deserialize!(24);
deserialize!(25);
deserialize!(26);
deserialize!(27);
deserialize!(28);
deserialize!(29);
deserialize!(30);
deserialize!(31);
deserialize!(32);
