// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::codepointtrie::error::Error;
use crate::codepointtrie::impl_const::*;

#[cfg(feature = "alloc")]
use crate::codepointinvlist::CodePointInversionList;
use core::char::CharTryFromError;
use core::convert::Infallible;
use core::convert::TryFrom;
use core::fmt::Display;
#[cfg(feature = "alloc")]
use core::iter::FromIterator;
use core::num::TryFromIntError;
use core::ops::RangeInclusive;
use yoke::Yokeable;
use zerofrom::ZeroFrom;
use zerovec::ule::AsULE;
#[cfg(feature = "alloc")]
use zerovec::ule::UleError;
use zerovec::ZeroSlice;
use zerovec::ZeroVec;

/// The type of trie represents whether the trie has an optimization that
/// would make it smaller or faster.
///
/// Regarding performance, a trie being a small or fast type affects the number of array lookups
/// needed for code points in the range `[0x1000, 0x10000)`. In this range, `Small` tries use 4 array lookups,
/// while `Fast` tries use 2 array lookups.
/// Code points before the interval (in `[0, 0x1000)`) will always use 2 array lookups.
/// Code points after the interval (in `[0x10000, 0x10FFFF]`) will always use 4 array lookups.
///
/// Regarding size, `Fast` type tries are larger than `Small` type tries because the minimum size of
/// the index array is larger. The minimum size is the "fast max" limit, which is the limit of the range
/// of code points with 2 array lookups.
///
/// See the document [Unicode Properties and Code Point Tries in ICU4X](https://github.com/unicode-org/icu4x/blob/main/documents/design/properties_code_point_trie.md).
///
/// Also see [`UCPTrieType`](https://unicode-org.github.io/icu-docs/apidoc/dev/icu4c/ucptrie_8h.html) in ICU4C.
#[derive(Clone, Copy, PartialEq, Debug, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "databake", derive(databake::Bake))]
#[cfg_attr(feature = "databake", databake(path = icu_collections::codepointtrie))]
pub enum TrieType {
    /// Represents the "fast" type code point tries for the
    /// [`TrieType`] trait. The "fast max" limit is set to `0xffff`.
    Fast = 0,
    /// Represents the "small" type code point tries for the
    /// [`TrieType`] trait. The "fast max" limit is set to `0x0fff`.
    Small = 1,
}

// TrieValue trait

// AsULE is AsUnalignedLittleEndian, i.e. "allowed in a zerovec"

/// A trait representing the values stored in the data array of a [`CodePointTrie`].
/// This trait is used as a type parameter in constructing a `CodePointTrie`.
///
/// This trait can be implemented on anything that can be represented as a u32s worth of data.
pub trait TrieValue: Copy + Eq + PartialEq + zerovec::ule::AsULE + 'static {
    /// Last-resort fallback value to return if we cannot read data from the trie.
    ///
    /// In most cases, the error value is read from the last element of the `data` array,
    /// this value is used for empty codepointtrie arrays
    /// Error type when converting from a u32 to this `TrieValue`.
    type TryFromU32Error: Display;
    /// A parsing function that is primarily motivated by deserialization contexts.
    /// When the serialization type width is smaller than 32 bits, then it is expected
    /// that the call site will widen the value to a `u32` first.
    fn try_from_u32(i: u32) -> Result<Self, Self::TryFromU32Error>;

    /// A method for converting back to a `u32` that can roundtrip through
    /// [`Self::try_from_u32()`]. The default implementation of this trait
    /// method panics in debug mode and returns 0 in release mode.
    ///
    /// This method is allowed to have GIGO behavior when fed a value that has
    /// no corresponding `u32` (since such values cannot be stored in the trie)
    fn to_u32(self) -> u32;
}

macro_rules! impl_primitive_trie_value {
    ($primitive:ty, $error:ty) => {
        impl TrieValue for $primitive {
            type TryFromU32Error = $error;
            fn try_from_u32(i: u32) -> Result<Self, Self::TryFromU32Error> {
                Self::try_from(i)
            }

            fn to_u32(self) -> u32 {
                // bitcast when the same size, zero-extend/sign-extend
                // when not the same size
                self as u32
            }
        }
    };
}

impl_primitive_trie_value!(u8, TryFromIntError);
impl_primitive_trie_value!(u16, TryFromIntError);
impl_primitive_trie_value!(u32, Infallible);
impl_primitive_trie_value!(i8, TryFromIntError);
impl_primitive_trie_value!(i16, TryFromIntError);
impl_primitive_trie_value!(i32, TryFromIntError);
impl_primitive_trie_value!(char, CharTryFromError);

/// Helper function used by [`get_range`]. Converts occurrences of trie's null
/// value into the provided `null_value`.
///
/// Note: the ICU version of this helper function uses a `ValueFilter` function
/// to apply a transform on a non-null value. But currently, this implementation
/// stops short of that functionality, and instead leaves the non-null trie value
/// untouched. This is equivalent to having a `ValueFilter` function that is the
/// identity function.
fn maybe_filter_value<T: TrieValue>(value: T, trie_null_value: T, null_value: T) -> T {
    if value == trie_null_value {
        null_value
    } else {
        value
    }
}

/// This struct represents a de-serialized [`CodePointTrie`] that was exported from
/// ICU binary data.
///
/// For more information:
/// - [ICU Site design doc](http://site.icu-project.org/design/struct/utrie)
/// - [ICU User Guide section on Properties lookup](https://unicode-org.github.io/icu/userguide/strings/properties.html#lookup)
// serde impls in crate::serde
#[derive(Debug, Eq, PartialEq, Yokeable, ZeroFrom)]
pub struct CodePointTrie<'trie, T: TrieValue> {
    /// # Safety Invariant
    ///
    /// The value of `header.trie_type` must not change after construction.
    pub(crate) header: CodePointTrieHeader,
    /// # Safety Invariant
    ///
    /// If `header.trie_type == TrieType::Fast`, `index.len()` must be greater
    /// than `FAST_TYPE_FAST_INDEXING_MAX`. Otherwise, `index.len()`
    /// must be greater than `SMALL_TYPE_FAST_INDEXING_MAX`. Furthermore,
    /// this field must not change after construction. (Strictly: It must
    /// not become shorter than the length requirement stated above and the
    /// values within the prefix up to the length requirement must not change.)
    pub(crate) index: ZeroVec<'trie, u16>,
    /// # Safety Invariant
    ///
    /// If `header.trie_type == TrieType::Fast`, `data.len()` must be greater
    /// than `FAST_TYPE_DATA_MASK` plus the largest value in
    /// `index[0..FAST_TYPE_FAST_INDEXING_MAX + 1]`. Otherwise, `data.len()`
    /// must be greater than `FAST_TYPE_DATA_MASK` plus the largest value in
    /// `index[0..SMALL_TYPE_FAST_INDEXING_MAX + 1]`. Furthermore, this field
    /// must not change after construction. (Strictly: The stated length
    /// requirement must continue to hold.)
    pub(crate) data: ZeroVec<'trie, T>,
    // serde impl skips this field
    #[zerofrom(clone)] // TrieValue is Copy, this allows us to avoid
    // a T: ZeroFrom bound
    pub(crate) error_value: T,
}

/// This struct contains the fixed-length header fields of a [`CodePointTrie`].
///
/// # Safety Invariant
///
/// The `trie_type` field must not change after construction.
///
/// (In practice, all the fields here remain unchanged during the lifetime
/// of the trie.)
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "databake", derive(databake::Bake))]
#[cfg_attr(feature = "databake", databake(path = icu_collections::codepointtrie))]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Yokeable, ZeroFrom)]
pub struct CodePointTrieHeader {
    /// The code point of the start of the last range of the trie. A
    /// range is defined as a partition of the code point space such that the
    /// value in this trie associated with all code points of the same range is
    /// the same.
    ///
    /// For the property value data for many Unicode properties,
    /// often times, `high_start` is `U+10000` or lower. In such cases, not
    /// reserving space in the `index` array for duplicate values is a large
    /// savings. The "highValue" associated with the `high_start` range is
    /// stored at the second-to-last position of the `data` array.
    /// (See `impl_const::HIGH_VALUE_NEG_DATA_OFFSET`.)
    pub high_start: u32,
    /// A version of the `high_start` value that is right-shifted 12 spaces,
    /// but is rounded up to a multiple `0x1000` for easy testing from UTF-8
    /// lead bytes.
    pub shifted12_high_start: u16,
    /// Offset for the null block in the "index-3" table of the `index` array.
    /// Set to an impossibly high value (e.g., `0xffff`) if there is no
    /// dedicated index-3 null block.
    pub index3_null_offset: u16,
    /// Internal data null block offset, not shifted.
    /// Set to an impossibly high value (e.g., `0xfffff`) if there is no
    /// dedicated data null block.
    pub data_null_offset: u32,
    /// The value stored in the trie that represents a null value being
    /// associated to a code point.
    pub null_value: u32,
    /// The enum value representing the type of trie, where trie type is as it
    /// is defined in ICU (ex: Fast, Small).
    ///
    /// # Safety Invariant
    ///
    /// Must not change after construction.
    pub trie_type: TrieType,
}

impl TryFrom<u8> for TrieType {
    type Error = crate::codepointtrie::error::Error;

    fn try_from(trie_type_int: u8) -> Result<TrieType, crate::codepointtrie::error::Error> {
        match trie_type_int {
            0 => Ok(TrieType::Fast),
            1 => Ok(TrieType::Small),
            _ => Err(crate::codepointtrie::error::Error::FromDeserialized {
                reason: "Cannot parse value for trie_type",
            }),
        }
    }
}

// Helper macro that turns arithmetic into wrapping-in-release, checked-in-debug arithmetic
//
// This is rustc's default behavior anyway, however some projects like Android deliberately
// enable overflow checks. CodePointTrie::get() is intended to be used in Android bionic which
// cares about codesize and we don't want the pile of panicking infrastructure brought in by overflow
// checks, so we force wrapping in release.
// See #6052
macro_rules! w(
    // Note: these matchers are not perfect since you cannot have an operator after an expr matcher
    // Use variables if you need complex first operands.
    ($a:tt + $b:expr) => {
        {
            #[allow(unused_parens)]
            let a = $a;
            let b = $b;
            debug_assert!(a.checked_add(b).is_some());
            $a.wrapping_add($b)
        }
    };
    ($a:tt - $b:expr) => {

        {
            #[allow(unused_parens)]
            let a = $a;
            let b = $b;
            debug_assert!(a.checked_sub(b).is_some());
            $a.wrapping_sub($b)
        }
    };
    ($a:tt * $b:expr) => {
        {
            #[allow(unused_parens)]
            let a = $a;
            let b = $b;
            debug_assert!(a.checked_mul(b).is_some());
            $a.wrapping_mul($b)
        }
    };
);

impl<'trie, T: TrieValue> CodePointTrie<'trie, T> {
    #[doc(hidden)] // databake internal
    /// # Safety
    ///
    /// `header.trie_type`, `index`, and `data` must
    /// satisfy the invariants for the fields of the
    /// same names on `CodePointTrie`.
    pub const unsafe fn from_parts_unstable_unchecked_v1(
        header: CodePointTrieHeader,
        index: ZeroVec<'trie, u16>,
        data: ZeroVec<'trie, T>,
        error_value: T,
    ) -> Self {
        // Field invariants upheld: The caller is responsible.
        // In practice, this means that datagen in the databake
        // mode upholds these invariants when constructing the
        // `CodePointTrie` that is then baked.
        Self {
            header,
            index,
            data,
            error_value,
        }
    }

    /// Returns a new [`CodePointTrie`] backed by borrowed data for the `index`
    /// array and `data` array, whose data values have width `W`.
    pub fn try_new(
        header: CodePointTrieHeader,
        index: ZeroVec<'trie, u16>,
        data: ZeroVec<'trie, T>,
    ) -> Result<CodePointTrie<'trie, T>, Error> {
        let error_value = Self::validate_fields(&header, &index, &data)?;
        // Field invariants upheld: Checked by `validate_fields` above.
        let trie: CodePointTrie<'trie, T> = CodePointTrie {
            header,
            index,
            data,
            error_value,
        };
        Ok(trie)
    }

    /// Checks the invariant on the fields that fast-path access relies on for
    /// safety in order to omit slice bound checks and upon success returns the
    /// `error_value` for the trie.
    ///
    /// # Safety Usable Invariant
    ///
    /// Iff this function returns `Ok(T)`, the arguments satisfy the invariants
    /// for corresponding fields of `CodePointTrie`. (Other than proving that
    /// nothing else changes the fields subsequently.)
    pub(crate) fn validate_fields(
        header: &CodePointTrieHeader,
        index: &ZeroSlice<u16>,
        data: &ZeroSlice<T>,
    ) -> Result<T, Error> {
        let error_value = data.last().ok_or(Error::EmptyDataVector)?;

        // `CodePointTrie` lookup has two stages: fast and small (the trie types
        // are also fast and small; they affect where the boundary between fast
        // and small lookups is).
        //
        // The length requirements for `index` and `data` are checked here only
        // for the fast lookup case so that the fast lookup can omit bound checks
        // at the time of access. In the small lookup case, bounds are checked at
        // the time of access.
        //
        // The fast lookup happens on the prefixes of `index` and `data` with
        // more items for the small lookup case afterwards. The check here
        // only looks at the prefixes relevant to the fast case.
        //
        // In the fast lookup case, the bits of the of the code point are
        // partitioned into a bit prefix and a bit suffix. First, a value
        // is read from `index` by indexing into it using the bit prefix.
        // Then `data` is accessed by the value just read from `index` plus
        // the bit suffix.
        //
        // Therefore, the length of `index` needs to accommodate access
        // by the maximum possible bit prefix, and the length of `data`
        // needs to accommodate access by the largest value stored in the part
        // of `data` reachable by the bit prefix plus the maximum possible bit
        // suffix.
        //
        // The maximum possible bit prefix depends on the trie type.

        // The maximum code point that can be used for fast-path access:
        let fast_max = match header.trie_type {
            TrieType::Fast => FAST_TYPE_FAST_INDEXING_MAX,
            TrieType::Small => SMALL_TYPE_FAST_INDEXING_MAX,
        };
        // Keep only the prefix bits:
        let max_bit_prefix = fast_max >> FAST_TYPE_SHIFT;
        // Attempt slice the part of `index` that the fast path can index into.
        // Since `max_bit_prefix` is the largest possible value used for
        // indexing (inclusive bound), we need to add one to get the exclusive
        // bound, which is what `get_subslice` wants.
        let fast_index = index
            .get_subslice(0..(max_bit_prefix as usize) + 1)
            .ok_or(Error::IndexTooShortForFastAccess)?;
        // Invariant upheld for `index`: If we got this far, the length of `index`
        // satisfies its length invariant on the assumption that `header.trie_type`
        // will not change subsequently.

        // Now find the largest offset in the part of `index` reachable by the
        // bit prefix. `max` can never actually return `None`, since we already
        // know the slice isn't empty. Hence, reusing the error kind instead of
        // minting a new one for this check.
        let max_offset = fast_index
            .iter()
            .max()
            .ok_or(Error::IndexTooShortForFastAccess)?;
        // `FAST_TYPE_DATA_MASK` is the maximum possible bit suffix, since the
        // maximum is when all the bits in the suffix are set, and the mask
        // has that many bits set.
        if (max_offset) as usize + (FAST_TYPE_DATA_MASK as usize) >= data.len() {
            return Err(Error::DataTooShortForFastAccess);
        }

        // Invariant upheld for `data`: If we got this far, the length of `data`
        // satisfies `data`'s length invariant on the assumption that the contents
        // of `fast_index` subslice of `index` and `header.trie_type` will not
        // change subsequently.

        Ok(error_value)
    }

    /// Turns this trie into a version whose trie type is encoded in the Rust type.
    #[inline]
    pub fn to_typed(self) -> Typed<FastCodePointTrie<'trie, T>, SmallCodePointTrie<'trie, T>> {
        match self.header.trie_type {
            TrieType::Fast => Typed::Fast(FastCodePointTrie { inner: self }),
            TrieType::Small => Typed::Small(SmallCodePointTrie { inner: self }),
        }
    }

    /// Obtains a reference to this trie as a Rust type that encodes the trie type in the Rust type.
    #[inline]
    pub fn as_typed_ref(
        &self,
    ) -> Typed<&FastCodePointTrie<'trie, T>, &SmallCodePointTrie<'trie, T>> {
        // SAFETY: `FastCodePointTrie` and `SmallCodePointTrie` are `repr(transparent)`
        // with `CodePointTrie`, so transmuting between the references is OK when the
        // actual trie type agrees with the semantics of the typed wrapper.
        match self.header.trie_type {
            TrieType::Fast => Typed::Fast(unsafe {
                core::mem::transmute::<&CodePointTrie<'trie, T>, &FastCodePointTrie<'trie, T>>(self)
            }),
            TrieType::Small => Typed::Small(unsafe {
                core::mem::transmute::<&CodePointTrie<'trie, T>, &SmallCodePointTrie<'trie, T>>(
                    self,
                )
            }),
        }
    }

    /// Returns the position in the data array containing the trie's stored
    /// error value.
    #[inline(always)] // `always` was based on previous normalizer benchmarking
    fn trie_error_val_index(&self) -> u32 {
        // We use wrapping_sub here to avoid panicky overflow checks.
        // len should always be > 1, but if it isn't this will just cause GIGO behavior of producing
        // None on `.get()`
        debug_assert!(self.data.len() as u32 >= ERROR_VALUE_NEG_DATA_OFFSET);
        w!((self.data.len() as u32) - ERROR_VALUE_NEG_DATA_OFFSET)
    }

    fn internal_small_index(&self, code_point: u32) -> u32 {
        // We use wrapping arithmetic here to avoid overflow checks making their way into binaries
        // with overflow checks enabled. Ultimately this code ends up as a checked index, so any
        // bugs here will cause GIGO
        let mut index1_pos: u32 = code_point >> SHIFT_1;
        if self.header.trie_type == TrieType::Fast {
            debug_assert!(
                FAST_TYPE_FAST_INDEXING_MAX < code_point && code_point < self.header.high_start
            );
            index1_pos = w!(index1_pos + BMP_INDEX_LENGTH - OMITTED_BMP_INDEX_1_LENGTH);
        } else {
            assert!(code_point < self.header.high_start && self.header.high_start > SMALL_LIMIT);
            index1_pos = w!(index1_pos + SMALL_INDEX_LENGTH);
        }
        let index1_val = if let Some(index1_val) = self.index.get(index1_pos as usize) {
            index1_val
        } else {
            return self.trie_error_val_index();
        };
        let index3_block_idx: u32 =
            w!((index1_val as u32) + (code_point >> SHIFT_2) & INDEX_2_MASK);
        let mut index3_block: u32 =
            if let Some(index3_block) = self.index.get(index3_block_idx as usize) {
                index3_block as u32
            } else {
                return self.trie_error_val_index();
            };
        let mut index3_pos: u32 = (code_point >> SHIFT_3) & INDEX_3_MASK;
        let mut data_block: u32;
        if index3_block & 0x8000 == 0 {
            // 16-bit indexes
            data_block =
                if let Some(data_block) = self.index.get(w!(index3_block + index3_pos) as usize) {
                    data_block as u32
                } else {
                    return self.trie_error_val_index();
                };
        } else {
            // 18-bit indexes stored in groups of 9 entries per 8 indexes.
            index3_block = w!((index3_block & 0x7fff) + w!((index3_pos & !7) + index3_pos >> 3));
            index3_pos &= 7;
            data_block = if let Some(data_block) = self.index.get(index3_block as usize) {
                data_block as u32
            } else {
                return self.trie_error_val_index();
            };
            data_block = (data_block << w!(2u32 + w!(2u32 * index3_pos))) & 0x30000;
            index3_block += 1;
            data_block =
                if let Some(index3_val) = self.index.get(w!(index3_block + index3_pos) as usize) {
                    data_block | (index3_val as u32)
                } else {
                    return self.trie_error_val_index();
                };
        }
        // Returns data_pos == data_block (offset) +
        //     portion of code_point bit field for last (4th) lookup
        w!(data_block + code_point & SMALL_DATA_MASK)
    }

    /// Returns the position in the `data` array for the given code point,
    /// where this code point is at or above the fast limit associated for the
    /// `trie_type`. We will refer to that limit as "`fastMax`" here.
    ///
    /// A lookup of the value in the code point trie for a code point in the
    /// code point space range [`fastMax`, `high_start`) will be a 4-step
    /// lookup: 3 lookups in the `index` array and one lookup in the `data`
    /// array. Lookups for code points in the range [`high_start`,
    /// `CODE_POINT_MAX`] are short-circuited to be a single lookup, see
    /// [`CodePointTrieHeader::high_start`].
    fn small_index(&self, code_point: u32) -> u32 {
        if code_point >= self.header.high_start {
            w!((self.data.len() as u32) - HIGH_VALUE_NEG_DATA_OFFSET)
        } else {
            self.internal_small_index(code_point) // helper fn
        }
    }

    /// Returns the position in the `data` array for the given code point,
    /// where this code point is below the fast limit associated for the
    /// `trie type`. We will refer to that limit as "`fastMax`" here.
    ///
    /// A lookup of the value in the code point trie for a code point in the
    /// code point space range [0, `fastMax`) will be a 2-step lookup: 1
    /// lookup in the `index` array and one lookup in the `data` array. By
    /// design, for trie type `T`, there is an element allocated in the `index`
    /// array for each block of code points in [0, `fastMax`), which in
    /// turn guarantees that those code points are represented and only need 1
    /// lookup.
    fn fast_index(&self, code_point: u32) -> u32 {
        let index_array_pos: u32 = code_point >> FAST_TYPE_SHIFT;
        let index_array_val: u16 =
            if let Some(index_array_val) = self.index.get(index_array_pos as usize) {
                index_array_val
            } else {
                return self.trie_error_val_index();
            };
        let masked_cp = code_point & FAST_TYPE_DATA_MASK;
        let index_array_val = index_array_val as u32;
        let fast_index_val: u32 = w!(index_array_val + masked_cp);
        fast_index_val
    }

    /// Returns the value that is associated with `code_point` in this [`CodePointTrie`]
    /// if `code_point` uses fast-path lookup or `None` if `code_point`
    /// should use small-path lookup or is above the supported range.
    #[inline(always)] // "always" to make the `Option` collapse away
    fn get32_by_fast_index(&self, code_point: u32) -> Option<T> {
        let fast_max = match self.header.trie_type {
            TrieType::Fast => FAST_TYPE_FAST_INDEXING_MAX,
            TrieType::Small => SMALL_TYPE_FAST_INDEXING_MAX,
        };
        if code_point <= fast_max {
            // SAFETY: We just checked the invariant of
            // `get32_assuming_fast_index`,
            // which is
            // "If `self.header.trie_type == TrieType::Small`, `code_point` must be at most
            // `SMALL_TYPE_FAST_INDEXING_MAX`. If `self.header.trie_type ==
            // TrieType::Fast`, `code_point` must be at most `FAST_TYPE_FAST_INDEXING_MAX`."
            Some(unsafe { self.get32_assuming_fast_index(code_point) })
        } else {
            // The caller needs to call `get32_by_small_index` or determine
            // that the argument is above the permitted range.
            None
        }
    }

    /// Performs the actual fast-mode lookup
    ///
    /// # Safety
    ///
    /// If `self.header.trie_type == TrieType::Small`, `code_point` must be at most
    /// `SMALL_TYPE_FAST_INDEXING_MAX`. If `self.header.trie_type ==
    /// TrieType::Fast`, `code_point` must be at most `FAST_TYPE_FAST_INDEXING_MAX`.
    #[inline(always)]
    unsafe fn get32_assuming_fast_index(&self, code_point: u32) -> T {
        // Check the safety invariant of this method.
        debug_assert!(
            code_point
                <= match self.header.trie_type {
                    TrieType::Fast => FAST_TYPE_FAST_INDEXING_MAX,
                    TrieType::Small => SMALL_TYPE_FAST_INDEXING_MAX,
                }
        );

        let bit_prefix = (code_point as usize) >> FAST_TYPE_SHIFT;
        debug_assert!(bit_prefix < self.index.len());
        // SAFETY: Relying on the length invariant of `self.index` having
        // been checked and on the unchangedness invariant of `self.index`
        // and `self.header.trie_type` after construction.
        let base_offset_to_data: usize = usize::from(u16::from_unaligned(*unsafe {
            self.index.as_ule_slice().get_unchecked(bit_prefix)
        }));
        let bit_suffix = (code_point & FAST_TYPE_DATA_MASK) as usize;
        // SAFETY: Cannot overflow with supported (32-bit and 64-bit) `usize`
        // sizes, since `base_offset_to_data` was extended from `u16` and
        // `bit_suffix` is at most `FAST_TYPE_DATA_MASK`, which is well
        // under what it takes to reach the 32-bit (or 64-bit) max with
        // additon from the max of `u16`.
        let offset_to_data = w!(base_offset_to_data + bit_suffix);
        debug_assert!(offset_to_data < self.data.len());
        // SAFETY: Relying on the length invariant of `self.data` having
        // been checked and on the unchangedness invariant of `self.data`,
        // `self.index`, and `self.header.trie_type` after construction.
        T::from_unaligned(*unsafe { self.data.as_ule_slice().get_unchecked(offset_to_data) })
    }

    /// Coldness wrapper for `get32_by_small_index` to also allow
    /// calls without the effects of `#[cold]`.
    #[cold]
    #[inline(always)]
    fn get32_by_small_index_cold(&self, code_point: u32) -> T {
        self.get32_by_small_index(code_point)
    }

    /// Returns the value that is associated with `code_point` in this [`CodePointTrie`]
    /// assuming that the small index path should be used.
    ///
    /// # Intended Precondition
    ///
    /// `code_point` must be at most `CODE_POINT_MAX` AND greter than
    /// `FAST_TYPE_FAST_INDEXING_MAX` if the trie type is fast or greater
    /// than `SMALL_TYPE_FAST_INDEXING_MAX` if the trie type is small.
    /// This is checked when debug assertions are enabled. If this
    /// precondition is violated, the behavior of this method is
    /// memory-safe, but the returned value may be bogus (not
    /// necessarily the designated error value).
    #[inline(never)]
    fn get32_by_small_index(&self, code_point: u32) -> T {
        debug_assert!(code_point <= CODE_POINT_MAX);
        debug_assert!(
            code_point
                > match self.header.trie_type {
                    TrieType::Fast => FAST_TYPE_FAST_INDEXING_MAX,
                    TrieType::Small => SMALL_TYPE_FAST_INDEXING_MAX,
                }
        );
        self.data
            .get(self.small_index(code_point) as usize)
            .unwrap_or(self.error_value)
    }

    /// Returns the value that is associated with `code_point` in this [`CodePointTrie`].
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::collections::codepointtrie::planes;
    /// let trie = planes::get_planes_trie();
    ///
    /// assert_eq!(0, trie.get32(0x41)); // 'A' as u32
    /// assert_eq!(0, trie.get32(0x13E0)); // 'Ꮰ' as u32
    /// assert_eq!(1, trie.get32(0x10044)); // '𐁄' as u32
    /// ```
    #[inline(always)] // `always` based on normalizer benchmarking
    pub fn get32(&self, code_point: u32) -> T {
        if let Some(v) = self.get32_by_fast_index(code_point) {
            v
        } else if code_point <= CODE_POINT_MAX {
            self.get32_by_small_index_cold(code_point)
        } else {
            self.error_value
        }
    }

    /// Returns the value that is associated with `char` in this [`CodePointTrie`].
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::collections::codepointtrie::planes;
    /// let trie = planes::get_planes_trie();
    ///
    /// assert_eq!(0, trie.get('A')); // 'A' as u32
    /// assert_eq!(0, trie.get('Ꮰ')); // 'Ꮰ' as u32
    /// assert_eq!(1, trie.get('𐁄')); // '𐁄' as u32
    /// ```
    #[inline(always)]
    pub fn get(&self, c: char) -> T {
        // LLVM's optimizations have been observed not to be 100%
        // reliable around collapsing away unnecessary parts of
        // `get32`, so not just calling `get32` here.
        let code_point = u32::from(c);
        if let Some(v) = self.get32_by_fast_index(code_point) {
            v
        } else {
            self.get32_by_small_index_cold(code_point)
        }
    }

    /// Returns the value that is associated with `bmp` in this [`CodePointTrie`].
    #[inline(always)]
    pub fn get16(&self, bmp: u16) -> T {
        // LLVM's optimizations have been observed not to be 100%
        // reliable around collapsing away unnecessary parts of
        // `get32`, so not just calling `get32` here.
        let code_point = u32::from(bmp);
        if let Some(v) = self.get32_by_fast_index(code_point) {
            v
        } else {
            self.get32_by_small_index_cold(code_point)
        }
    }

    /// Lookup trie value by non-Basic Multilingual Plane Scalar Value.
    ///
    /// The return value may be bogus (not necessarily `error_value`) is the argument is actually in
    /// the Basic Multilingual Plane or above the Unicode Scalar Value
    /// range (panics instead with debug assertions enabled).
    #[inline(always)]
    pub fn get32_supplementary(&self, supplementary: u32) -> T {
        debug_assert!(supplementary > 0xFFFF);
        debug_assert!(supplementary <= CODE_POINT_MAX);
        self.get32_by_small_index(supplementary)
    }

    /// Returns a reference to the ULE of the value that is associated with `code_point` in this [`CodePointTrie`].
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::collections::codepointtrie::planes;
    /// let trie = planes::get_planes_trie();
    ///
    /// assert_eq!(Some(&0), trie.get32_ule(0x41)); // 'A' as u32
    /// assert_eq!(Some(&0), trie.get32_ule(0x13E0)); // 'Ꮰ' as u32
    /// assert_eq!(Some(&1), trie.get32_ule(0x10044)); // '𐁄' as u32
    /// ```
    #[inline(always)] // `always` was based on previous normalizer benchmarking
    pub fn get32_ule(&self, code_point: u32) -> Option<&T::ULE> {
        // All code points up to the fast max limit are represented
        // individually in the `index` array to hold their `data` array position, and
        // thus only need 2 lookups for a [CodePointTrie::get()](`crate::codepointtrie::CodePointTrie::get`).
        // Code points above the "fast max" limit require 4 lookups.
        let fast_max = match self.header.trie_type {
            TrieType::Fast => FAST_TYPE_FAST_INDEXING_MAX,
            TrieType::Small => SMALL_TYPE_FAST_INDEXING_MAX,
        };
        let data_pos: u32 = if code_point <= fast_max {
            Self::fast_index(self, code_point)
        } else if code_point <= CODE_POINT_MAX {
            Self::small_index(self, code_point)
        } else {
            self.trie_error_val_index()
        };
        // Returns the trie value (or trie's error value).
        self.data.as_ule_slice().get(data_pos as usize)
    }

    /// Converts the [`CodePointTrie`] into one that returns another type of the same size.
    ///
    /// Borrowed data remains borrowed, and owned data remains owned.
    ///
    /// If the old and new types are not the same size, use
    /// [`CodePointTrie::try_alloc_map_value`].
    ///
    /// # Panics
    ///
    /// Panics if `T` and `P` are different sizes.
    ///
    /// More specifically, panics if [`ZeroVec::try_into_converted()`] panics when converting
    /// `ZeroVec<T>` into `ZeroVec<P>`, which happens if `T::ULE` and `P::ULE` differ in size.
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use icu::collections::codepointtrie::planes;
    /// use icu::collections::codepointtrie::CodePointTrie;
    ///
    /// let planes_trie_u8: CodePointTrie<u8> = planes::get_planes_trie();
    /// let planes_trie_i8: CodePointTrie<i8> =
    ///     planes_trie_u8.try_into_converted().expect("infallible");
    ///
    /// assert_eq!(planes_trie_i8.get32(0x30000), 3);
    /// ```
    #[cfg(feature = "alloc")]
    pub fn try_into_converted<P>(self) -> Result<CodePointTrie<'trie, P>, UleError>
    where
        P: TrieValue,
    {
        let converted_data = self.data.try_into_converted()?;
        let error_ule = self.error_value.to_unaligned();
        let slice = &[error_ule];
        let error_vec = ZeroVec::<T>::new_borrowed(slice);
        let error_converted = error_vec.try_into_converted::<P>()?;
        #[expect(clippy::expect_used)] // we know this cannot fail
        Ok(CodePointTrie {
            header: self.header,
            index: self.index,
            data: converted_data,
            error_value: error_converted
                .get(0)
                .expect("vector known to have one element"),
        })
    }

    /// Maps the [`CodePointTrie`] into one that returns a different type.
    ///
    /// This function returns owned data.
    ///
    /// If the old and new types are the same size, use the more efficient
    /// [`CodePointTrie::try_into_converted`].
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::collections::codepointtrie::planes;
    /// use icu::collections::codepointtrie::CodePointTrie;
    ///
    /// let planes_trie_u8: CodePointTrie<u8> = planes::get_planes_trie();
    /// let planes_trie_u16: CodePointTrie<u16> = planes_trie_u8
    ///     .try_alloc_map_value(TryFrom::try_from)
    ///     .expect("infallible");
    ///
    /// assert_eq!(planes_trie_u16.get32(0x30000), 3);
    /// ```
    #[cfg(feature = "alloc")]
    pub fn try_alloc_map_value<P, E>(
        &self,
        mut f: impl FnMut(T) -> Result<P, E>,
    ) -> Result<CodePointTrie<'trie, P>, E>
    where
        P: TrieValue,
    {
        let error_converted = f(self.error_value)?;
        let converted_data = self.data.iter().map(f).collect::<Result<ZeroVec<P>, E>>()?;
        Ok(CodePointTrie {
            header: self.header,
            index: self.index.clone(),
            data: converted_data,
            error_value: error_converted,
        })
    }

    /// Returns a [`CodePointMapRange`] struct which represents a range of code
    /// points associated with the same trie value. The returned range will be
    /// the longest stretch of consecutive code points starting at `start` that
    /// share this value.
    ///
    /// This method is designed to use the internal details of
    /// the structure of [`CodePointTrie`] to be optimally efficient. This will
    /// outperform a naive approach that just uses [`CodePointTrie::get()`].
    ///
    /// This method provides lower-level functionality that can be used in the
    /// implementation of other methods that are more convenient to the user.
    /// To obtain an optimal partition of the code point space for
    /// this trie resulting in the fewest number of ranges, see
    /// [`CodePointTrie::iter_ranges()`].
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::collections::codepointtrie::planes;
    ///
    /// let trie = planes::get_planes_trie();
    ///
    /// const CODE_POINT_MAX: u32 = 0x10ffff;
    /// let start = 0x1_0000;
    /// let exp_end = 0x1_ffff;
    ///
    /// let start_val = trie.get32(start);
    /// assert_eq!(trie.get32(exp_end), start_val);
    /// assert_ne!(trie.get32(exp_end + 1), start_val);
    ///
    /// use icu::collections::codepointtrie::CodePointMapRange;
    ///
    /// let cpm_range: CodePointMapRange<u8> = trie.get_range(start).unwrap();
    ///
    /// assert_eq!(cpm_range.range.start(), &start);
    /// assert_eq!(cpm_range.range.end(), &exp_end);
    /// assert_eq!(cpm_range.value, start_val);
    ///
    /// // `start` can be any code point, whether or not it lies on the boundary
    /// // of a maximally large range that still contains `start`
    ///
    /// let submaximal_1_start = start + 0x1234;
    /// let submaximal_1 = trie.get_range(submaximal_1_start).unwrap();
    /// assert_eq!(submaximal_1.range.start(), &0x1_1234);
    /// assert_eq!(submaximal_1.range.end(), &0x1_ffff);
    /// assert_eq!(submaximal_1.value, start_val);
    ///
    /// let submaximal_2_start = start + 0xffff;
    /// let submaximal_2 = trie.get_range(submaximal_2_start).unwrap();
    /// assert_eq!(submaximal_2.range.start(), &0x1_ffff);
    /// assert_eq!(submaximal_2.range.end(), &0x1_ffff);
    /// assert_eq!(submaximal_2.value, start_val);
    /// ```
    pub fn get_range(&self, start: u32) -> Option<CodePointMapRange<T>> {
        // Exit early if the start code point is out of range, or if it is
        // in the last range of code points in high_start..=CODE_POINT_MAX
        // (start- and end-inclusive) that all share the same trie value.
        if CODE_POINT_MAX < start {
            return None;
        }
        if start >= self.header.high_start {
            let di: usize = self.data.len() - (HIGH_VALUE_NEG_DATA_OFFSET as usize);
            let value: T = self.data.get(di)?;
            return Some(CodePointMapRange {
                range: start..=CODE_POINT_MAX,
                value,
            });
        }

        let null_value: T = T::try_from_u32(self.header.null_value).ok()?;

        let mut prev_i3_block: u32 = u32::MAX; // using u32::MAX (instead of -1 as an i32 in ICU)
        let mut prev_block: u32 = u32::MAX; // using u32::MAX (instead of -1 as an i32 in ICU)
        let mut c: u32 = start;
        let mut trie_value: T = self.error_value();
        let mut value: T = self.error_value();
        let mut have_value: bool = false;

        loop {
            let i3_block: u32;
            let mut i3: u32;
            let i3_block_length: u32;
            let data_block_length: u32;

            // Initialize values before beginning the iteration in the subsequent
            // `loop` block. In particular, use the "i3*" local variables
            // (representing the `index` array position's offset + increment
            // for a 3rd-level trie lookup) to help initialize the data block
            // variable `block` in the loop for the `data` array.
            //
            // When a lookup code point is <= the trie's *_FAST_INDEXING_MAX that
            // corresponds to its `trie_type`, the lookup only takes 2 steps
            // (once into the `index`, once into the `data` array); otherwise,
            // takes 4 steps (3 iterative lookups into the `index`, once more
            // into the `data` array). So for convenience's sake, when we have the
            // 2-stage lookup, reuse the "i3*" variable names for the first lookup.
            if c <= 0xffff
                && (self.header.trie_type == TrieType::Fast || c <= SMALL_TYPE_FAST_INDEXING_MAX)
            {
                i3_block = 0;
                i3 = c >> FAST_TYPE_SHIFT;
                i3_block_length = if self.header.trie_type == TrieType::Fast {
                    BMP_INDEX_LENGTH
                } else {
                    SMALL_INDEX_LENGTH
                };
                data_block_length = FAST_TYPE_DATA_BLOCK_LENGTH;
            } else {
                // Use the multi-stage index.
                let mut i1: u32 = c >> SHIFT_1;
                if self.header.trie_type == TrieType::Fast {
                    debug_assert!(0xffff < c && c < self.header.high_start);
                    i1 = i1 + BMP_INDEX_LENGTH - OMITTED_BMP_INDEX_1_LENGTH;
                } else {
                    debug_assert!(
                        c < self.header.high_start && self.header.high_start > SMALL_LIMIT
                    );
                    i1 += SMALL_INDEX_LENGTH;
                }
                let i2: u16 = self.index.get(i1 as usize)?;
                let i3_block_idx: u32 = (i2 as u32) + ((c >> SHIFT_2) & INDEX_2_MASK);
                i3_block = if let Some(i3b) = self.index.get(i3_block_idx as usize) {
                    i3b as u32
                } else {
                    return None;
                };
                if i3_block == prev_i3_block && (c - start) >= CP_PER_INDEX_2_ENTRY {
                    // The index-3 block is the same as the previous one, and filled with value.
                    debug_assert!((c & (CP_PER_INDEX_2_ENTRY - 1)) == 0);
                    c += CP_PER_INDEX_2_ENTRY;

                    if c >= self.header.high_start {
                        break;
                    } else {
                        continue;
                    }
                }
                prev_i3_block = i3_block;
                if i3_block == self.header.index3_null_offset as u32 {
                    // This is the index-3 null block.
                    // All of the `data` array blocks pointed to by the values
                    // in this block of the `index` 3rd-stage subarray will
                    // contain this trie's null_value. So if we are in the middle
                    // of a range, end it and return early, otherwise start a new
                    // range of null values.
                    if have_value {
                        if null_value != value {
                            return Some(CodePointMapRange {
                                range: start..=(c - 1),
                                value,
                            });
                        }
                    } else {
                        trie_value = T::try_from_u32(self.header.null_value).ok()?;
                        value = null_value;
                        have_value = true;
                    }
                    prev_block = self.header.data_null_offset;
                    c = (c + CP_PER_INDEX_2_ENTRY) & !(CP_PER_INDEX_2_ENTRY - 1);

                    if c >= self.header.high_start {
                        break;
                    } else {
                        continue;
                    }
                }
                i3 = (c >> SHIFT_3) & INDEX_3_MASK;
                i3_block_length = INDEX_3_BLOCK_LENGTH;
                data_block_length = SMALL_DATA_BLOCK_LENGTH;
            }

            // Enumerate data blocks for one index-3 block.
            loop {
                let mut block: u32;
                if (i3_block & 0x8000) == 0 {
                    block = if let Some(b) = self.index.get((i3_block + i3) as usize) {
                        b as u32
                    } else {
                        return None;
                    };
                } else {
                    // 18-bit indexes stored in groups of 9 entries per 8 indexes.
                    let mut group: u32 = (i3_block & 0x7fff) + (i3 & !7) + (i3 >> 3);
                    let gi: u32 = i3 & 7;
                    let gi_val: u32 = if let Some(giv) = self.index.get(group as usize) {
                        giv.into()
                    } else {
                        return None;
                    };
                    block = (gi_val << (2 + (2 * gi))) & 0x30000;
                    group += 1;
                    let ggi_val: u32 = if let Some(ggiv) = self.index.get((group + gi) as usize) {
                        ggiv as u32
                    } else {
                        return None;
                    };
                    block |= ggi_val;
                }

                // If our previous and current return values of the 3rd-stage `index`
                // lookup yield the same `data` block offset, and if we already know that
                // the entire `data` block / subarray starting at that offset stores
                // `value` and nothing else, then we can extend our range by the length
                // of a data block and continue.
                // Otherwise, we have to iterate over the values stored in the
                // new data block to see if they differ from `value`.
                if block == prev_block && (c - start) >= data_block_length {
                    // The block is the same as the previous one, and filled with value.
                    debug_assert!((c & (data_block_length - 1)) == 0);
                    c += data_block_length;
                } else {
                    let data_mask: u32 = data_block_length - 1;
                    prev_block = block;
                    if block == self.header.data_null_offset {
                        // This is the data null block.
                        // If we are in the middle of a range, end it and
                        // return early, otherwise start a new range of null
                        // values.
                        if have_value {
                            if null_value != value {
                                return Some(CodePointMapRange {
                                    range: start..=(c - 1),
                                    value,
                                });
                            }
                        } else {
                            trie_value = T::try_from_u32(self.header.null_value).ok()?;
                            value = null_value;
                            have_value = true;
                        }
                        c = (c + data_block_length) & !data_mask;
                    } else {
                        let mut di: u32 = block + (c & data_mask);
                        let mut trie_value_2: T = self.data.get(di as usize)?;
                        if have_value {
                            if trie_value_2 != trie_value {
                                if maybe_filter_value(
                                    trie_value_2,
                                    T::try_from_u32(self.header.null_value).ok()?,
                                    null_value,
                                ) != value
                                {
                                    return Some(CodePointMapRange {
                                        range: start..=(c - 1),
                                        value,
                                    });
                                }
                                // `trie_value` stores the previous value that was retrieved
                                // from the trie.
                                // `value` stores the value associated for the range (return
                                // value) that we are currently building, which is computed
                                // as a transformation by applying maybe_filter_value()
                                // to the trie value.
                                // The current trie value `trie_value_2` within this data block
                                // differs here from the previous value in `trie_value`.
                                // But both map to `value` after applying `maybe_filter_value`.
                                // It is not clear whether the previous or the current trie value
                                // (or neither) is more likely to match potential subsequent trie
                                // values that would extend the range by mapping to `value`.
                                // On the assumption of locality -- often times consecutive
                                // characters map to the same trie values -- remembering the new
                                // one might make it faster to extend this range further
                                // (by increasing the chance that the next `trie_value_2 !=
                                // trie_value` test will be false).
                                trie_value = trie_value_2; // may or may not help
                            }
                        } else {
                            trie_value = trie_value_2;
                            value = maybe_filter_value(
                                trie_value_2,
                                T::try_from_u32(self.header.null_value).ok()?,
                                null_value,
                            );
                            have_value = true;
                        }

                        c += 1;
                        while (c & data_mask) != 0 {
                            di += 1;
                            trie_value_2 = self.data.get(di as usize)?;
                            if trie_value_2 != trie_value {
                                if maybe_filter_value(
                                    trie_value_2,
                                    T::try_from_u32(self.header.null_value).ok()?,
                                    null_value,
                                ) != value
                                {
                                    return Some(CodePointMapRange {
                                        range: start..=(c - 1),
                                        value,
                                    });
                                }
                                // `trie_value` stores the previous value that was retrieved
                                // from the trie.
                                // `value` stores the value associated for the range (return
                                // value) that we are currently building, which is computed
                                // as a transformation by applying maybe_filter_value()
                                // to the trie value.
                                // The current trie value `trie_value_2` within this data block
                                // differs here from the previous value in `trie_value`.
                                // But both map to `value` after applying `maybe_filter_value`.
                                // It is not clear whether the previous or the current trie value
                                // (or neither) is more likely to match potential subsequent trie
                                // values that would extend the range by mapping to `value`.
                                // On the assumption of locality -- often times consecutive
                                // characters map to the same trie values -- remembering the new
                                // one might make it faster to extend this range further
                                // (by increasing the chance that the next `trie_value_2 !=
                                // trie_value` test will be false).
                                trie_value = trie_value_2; // may or may not help
                            }

                            c += 1;
                        }
                    }
                }

                i3 += 1;
                if i3 >= i3_block_length {
                    break;
                }
            }

            if c >= self.header.high_start {
                break;
            }
        }

        debug_assert!(have_value);

        // Now that c >= high_start, compare `value` to `high_value` to see
        // if we can merge our current range with the high_value range
        // high_start..=CODE_POINT_MAX (start- and end-inclusive), otherwise
        // stop at high_start - 1.
        let di: u32 = self.data.len() as u32 - HIGH_VALUE_NEG_DATA_OFFSET;
        let high_value: T = self.data.get(di as usize)?;
        if maybe_filter_value(
            high_value,
            T::try_from_u32(self.header.null_value).ok()?,
            null_value,
        ) != value
        {
            c -= 1;
        } else {
            c = CODE_POINT_MAX;
        }
        Some(CodePointMapRange {
            range: start..=c,
            value,
        })
    }

    /// Yields an [`Iterator`] returning ranges of consecutive code points that
    /// share the same value in the [`CodePointTrie`], as given by
    /// [`CodePointTrie::get_range()`].
    ///
    /// # Examples
    ///
    /// ```
    /// use core::ops::RangeInclusive;
    /// use icu::collections::codepointtrie::planes;
    /// use icu::collections::codepointtrie::CodePointMapRange;
    ///
    /// let planes_trie = planes::get_planes_trie();
    ///
    /// let mut ranges = planes_trie.iter_ranges();
    ///
    /// for plane in 0..=16 {
    ///     let exp_start = plane * 0x1_0000;
    ///     let exp_end = exp_start + 0xffff;
    ///     assert_eq!(
    ///         ranges.next(),
    ///         Some(CodePointMapRange {
    ///             range: exp_start..=exp_end,
    ///             value: plane as u8
    ///         })
    ///     );
    /// }
    ///
    /// // Hitting the end of the iterator returns `None`, as will subsequent
    /// // calls to .next().
    /// assert_eq!(ranges.next(), None);
    /// assert_eq!(ranges.next(), None);
    /// ```
    pub fn iter_ranges(&self) -> CodePointMapRangeIterator<'_, T> {
        let init_range = Some(CodePointMapRange {
            range: u32::MAX..=u32::MAX,
            value: self.error_value(),
        });
        CodePointMapRangeIterator::<T> {
            cpt: self,
            cpm_range: init_range,
        }
    }

    /// Yields an [`Iterator`] returning the ranges of the code points whose values
    /// match `value` in the [`CodePointTrie`].
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::collections::codepointtrie::planes;
    ///
    /// let trie = planes::get_planes_trie();
    ///
    /// let plane_val = 2;
    /// let mut sip_range_iter = trie.iter_ranges_for_value(plane_val as u8);
    ///
    /// let start = plane_val * 0x1_0000;
    /// let end = start + 0xffff;
    ///
    /// let sip_range = sip_range_iter.next()
    ///     .expect("Plane 2 (SIP) should exist in planes data");
    /// assert_eq!(start..=end, sip_range);
    ///
    /// assert!(sip_range_iter.next().is_none());
    pub fn iter_ranges_for_value(
        &self,
        value: T,
    ) -> impl Iterator<Item = RangeInclusive<u32>> + '_ {
        self.iter_ranges()
            .filter(move |cpm_range| cpm_range.value == value)
            .map(|cpm_range| cpm_range.range)
    }

    /// Yields an [`Iterator`] returning the ranges of the code points after passing
    /// the value through a mapping function.
    ///
    /// This is preferable to calling `.get_ranges().map()` since it will coalesce
    /// adjacent ranges into one.
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::collections::codepointtrie::planes;
    ///
    /// let trie = planes::get_planes_trie();
    ///
    /// let plane_val = 2;
    /// let mut sip_range_iter = trie.iter_ranges_mapped(|value| value != plane_val as u8).filter(|range| range.value);
    ///
    /// let end = plane_val * 0x1_0000 - 1;
    ///
    /// let sip_range = sip_range_iter.next()
    ///     .expect("Complemented planes data should have at least one entry");
    /// assert_eq!(0..=end, sip_range.range);
    pub fn iter_ranges_mapped<'a, U: Eq + 'a>(
        &'a self,
        mut map: impl FnMut(T) -> U + Copy + 'a,
    ) -> impl Iterator<Item = CodePointMapRange<U>> + 'a {
        crate::iterator_utils::RangeListIteratorCoalescer::new(self.iter_ranges().map(
            move |range| CodePointMapRange {
                range: range.range,
                value: map(range.value),
            },
        ))
    }

    /// Returns a [`CodePointInversionList`] for the code points that have the given
    /// [`TrieValue`] in the trie.
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::collections::codepointtrie::planes;
    ///
    /// let trie = planes::get_planes_trie();
    ///
    /// let plane_val = 2;
    /// let sip = trie.get_set_for_value(plane_val as u8);
    ///
    /// let start = plane_val * 0x1_0000;
    /// let end = start + 0xffff;
    ///
    /// assert!(!sip.contains32(start - 1));
    /// assert!(sip.contains32(start));
    /// assert!(sip.contains32(end));
    /// assert!(!sip.contains32(end + 1));
    /// ```
    #[cfg(feature = "alloc")]
    pub fn get_set_for_value(&self, value: T) -> CodePointInversionList<'static> {
        let value_ranges = self.iter_ranges_for_value(value);
        CodePointInversionList::from_iter(value_ranges)
    }

    /// Returns the value used as an error value for this trie
    #[inline]
    pub fn error_value(&self) -> T {
        self.error_value
    }
}

#[cfg(feature = "databake")]
impl<T: TrieValue + databake::Bake> databake::Bake for CodePointTrie<'_, T> {
    fn bake(&self, env: &databake::CrateEnv) -> databake::TokenStream {
        let header = self.header.bake(env);
        let index = self.index.bake(env);
        let data = self.data.bake(env);
        let error_value = self.error_value.bake(env);
        databake::quote! { unsafe { icu_collections::codepointtrie::CodePointTrie::from_parts_unstable_unchecked_v1(#header, #index, #data, #error_value) } }
    }
}

#[cfg(feature = "databake")]
impl<T: TrieValue + databake::Bake> databake::BakeSize for CodePointTrie<'_, T> {
    fn borrows_size(&self) -> usize {
        self.header.borrows_size() + self.index.borrows_size() + self.data.borrows_size()
    }
}

impl<T: TrieValue + Into<u32>> CodePointTrie<'_, T> {
    /// Returns the value that is associated with `code_point` for this [`CodePointTrie`]
    /// as a `u32`.
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::collections::codepointtrie::planes;
    /// let trie = planes::get_planes_trie();
    ///
    /// let cp = '𑖎' as u32;
    /// assert_eq!(cp, 0x1158E);
    ///
    /// let plane_num: u8 = trie.get32(cp);
    /// assert_eq!(trie.get32_u32(cp), plane_num as u32);
    /// ```
    // Note: This API method maintains consistency with the corresponding
    // original ICU APIs.
    pub fn get32_u32(&self, code_point: u32) -> u32 {
        self.get32(code_point).into()
    }
}

impl<T: TrieValue> Clone for CodePointTrie<'_, T>
where
    <T as zerovec::ule::AsULE>::ULE: Clone,
{
    fn clone(&self) -> Self {
        CodePointTrie {
            header: self.header,
            index: self.index.clone(),
            data: self.data.clone(),
            error_value: self.error_value,
        }
    }
}

/// Represents a range of consecutive code points sharing the same value in a
/// code point map.
///
/// The start and end of the interval is represented as a
/// `RangeInclusive<u32>`, and the value is represented as `T`.
#[derive(PartialEq, Eq, Debug, Clone)]
pub struct CodePointMapRange<T> {
    /// Range of code points from start to end (inclusive).
    pub range: RangeInclusive<u32>,
    /// Trie value associated with this range.
    pub value: T,
}

/// A custom [`Iterator`] type specifically for a code point trie that returns
/// [`CodePointMapRange`]s.
pub struct CodePointMapRangeIterator<'a, T: TrieValue> {
    cpt: &'a CodePointTrie<'a, T>,
    // Initialize `range` to Some(CodePointMapRange{ start: u32::MAX, end: u32::MAX, value: 0}).
    // When `range` is Some(...) and has a start value different from u32::MAX, then we have
    // returned at least one code point range due to a call to `next()`.
    // When `range` == `None`, it means that we have hit the end of iteration. It would occur
    // after a call to `next()` returns a None <=> we attempted to call `get_range()`
    // with a start code point that is > CODE_POINT_MAX.
    cpm_range: Option<CodePointMapRange<T>>,
}

impl<T: TrieValue> Iterator for CodePointMapRangeIterator<'_, T> {
    type Item = CodePointMapRange<T>;

    fn next(&mut self) -> Option<Self::Item> {
        self.cpm_range = match &self.cpm_range {
            Some(cpmr) => {
                if *cpmr.range.start() == u32::MAX {
                    self.cpt.get_range(0)
                } else {
                    self.cpt.get_range(cpmr.range.end() + 1)
                }
            }
            None => None,
        };
        // Note: Clone is cheap. We can't Copy because RangeInclusive does not impl Copy.
        self.cpm_range.clone()
    }
}

/// For sealing `TypedCodePointTrie`
///
/// # Safety Usable Invariant
///
/// All implementations of `TypedCodePointTrie` are reviewable in this module.
trait Seal {}

/// Trait for writing trait bounds for monomorphizing over either
/// `FastCodePointTrie` or `SmallCodePointTrie`.
#[allow(private_bounds)] // Permit sealing
pub trait TypedCodePointTrie<'trie, T: TrieValue>: Seal {
    /// The `TrieType` associated with this `TypedCodePointTrie`
    ///
    /// # Safety Usable Invariant
    ///
    /// This constant matches `self.as_untyped_ref().header.trie_type`.
    const TRIE_TYPE: TrieType;

    /// Lookup trie value as `u32` by Unicode Scalar Value without branching on trie type.
    #[inline(always)]
    fn get32_u32(&self, code_point: u32) -> u32 {
        self.get32(code_point).to_u32()
    }

    /// Lookup trie value by Basic Multilingual Plane Code Point without branching on trie type.
    #[inline(always)]
    fn get16(&self, bmp: u16) -> T {
        // LLVM's optimizations have been observed not to be 100%
        // reliable around collapsing away unnecessary parts of
        // `get32`, so not just calling `get32` here.
        let code_point = u32::from(bmp);
        if let Some(v) = self.get32_by_fast_index(code_point) {
            v
        } else {
            self.as_untyped_ref().get32_by_small_index_cold(code_point)
        }
    }

    /// Lookup trie value by non-Basic Multilingual Plane Scalar Value without branching on trie type.
    #[inline(always)]
    fn get32_supplementary(&self, supplementary: u32) -> T {
        self.as_untyped_ref().get32_supplementary(supplementary)
    }

    /// Lookup trie value by Unicode Scalar Value without branching on trie type.
    #[inline(always)]
    fn get(&self, c: char) -> T {
        // LLVM's optimizations have been observed not to be 100%
        // reliable around collapsing away unnecessary parts of
        // `get32`, so not just calling `get32` here.
        let code_point = u32::from(c);
        if let Some(v) = self.get32_by_fast_index(code_point) {
            v
        } else {
            self.as_untyped_ref().get32_by_small_index_cold(code_point)
        }
    }

    /// Lookup trie value by Unicode Code Point without branching on trie type.
    #[inline(always)]
    fn get32(&self, code_point: u32) -> T {
        if let Some(v) = self.get32_by_fast_index(code_point) {
            v
        } else if code_point <= CODE_POINT_MAX {
            self.as_untyped_ref().get32_by_small_index_cold(code_point)
        } else {
            self.as_untyped_ref().error_value
        }
    }

    /// Returns the value that is associated with `code_point` in this [`CodePointTrie`]
    /// if `code_point` uses fast-path lookup or `None` if `code_point`
    /// should use small-path lookup or is above the supported range.
    #[inline(always)] // "always" to make the `Option` collapse away
    fn get32_by_fast_index(&self, code_point: u32) -> Option<T> {
        debug_assert_eq!(Self::TRIE_TYPE, self.as_untyped_ref().header.trie_type);
        let fast_max = match Self::TRIE_TYPE {
            TrieType::Fast => FAST_TYPE_FAST_INDEXING_MAX,
            TrieType::Small => SMALL_TYPE_FAST_INDEXING_MAX,
        };
        if code_point <= fast_max {
            // SAFETY: We just checked the invariant of
            // `get32_assuming_fast_index`,
            // which is
            // "If `self.header.trie_type == TrieType::Small`, `code_point` must be at most
            // `SMALL_TYPE_FAST_INDEXING_MAX`. If `self.header.trie_type ==
            // TrieType::Fast`, `code_point` must be at most `FAST_TYPE_FAST_INDEXING_MAX`."
            // ... assuming that `Self::TRIE_TYPE` always matches
            // `self.as_untyped_ref().header.trie_type`, i.e. we're relying on
            // `CodePointTrie::to_typed` and `CodePointTrie::as_typed_ref` being correct
            // and the exclusive ways of obtaining `Self`.
            Some(unsafe { self.as_untyped_ref().get32_assuming_fast_index(code_point) })
        } else {
            // The caller needs to call `get32_by_small_index` or determine
            // that the argument is above the permitted range.
            None
        }
    }

    /// Returns a reference to the wrapped `CodePointTrie`.
    fn as_untyped_ref(&self) -> &CodePointTrie<'trie, T>;

    /// Extracts the wrapped `CodePointTrie`.
    fn to_untyped(self) -> CodePointTrie<'trie, T>;
}

/// Type-safe wrapper for a fast trie guaranteeing
/// the the getters don't branch on the trie type
/// and for guarenteeing that `get16` is branchless
/// in release builds.
#[derive(Debug)]
#[repr(transparent)]
pub struct FastCodePointTrie<'trie, T: TrieValue> {
    inner: CodePointTrie<'trie, T>,
}

impl<'trie, T: TrieValue> TypedCodePointTrie<'trie, T> for FastCodePointTrie<'trie, T> {
    const TRIE_TYPE: TrieType = TrieType::Fast;

    /// Returns a reference to the wrapped `CodePointTrie`.
    #[inline(always)]
    fn as_untyped_ref(&self) -> &CodePointTrie<'trie, T> {
        &self.inner
    }

    /// Extracts the wrapped `CodePointTrie`.
    #[inline(always)]
    fn to_untyped(self) -> CodePointTrie<'trie, T> {
        self.inner
    }

    /// Lookup trie value by Basic Multilingual Plane Code Point without branching on trie type.
    #[inline(always)]
    fn get16(&self, bmp: u16) -> T {
        debug_assert!(u32::from(u16::MAX) <= FAST_TYPE_FAST_INDEXING_MAX);
        debug_assert_eq!(Self::TRIE_TYPE, TrieType::Fast);
        debug_assert_eq!(self.as_untyped_ref().header.trie_type, TrieType::Fast);
        let code_point = u32::from(bmp);
        // SAFETY: With `TrieType::Fast`, the `u16` range satisfies
        // the invariant of `get32_assuming_fast_index`, which is
        // "If `self.header.trie_type == TrieType::Small`, `code_point` must be at most
        // `SMALL_TYPE_FAST_INDEXING_MAX`. If `self.header.trie_type ==
        // TrieType::Fast`, `code_point` must be at most `FAST_TYPE_FAST_INDEXING_MAX`."
        //
        // We're relying on `CodePointTrie::to_typed` and `CodePointTrie::as_typed_ref`
        // being correct and the exclusive ways of obtaining `Self`.
        unsafe { self.as_untyped_ref().get32_assuming_fast_index(code_point) }
    }
}

impl<'trie, T: TrieValue> Seal for FastCodePointTrie<'trie, T> {}

impl<'trie, T: TrieValue> TryFrom<&'trie CodePointTrie<'trie, T>>
    for &'trie FastCodePointTrie<'trie, T>
{
    type Error = TypedCodePointTrieError;

    fn try_from(
        reference: &'trie CodePointTrie<'trie, T>,
    ) -> Result<&'trie FastCodePointTrie<'trie, T>, TypedCodePointTrieError> {
        match reference.as_typed_ref() {
            Typed::Fast(trie) => Ok(trie),
            Typed::Small(_) => Err(TypedCodePointTrieError),
        }
    }
}

impl<'trie, T: TrieValue> TryFrom<CodePointTrie<'trie, T>> for FastCodePointTrie<'trie, T> {
    type Error = TypedCodePointTrieError;

    fn try_from(
        value: CodePointTrie<'trie, T>,
    ) -> Result<FastCodePointTrie<'trie, T>, TypedCodePointTrieError> {
        match value.to_typed() {
            Typed::Fast(trie) => Ok(trie),
            Typed::Small(_) => Err(TypedCodePointTrieError),
        }
    }
}

/// Type-safe wrapper for a small trie guaranteeing
/// the the getters don't branch on the trie type.
#[derive(Debug)]
#[repr(transparent)]
pub struct SmallCodePointTrie<'trie, T: TrieValue> {
    inner: CodePointTrie<'trie, T>,
}

impl<'trie, T: TrieValue> TypedCodePointTrie<'trie, T> for SmallCodePointTrie<'trie, T> {
    const TRIE_TYPE: TrieType = TrieType::Small;

    /// Returns a reference to the wrapped `CodePointTrie`.
    #[inline(always)]
    fn as_untyped_ref(&self) -> &CodePointTrie<'trie, T> {
        &self.inner
    }

    /// Extracts the wrapped `CodePointTrie`.
    #[inline(always)]
    fn to_untyped(self) -> CodePointTrie<'trie, T> {
        self.inner
    }
}

impl<'trie, T: TrieValue> Seal for SmallCodePointTrie<'trie, T> {}

impl<'trie, T: TrieValue> TryFrom<&'trie CodePointTrie<'trie, T>>
    for &'trie SmallCodePointTrie<'trie, T>
{
    type Error = TypedCodePointTrieError;

    fn try_from(
        reference: &'trie CodePointTrie<'trie, T>,
    ) -> Result<&'trie SmallCodePointTrie<'trie, T>, TypedCodePointTrieError> {
        match reference.as_typed_ref() {
            Typed::Fast(_) => Err(TypedCodePointTrieError),
            Typed::Small(trie) => Ok(trie),
        }
    }
}

impl<'trie, T: TrieValue> TryFrom<CodePointTrie<'trie, T>> for SmallCodePointTrie<'trie, T> {
    type Error = TypedCodePointTrieError;

    fn try_from(
        value: CodePointTrie<'trie, T>,
    ) -> Result<SmallCodePointTrie<'trie, T>, TypedCodePointTrieError> {
        match value.to_typed() {
            Typed::Fast(_) => Err(TypedCodePointTrieError),
            Typed::Small(trie) => Ok(trie),
        }
    }
}

/// Error indicating that the `TrieType` of an untyped trie
/// does not match the requested typed trie type.
#[derive(Debug)]
#[non_exhaustive]
pub struct TypedCodePointTrieError;

/// Holder for either fast or small trie with the trie
/// type encoded into the Rust type.
pub enum Typed<F, S> {
    /// The trie type is fast.
    Fast(F),
    /// The trie type is small.
    Small(S),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codepointtrie::planes;
    use alloc::vec::Vec;

    #[test]
    #[cfg(feature = "serde")]
    fn test_serde_with_postcard_roundtrip() -> Result<(), postcard::Error> {
        let trie = crate::codepointtrie::planes::get_planes_trie();
        let trie_serialized: Vec<u8> = postcard::to_allocvec(&trie).unwrap();

        // Assert an expected (golden data) version of the serialized trie.
        const EXP_TRIE_SERIALIZED: &[u8] = &[
            128, 128, 64, 128, 2, 2, 0, 0, 1, 160, 18, 0, 0, 64, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 136,
            2, 144, 2, 144, 2, 144, 2, 176, 2, 176, 2, 176, 2, 176, 2, 208, 2, 208, 2, 208, 2, 208,
            2, 240, 2, 240, 2, 240, 2, 240, 2, 16, 3, 16, 3, 16, 3, 16, 3, 48, 3, 48, 3, 48, 3, 48,
            3, 80, 3, 80, 3, 80, 3, 80, 3, 112, 3, 112, 3, 112, 3, 112, 3, 144, 3, 144, 3, 144, 3,
            144, 3, 176, 3, 176, 3, 176, 3, 176, 3, 208, 3, 208, 3, 208, 3, 208, 3, 240, 3, 240, 3,
            240, 3, 240, 3, 16, 4, 16, 4, 16, 4, 16, 4, 48, 4, 48, 4, 48, 4, 48, 4, 80, 4, 80, 4,
            80, 4, 80, 4, 112, 4, 112, 4, 112, 4, 112, 4, 0, 0, 16, 0, 32, 0, 48, 0, 64, 0, 80, 0,
            96, 0, 112, 0, 0, 0, 16, 0, 32, 0, 48, 0, 0, 0, 16, 0, 32, 0, 48, 0, 0, 0, 16, 0, 32,
            0, 48, 0, 0, 0, 16, 0, 32, 0, 48, 0, 0, 0, 16, 0, 32, 0, 48, 0, 0, 0, 16, 0, 32, 0, 48,
            0, 0, 0, 16, 0, 32, 0, 48, 0, 0, 0, 16, 0, 32, 0, 48, 0, 128, 0, 128, 0, 128, 0, 128,
            0, 128, 0, 128, 0, 128, 0, 128, 0, 128, 0, 128, 0, 128, 0, 128, 0, 128, 0, 128, 0, 128,
            0, 128, 0, 128, 0, 128, 0, 128, 0, 128, 0, 128, 0, 128, 0, 128, 0, 128, 0, 128, 0, 128,
            0, 128, 0, 128, 0, 128, 0, 128, 0, 128, 0, 128, 0, 144, 0, 144, 0, 144, 0, 144, 0, 144,
            0, 144, 0, 144, 0, 144, 0, 144, 0, 144, 0, 144, 0, 144, 0, 144, 0, 144, 0, 144, 0, 144,
            0, 144, 0, 144, 0, 144, 0, 144, 0, 144, 0, 144, 0, 144, 0, 144, 0, 144, 0, 144, 0, 144,
            0, 144, 0, 144, 0, 144, 0, 144, 0, 144, 0, 160, 0, 160, 0, 160, 0, 160, 0, 160, 0, 160,
            0, 160, 0, 160, 0, 160, 0, 160, 0, 160, 0, 160, 0, 160, 0, 160, 0, 160, 0, 160, 0, 160,
            0, 160, 0, 160, 0, 160, 0, 160, 0, 160, 0, 160, 0, 160, 0, 160, 0, 160, 0, 160, 0, 160,
            0, 160, 0, 160, 0, 160, 0, 160, 0, 176, 0, 176, 0, 176, 0, 176, 0, 176, 0, 176, 0, 176,
            0, 176, 0, 176, 0, 176, 0, 176, 0, 176, 0, 176, 0, 176, 0, 176, 0, 176, 0, 176, 0, 176,
            0, 176, 0, 176, 0, 176, 0, 176, 0, 176, 0, 176, 0, 176, 0, 176, 0, 176, 0, 176, 0, 176,
            0, 176, 0, 176, 0, 176, 0, 192, 0, 192, 0, 192, 0, 192, 0, 192, 0, 192, 0, 192, 0, 192,
            0, 192, 0, 192, 0, 192, 0, 192, 0, 192, 0, 192, 0, 192, 0, 192, 0, 192, 0, 192, 0, 192,
            0, 192, 0, 192, 0, 192, 0, 192, 0, 192, 0, 192, 0, 192, 0, 192, 0, 192, 0, 192, 0, 192,
            0, 192, 0, 192, 0, 208, 0, 208, 0, 208, 0, 208, 0, 208, 0, 208, 0, 208, 0, 208, 0, 208,
            0, 208, 0, 208, 0, 208, 0, 208, 0, 208, 0, 208, 0, 208, 0, 208, 0, 208, 0, 208, 0, 208,
            0, 208, 0, 208, 0, 208, 0, 208, 0, 208, 0, 208, 0, 208, 0, 208, 0, 208, 0, 208, 0, 208,
            0, 208, 0, 224, 0, 224, 0, 224, 0, 224, 0, 224, 0, 224, 0, 224, 0, 224, 0, 224, 0, 224,
            0, 224, 0, 224, 0, 224, 0, 224, 0, 224, 0, 224, 0, 224, 0, 224, 0, 224, 0, 224, 0, 224,
            0, 224, 0, 224, 0, 224, 0, 224, 0, 224, 0, 224, 0, 224, 0, 224, 0, 224, 0, 224, 0, 224,
            0, 240, 0, 240, 0, 240, 0, 240, 0, 240, 0, 240, 0, 240, 0, 240, 0, 240, 0, 240, 0, 240,
            0, 240, 0, 240, 0, 240, 0, 240, 0, 240, 0, 240, 0, 240, 0, 240, 0, 240, 0, 240, 0, 240,
            0, 240, 0, 240, 0, 240, 0, 240, 0, 240, 0, 240, 0, 240, 0, 240, 0, 240, 0, 240, 0, 0,
            1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1,
            0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0,
            1, 0, 1, 0, 1, 16, 1, 16, 1, 16, 1, 16, 1, 16, 1, 16, 1, 16, 1, 16, 1, 16, 1, 16, 1,
            16, 1, 16, 1, 16, 1, 16, 1, 16, 1, 16, 1, 16, 1, 16, 1, 16, 1, 16, 1, 16, 1, 16, 1, 16,
            1, 16, 1, 16, 1, 16, 1, 16, 1, 16, 1, 16, 1, 16, 1, 16, 1, 16, 1, 32, 1, 32, 1, 32, 1,
            32, 1, 32, 1, 32, 1, 32, 1, 32, 1, 32, 1, 32, 1, 32, 1, 32, 1, 32, 1, 32, 1, 32, 1, 32,
            1, 32, 1, 32, 1, 32, 1, 32, 1, 32, 1, 32, 1, 32, 1, 32, 1, 32, 1, 32, 1, 32, 1, 32, 1,
            32, 1, 32, 1, 32, 1, 32, 1, 48, 1, 48, 1, 48, 1, 48, 1, 48, 1, 48, 1, 48, 1, 48, 1, 48,
            1, 48, 1, 48, 1, 48, 1, 48, 1, 48, 1, 48, 1, 48, 1, 48, 1, 48, 1, 48, 1, 48, 1, 48, 1,
            48, 1, 48, 1, 48, 1, 48, 1, 48, 1, 48, 1, 48, 1, 48, 1, 48, 1, 48, 1, 48, 1, 64, 1, 64,
            1, 64, 1, 64, 1, 64, 1, 64, 1, 64, 1, 64, 1, 64, 1, 64, 1, 64, 1, 64, 1, 64, 1, 64, 1,
            64, 1, 64, 1, 64, 1, 64, 1, 64, 1, 64, 1, 64, 1, 64, 1, 64, 1, 64, 1, 64, 1, 64, 1, 64,
            1, 64, 1, 64, 1, 64, 1, 64, 1, 64, 1, 80, 1, 80, 1, 80, 1, 80, 1, 80, 1, 80, 1, 80, 1,
            80, 1, 80, 1, 80, 1, 80, 1, 80, 1, 80, 1, 80, 1, 80, 1, 80, 1, 80, 1, 80, 1, 80, 1, 80,
            1, 80, 1, 80, 1, 80, 1, 80, 1, 80, 1, 80, 1, 80, 1, 80, 1, 80, 1, 80, 1, 80, 1, 80, 1,
            96, 1, 96, 1, 96, 1, 96, 1, 96, 1, 96, 1, 96, 1, 96, 1, 96, 1, 96, 1, 96, 1, 96, 1, 96,
            1, 96, 1, 96, 1, 96, 1, 96, 1, 96, 1, 96, 1, 96, 1, 96, 1, 96, 1, 96, 1, 96, 1, 96, 1,
            96, 1, 96, 1, 96, 1, 96, 1, 96, 1, 96, 1, 96, 1, 128, 0, 136, 0, 136, 0, 136, 0, 136,
            0, 136, 0, 136, 0, 136, 0, 2, 0, 2, 0, 2, 0, 2, 0, 2, 0, 2, 0, 2, 0, 2, 0, 2, 0, 2, 0,
            2, 0, 2, 0, 2, 0, 2, 0, 2, 0, 2, 0, 2, 0, 2, 0, 2, 0, 2, 0, 2, 0, 2, 0, 2, 0, 2, 0, 2,
            0, 2, 0, 2, 0, 2, 0, 2, 0, 2, 0, 2, 0, 2, 0, 168, 0, 168, 0, 168, 0, 168, 0, 168, 0,
            168, 0, 168, 0, 168, 0, 168, 0, 168, 0, 168, 0, 168, 0, 168, 0, 168, 0, 168, 0, 168, 0,
            168, 0, 168, 0, 168, 0, 168, 0, 168, 0, 168, 0, 168, 0, 168, 0, 168, 0, 168, 0, 168, 0,
            168, 0, 168, 0, 168, 0, 168, 0, 168, 0, 200, 0, 200, 0, 200, 0, 200, 0, 200, 0, 200, 0,
            200, 0, 200, 0, 200, 0, 200, 0, 200, 0, 200, 0, 200, 0, 200, 0, 200, 0, 200, 0, 200, 0,
            200, 0, 200, 0, 200, 0, 200, 0, 200, 0, 200, 0, 200, 0, 200, 0, 200, 0, 200, 0, 200, 0,
            200, 0, 200, 0, 200, 0, 200, 0, 232, 0, 232, 0, 232, 0, 232, 0, 232, 0, 232, 0, 232, 0,
            232, 0, 232, 0, 232, 0, 232, 0, 232, 0, 232, 0, 232, 0, 232, 0, 232, 0, 232, 0, 232, 0,
            232, 0, 232, 0, 232, 0, 232, 0, 232, 0, 232, 0, 232, 0, 232, 0, 232, 0, 232, 0, 232, 0,
            232, 0, 232, 0, 232, 0, 8, 1, 8, 1, 8, 1, 8, 1, 8, 1, 8, 1, 8, 1, 8, 1, 8, 1, 8, 1, 8,
            1, 8, 1, 8, 1, 8, 1, 8, 1, 8, 1, 8, 1, 8, 1, 8, 1, 8, 1, 8, 1, 8, 1, 8, 1, 8, 1, 8, 1,
            8, 1, 8, 1, 8, 1, 8, 1, 8, 1, 8, 1, 8, 1, 40, 1, 40, 1, 40, 1, 40, 1, 40, 1, 40, 1, 40,
            1, 40, 1, 40, 1, 40, 1, 40, 1, 40, 1, 40, 1, 40, 1, 40, 1, 40, 1, 40, 1, 40, 1, 40, 1,
            40, 1, 40, 1, 40, 1, 40, 1, 40, 1, 40, 1, 40, 1, 40, 1, 40, 1, 40, 1, 40, 1, 40, 1, 40,
            1, 72, 1, 72, 1, 72, 1, 72, 1, 72, 1, 72, 1, 72, 1, 72, 1, 72, 1, 72, 1, 72, 1, 72, 1,
            72, 1, 72, 1, 72, 1, 72, 1, 72, 1, 72, 1, 72, 1, 72, 1, 72, 1, 72, 1, 72, 1, 72, 1, 72,
            1, 72, 1, 72, 1, 72, 1, 72, 1, 72, 1, 72, 1, 72, 1, 104, 1, 104, 1, 104, 1, 104, 1,
            104, 1, 104, 1, 104, 1, 104, 1, 104, 1, 104, 1, 104, 1, 104, 1, 104, 1, 104, 1, 104, 1,
            104, 1, 104, 1, 104, 1, 104, 1, 104, 1, 104, 1, 104, 1, 104, 1, 104, 1, 104, 1, 104, 1,
            104, 1, 104, 1, 104, 1, 104, 1, 104, 1, 104, 1, 136, 1, 136, 1, 136, 1, 136, 1, 136, 1,
            136, 1, 136, 1, 136, 1, 136, 1, 136, 1, 136, 1, 136, 1, 136, 1, 136, 1, 136, 1, 136, 1,
            136, 1, 136, 1, 136, 1, 136, 1, 136, 1, 136, 1, 136, 1, 136, 1, 136, 1, 136, 1, 136, 1,
            136, 1, 136, 1, 136, 1, 136, 1, 136, 1, 168, 1, 168, 1, 168, 1, 168, 1, 168, 1, 168, 1,
            168, 1, 168, 1, 168, 1, 168, 1, 168, 1, 168, 1, 168, 1, 168, 1, 168, 1, 168, 1, 168, 1,
            168, 1, 168, 1, 168, 1, 168, 1, 168, 1, 168, 1, 168, 1, 168, 1, 168, 1, 168, 1, 168, 1,
            168, 1, 168, 1, 168, 1, 168, 1, 200, 1, 200, 1, 200, 1, 200, 1, 200, 1, 200, 1, 200, 1,
            200, 1, 200, 1, 200, 1, 200, 1, 200, 1, 200, 1, 200, 1, 200, 1, 200, 1, 200, 1, 200, 1,
            200, 1, 200, 1, 200, 1, 200, 1, 200, 1, 200, 1, 200, 1, 200, 1, 200, 1, 200, 1, 200, 1,
            200, 1, 200, 1, 200, 1, 232, 1, 232, 1, 232, 1, 232, 1, 232, 1, 232, 1, 232, 1, 232, 1,
            232, 1, 232, 1, 232, 1, 232, 1, 232, 1, 232, 1, 232, 1, 232, 1, 232, 1, 232, 1, 232, 1,
            232, 1, 232, 1, 232, 1, 232, 1, 232, 1, 232, 1, 232, 1, 232, 1, 232, 1, 232, 1, 232, 1,
            232, 1, 232, 1, 8, 2, 8, 2, 8, 2, 8, 2, 8, 2, 8, 2, 8, 2, 8, 2, 8, 2, 8, 2, 8, 2, 8, 2,
            8, 2, 8, 2, 8, 2, 8, 2, 8, 2, 8, 2, 8, 2, 8, 2, 8, 2, 8, 2, 8, 2, 8, 2, 8, 2, 8, 2, 8,
            2, 8, 2, 8, 2, 8, 2, 8, 2, 8, 2, 40, 2, 40, 2, 40, 2, 40, 2, 40, 2, 40, 2, 40, 2, 40,
            2, 40, 2, 40, 2, 40, 2, 40, 2, 40, 2, 40, 2, 40, 2, 40, 2, 40, 2, 40, 2, 40, 2, 40, 2,
            40, 2, 40, 2, 40, 2, 40, 2, 40, 2, 40, 2, 40, 2, 40, 2, 40, 2, 40, 2, 40, 2, 40, 2, 72,
            2, 72, 2, 72, 2, 72, 2, 72, 2, 72, 2, 72, 2, 72, 2, 72, 2, 72, 2, 72, 2, 72, 2, 72, 2,
            72, 2, 72, 2, 72, 2, 72, 2, 72, 2, 72, 2, 72, 2, 72, 2, 72, 2, 72, 2, 72, 2, 72, 2, 72,
            2, 72, 2, 72, 2, 72, 2, 72, 2, 72, 2, 72, 2, 104, 2, 104, 2, 104, 2, 104, 2, 104, 2,
            104, 2, 104, 2, 104, 2, 104, 2, 104, 2, 104, 2, 104, 2, 104, 2, 104, 2, 104, 2, 104, 2,
            104, 2, 104, 2, 104, 2, 104, 2, 104, 2, 104, 2, 104, 2, 104, 2, 104, 2, 104, 2, 104, 2,
            104, 2, 104, 2, 104, 2, 104, 2, 104, 2, 244, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
            1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
            2, 2, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
            4, 4, 4, 4, 4, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 6, 6, 6, 6, 6, 6, 6, 6,
            6, 6, 6, 6, 6, 6, 6, 6, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 8, 8, 8, 8, 8,
            8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 9, 10,
            10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10, 11, 11, 11, 11, 11, 11, 11,
            11, 11, 11, 11, 11, 11, 11, 11, 11, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12,
            12, 12, 12, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 13, 14, 14, 14,
            14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 14, 15, 15, 15, 15, 15, 15, 15, 15, 15,
            15, 15, 15, 15, 15, 15, 15, 16, 16, 16, 0,
        ];
        assert_eq!(trie_serialized, EXP_TRIE_SERIALIZED);

        let trie_deserialized = postcard::from_bytes::<CodePointTrie<u8>>(&trie_serialized)?;

        assert_eq!(&trie.index, &trie_deserialized.index);
        assert_eq!(&trie.data, &trie_deserialized.data);

        assert!(!trie_deserialized.index.is_owned());
        assert!(!trie_deserialized.data.is_owned());

        Ok(())
    }

    #[test]
    fn test_typed() {
        let untyped = planes::get_planes_trie();
        assert_eq!(untyped.get('\u{10000}'), 1);
        let small_ref = <&SmallCodePointTrie<_>>::try_from(&untyped).unwrap();
        assert_eq!(small_ref.get('\u{10000}'), 1);
        let _ = <&FastCodePointTrie<_>>::try_from(&untyped).is_err();
        let small = <SmallCodePointTrie<_>>::try_from(untyped).unwrap();
        assert_eq!(small.get('\u{10000}'), 1);
    }

    #[test]
    fn test_get_range() {
        let planes_trie = planes::get_planes_trie();

        let first_range: Option<CodePointMapRange<u8>> = planes_trie.get_range(0x0);
        assert_eq!(
            first_range,
            Some(CodePointMapRange {
                range: 0x0..=0xffff,
                value: 0
            })
        );

        let second_range: Option<CodePointMapRange<u8>> = planes_trie.get_range(0x1_0000);
        assert_eq!(
            second_range,
            Some(CodePointMapRange {
                range: 0x10000..=0x1ffff,
                value: 1
            })
        );

        let penultimate_range: Option<CodePointMapRange<u8>> = planes_trie.get_range(0xf_0000);
        assert_eq!(
            penultimate_range,
            Some(CodePointMapRange {
                range: 0xf_0000..=0xf_ffff,
                value: 15
            })
        );

        let last_range: Option<CodePointMapRange<u8>> = planes_trie.get_range(0x10_0000);
        assert_eq!(
            last_range,
            Some(CodePointMapRange {
                range: 0x10_0000..=0x10_ffff,
                value: 16
            })
        );
    }

    #[test]
    #[allow(unused_unsafe)] // `unsafe` below is both necessary and unnecessary
    fn databake() {
        databake::test_bake!(
            CodePointTrie<'static, u32>,
            const,
            unsafe {
                crate::codepointtrie::CodePointTrie::from_parts_unstable_unchecked_v1(
                    crate::codepointtrie::CodePointTrieHeader {
                        high_start: 1u32,
                        shifted12_high_start: 2u16,
                        index3_null_offset: 3u16,
                        data_null_offset: 4u32,
                        null_value: 5u32,
                        trie_type: crate::codepointtrie::TrieType::Small,
                    },
                    zerovec::ZeroVec::new(),
                    zerovec::ZeroVec::new(),
                    0u32,
                )
            },
            icu_collections,
            [zerovec],
        );
    }
}
