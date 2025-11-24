// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

#[cfg(feature = "alloc")]
use alloc::boxed::Box;
use core::cmp::Ordering;
use core::fmt;
use core::ops::Deref;

/// A byte slice that is expected to be a UTF-8 string but does not enforce that invariant.
///
/// Use this type instead of `str` if you don't need to enforce UTF-8 during deserialization. For
/// example, strings that are keys of a map don't need to ever be reified as `str`s.
///
/// [`PotentialUtf8`] derefs to `[u8]`. To obtain a `str`, use [`Self::try_as_str()`].
///
/// The main advantage of this type over `[u8]` is that it serializes as a string in
/// human-readable formats like JSON.
///
/// # Examples
///
/// Using an [`PotentialUtf8`] as the key of a [`ZeroMap`]:
///
/// ```
/// use potential_utf::PotentialUtf8;
/// use zerovec::ZeroMap;
///
/// // This map is cheap to deserialize, as we don't need to perform UTF-8 validation.
/// let map: ZeroMap<PotentialUtf8, u8> = [
///     (PotentialUtf8::from_bytes(b"abc"), 11),
///     (PotentialUtf8::from_bytes(b"def"), 22),
///     (PotentialUtf8::from_bytes(b"ghi"), 33),
/// ]
/// .into_iter()
/// .collect();
///
/// let key = "abc";
/// let value = map.get_copied(PotentialUtf8::from_str(key));
/// assert_eq!(Some(11), value);
/// ```
///
/// [`ZeroMap`]: zerovec::ZeroMap
#[repr(transparent)]
#[derive(PartialEq, Eq, PartialOrd, Ord)]
#[allow(clippy::exhaustive_structs)] // transparent newtype
pub struct PotentialUtf8(pub [u8]);

impl fmt::Debug for PotentialUtf8 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Debug as a string if possible
        match self.try_as_str() {
            Ok(s) => fmt::Debug::fmt(s, f),
            Err(_) => fmt::Debug::fmt(&self.0, f),
        }
    }
}

impl PotentialUtf8 {
    /// Create a [`PotentialUtf8`] from a byte slice.
    #[inline]
    pub const fn from_bytes(other: &[u8]) -> &Self {
        // Safety: PotentialUtf8 is transparent over [u8]
        unsafe { core::mem::transmute(other) }
    }

    /// Create a [`PotentialUtf8`] from a string slice.
    #[inline]
    pub const fn from_str(s: &str) -> &Self {
        Self::from_bytes(s.as_bytes())
    }

    /// Create a [`PotentialUtf8`] from boxed bytes.
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    #[inline]
    #[cfg(feature = "alloc")]
    pub fn from_boxed_bytes(other: Box<[u8]>) -> Box<Self> {
        // Safety: PotentialUtf8 is transparent over [u8]
        unsafe { core::mem::transmute(other) }
    }

    /// Create a [`PotentialUtf8`] from a boxed `str`.
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    #[inline]
    #[cfg(feature = "alloc")]
    pub fn from_boxed_str(other: Box<str>) -> Box<Self> {
        Self::from_boxed_bytes(other.into_boxed_bytes())
    }

    /// Get the bytes from a [`PotentialUtf8].
    #[inline]
    pub const fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Attempt to convert a [`PotentialUtf8`] to a `str`.
    ///
    /// # Examples
    ///
    /// ```
    /// use potential_utf::PotentialUtf8;
    ///
    /// static A: &PotentialUtf8 = PotentialUtf8::from_bytes(b"abc");
    ///
    /// let b = A.try_as_str().unwrap();
    /// assert_eq!(b, "abc");
    /// ```
    // Note: this is const starting in 1.63
    #[inline]
    pub fn try_as_str(&self) -> Result<&str, core::str::Utf8Error> {
        core::str::from_utf8(&self.0)
    }
}

impl<'a> From<&'a str> for &'a PotentialUtf8 {
    #[inline]
    fn from(other: &'a str) -> Self {
        PotentialUtf8::from_str(other)
    }
}

impl PartialEq<str> for PotentialUtf8 {
    fn eq(&self, other: &str) -> bool {
        self.eq(Self::from_str(other))
    }
}

impl PartialOrd<str> for PotentialUtf8 {
    fn partial_cmp(&self, other: &str) -> Option<Ordering> {
        self.partial_cmp(Self::from_str(other))
    }
}

impl PartialEq<PotentialUtf8> for str {
    fn eq(&self, other: &PotentialUtf8) -> bool {
        PotentialUtf8::from_str(self).eq(other)
    }
}

impl PartialOrd<PotentialUtf8> for str {
    fn partial_cmp(&self, other: &PotentialUtf8) -> Option<Ordering> {
        PotentialUtf8::from_str(self).partial_cmp(other)
    }
}

#[cfg(feature = "alloc")]
impl From<Box<str>> for Box<PotentialUtf8> {
    #[inline]
    fn from(other: Box<str>) -> Self {
        PotentialUtf8::from_boxed_str(other)
    }
}

impl Deref for PotentialUtf8 {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// This impl requires enabling the optional `zerovec` Cargo feature
#[cfg(all(feature = "zerovec", feature = "alloc"))]
impl<'a> zerovec::maps::ZeroMapKV<'a> for PotentialUtf8 {
    type Container = zerovec::VarZeroVec<'a, PotentialUtf8>;
    type Slice = zerovec::VarZeroSlice<PotentialUtf8>;
    type GetType = PotentialUtf8;
    type OwnedType = Box<PotentialUtf8>;
}

// Safety (based on the safety checklist on the VarULE trait):
//  1. PotentialUtf8 does not include any uninitialized or padding bytes (transparent over a ULE)
//  2. PotentialUtf8 is aligned to 1 byte (transparent over a ULE)
//  3. The impl of `validate_bytes()` returns an error if any byte is not valid (impossible)
//  4. The impl of `validate_bytes()` returns an error if the slice cannot be used in its entirety (impossible)
//  5. The impl of `from_bytes_unchecked()` returns a reference to the same data (returns the argument directly)
//  6. All other methods are defaulted
//  7. `[T]` byte equality is semantic equality (transparent over a ULE)
/// This impl requires enabling the optional `zerovec` Cargo feature
#[cfg(feature = "zerovec")]
unsafe impl zerovec::ule::VarULE for PotentialUtf8 {
    #[inline]
    fn validate_bytes(_: &[u8]) -> Result<(), zerovec::ule::UleError> {
        Ok(())
    }
    #[inline]
    unsafe fn from_bytes_unchecked(bytes: &[u8]) -> &Self {
        PotentialUtf8::from_bytes(bytes)
    }
}

/// This impl requires enabling the optional `serde` Cargo feature
#[cfg(feature = "serde")]
impl serde_core::Serialize for PotentialUtf8 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde_core::Serializer,
    {
        use serde_core::ser::Error;
        let s = self
            .try_as_str()
            .map_err(|_| S::Error::custom("invalid UTF-8 in PotentialUtf8"))?;
        if serializer.is_human_readable() {
            serializer.serialize_str(s)
        } else {
            serializer.serialize_bytes(s.as_bytes())
        }
    }
}

/// This impl requires enabling the optional `serde` Cargo feature
#[cfg(all(feature = "serde", feature = "alloc"))]
impl<'de> serde_core::Deserialize<'de> for Box<PotentialUtf8> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde_core::Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            let boxed_str = Box::<str>::deserialize(deserializer)?;
            Ok(PotentialUtf8::from_boxed_str(boxed_str))
        } else {
            let boxed_bytes = Box::<[u8]>::deserialize(deserializer)?;
            Ok(PotentialUtf8::from_boxed_bytes(boxed_bytes))
        }
    }
}

/// This impl requires enabling the optional `serde` Cargo feature
#[cfg(feature = "serde")]
impl<'de, 'a> serde_core::Deserialize<'de> for &'a PotentialUtf8
where
    'de: 'a,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde_core::Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            let s = <&str>::deserialize(deserializer)?;
            Ok(PotentialUtf8::from_str(s))
        } else {
            let bytes = <&[u8]>::deserialize(deserializer)?;
            Ok(PotentialUtf8::from_bytes(bytes))
        }
    }
}

#[repr(transparent)]
#[derive(PartialEq, Eq, PartialOrd, Ord)]
#[allow(clippy::exhaustive_structs)] // transparent newtype
pub struct PotentialUtf16(pub [u16]);

impl fmt::Debug for PotentialUtf16 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Debug as a string if possible
        for c in char::decode_utf16(self.0.iter().copied()) {
            match c {
                Ok(c) => write!(f, "{c}")?,
                Err(e) => write!(f, "\\0x{:x}", e.unpaired_surrogate())?,
            }
        }
        Ok(())
    }
}

impl PotentialUtf16 {
    /// Create a [`PotentialUtf16`] from a u16 slice.
    #[inline]
    pub const fn from_slice(other: &[u16]) -> &Self {
        // Safety: PotentialUtf16 is transparent over [u16]
        unsafe { core::mem::transmute(other) }
    }

    pub fn chars(&self) -> impl Iterator<Item = char> + '_ {
        char::decode_utf16(self.0.iter().copied()).map(|c| c.unwrap_or(char::REPLACEMENT_CHARACTER))
    }
}
