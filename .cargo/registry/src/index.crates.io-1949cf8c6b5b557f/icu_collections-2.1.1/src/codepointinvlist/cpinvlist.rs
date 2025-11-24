// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

#[cfg(feature = "serde")]
use alloc::format;
#[cfg(feature = "serde")]
use alloc::string::String;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;
use core::{char, ops::RangeBounds, ops::RangeInclusive};
use potential_utf::PotentialCodePoint;
use yoke::Yokeable;
use zerofrom::ZeroFrom;
use zerovec::{ule::AsULE, zerovec, ZeroVec};

use super::InvalidSetError;
use crate::codepointinvlist::utils::{deconstruct_range, is_valid_zv};

/// Represents the end code point of the Basic Multilingual Plane range, starting from code point 0, inclusive
const BMP_MAX: u32 = 0xFFFF;

/// Represents the inversion list for a set of all code points in the Basic Multilingual Plane.
const BMP_INV_LIST_VEC: ZeroVec<PotentialCodePoint> = zerovec!(PotentialCodePoint; PotentialCodePoint::to_unaligned; [PotentialCodePoint::from_u24(0x0), PotentialCodePoint::from_u24(BMP_MAX + 1)]);

/// Represents the inversion list for all of the code points in the Unicode range.
const ALL_VEC: ZeroVec<PotentialCodePoint> = zerovec!(PotentialCodePoint; PotentialCodePoint::to_unaligned; [PotentialCodePoint::from_u24(0x0), PotentialCodePoint::from_u24((char::MAX as u32) + 1)]);

/// A membership wrapper for [`CodePointInversionList`].
///
/// Provides exposure to membership functions and constructors from serialized `CodePointSet`s (sets of code points)
/// and predefined ranges.
#[zerovec::make_varule(CodePointInversionListULE)]
#[zerovec::skip_derive(Ord)]
#[zerovec::derive(Debug)]
#[derive(Debug, Eq, PartialEq, Clone, Yokeable, ZeroFrom)]
#[cfg_attr(not(feature = "alloc"), zerovec::skip_derive(ZeroMapKV, ToOwned))]
pub struct CodePointInversionList<'data> {
    // If we wanted to use an array to keep the memory on the stack, there is an unsafe nightly feature
    // https://doc.rust-lang.org/nightly/core/array/trait.FixedSizeArray.html
    // Allows for traits of fixed size arrays

    // Implements an [inversion list.](https://en.wikipedia.org/wiki/Inversion_list)
    inv_list: ZeroVec<'data, PotentialCodePoint>,
    size: u32,
}

#[cfg(feature = "serde")]
impl<'de: 'a, 'a> serde::Deserialize<'de> for CodePointInversionList<'a> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;

        let parsed_inv_list = if deserializer.is_human_readable() {
            let parsed_strings = Vec::<alloc::borrow::Cow<'de, str>>::deserialize(deserializer)?;
            let mut inv_list = ZeroVec::new_owned(Vec::with_capacity(parsed_strings.len() * 2));
            for range in parsed_strings {
                fn internal(range: &str) -> Option<(u32, u32)> {
                    let (start, range) = UnicodeCodePoint::parse(range)?;
                    if range.is_empty() {
                        return Some((start.0, start.0));
                    }
                    let (hyphen, range) = UnicodeCodePoint::parse(range)?;
                    if hyphen.0 != '-' as u32 {
                        return None;
                    }
                    let (end, range) = UnicodeCodePoint::parse(range)?;
                    range.is_empty().then_some((start.0, end.0))
                }
                let (start, end) = internal(&range).ok_or_else(|| Error::custom(format!(
                    "Cannot deserialize invalid inversion list for CodePointInversionList: {range:?}"
                )))?;
                inv_list.with_mut(|v| {
                    v.push(PotentialCodePoint::from_u24(start).to_unaligned());
                    v.push(PotentialCodePoint::from_u24(end + 1).to_unaligned());
                });
            }
            inv_list
        } else {
            ZeroVec::<PotentialCodePoint>::deserialize(deserializer)?
        };
        CodePointInversionList::try_from_inversion_list(parsed_inv_list).map_err(|e| {
            Error::custom(format!(
                "Cannot deserialize invalid inversion list for CodePointInversionList: {e:?}"
            ))
        })
    }
}

#[cfg(feature = "databake")]
impl databake::Bake for CodePointInversionList<'_> {
    fn bake(&self, env: &databake::CrateEnv) -> databake::TokenStream {
        env.insert("icu_collections");
        let inv_list = self.inv_list.bake(env);
        let size = self.size.bake(env);
        // Safe because our parts are safe.
        databake::quote! { unsafe {
            #[allow(unused_unsafe)]
            icu_collections::codepointinvlist::CodePointInversionList::from_parts_unchecked(#inv_list, #size)
        }}
    }
}

#[cfg(feature = "databake")]
impl databake::BakeSize for CodePointInversionList<'_> {
    fn borrows_size(&self) -> usize {
        self.inv_list.borrows_size()
    }
}

#[cfg(feature = "serde")]
#[derive(Debug, Copy, Clone)]
struct UnicodeCodePoint(u32);

#[cfg(feature = "serde")]
impl UnicodeCodePoint {
    fn from_u32(cp: u32) -> Result<Self, String> {
        if cp <= char::MAX as u32 {
            Ok(Self(cp))
        } else {
            Err(format!("Not a Unicode code point {cp}"))
        }
    }

    fn parse(value: &str) -> Option<(Self, &str)> {
        Some(if let Some(hex) = value.strip_prefix("U+") {
            let (escape, remainder) = (hex.get(..4)?, hex.get(4..)?);
            (Self(u32::from_str_radix(escape, 16).ok()?), remainder)
        } else {
            let c = value.chars().next()?;
            (Self(c as u32), value.get(c.len_utf8()..)?)
        })
    }
}

#[cfg(feature = "serde")]
impl core::fmt::Display for UnicodeCodePoint {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.0 {
            s @ 0xD800..=0xDFFF => write!(f, "U+{s:X}"),
            // char should be in range by construction but this code is not so performance-sensitive
            // so we just use the replacement character
            c => write!(
                f,
                "{}",
                char::from_u32(c).unwrap_or(char::REPLACEMENT_CHARACTER)
            ),
        }
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for CodePointInversionList<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if serializer.is_human_readable() {
            use serde::ser::Error;
            use serde::ser::SerializeSeq;
            let mut seq = serializer.serialize_seq(Some(self.inv_list.len() / 2))?;
            for range in self.iter_ranges() {
                let start = UnicodeCodePoint::from_u32(*range.start()).map_err(S::Error::custom)?;
                if range.start() == range.end() {
                    seq.serialize_element(&format!("{start}"))?;
                } else {
                    let end = UnicodeCodePoint::from_u32(*range.end()).map_err(S::Error::custom)?;
                    seq.serialize_element(&format!("{start}-{end}",))?;
                }
            }
            seq.end()
        } else {
            // Note: serde(flatten) currently does not promote a struct field of type Vec
            // to replace the struct when serializing. The error message from the default
            // serialization is: "can only flatten structs and maps (got a sequence)".
            self.inv_list.serialize(serializer)
        }
    }
}

impl<'data> CodePointInversionList<'data> {
    /// Returns a new [`CodePointInversionList`] from an [inversion list](https://en.wikipedia.org/wiki/Inversion_list)
    /// represented as a [`ZeroVec`]`<`[`PotentialCodePoint`]`>` of code points.
    ///
    /// The inversion list must be of even length, sorted ascending non-overlapping,
    /// and within the bounds of `0x0 -> 0x10FFFF` inclusive, and end points being exclusive.
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::collections::codepointinvlist::CodePointInversionList;
    /// use icu::collections::codepointinvlist::InvalidSetError;
    /// use potential_utf::PotentialCodePoint;
    /// use zerovec::ZeroVec;
    ///
    /// let valid = [0x0, 0x10000];
    /// let inv_list: ZeroVec<PotentialCodePoint> = valid
    ///     .into_iter()
    ///     .map(PotentialCodePoint::from_u24)
    ///     .collect();
    /// let result = CodePointInversionList::try_from_inversion_list(inv_list);
    /// assert!(matches!(result, CodePointInversionList));
    ///
    /// let invalid = vec![0x0, 0x80, 0x3];
    /// let inv_list: ZeroVec<PotentialCodePoint> = invalid
    ///     .iter()
    ///     .copied()
    ///     .map(PotentialCodePoint::from_u24)
    ///     .collect();
    /// let result = CodePointInversionList::try_from_inversion_list(inv_list);
    /// assert!(matches!(result, Err(InvalidSetError(_))));
    /// if let Err(InvalidSetError(actual)) = result {
    ///     assert_eq!(
    ///         &invalid,
    ///         &actual.into_iter().map(u32::from).collect::<Vec<_>>()
    ///     );
    /// }
    /// ```
    pub fn try_from_inversion_list(
        inv_list: ZeroVec<'data, PotentialCodePoint>,
    ) -> Result<Self, InvalidSetError> {
        #[expect(clippy::indexing_slicing)] // chunks
        if is_valid_zv(&inv_list) {
            let size = inv_list
                .as_ule_slice()
                .chunks(2)
                .map(|end_points| {
                    u32::from(<PotentialCodePoint as AsULE>::from_unaligned(end_points[1]))
                        - u32::from(<PotentialCodePoint as AsULE>::from_unaligned(end_points[0]))
                })
                .sum::<u32>();
            Ok(Self { inv_list, size })
        } else {
            Err(InvalidSetError(
                #[cfg(feature = "alloc")]
                inv_list.to_vec(),
            ))
        }
    }

    /// Safety: no actual safety invariants, however has correctness invariants
    #[doc(hidden)] // databake internal
    pub const unsafe fn from_parts_unchecked(
        inv_list: ZeroVec<'data, PotentialCodePoint>,
        size: u32,
    ) -> Self {
        Self { inv_list, size }
    }

    /// Returns a new, fully-owned [`CodePointInversionList`] by cloning an [inversion list](https://en.wikipedia.org/wiki/Inversion_list)
    /// represented as a slice of [`PotentialCodePoint`] code points.
    ///
    /// The inversion list must be of even length, sorted ascending non-overlapping,
    /// and within the bounds of `0x0 -> 0x10FFFF` inclusive, and end points being exclusive.
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::collections::codepointinvlist::CodePointInversionList;
    ///
    /// let bmp_list = &[0x0, 0x10000];
    /// let smp_list = &[0x10000, 0x20000];
    /// let sip_list = &[0x20000, 0x30000];
    ///
    /// let lists: Vec<CodePointInversionList> =
    ///     [&bmp_list[..], smp_list, sip_list]
    ///         .into_iter()
    ///         .map(|l| {
    ///             CodePointInversionList::try_from_u32_inversion_list_slice(l)
    ///                 .unwrap()
    ///         })
    ///         .collect();
    ///
    /// let bmp = &lists[0];
    /// assert!(bmp.contains32(0xFFFF));
    /// assert!(!bmp.contains32(0x10000));
    ///
    /// assert!(!lists.iter().any(|set| set.contains32(0x40000)));
    /// ```
    #[cfg(feature = "alloc")]
    pub fn try_from_u32_inversion_list_slice(inv_list: &[u32]) -> Result<Self, InvalidSetError> {
        let inv_list_zv: ZeroVec<PotentialCodePoint> = inv_list
            .iter()
            .copied()
            .map(PotentialCodePoint::from_u24)
            .collect();
        CodePointInversionList::try_from_inversion_list(inv_list_zv)
    }

    /// Attempts to convert this list into a fully-owned one. No-op if already fully owned
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    #[cfg(feature = "alloc")]
    pub fn into_owned(self) -> CodePointInversionList<'static> {
        CodePointInversionList {
            inv_list: self.inv_list.into_owned(),
            size: self.size,
        }
    }

    /// Returns an owned inversion list representing the current [`CodePointInversionList`]
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    #[cfg(feature = "alloc")]
    pub fn get_inversion_list_vec(&self) -> Vec<u32> {
        self.as_inversion_list().iter().map(u32::from).collect()
    }

    /// Returns [`CodePointInversionList`] spanning entire Unicode range
    ///
    /// The range spans from `0x0 -> 0x10FFFF` inclusive.
    ///  
    /// # Examples
    ///
    /// ```
    /// use icu::collections::codepointinvlist::CodePointInversionList;
    ///
    /// let expected = [0x0, (char::MAX as u32) + 1];
    /// assert_eq!(
    ///     CodePointInversionList::all().get_inversion_list_vec(),
    ///     expected
    /// );
    /// assert_eq!(
    ///     CodePointInversionList::all().size(),
    ///     (expected[1] - expected[0]) as usize
    /// );
    /// ```
    pub fn all() -> Self {
        Self {
            inv_list: ALL_VEC,
            size: (char::MAX as u32) + 1,
        }
    }

    /// Returns [`CodePointInversionList`] spanning BMP range
    ///
    /// The range spans from `0x0 -> 0xFFFF` inclusive.
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::collections::codepointinvlist::CodePointInversionList;
    ///
    /// const BMP_MAX: u32 = 0xFFFF;
    ///
    /// let expected = [0x0, BMP_MAX + 1];
    /// assert_eq!(
    ///     CodePointInversionList::bmp().get_inversion_list_vec(),
    ///     expected
    /// );
    /// assert_eq!(
    ///     CodePointInversionList::bmp().size(),
    ///     (expected[1] - expected[0]) as usize
    /// );
    /// ```
    pub fn bmp() -> Self {
        Self {
            inv_list: BMP_INV_LIST_VEC,
            size: BMP_MAX + 1,
        }
    }

    /// Returns the inversion list as a slice
    ///
    /// Public only to the crate, not exposed to public
    #[cfg(feature = "alloc")]
    pub(crate) fn as_inversion_list(&self) -> &ZeroVec<'_, PotentialCodePoint> {
        &self.inv_list
    }

    /// Yields an [`Iterator`] going through the character set in the [`CodePointInversionList`]
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::collections::codepointinvlist::CodePointInversionList;
    /// let example_list = [0x41, 0x44, 0x45, 0x46];
    /// let example = CodePointInversionList::try_from_u32_inversion_list_slice(
    ///     &example_list,
    /// )
    /// .unwrap();
    /// let mut ex_iter_chars = example.iter_chars();
    /// assert_eq!(Some('A'), ex_iter_chars.next());
    /// assert_eq!(Some('B'), ex_iter_chars.next());
    /// assert_eq!(Some('C'), ex_iter_chars.next());
    /// assert_eq!(Some('E'), ex_iter_chars.next());
    /// assert_eq!(None, ex_iter_chars.next());
    /// ```
    pub fn iter_chars(&self) -> impl Iterator<Item = char> + '_ {
        #[expect(clippy::indexing_slicing)] // chunks
        self.inv_list
            .as_ule_slice()
            .chunks(2)
            .flat_map(|pair| {
                u32::from(PotentialCodePoint::from_unaligned(pair[0]))
                    ..u32::from(PotentialCodePoint::from_unaligned(pair[1]))
            })
            .filter_map(char::from_u32)
    }

    /// Yields an [`Iterator`] returning the ranges of the code points that are
    /// included in the [`CodePointInversionList`]
    ///
    /// Ranges are returned as [`RangeInclusive`], which is inclusive of its
    /// `end` bound value. An end-inclusive behavior matches the ICU4C/J
    /// behavior of ranges, ex: `CodePointInversionList::contains(UChar32 start, UChar32 end)`.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::collections::codepointinvlist::CodePointInversionList;
    /// let example_list = [0x41, 0x44, 0x45, 0x46];
    /// let example = CodePointInversionList::try_from_u32_inversion_list_slice(
    ///     &example_list,
    /// )
    /// .unwrap();
    /// let mut example_iter_ranges = example.iter_ranges();
    /// assert_eq!(Some(0x41..=0x43), example_iter_ranges.next());
    /// assert_eq!(Some(0x45..=0x45), example_iter_ranges.next());
    /// assert_eq!(None, example_iter_ranges.next());
    /// ```
    pub fn iter_ranges(&self) -> impl ExactSizeIterator<Item = RangeInclusive<u32>> + '_ {
        #[expect(clippy::indexing_slicing)] // chunks
        self.inv_list.as_ule_slice().chunks(2).map(|pair| {
            let range_start = u32::from(PotentialCodePoint::from_unaligned(pair[0]));
            let range_limit = u32::from(PotentialCodePoint::from_unaligned(pair[1]));
            range_start..=(range_limit - 1)
        })
    }

    /// Yields an [`Iterator`] returning the ranges of the code points that are
    /// *not* included in the [`CodePointInversionList`]
    ///
    /// Ranges are returned as [`RangeInclusive`], which is inclusive of its
    /// `end` bound value. An end-inclusive behavior matches the ICU4C/J
    /// behavior of ranges, ex: `CodePointInversionList::contains(UChar32 start, UChar32 end)`.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::collections::codepointinvlist::CodePointInversionList;
    /// let example_list = [0x41, 0x44, 0x45, 0x46];
    /// let example = CodePointInversionList::try_from_u32_inversion_list_slice(
    ///     &example_list,
    /// )
    /// .unwrap();
    /// let mut example_iter_ranges = example.iter_ranges_complemented();
    /// assert_eq!(Some(0..=0x40), example_iter_ranges.next());
    /// assert_eq!(Some(0x44..=0x44), example_iter_ranges.next());
    /// assert_eq!(Some(0x46..=char::MAX as u32), example_iter_ranges.next());
    /// assert_eq!(None, example_iter_ranges.next());
    /// ```
    pub fn iter_ranges_complemented(&self) -> impl Iterator<Item = RangeInclusive<u32>> + '_ {
        let inv_ule = self.inv_list.as_ule_slice();
        let middle = inv_ule.get(1..inv_ule.len() - 1).unwrap_or(&[]);
        let beginning = if let Some(first) = self.inv_list.first() {
            let first = u32::from(first);
            if first == 0 {
                None
            } else {
                Some(0..=first - 1)
            }
        } else {
            None
        };
        let end = if let Some(last) = self.inv_list.last() {
            let last = u32::from(last);
            if last == char::MAX as u32 {
                None
            } else {
                Some(last..=char::MAX as u32)
            }
        } else {
            None
        };
        #[expect(clippy::indexing_slicing)] // chunks
        let chunks = middle.chunks(2).map(|pair| {
            let range_start = u32::from(PotentialCodePoint::from_unaligned(pair[0]));
            let range_limit = u32::from(PotentialCodePoint::from_unaligned(pair[1]));
            range_start..=(range_limit - 1)
        });
        beginning.into_iter().chain(chunks).chain(end)
    }

    /// Returns the number of ranges contained in this [`CodePointInversionList`]
    pub fn get_range_count(&self) -> usize {
        self.inv_list.len() / 2
    }

    /// Returns a specific range contained in this [`CodePointInversionList`] by index.
    /// Intended for use in FFI.
    pub fn get_nth_range(&self, idx: usize) -> Option<RangeInclusive<u32>> {
        let start_idx = idx * 2;
        let end_idx = start_idx + 1;
        let start = u32::from(self.inv_list.get(start_idx)?);
        let end = u32::from(self.inv_list.get(end_idx)?);
        Some(start..=(end - 1))
    }

    /// Returns the number of elements of the [`CodePointInversionList`]
    pub fn size(&self) -> usize {
        if self.is_empty() {
            return 0;
        }
        self.size as usize
    }

    /// Returns whether or not the [`CodePointInversionList`] is empty
    pub fn is_empty(&self) -> bool {
        self.inv_list.is_empty()
    }

    /// Wrapper for contains
    ///
    /// Returns an [`Option`] as to whether or not it is possible for the query to be contained.
    /// The value in the [`Option`] is the start index of the range that contains the query.
    fn contains_query(&self, query: u32) -> Option<usize> {
        let query = PotentialCodePoint::try_from(query).ok()?;
        match self.inv_list.binary_search(&query) {
            Ok(pos) => {
                if pos % 2 == 0 {
                    Some(pos)
                } else {
                    None
                }
            }
            Err(pos) => {
                if pos % 2 != 0 && pos < self.inv_list.len() {
                    Some(pos - 1)
                } else {
                    None
                }
            }
        }
    }

    /// Checks to see the query is in the [`CodePointInversionList`]
    ///
    /// Runs a binary search in `O(log(n))` where `n` is the number of start and end points
    /// in the set using [`core`] implementation
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::collections::codepointinvlist::CodePointInversionList;
    /// let example_list = [0x41, 0x43, 0x44, 0x45];
    /// let example = CodePointInversionList::try_from_u32_inversion_list_slice(
    ///     &example_list,
    /// )
    /// .unwrap();
    /// assert!(example.contains('A'));
    /// assert!(!example.contains('C'));
    /// ```
    pub fn contains(&self, query: char) -> bool {
        self.contains_query(query as u32).is_some()
    }

    /// Checks to see the unsigned int is in the [`CodePointInversionList::all()`](CodePointInversionList::all())
    ///
    /// Note: Even though [`u32`] and [`prim@char`] in Rust are non-negative 4-byte
    /// values, there is an important difference. A [`u32`] can take values up to
    /// a very large integer value, while a [`prim@char`] in Rust is defined to be in
    /// the range from 0 to the maximum valid Unicode Scalar Value.
    ///
    /// Runs a binary search in `O(log(n))` where `n` is the number of start and end points
    /// in the set using [`core`] implementation
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::collections::codepointinvlist::CodePointInversionList;
    /// let example_list = [0x41, 0x43, 0x44, 0x45];
    /// let example = CodePointInversionList::try_from_u32_inversion_list_slice(
    ///     &example_list,
    /// )
    /// .unwrap();
    /// assert!(example.contains32(0x41));
    /// assert!(!example.contains32(0x43));
    /// ```
    pub fn contains32(&self, query: u32) -> bool {
        self.contains_query(query).is_some()
    }

    /// Checks to see if the range is in the [`CodePointInversionList`]
    ///
    /// Runs a binary search in `O(log(n))` where `n` is the number of start and end points
    /// in the set using [`Vec`] implementation. Only runs the search once on the `start`
    /// parameter, while the `end` parameter is checked in a single `O(1)` step.
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::collections::codepointinvlist::CodePointInversionList;
    /// let example_list = [0x41, 0x43, 0x44, 0x45];
    /// let example = CodePointInversionList::try_from_u32_inversion_list_slice(
    ///     &example_list,
    /// )
    /// .unwrap();
    /// assert!(example.contains_range('A'..'C'));
    /// assert!(example.contains_range('A'..='B'));
    /// assert!(!example.contains_range('A'..='C'));
    /// ```
    ///
    /// Surrogate points (`0xD800 -> 0xDFFF`) will return [`false`] if the Range contains them but the
    /// [`CodePointInversionList`] does not.
    ///
    /// Note: when comparing to ICU4C/J, keep in mind that `Range`s in Rust are
    /// constructed inclusive of start boundary and exclusive of end boundary.
    /// The ICU4C/J `CodePointInversionList::contains(UChar32 start, UChar32 end)` method
    /// differs by including the end boundary.
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::collections::codepointinvlist::CodePointInversionList;
    /// use std::char;
    /// let check =
    ///     char::from_u32(0xD7FE).unwrap()..char::from_u32(0xE001).unwrap();
    /// let example_list = [0xD7FE, 0xD7FF, 0xE000, 0xE001];
    /// let example = CodePointInversionList::try_from_u32_inversion_list_slice(
    ///     &example_list,
    /// )
    /// .unwrap();
    /// assert!(!example.contains_range(check));
    /// ```
    pub fn contains_range(&self, range: impl RangeBounds<char>) -> bool {
        let (from, till) = deconstruct_range(range);
        if from >= till {
            return false;
        }
        match self.contains_query(from) {
            Some(pos) => {
                if let Some(x) = self.inv_list.get(pos + 1) {
                    (till) <= x.into()
                } else {
                    debug_assert!(
                        false,
                        "Inversion list query should not return out of bounds index"
                    );
                    false
                }
            }
            None => false,
        }
    }

    /// Check if the calling [`CodePointInversionList`] contains all the characters of the given [`CodePointInversionList`]
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::collections::codepointinvlist::CodePointInversionList;
    /// let example_list = [0x41, 0x46, 0x55, 0x5B]; // A - E, U - Z
    /// let example = CodePointInversionList::try_from_u32_inversion_list_slice(
    ///     &example_list,
    /// )
    /// .unwrap();
    /// let a_to_d = CodePointInversionList::try_from_u32_inversion_list_slice(&[
    ///     0x41, 0x45,
    /// ])
    /// .unwrap();
    /// let f_to_t = CodePointInversionList::try_from_u32_inversion_list_slice(&[
    ///     0x46, 0x55,
    /// ])
    /// .unwrap();
    /// let r_to_x = CodePointInversionList::try_from_u32_inversion_list_slice(&[
    ///     0x52, 0x58,
    /// ])
    /// .unwrap();
    /// assert!(example.contains_set(&a_to_d)); // contains all
    /// assert!(!example.contains_set(&f_to_t)); // contains none
    /// assert!(!example.contains_set(&r_to_x)); // contains some
    /// ```
    pub fn contains_set(&self, set: &Self) -> bool {
        if set.size() > self.size() {
            return false;
        }

        let mut set_ranges = set.iter_ranges();
        let mut check_elem = set_ranges.next();

        let ranges = self.iter_ranges();
        for range in ranges {
            match check_elem {
                Some(ref check_range) => {
                    if check_range.start() >= range.start()
                        && check_range.end() <= &(range.end() + 1)
                    {
                        check_elem = set_ranges.next();
                    }
                }
                _ => break,
            }
        }
        check_elem.is_none()
    }

    /// Returns the end of the initial substring where the characters are either contained/not contained
    /// in the set.
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::collections::codepointinvlist::CodePointInversionList;
    /// let example_list = [0x41, 0x44]; // {A, B, C}
    /// let example = CodePointInversionList::try_from_u32_inversion_list_slice(
    ///     &example_list,
    /// )
    /// .unwrap();
    /// assert_eq!(example.span("CABXYZ", true), 3);
    /// assert_eq!(example.span("XYZC", false), 3);
    /// assert_eq!(example.span("XYZ", true), 0);
    /// assert_eq!(example.span("ABC", false), 0);
    /// ```
    pub fn span(&self, span_str: &str, contained: bool) -> usize {
        span_str
            .chars()
            .take_while(|&x| self.contains(x) == contained)
            .count()
    }

    /// Returns the start of the trailing substring (starting from end of string) where the characters are
    /// either contained/not contained in the set. Returns the length of the string if no valid return.
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::collections::codepointinvlist::CodePointInversionList;
    /// let example_list = [0x41, 0x44]; // {A, B, C}
    /// let example = CodePointInversionList::try_from_u32_inversion_list_slice(
    ///     &example_list,
    /// )
    /// .unwrap();
    /// assert_eq!(example.span_back("XYZCAB", true), 3);
    /// assert_eq!(example.span_back("ABCXYZ", true), 6);
    /// assert_eq!(example.span_back("CABXYZ", false), 3);
    /// ```
    pub fn span_back(&self, span_str: &str, contained: bool) -> usize {
        span_str.len()
            - span_str
                .chars()
                .rev()
                .take_while(|&x| self.contains(x) == contained)
                .count()
    }
}

#[cfg(test)]
mod tests {
    use super::{CodePointInversionList, InvalidSetError};
    use std::{char, vec::Vec};
    use zerovec::ZeroVec;

    #[test]
    fn test_codepointinversionlist_try_from_vec() {
        let ex = vec![0x2, 0x3, 0x4, 0x5];
        let check = CodePointInversionList::try_from_u32_inversion_list_slice(&ex).unwrap();
        assert_eq!(ex, check.get_inversion_list_vec());
        assert_eq!(2, check.size());
    }

    #[test]
    fn test_codepointinversionlist_try_from_vec_error() {
        let check = vec![0x1, 0x1, 0x2, 0x3, 0x4];
        let set = CodePointInversionList::try_from_u32_inversion_list_slice(&check);
        assert!(matches!(set, Err(InvalidSetError(_))));
        if let Err(InvalidSetError(actual)) = set {
            assert_eq!(
                &check,
                &actual.into_iter().map(u32::from).collect::<Vec<_>>()
            );
        }
    }

    // CodePointInversionList membership functions
    #[test]
    fn test_codepointinversionlist_contains_query() {
        let ex = vec![0x41, 0x46, 0x4B, 0x55];
        let check = CodePointInversionList::try_from_u32_inversion_list_slice(&ex).unwrap();
        assert!(check.contains_query(0x40).is_none());
        assert_eq!(check.contains_query(0x41).unwrap(), 0);
        assert_eq!(check.contains_query(0x44).unwrap(), 0);
        assert!(check.contains_query(0x46).is_none());
        assert_eq!(check.contains_query(0x4C).unwrap(), 2);
        assert!(check.contains_query(0x56).is_none());
    }

    #[test]
    fn test_codepointinversionlist_contains() {
        let ex = vec![0x2, 0x5, 0xA, 0xF];
        let check = CodePointInversionList::try_from_u32_inversion_list_slice(&ex).unwrap();
        assert!(check.contains(0x2 as char));
        assert!(check.contains(0x4 as char));
        assert!(check.contains(0xA as char));
        assert!(check.contains(0xE as char));
    }

    #[test]
    fn test_codepointinversionlist_contains_false() {
        let ex = vec![0x2, 0x5, 0xA, 0xF];
        let check = CodePointInversionList::try_from_u32_inversion_list_slice(&ex).unwrap();
        assert!(!check.contains(0x1 as char));
        assert!(!check.contains(0x5 as char));
        assert!(!check.contains(0x9 as char));
        assert!(!check.contains(0xF as char));
        assert!(!check.contains(0x10 as char));
    }

    #[test]
    fn test_codepointinversionlist_contains_range() {
        let ex = vec![0x41, 0x46, 0x4B, 0x55];
        let check = CodePointInversionList::try_from_u32_inversion_list_slice(&ex).unwrap();
        assert!(check.contains_range('A'..='E')); // 65 - 69
        assert!(check.contains_range('C'..'D')); // 67 - 67
        assert!(check.contains_range('L'..'P')); // 76 - 80
        assert!(!check.contains_range('L'..='U')); // 76 - 85
    }

    #[test]
    fn test_codepointinversionlist_contains_range_false() {
        let ex = vec![0x41, 0x46, 0x4B, 0x55];
        let check = CodePointInversionList::try_from_u32_inversion_list_slice(&ex).unwrap();
        assert!(!check.contains_range('!'..'A')); // 33 - 65
        assert!(!check.contains_range('F'..'K')); // 70 - 74
        assert!(!check.contains_range('U'..)); // 85 - ..
    }

    #[test]
    fn test_codepointinversionlist_contains_range_invalid() {
        let check = CodePointInversionList::all();
        assert!(!check.contains_range('A'..'!')); // 65 - 33
        assert!(!check.contains_range('A'..'A')); // 65 - 65
    }

    #[test]
    fn test_codepointinversionlist_contains_set_u() {
        let ex = vec![0xA, 0x14, 0x28, 0x32, 0x46, 0x50, 0x64, 0x6E];
        let u = CodePointInversionList::try_from_u32_inversion_list_slice(&ex).unwrap();
        let inside = vec![0xF, 0x14, 0x2C, 0x31, 0x46, 0x50, 0x64, 0x6D];
        let s = CodePointInversionList::try_from_u32_inversion_list_slice(&inside).unwrap();
        assert!(u.contains_set(&s));
    }

    #[test]
    fn test_codepointinversionlist_contains_set_u_false() {
        let ex = vec![0xA, 0x14, 0x28, 0x32, 0x46, 0x50, 0x64, 0x78];
        let u = CodePointInversionList::try_from_u32_inversion_list_slice(&ex).unwrap();
        let outside = vec![0x0, 0xA, 0x16, 0x2C, 0x32, 0x46, 0x4F, 0x51, 0x6D, 0x6F];
        let s = CodePointInversionList::try_from_u32_inversion_list_slice(&outside).unwrap();
        assert!(!u.contains_set(&s));
    }

    #[test]
    fn test_codepointinversionlist_size() {
        let ex = vec![0x2, 0x5, 0xA, 0xF];
        let check = CodePointInversionList::try_from_u32_inversion_list_slice(&ex).unwrap();
        assert_eq!(8, check.size());
        let check = CodePointInversionList::all();
        let expected = (char::MAX as u32) + 1;
        assert_eq!(expected as usize, check.size());
        let inv_list_vec = vec![];
        let check = CodePointInversionList {
            inv_list: ZeroVec::from_slice_or_alloc(&inv_list_vec),
            size: 0,
        };
        assert_eq!(check.size(), 0);
    }

    #[test]
    fn test_codepointinversionlist_is_empty() {
        let inv_list_vec = vec![];
        let check = CodePointInversionList {
            inv_list: ZeroVec::from_slice_or_alloc(&inv_list_vec),
            size: 0,
        };
        assert!(check.is_empty());
    }

    #[test]
    fn test_codepointinversionlist_is_not_empty() {
        let check = CodePointInversionList::all();
        assert!(!check.is_empty());
    }

    #[test]
    fn test_codepointinversionlist_iter_chars() {
        let ex = vec![0x41, 0x44, 0x45, 0x46, 0xD800, 0xD801];
        let check = CodePointInversionList::try_from_u32_inversion_list_slice(&ex).unwrap();
        let mut iter = check.iter_chars();
        assert_eq!(Some('A'), iter.next());
        assert_eq!(Some('B'), iter.next());
        assert_eq!(Some('C'), iter.next());
        assert_eq!(Some('E'), iter.next());
        assert_eq!(None, iter.next());
    }

    #[test]
    fn test_codepointinversionlist_iter_ranges() {
        let ex = vec![0x41, 0x44, 0x45, 0x46, 0xD800, 0xD801];
        let set = CodePointInversionList::try_from_u32_inversion_list_slice(&ex).unwrap();
        let mut ranges = set.iter_ranges();
        assert_eq!(Some(0x41..=0x43), ranges.next());
        assert_eq!(Some(0x45..=0x45), ranges.next());
        assert_eq!(Some(0xD800..=0xD800), ranges.next());
        assert_eq!(None, ranges.next());
    }

    #[test]
    fn test_codepointinversionlist_iter_ranges_exactsizeiter_trait() {
        let ex = vec![0x41, 0x44, 0x45, 0x46, 0xD800, 0xD801];
        let set = CodePointInversionList::try_from_u32_inversion_list_slice(&ex).unwrap();
        let ranges = set.iter_ranges();
        assert_eq!(3, ranges.len());
    }

    #[test]
    fn test_codepointinversionlist_range_count() {
        let ex = vec![0x41, 0x44, 0x45, 0x46, 0xD800, 0xD801];
        let set = CodePointInversionList::try_from_u32_inversion_list_slice(&ex).unwrap();
        assert_eq!(3, set.get_range_count());
    }

    #[test]
    fn test_codepointinversionlist_get_nth_range() {
        let ex = vec![0x41, 0x44, 0x45, 0x46, 0xD800, 0xD801];
        let set = CodePointInversionList::try_from_u32_inversion_list_slice(&ex).unwrap();
        assert_eq!(Some(0x41..=0x43), set.get_nth_range(0));
        assert_eq!(Some(0x45..=0x45), set.get_nth_range(1));
        assert_eq!(Some(0xD800..=0xD800), set.get_nth_range(2));
        assert_eq!(None, set.get_nth_range(3));
    }

    // Range<char> cannot represent the upper bound (non-inclusive) for
    // char::MAX, whereas Range<u32> can.
    #[test]
    fn test_codepointinversionlist_iter_ranges_with_max_code_point() {
        let ex = vec![0x80, (char::MAX as u32) + 1];
        let set = CodePointInversionList::try_from_u32_inversion_list_slice(&ex).unwrap();
        let mut ranges = set.iter_ranges();
        assert_eq!(Some(0x80..=(char::MAX as u32)), ranges.next());
        assert_eq!(None, ranges.next());
    }

    #[test]
    fn test_codepointinversionlist_span_contains() {
        let ex = vec![0x41, 0x44, 0x46, 0x4B]; // A - D, F - K
        let check = CodePointInversionList::try_from_u32_inversion_list_slice(&ex).unwrap();
        assert_eq!(check.span("ABCDE", true), 3);
        assert_eq!(check.span("E", true), 0);
    }

    #[test]
    fn test_codepointinversionlist_span_does_not_contain() {
        let ex = vec![0x41, 0x44, 0x46, 0x4B]; // A - D, F - K
        let check = CodePointInversionList::try_from_u32_inversion_list_slice(&ex).unwrap();
        assert_eq!(check.span("DEF", false), 2);
        assert_eq!(check.span("KLMA", false), 3);
    }

    #[test]
    fn test_codepointinversionlist_span_back_contains() {
        let ex = vec![0x41, 0x44, 0x46, 0x4B]; // A - D, F - K
        let check = CodePointInversionList::try_from_u32_inversion_list_slice(&ex).unwrap();
        assert_eq!(check.span_back("XYZABFH", true), 3);
        assert_eq!(check.span_back("ABCXYZ", true), 6);
    }

    #[test]
    fn test_codepointinversionlist_span_back_does_not_contain() {
        let ex = vec![0x41, 0x44, 0x46, 0x4B]; // A - D, F - K
        let check = CodePointInversionList::try_from_u32_inversion_list_slice(&ex).unwrap();
        assert_eq!(check.span_back("ABCXYZ", false), 3);
        assert_eq!(check.span_back("XYZABC", false), 6);
    }

    #[test]
    fn test_uniset_to_inv_list() {
        let inv_list = [
            0x9, 0xE, 0x20, 0x21, 0x85, 0x86, 0xA0, 0xA1, 0x1626, 0x1627, 0x2000, 0x2003, 0x2028,
            0x202A, 0x202F, 0x2030, 0x205F, 0x2060, 0x3000, 0x3001,
        ];
        let s: CodePointInversionList =
            CodePointInversionList::try_from_u32_inversion_list_slice(&inv_list).unwrap();
        let round_trip_inv_list = s.get_inversion_list_vec();
        assert_eq!(
            round_trip_inv_list.into_iter().collect::<ZeroVec<u32>>(),
            inv_list
        );
    }

    #[test]
    fn test_serde_serialize() {
        let inv_list = [0x41, 0x46, 0x4B, 0x55];
        let uniset = CodePointInversionList::try_from_u32_inversion_list_slice(&inv_list).unwrap();
        let json_str = serde_json::to_string(&uniset).unwrap();
        assert_eq!(json_str, r#"["A-E","K-T"]"#);
    }

    #[test]
    fn test_serde_serialize_surrogates() {
        let inv_list = [0xDFAB, 0xDFFF];
        let uniset = CodePointInversionList::try_from_u32_inversion_list_slice(&inv_list).unwrap();
        let json_str = serde_json::to_string(&uniset).unwrap();
        assert_eq!(json_str, r#"["U+DFAB-U+DFFE"]"#);
    }

    #[test]
    fn test_serde_deserialize() {
        let inv_list_str = r#"["A-E","K-T"]"#;
        let exp_inv_list = [0x41, 0x46, 0x4B, 0x55];
        let exp_uniset =
            CodePointInversionList::try_from_u32_inversion_list_slice(&exp_inv_list).unwrap();
        let act_uniset: CodePointInversionList = serde_json::from_str(inv_list_str).unwrap();
        assert_eq!(act_uniset, exp_uniset);
    }

    #[test]
    fn test_serde_deserialize_surrogates() {
        let inv_list_str = r#"["U+DFAB-U+DFFE"]"#;
        let exp_inv_list = [0xDFAB, 0xDFFF];
        let exp_uniset =
            CodePointInversionList::try_from_u32_inversion_list_slice(&exp_inv_list).unwrap();
        let act_uniset: CodePointInversionList = serde_json::from_str(inv_list_str).unwrap();
        assert_eq!(act_uniset, exp_uniset);
    }

    #[test]
    fn test_serde_deserialize_invalid() {
        assert!(serde_json::from_str::<CodePointInversionList>("[65,70,98775,85]").is_err());
        assert!(serde_json::from_str::<CodePointInversionList>("[65,70,U+FFFFFFFFFF,85]").is_err());
    }

    #[test]
    fn test_serde_with_postcard_roundtrip() -> Result<(), postcard::Error> {
        let set = CodePointInversionList::bmp();
        let set_serialized: Vec<u8> = postcard::to_allocvec(&set).unwrap();
        let set_deserialized: CodePointInversionList =
            postcard::from_bytes::<CodePointInversionList>(&set_serialized)?;

        assert_eq!(&set, &set_deserialized);
        assert!(!set_deserialized.inv_list.is_owned());

        Ok(())
    }

    #[test]
    fn databake() {
        databake::test_bake!(
            CodePointInversionList<'static>,
            const,
            unsafe {
                #[allow(unused_unsafe)]
                crate::codepointinvlist::CodePointInversionList::from_parts_unchecked(
                    unsafe {
                        zerovec::ZeroVec::from_bytes_unchecked(
                            b"0\0\0\0:\0\0\0A\0\0\0G\0\0\0a\0\0\0g\0\0\0",
                        )
                    },
                    22u32,
                )
            },
            icu_collections,
            [zerovec],
        );
    }
}
