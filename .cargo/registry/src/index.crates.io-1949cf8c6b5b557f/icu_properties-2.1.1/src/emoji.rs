// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::provider::*;
use icu_collections::codepointinvliststringlist::CodePointInversionListAndStringList;
use icu_provider::marker::ErasedMarker;
use icu_provider::prelude::*;

/// A wrapper around `UnicodeSet` data (characters and strings)
#[derive(Debug)]
pub struct EmojiSetData {
    data: DataPayload<ErasedMarker<PropertyUnicodeSet<'static>>>,
}

impl EmojiSetData {
    /// Creates a new [`EmojiSetDataBorrowed`] for a [`EmojiSet`].
    ///
    /// See the documentation on [`EmojiSet`] implementations for details.
    ///
    /// ✨ *Enabled with the `compiled_data` Cargo feature.*
    ///
    /// [📚 Help choosing a constructor](icu_provider::constructors)
    #[cfg(feature = "compiled_data")]
    #[expect(clippy::new_ret_no_self)]
    pub const fn new<P: EmojiSet>() -> EmojiSetDataBorrowed<'static> {
        EmojiSetDataBorrowed::new::<P>()
    }

    /// A version of `new()` that uses custom data provided by a [`DataProvider`].
    ///
    /// Note that this will return an owned version of the data. Functionality is available on
    /// the borrowed version, accessible through [`EmojiSetData::as_borrowed`].
    pub fn try_new_unstable<P: EmojiSet>(
        provider: &(impl DataProvider<P::DataMarker> + ?Sized),
    ) -> Result<EmojiSetData, DataError> {
        Ok(EmojiSetData::from_data(
            provider.load(Default::default())?.payload,
        ))
    }

    /// Construct a borrowed version of this type that can be queried.
    ///
    /// This avoids a potential small underlying cost per API call (ex: `contains()`) by consolidating it
    /// up front.
    #[inline]
    pub fn as_borrowed(&self) -> EmojiSetDataBorrowed<'_> {
        EmojiSetDataBorrowed {
            set: self.data.get(),
        }
    }

    /// Construct a new one from loaded data
    ///
    /// Typically it is preferable to use getters instead
    pub(crate) fn from_data<M>(data: DataPayload<M>) -> Self
    where
        M: DynamicDataMarker<DataStruct = PropertyUnicodeSet<'static>>,
    {
        Self { data: data.cast() }
    }

    /// Construct a new owned [`CodePointInversionListAndStringList`]
    pub fn from_code_point_inversion_list_string_list(
        set: CodePointInversionListAndStringList<'static>,
    ) -> Self {
        let set = PropertyUnicodeSet::from_code_point_inversion_list_string_list(set);
        EmojiSetData::from_data(
            DataPayload::<ErasedMarker<PropertyUnicodeSet<'static>>>::from_owned(set),
        )
    }

    /// Convert this type to a [`CodePointInversionListAndStringList`] as a borrowed value.
    ///
    /// The data backing this is extensible and supports multiple implementations.
    /// Currently it is always [`CodePointInversionListAndStringList`]; however in the future more backends may be
    /// added, and users may select which at data generation time.
    ///
    /// This method returns an `Option` in order to return `None` when the backing data provider
    /// cannot return a [`CodePointInversionListAndStringList`], or cannot do so within the expected constant time
    /// constraint.
    pub fn as_code_point_inversion_list_string_list(
        &self,
    ) -> Option<&CodePointInversionListAndStringList<'_>> {
        self.data.get().as_code_point_inversion_list_string_list()
    }

    /// Convert this type to a [`CodePointInversionListAndStringList`], borrowing if possible,
    /// otherwise allocating a new [`CodePointInversionListAndStringList`].
    ///
    /// The data backing this is extensible and supports multiple implementations.
    /// Currently it is always [`CodePointInversionListAndStringList`]; however in the future more backends may be
    /// added, and users may select which at data generation time.
    ///
    /// The performance of the conversion to this specific return type will vary
    /// depending on the data structure that is backing `self`.
    pub fn to_code_point_inversion_list_string_list(
        &self,
    ) -> CodePointInversionListAndStringList<'_> {
        self.data.get().to_code_point_inversion_list_string_list()
    }
}

/// A borrowed wrapper around code point set data, returned by
/// [`EmojiSetData::as_borrowed()`]. More efficient to query.
#[derive(Clone, Copy, Debug)]
pub struct EmojiSetDataBorrowed<'a> {
    set: &'a PropertyUnicodeSet<'a>,
}

impl EmojiSetDataBorrowed<'_> {
    /// Check if the set contains the string. Strings consisting of one character
    /// are treated as a character/code point.
    ///
    /// This matches ICU behavior for ICU's `UnicodeSet`.
    #[inline]
    pub fn contains_str(self, s: &str) -> bool {
        self.set.contains_str(s)
    }

    /// Check if the set contains the code point.
    #[inline]
    pub fn contains(self, ch: char) -> bool {
        self.set.contains(ch)
    }

    /// See [`Self::contains`].
    #[inline]
    pub fn contains32(self, cp: u32) -> bool {
        self.set.contains32(cp)
    }
}

impl EmojiSetDataBorrowed<'static> {
    /// Creates a new [`EmojiSetDataBorrowed`] for a [`EmojiSet`].
    ///
    /// See the documentation on [`EmojiSet`] implementations for details.
    ///
    /// ✨ *Enabled with the `compiled_data` Cargo feature.*
    ///
    /// [📚 Help choosing a constructor](icu_provider::constructors)
    #[inline]
    #[cfg(feature = "compiled_data")]
    pub const fn new<P: EmojiSet>() -> Self {
        EmojiSetDataBorrowed { set: P::SINGLETON }
    }

    /// Cheaply converts a [`EmojiSetDataBorrowed<'static>`] into a [`EmojiSetData`].
    ///
    /// Note: Due to branching and indirection, using [`EmojiSetData`] might inhibit some
    /// compile-time optimizations that are possible with [`EmojiSetDataBorrowed`].
    pub const fn static_to_owned(self) -> EmojiSetData {
        EmojiSetData {
            data: DataPayload::from_static_ref(self.set),
        }
    }
}

/// An Emoji set as defined by [`Unicode Technical Standard #51`](https://unicode.org/reports/tr51/#Emoji_Sets>).
///
/// <div class="stab unstable">
/// 🚫 This trait is sealed; it cannot be implemented by user code. If an API requests an item that implements this
/// trait, please consider using a type from the implementors listed below.
/// </div>
pub trait EmojiSet: crate::private::Sealed {
    #[doc(hidden)]
    type DataMarker: DataMarker<DataStruct = PropertyUnicodeSet<'static>>;
    #[doc(hidden)]
    #[cfg(feature = "compiled_data")]
    const SINGLETON: &'static PropertyUnicodeSet<'static>;
}
