// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

#[cfg(feature = "alloc")]
use crate::code_point_set::CodePointSetData;
use crate::props::GeneralCategory;
use crate::props::GeneralCategoryGroup;
use crate::provider::*;
use core::ops::RangeInclusive;
use icu_collections::codepointtrie::{CodePointMapRange, CodePointTrie, TrieValue};
use icu_provider::marker::ErasedMarker;
use icu_provider::prelude::*;

/// A wrapper around code point map data.
///
/// It is returned by APIs that return Unicode
/// property data in a map-like form, ex: enumerated property value data keyed
/// by code point. Access its data via the borrowed version,
/// [`CodePointMapDataBorrowed`].
#[derive(Debug, Clone)]
pub struct CodePointMapData<T: TrieValue> {
    data: DataPayload<ErasedMarker<PropertyCodePointMap<'static, T>>>,
}

impl<T: TrieValue> CodePointMapData<T> {
    /// Creates a new [`CodePointMapData`] for a [`EnumeratedProperty`].
    ///
    /// See the documentation on [`EnumeratedProperty`] implementations for details.
    ///
    /// ✨ *Enabled with the `compiled_data` Cargo feature.*
    ///
    /// [📚 Help choosing a constructor](icu_provider::constructors)
    #[cfg(feature = "compiled_data")]
    #[expect(clippy::new_ret_no_self)]
    pub const fn new() -> CodePointMapDataBorrowed<'static, T>
    where
        T: EnumeratedProperty,
    {
        CodePointMapDataBorrowed::new()
    }

    #[doc = icu_provider::gen_buffer_unstable_docs!(UNSTABLE, Self::new)]
    pub fn try_new_unstable(
        provider: &(impl DataProvider<T::DataMarker> + ?Sized),
    ) -> Result<Self, DataError>
    where
        T: EnumeratedProperty,
    {
        Ok(Self {
            data: provider.load(Default::default())?.payload.cast(),
        })
    }

    /// Construct a borrowed version of this type that can be queried.
    ///
    /// This avoids a potential small underlying cost per API call (like `get()`) by consolidating it
    /// up front.
    ///
    /// This owned version if returned by functions that use a runtime data provider.
    #[inline]
    pub fn as_borrowed(&self) -> CodePointMapDataBorrowed<'_, T> {
        CodePointMapDataBorrowed {
            map: self.data.get(),
        }
    }

    /// Convert this map to a map around another type
    ///
    /// Typically useful for type-erasing maps into maps around integers.
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    ///
    /// # Panics
    /// Will panic if T and P are different sizes
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointMapData;
    /// use icu::properties::props::GeneralCategory;
    ///
    /// let data = CodePointMapData::<GeneralCategory>::new().static_to_owned();
    ///
    /// let gc = data.try_into_converted::<u8>().unwrap();
    /// let gc = gc.as_borrowed();
    ///
    /// assert_eq!(gc.get('木'), GeneralCategory::OtherLetter as u8);  // U+6728
    /// assert_eq!(gc.get('🎃'), GeneralCategory::OtherSymbol as u8);  // U+1F383 JACK-O-LANTERN
    /// ```
    #[cfg(feature = "alloc")]
    pub fn try_into_converted<P>(self) -> Result<CodePointMapData<P>, zerovec::ule::UleError>
    where
        P: TrieValue,
    {
        self.data
            .try_map_project(|data, _| data.try_into_converted())
            .map(CodePointMapData::from_data::<ErasedMarker<PropertyCodePointMap<'static, P>>>)
    }

    /// Construct a new one from loaded data
    ///
    /// Typically it is preferable to use getters like [`load_general_category()`] instead
    pub(crate) fn from_data<M>(data: DataPayload<M>) -> Self
    where
        M: DynamicDataMarker<DataStruct = PropertyCodePointMap<'static, T>>,
    {
        Self { data: data.cast() }
    }

    /// Construct a new one an owned [`CodePointTrie`]
    pub fn from_code_point_trie(trie: CodePointTrie<'static, T>) -> Self {
        let set = PropertyCodePointMap::from_code_point_trie(trie);
        CodePointMapData::from_data(
            DataPayload::<ErasedMarker<PropertyCodePointMap<'static, T>>>::from_owned(set),
        )
    }

    /// Convert this type to a [`CodePointTrie`] as a borrowed value.
    ///
    /// The data backing this is extensible and supports multiple implementations.
    /// Currently it is always [`CodePointTrie`]; however in the future more backends may be
    /// added, and users may select which at data generation time.
    ///
    /// This method returns an `Option` in order to return `None` when the backing data provider
    /// cannot return a [`CodePointTrie`], or cannot do so within the expected constant time
    /// constraint.
    pub fn as_code_point_trie(&self) -> Option<&CodePointTrie<'_, T>> {
        self.data.get().as_code_point_trie()
    }

    /// Convert this type to a [`CodePointTrie`], borrowing if possible,
    /// otherwise allocating a new [`CodePointTrie`].
    ///
    /// The data backing this is extensible and supports multiple implementations.
    /// Currently it is always [`CodePointTrie`]; however in the future more backends may be
    /// added, and users may select which at data generation time.
    ///
    /// The performance of the conversion to this specific return type will vary
    /// depending on the data structure that is backing `self`.
    pub fn to_code_point_trie(&self) -> CodePointTrie<'_, T> {
        self.data.get().to_code_point_trie()
    }
}

/// A borrowed wrapper around code point set data, returned by
/// [`CodePointSetData::as_borrowed()`]. More efficient to query.
#[derive(Clone, Copy, Debug)]
pub struct CodePointMapDataBorrowed<'a, T: TrieValue> {
    map: &'a PropertyCodePointMap<'a, T>,
}

impl<'a, T: TrieValue> CodePointMapDataBorrowed<'a, T> {
    /// Get the value this map has associated with code point `ch`
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::CodePointMapData;
    /// use icu::properties::props::GeneralCategory;
    ///
    /// let gc = CodePointMapData::<GeneralCategory>::new();
    ///
    /// assert_eq!(gc.get('木'), GeneralCategory::OtherLetter);  // U+6728
    /// assert_eq!(gc.get('🎃'), GeneralCategory::OtherSymbol);  // U+1F383 JACK-O-LANTERN
    /// ```
    #[inline]
    pub fn get(self, ch: char) -> T {
        self.map.get(ch)
    }

    /// See [`Self::get`].
    #[inline]
    pub fn get32(self, ch: u32) -> T {
        self.map.get32(ch)
    }

    /// Get a [`CodePointSetData`] for all elements corresponding to a particular value
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::props::GeneralCategory;
    /// use icu::properties::CodePointMapData;
    ///
    /// let gc = CodePointMapData::<GeneralCategory>::new();
    ///
    /// let other_letter_set_data =
    ///     gc.get_set_for_value(GeneralCategory::OtherLetter);
    /// let other_letter_set = other_letter_set_data.as_borrowed();
    ///
    /// assert!(other_letter_set.contains('木')); // U+6728
    /// assert!(!other_letter_set.contains('🎃')); // U+1F383 JACK-O-LANTERN
    /// ```
    #[cfg(feature = "alloc")]
    pub fn get_set_for_value(self, value: T) -> CodePointSetData {
        let set = self.map.get_set_for_value(value);
        CodePointSetData::from_code_point_inversion_list(set)
    }

    /// Yields an [`Iterator`] returning ranges of consecutive code points that
    /// share the same value in the [`CodePointMapData`].
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::properties::props::GeneralCategory;
    /// use icu::properties::CodePointMapData;
    ///
    /// let gc = CodePointMapData::<GeneralCategory>::new();
    /// let mut ranges = gc.iter_ranges();
    /// let next = ranges.next().unwrap();
    /// assert_eq!(next.range, 0..=31);
    /// assert_eq!(next.value, GeneralCategory::Control);
    /// let next = ranges.next().unwrap();
    /// assert_eq!(next.range, 32..=32);
    /// assert_eq!(next.value, GeneralCategory::SpaceSeparator);
    /// ```
    pub fn iter_ranges(self) -> impl Iterator<Item = CodePointMapRange<T>> + 'a {
        self.map.iter_ranges()
    }

    /// Yields an [`Iterator`] returning ranges of consecutive code points that
    /// share the same value `v` in the [`CodePointMapData`].
    ///
    /// # Examples
    ///
    ///
    /// ```
    /// use icu::properties::props::GeneralCategory;
    /// use icu::properties::CodePointMapData;
    ///
    /// let gc = CodePointMapData::<GeneralCategory>::new();
    /// let mut ranges = gc.iter_ranges_for_value(GeneralCategory::UppercaseLetter);
    /// assert_eq!(ranges.next().unwrap(), 'A' as u32..='Z' as u32);
    /// assert_eq!(ranges.next().unwrap(), 'À' as u32..='Ö' as u32);
    /// assert_eq!(ranges.next().unwrap(), 'Ø' as u32..='Þ' as u32);
    /// ```
    pub fn iter_ranges_for_value(self, val: T) -> impl Iterator<Item = RangeInclusive<u32>> + 'a {
        self.map
            .iter_ranges()
            .filter(move |r| r.value == val)
            .map(|r| r.range)
    }

    /// Yields an [`Iterator`] returning ranges of consecutive code points that
    /// do *not* have the value `v` in the [`CodePointMapData`].
    pub fn iter_ranges_for_value_complemented(
        self,
        val: T,
    ) -> impl Iterator<Item = RangeInclusive<u32>> + 'a {
        self.map
            .iter_ranges_mapped(move |value| value != val)
            .filter(|v| v.value)
            .map(|v| v.range)
    }

    /// Exposed for FFI needs, could be exposed in general in the future but we should
    /// have a use case first.
    ///
    /// FFI needs this since it operates on erased maps and can't use `iter_ranges_for_group()`
    #[doc(hidden)] // used by FFI code
    pub fn iter_ranges_mapped<U: Eq + 'a>(
        self,
        predicate: impl FnMut(T) -> U + Copy + 'a,
    ) -> impl Iterator<Item = CodePointMapRange<U>> + 'a {
        self.map.iter_ranges_mapped(predicate)
    }
}

impl CodePointMapDataBorrowed<'_, GeneralCategory> {
    /// Get a [`CodePointSetData`] for all elements corresponding to a particular value group
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::props::{GeneralCategory, GeneralCategoryGroup};
    /// use icu::properties::CodePointMapData;
    ///
    /// let gc = CodePointMapData::<GeneralCategory>::new();
    ///
    /// let other_letter_set_data =
    ///     gc.get_set_for_value_group(GeneralCategoryGroup::OtherLetter);
    /// let other_letter_set = other_letter_set_data.as_borrowed();
    ///
    /// assert!(other_letter_set.contains('木')); // U+6728
    /// assert!(!other_letter_set.contains('🎃')); // U+1F383 JACK-O-LANTERN
    /// ```
    #[cfg(feature = "alloc")]
    pub fn get_set_for_value_group(self, value: GeneralCategoryGroup) -> crate::CodePointSetData {
        let matching_gc_ranges = self
            .iter_ranges()
            .filter(|cpm_range| (1 << cpm_range.value as u32) & value.0 != 0)
            .map(|cpm_range| cpm_range.range);
        CodePointSetData::from_code_point_inversion_list(matching_gc_ranges.collect())
    }
}

#[cfg(feature = "compiled_data")]
impl<T: EnumeratedProperty> Default for CodePointMapDataBorrowed<'static, T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: TrieValue> CodePointMapDataBorrowed<'static, T> {
    /// Creates a new [`CodePointMapDataBorrowed`] for a [`EnumeratedProperty`].
    ///
    /// See the documentation on [`EnumeratedProperty`] implementations for details.
    ///
    /// ✨ *Enabled with the `compiled_data` Cargo feature.*
    ///
    /// [📚 Help choosing a constructor](icu_provider::constructors)
    #[cfg(feature = "compiled_data")]
    pub const fn new() -> Self
    where
        T: EnumeratedProperty,
    {
        CodePointMapDataBorrowed { map: T::SINGLETON }
    }

    /// Cheaply converts a [`CodePointMapDataBorrowed<'static>`] into a [`CodePointMapData`].
    ///
    /// Note: Due to branching and indirection, using [`CodePointMapData`] might inhibit some
    /// compile-time optimizations that are possible with [`CodePointMapDataBorrowed`].
    pub const fn static_to_owned(self) -> CodePointMapData<T> {
        CodePointMapData {
            data: DataPayload::from_static_ref(self.map),
        }
    }
}

impl<'a> CodePointMapDataBorrowed<'a, GeneralCategory> {
    /// Yields an [`Iterator`] returning ranges of consecutive code points that
    /// have a `General_Category` value belonging to the specified [`GeneralCategoryGroup`]
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::properties::props::{GeneralCategory, GeneralCategoryGroup};
    /// use icu::properties::CodePointMapData;
    ///
    /// let gc = CodePointMapData::<GeneralCategory>::new();
    /// let mut ranges = gc.iter_ranges_for_group(GeneralCategoryGroup::Letter);
    /// assert_eq!(ranges.next().unwrap(), 'A' as u32..='Z' as u32);
    /// assert_eq!(ranges.next().unwrap(), 'a' as u32..='z' as u32);
    /// assert_eq!(ranges.next().unwrap(), 'ª' as u32..='ª' as u32);
    /// assert_eq!(ranges.next().unwrap(), 'µ' as u32..='µ' as u32);
    /// assert_eq!(ranges.next().unwrap(), 'º' as u32..='º' as u32);
    /// assert_eq!(ranges.next().unwrap(), 'À' as u32..='Ö' as u32);
    /// assert_eq!(ranges.next().unwrap(), 'Ø' as u32..='ö' as u32);
    /// ```
    pub fn iter_ranges_for_group(
        self,
        group: GeneralCategoryGroup,
    ) -> impl Iterator<Item = RangeInclusive<u32>> + 'a {
        self.map
            .iter_ranges_mapped(move |value| group.contains(value))
            .filter(|v| v.value)
            .map(|v| v.range)
    }
}

/// A Unicode character property that assigns a value to each code point.
///
/// The descriptions of most properties are taken from [`TR44`], the documentation for the
/// Unicode Character Database.
///
/// <div class="stab unstable">
/// 🚫 This trait is sealed; it cannot be implemented by user code. If an API requests an item that implements this
/// trait, please consider using a type from the implementors listed below.
/// </div>
///
/// [`TR44`]: https://www.unicode.org/reports/tr44
pub trait EnumeratedProperty: crate::private::Sealed + TrieValue {
    #[doc(hidden)]
    type DataMarker: DataMarker<DataStruct = PropertyCodePointMap<'static, Self>>;
    #[doc(hidden)]
    #[cfg(feature = "compiled_data")]
    const SINGLETON: &'static PropertyCodePointMap<'static, Self>;
    /// The name of this property
    const NAME: &'static [u8];
    /// The abbreviated name of this property, if it exists, otherwise the name
    const SHORT_NAME: &'static [u8];

    /// Convenience method for `CodePointMapData::new().get(ch)`
    ///
    /// ✨ *Enabled with the `compiled_data` Cargo feature.*
    #[cfg(feature = "compiled_data")]
    fn for_char(ch: char) -> Self {
        CodePointMapData::new().get(ch)
    }
}
