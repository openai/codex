// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::provider::*;
use core::ops::RangeInclusive;
use icu_collections::codepointinvlist::CodePointInversionList;
use icu_provider::marker::ErasedMarker;
use icu_provider::prelude::*;

/// A set of Unicode code points. Access its data via the borrowed version,
/// [`CodePointSetDataBorrowed`].
///
/// # Example
/// ```rust
/// use icu::properties::CodePointSetData;
/// use icu::properties::props::Alphabetic;
///
/// let alphabetic = CodePointSetData::new::<Alphabetic>();
///
/// assert!(!alphabetic.contains('3'));
/// assert!(!alphabetic.contains('੩'));  // U+0A69 GURMUKHI DIGIT THREE
/// assert!(alphabetic.contains('A'));
/// assert!(alphabetic.contains('Ä'));  // U+00C4 LATIN CAPITAL LETTER A WITH DIAERESIS
/// ```
#[derive(Debug)]
pub struct CodePointSetData {
    data: DataPayload<ErasedMarker<PropertyCodePointSet<'static>>>,
}

impl CodePointSetData {
    /// Creates a new [`CodePointSetDataBorrowed`] for a [`BinaryProperty`].
    ///
    /// ✨ *Enabled with the `compiled_data` Cargo feature.*
    ///
    /// [📚 Help choosing a constructor](icu_provider::constructors)
    #[expect(clippy::new_ret_no_self)]
    #[cfg(feature = "compiled_data")]
    pub const fn new<P: BinaryProperty>() -> CodePointSetDataBorrowed<'static> {
        CodePointSetDataBorrowed::new::<P>()
    }

    #[doc = icu_provider::gen_buffer_unstable_docs!(UNSTABLE, Self::new)]
    pub fn try_new_unstable<P: BinaryProperty>(
        provider: &(impl DataProvider<P::DataMarker> + ?Sized),
    ) -> Result<CodePointSetData, DataError> {
        Ok(CodePointSetData::from_data(
            provider.load(Default::default())?.payload,
        ))
    }

    /// Construct a borrowed version of this type that can be queried.
    ///
    /// This owned version if returned by functions that use a runtime data provider.
    #[inline]
    pub fn as_borrowed(&self) -> CodePointSetDataBorrowed<'_> {
        CodePointSetDataBorrowed {
            set: self.data.get(),
        }
    }

    /// Construct a new one from loaded data
    ///
    /// Typically it is preferable to use getters like [`load_ascii_hex_digit()`] instead
    pub(crate) fn from_data<M>(data: DataPayload<M>) -> Self
    where
        M: DynamicDataMarker<DataStruct = PropertyCodePointSet<'static>>,
    {
        Self { data: data.cast() }
    }

    /// Construct a new owned [`CodePointInversionList`]
    pub fn from_code_point_inversion_list(set: CodePointInversionList<'static>) -> Self {
        let set = PropertyCodePointSet::from_code_point_inversion_list(set);
        CodePointSetData::from_data(
            DataPayload::<ErasedMarker<PropertyCodePointSet<'static>>>::from_owned(set),
        )
    }

    /// Convert this type to a [`CodePointInversionList`] as a borrowed value.
    ///
    /// The data backing this is extensible and supports multiple implementations.
    /// Currently it is always [`CodePointInversionList`]; however in the future more backends may be
    /// added, and users may select which at data generation time.
    ///
    /// This method returns an `Option` in order to return `None` when the backing data provider
    /// cannot return a [`CodePointInversionList`], or cannot do so within the expected constant time
    /// constraint.
    pub fn as_code_point_inversion_list(&self) -> Option<&CodePointInversionList<'_>> {
        self.data.get().as_code_point_inversion_list()
    }

    /// Convert this type to a [`CodePointInversionList`], borrowing if possible,
    /// otherwise allocating a new [`CodePointInversionList`].
    ///
    /// The data backing this is extensible and supports multiple implementations.
    /// Currently it is always [`CodePointInversionList`]; however in the future more backends may be
    /// added, and users may select which at data generation time.
    ///
    /// The performance of the conversion to this specific return type will vary
    /// depending on the data structure that is backing `self`.
    pub fn to_code_point_inversion_list(&self) -> CodePointInversionList<'_> {
        self.data.get().to_code_point_inversion_list()
    }
}

/// A borrowed wrapper around code point set data, returned by
/// [`CodePointSetData::as_borrowed()`]. More efficient to query.
#[derive(Clone, Copy, Debug)]
pub struct CodePointSetDataBorrowed<'a> {
    set: &'a PropertyCodePointSet<'a>,
}

impl CodePointSetDataBorrowed<'static> {
    /// Creates a new [`CodePointSetData`] for a [`BinaryProperty`].
    ///
    /// ✨ *Enabled with the `compiled_data` Cargo feature.*
    ///
    /// [📚 Help choosing a constructor](icu_provider::constructors)
    #[inline]
    #[cfg(feature = "compiled_data")]
    pub const fn new<P: BinaryProperty>() -> Self {
        CodePointSetDataBorrowed { set: P::SINGLETON }
    }
    /// Cheaply converts a [`CodePointSetDataBorrowed<'static>`] into a [`CodePointSetData`].
    ///
    /// Note: Due to branching and indirection, using [`CodePointSetData`] might inhibit some
    /// compile-time optimizations that are possible with [`CodePointSetDataBorrowed`].
    pub const fn static_to_owned(self) -> CodePointSetData {
        CodePointSetData {
            data: DataPayload::from_static_ref(self.set),
        }
    }
}

impl<'a> CodePointSetDataBorrowed<'a> {
    /// Check if the set contains a character
    ///
    /// ```rust
    /// use icu::properties::CodePointSetData;
    /// use icu::properties::props::Alphabetic;
    ///
    /// let alphabetic = CodePointSetData::new::<Alphabetic>();
    ///
    /// assert!(!alphabetic.contains('3'));
    /// assert!(!alphabetic.contains('੩'));  // U+0A69 GURMUKHI DIGIT THREE
    /// assert!(alphabetic.contains('A'));
    /// assert!(alphabetic.contains('Ä'));  // U+00C4 LATIN CAPITAL LETTER A WITH DIAERESIS
    /// ```
    #[inline]
    pub fn contains(self, ch: char) -> bool {
        self.set.contains(ch)
    }

    /// See [`Self::contains`].
    #[inline]
    pub fn contains32(self, ch: u32) -> bool {
        self.set.contains32(ch)
    }

    // Yields an [`Iterator`] returning the ranges of the code points that are
    /// included in the [`CodePointSetData`]
    ///
    /// Ranges are returned as [`RangeInclusive`], which is inclusive of its
    /// `end` bound value. An end-inclusive behavior matches the ICU4C/J
    /// behavior of ranges, ex: `UnicodeSet::contains(UChar32 start, UChar32 end)`.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::props::Alphabetic;
    /// use icu::properties::CodePointSetData;
    ///
    /// let alphabetic = CodePointSetData::new::<Alphabetic>();
    /// let mut ranges = alphabetic.iter_ranges();
    ///
    /// assert_eq!(Some(0x0041..=0x005A), ranges.next()); // 'A'..'Z'
    /// assert_eq!(Some(0x0061..=0x007A), ranges.next()); // 'a'..'z'
    /// ```
    #[inline]
    pub fn iter_ranges(self) -> impl Iterator<Item = RangeInclusive<u32>> + 'a {
        self.set.iter_ranges()
    }

    // Yields an [`Iterator`] returning the ranges of the code points that are
    /// *not* included in the [`CodePointSetData`]
    ///
    /// Ranges are returned as [`RangeInclusive`], which is inclusive of its
    /// `end` bound value. An end-inclusive behavior matches the ICU4C/J
    /// behavior of ranges, ex: `UnicodeSet::contains(UChar32 start, UChar32 end)`.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::props::Alphabetic;
    /// use icu::properties::CodePointSetData;
    ///
    /// let alphabetic = CodePointSetData::new::<Alphabetic>();
    /// let mut ranges = alphabetic.iter_ranges();
    ///
    /// assert_eq!(Some(0x0041..=0x005A), ranges.next()); // 'A'..'Z'
    /// assert_eq!(Some(0x0061..=0x007A), ranges.next()); // 'a'..'z'
    /// ```
    #[inline]
    pub fn iter_ranges_complemented(self) -> impl Iterator<Item = RangeInclusive<u32>> + 'a {
        self.set.iter_ranges_complemented()
    }
}

/// A binary Unicode character property.
///
/// The descriptions of most properties are taken from [`TR44`], the documentation for the
/// Unicode Character Database.  Some properties are instead defined in [`TR18`], the
/// documentation for Unicode regular expressions. In particular, Annex C of this document
/// defines properties for POSIX compatibility.
///
/// <div class="stab unstable">
/// 🚫 This trait is sealed; it cannot be implemented by user code. If an API requests an item that implements this
/// trait, please consider using a type from the implementors listed below.
/// </div>
///
/// [`TR44`]: https://www.unicode.org/reports/tr44
/// [`TR18`]: https://www.unicode.org/reports/tr18
pub trait BinaryProperty: crate::private::Sealed + Sized {
    #[doc(hidden)]
    type DataMarker: DataMarker<DataStruct = PropertyCodePointSet<'static>>;
    #[doc(hidden)]
    #[cfg(feature = "compiled_data")]
    const SINGLETON: &'static PropertyCodePointSet<'static>;
    /// The name of this property
    const NAME: &'static [u8];
    /// The abbreviated name of this property, if it exists, otherwise the name
    const SHORT_NAME: &'static [u8];

    /// Convenience method for `CodePointSetData::new().contains(ch)`
    ///
    /// ✨ *Enabled with the `compiled_data` Cargo feature.*
    #[cfg(feature = "compiled_data")]
    fn for_char(ch: char) -> bool {
        CodePointSetData::new::<Self>().contains(ch)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_general_category() {
        use icu::properties::props::GeneralCategory;
        use icu::properties::props::GeneralCategoryGroup;
        use icu::properties::CodePointMapData;

        let digits_data = CodePointMapData::<GeneralCategory>::new()
            .get_set_for_value_group(GeneralCategoryGroup::Number);
        let digits = digits_data.as_borrowed();

        assert!(digits.contains('5'));
        assert!(digits.contains('\u{0665}')); // U+0665 ARABIC-INDIC DIGIT FIVE
        assert!(digits.contains('\u{096b}')); // U+0969 DEVANAGARI DIGIT FIVE

        assert!(!digits.contains('A'));
    }

    #[test]
    fn test_script() {
        use icu::properties::props::Script;
        use icu::properties::CodePointMapData;

        let thai_data = CodePointMapData::<Script>::new().get_set_for_value(Script::Thai);
        let thai = thai_data.as_borrowed();

        assert!(thai.contains('\u{0e01}')); // U+0E01 THAI CHARACTER KO KAI
        assert!(thai.contains('\u{0e50}')); // U+0E50 THAI DIGIT ZERO

        assert!(!thai.contains('A'));
        assert!(!thai.contains('\u{0e3f}')); // U+0E50 THAI CURRENCY SYMBOL BAHT
    }

    #[test]
    fn test_gc_groupings() {
        use icu::properties::props::{GeneralCategory, GeneralCategoryGroup};
        use icu::properties::CodePointMapData;
        use icu_collections::codepointinvlist::CodePointInversionListBuilder;

        let test_group = |category: GeneralCategoryGroup, subcategories: &[GeneralCategory]| {
            let category_set =
                CodePointMapData::<GeneralCategory>::new().get_set_for_value_group(category);
            let category_set = category_set
                .as_code_point_inversion_list()
                .expect("The data should be valid");

            let mut builder = CodePointInversionListBuilder::new();
            for &subcategory in subcategories {
                let gc_set_data =
                    CodePointMapData::<GeneralCategory>::new().get_set_for_value(subcategory);
                let gc_set = gc_set_data.as_borrowed();
                for range in gc_set.iter_ranges() {
                    builder.add_range32(range);
                }
            }
            let combined_set = builder.build();
            println!("{category:?} {subcategories:?}");
            assert_eq!(
                category_set.get_inversion_list_vec(),
                combined_set.get_inversion_list_vec()
            );
        };

        test_group(
            GeneralCategoryGroup::Letter,
            &[
                GeneralCategory::UppercaseLetter,
                GeneralCategory::LowercaseLetter,
                GeneralCategory::TitlecaseLetter,
                GeneralCategory::ModifierLetter,
                GeneralCategory::OtherLetter,
            ],
        );
        test_group(
            GeneralCategoryGroup::Other,
            &[
                GeneralCategory::Control,
                GeneralCategory::Format,
                GeneralCategory::Unassigned,
                GeneralCategory::PrivateUse,
                GeneralCategory::Surrogate,
            ],
        );
        test_group(
            GeneralCategoryGroup::Mark,
            &[
                GeneralCategory::SpacingMark,
                GeneralCategory::EnclosingMark,
                GeneralCategory::NonspacingMark,
            ],
        );
        test_group(
            GeneralCategoryGroup::Number,
            &[
                GeneralCategory::DecimalNumber,
                GeneralCategory::LetterNumber,
                GeneralCategory::OtherNumber,
            ],
        );
        test_group(
            GeneralCategoryGroup::Punctuation,
            &[
                GeneralCategory::ConnectorPunctuation,
                GeneralCategory::DashPunctuation,
                GeneralCategory::ClosePunctuation,
                GeneralCategory::FinalPunctuation,
                GeneralCategory::InitialPunctuation,
                GeneralCategory::OtherPunctuation,
                GeneralCategory::OpenPunctuation,
            ],
        );
        test_group(
            GeneralCategoryGroup::Symbol,
            &[
                GeneralCategory::CurrencySymbol,
                GeneralCategory::ModifierSymbol,
                GeneralCategory::MathSymbol,
                GeneralCategory::OtherSymbol,
            ],
        );
        test_group(
            GeneralCategoryGroup::Separator,
            &[
                GeneralCategory::LineSeparator,
                GeneralCategory::ParagraphSeparator,
                GeneralCategory::SpaceSeparator,
            ],
        );
    }

    #[test]
    fn test_gc_surrogate() {
        use icu::properties::props::GeneralCategory;
        use icu::properties::CodePointMapData;

        let surrogates_data = CodePointMapData::<GeneralCategory>::new()
            .get_set_for_value(GeneralCategory::Surrogate);
        let surrogates = surrogates_data.as_borrowed();

        assert!(surrogates.contains32(0xd800));
        assert!(surrogates.contains32(0xd900));
        assert!(surrogates.contains32(0xdfff));

        assert!(!surrogates.contains('A'));
    }
}
