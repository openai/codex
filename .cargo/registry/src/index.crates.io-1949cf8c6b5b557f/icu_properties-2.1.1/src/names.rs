// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::props::*;
use crate::provider::names::*;
use core::marker::PhantomData;
use icu_collections::codepointtrie::TrieValue;
use icu_provider::marker::ErasedMarker;
use icu_provider::prelude::*;
use yoke::Yokeable;
use zerotrie::cursor::ZeroTrieSimpleAsciiCursor;

/// A struct capable of looking up a property value from a string name.
/// Access its data by calling [`Self::as_borrowed()`] and using the methods on
/// [`PropertyParserBorrowed`].
///
/// The name can be a short name (`Lu`), a long name(`Uppercase_Letter`),
/// or an alias.
///
/// Property names can be looked up using "strict" matching (looking for a name
/// that matches exactly), or "loose matching", where the name is allowed to deviate
/// in terms of ASCII casing, whitespace, underscores, and hyphens.
///
/// # Example
///
/// ```
/// use icu::properties::props::GeneralCategory;
/// use icu::properties::PropertyParser;
///
/// let lookup = PropertyParser::<GeneralCategory>::new();
/// // short name for value
/// assert_eq!(
///     lookup.get_strict("Lu"),
///     Some(GeneralCategory::UppercaseLetter)
/// );
/// assert_eq!(
///     lookup.get_strict("Pd"),
///     Some(GeneralCategory::DashPunctuation)
/// );
/// // long name for value
/// assert_eq!(
///     lookup.get_strict("Uppercase_Letter"),
///     Some(GeneralCategory::UppercaseLetter)
/// );
/// assert_eq!(
///     lookup.get_strict("Dash_Punctuation"),
///     Some(GeneralCategory::DashPunctuation)
/// );
/// // name has incorrect casing
/// assert_eq!(lookup.get_strict("dashpunctuation"), None);
/// // loose matching of name
/// assert_eq!(
///     lookup.get_loose("dash-punctuation"),
///     Some(GeneralCategory::DashPunctuation)
/// );
/// // fake property
/// assert_eq!(lookup.get_strict("Animated_Gif"), None);
/// ```
#[derive(Debug)]
pub struct PropertyParser<T> {
    map: DataPayload<ErasedMarker<PropertyValueNameToEnumMap<'static>>>,
    markers: PhantomData<fn() -> T>,
}

/// A borrowed wrapper around property value name-to-enum data, returned by
/// [`PropertyParser::as_borrowed()`]. More efficient to query.
#[derive(Debug)]
pub struct PropertyParserBorrowed<'a, T> {
    map: &'a PropertyValueNameToEnumMap<'a>,
    markers: PhantomData<fn() -> T>,
}

impl<T> Clone for PropertyParserBorrowed<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for PropertyParserBorrowed<'_, T> {}

impl<T> PropertyParser<T> {
    /// Creates a new instance of `PropertyParser<T>` using compiled data.
    ///
    /// ✨ *Enabled with the `compiled_data` Cargo feature.*
    ///
    /// [📚 Help choosing a constructor](icu_provider::constructors)
    #[cfg(feature = "compiled_data")]
    #[expect(clippy::new_ret_no_self)]
    pub fn new() -> PropertyParserBorrowed<'static, T>
    where
        T: ParseableEnumeratedProperty,
    {
        PropertyParserBorrowed::new()
    }

    #[doc = icu_provider::gen_buffer_unstable_docs!(UNSTABLE, Self::new)]
    pub fn try_new_unstable(
        provider: &(impl DataProvider<T::DataMarker> + ?Sized),
    ) -> Result<Self, DataError>
    where
        T: ParseableEnumeratedProperty,
    {
        Ok(Self {
            map: provider.load(Default::default())?.payload.cast(),
            markers: PhantomData,
        })
    }

    /// Construct a borrowed version of this type that can be queried.
    ///
    /// This avoids a potential small underlying cost per API call (like `get_strict()`) by consolidating it
    /// up front.
    #[inline]
    pub fn as_borrowed(&self) -> PropertyParserBorrowed<'_, T> {
        PropertyParserBorrowed {
            map: self.map.get(),
            markers: PhantomData,
        }
    }

    #[doc(hidden)] // used by FFI code
    pub fn erase(self) -> PropertyParser<u16> {
        PropertyParser {
            map: self.map.cast(),
            markers: PhantomData,
        }
    }
}

impl<T: TrieValue> PropertyParserBorrowed<'_, T> {
    /// Get the property value as a u16, doing a strict search looking for
    /// names that match exactly
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::props::GeneralCategory;
    /// use icu::properties::PropertyParser;
    ///
    /// let lookup = PropertyParser::<GeneralCategory>::new();
    /// assert_eq!(
    ///     lookup.get_strict_u16("Lu"),
    ///     Some(GeneralCategory::UppercaseLetter as u16)
    /// );
    /// assert_eq!(
    ///     lookup.get_strict_u16("Uppercase_Letter"),
    ///     Some(GeneralCategory::UppercaseLetter as u16)
    /// );
    /// // does not do loose matching
    /// assert_eq!(lookup.get_strict_u16("UppercaseLetter"), None);
    /// ```
    #[inline]
    pub fn get_strict_u16(self, name: &str) -> Option<u16> {
        get_strict_u16(self.map, name)
    }

    /// Get the property value as a `T`, doing a strict search looking for
    /// names that match exactly
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::props::GeneralCategory;
    /// use icu::properties::PropertyParser;
    ///
    /// let lookup = PropertyParser::<GeneralCategory>::new();
    /// assert_eq!(
    ///     lookup.get_strict("Lu"),
    ///     Some(GeneralCategory::UppercaseLetter)
    /// );
    /// assert_eq!(
    ///     lookup.get_strict("Uppercase_Letter"),
    ///     Some(GeneralCategory::UppercaseLetter)
    /// );
    /// // does not do loose matching
    /// assert_eq!(lookup.get_strict("UppercaseLetter"), None);
    /// ```
    #[inline]
    pub fn get_strict(self, name: &str) -> Option<T> {
        T::try_from_u32(self.get_strict_u16(name)? as u32).ok()
    }

    /// Get the property value as a u16, doing a loose search looking for
    /// names that match case-insensitively, ignoring ASCII hyphens, underscores, and
    /// whitespaces.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::props::GeneralCategory;
    /// use icu::properties::PropertyParser;
    ///
    /// let lookup = PropertyParser::<GeneralCategory>::new();
    /// assert_eq!(
    ///     lookup.get_loose_u16("Lu"),
    ///     Some(GeneralCategory::UppercaseLetter as u16)
    /// );
    /// assert_eq!(
    ///     lookup.get_loose_u16("Uppercase_Letter"),
    ///     Some(GeneralCategory::UppercaseLetter as u16)
    /// );
    /// // does do loose matching
    /// assert_eq!(
    ///     lookup.get_loose_u16("UppercaseLetter"),
    ///     Some(GeneralCategory::UppercaseLetter as u16)
    /// );
    /// ```
    #[inline]
    pub fn get_loose_u16(self, name: &str) -> Option<u16> {
        get_loose_u16(self.map, name)
    }

    /// Get the property value as a `T`, doing a loose search looking for
    /// names that match case-insensitively, ignoring ASCII hyphens, underscores, and
    /// whitespaces.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::properties::props::GeneralCategory;
    /// use icu::properties::PropertyParser;
    ///
    /// let lookup = PropertyParser::<GeneralCategory>::new();
    /// assert_eq!(
    ///     lookup.get_loose("Lu"),
    ///     Some(GeneralCategory::UppercaseLetter)
    /// );
    /// assert_eq!(
    ///     lookup.get_loose("Uppercase_Letter"),
    ///     Some(GeneralCategory::UppercaseLetter)
    /// );
    /// // does do loose matching
    /// assert_eq!(
    ///     lookup.get_loose("UppercaseLetter"),
    ///     Some(GeneralCategory::UppercaseLetter)
    /// );
    /// ```
    #[inline]
    pub fn get_loose(self, name: &str) -> Option<T> {
        T::try_from_u32(self.get_loose_u16(name)? as u32).ok()
    }
}

#[cfg(feature = "compiled_data")]
impl<T: ParseableEnumeratedProperty> Default for PropertyParserBorrowed<'static, T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: TrieValue> PropertyParserBorrowed<'static, T> {
    /// Creates a new instance of `PropertyParserBorrowed<T>` using compiled data.
    ///
    /// ✨ *Enabled with the `compiled_data` Cargo feature.*
    ///
    /// [📚 Help choosing a constructor](icu_provider::constructors)
    #[cfg(feature = "compiled_data")]
    pub fn new() -> Self
    where
        T: ParseableEnumeratedProperty,
    {
        Self {
            map: T::SINGLETON,
            markers: PhantomData,
        }
    }

    /// Cheaply converts a [`PropertyParserBorrowed<'static>`] into a [`PropertyParser`].
    ///
    /// Note: Due to branching and indirection, using [`PropertyParser`] might inhibit some
    /// compile-time optimizations that are possible with [`PropertyParserBorrowed`].
    pub const fn static_to_owned(self) -> PropertyParser<T> {
        PropertyParser {
            map: DataPayload::from_static_ref(self.map),
            markers: PhantomData,
        }
    }
}

/// Avoid monomorphizing multiple copies of this function
fn get_strict_u16(payload: &PropertyValueNameToEnumMap<'_>, name: &str) -> Option<u16> {
    payload.map.get(name).and_then(|i| i.try_into().ok())
}

/// Avoid monomorphizing multiple copies of this function
fn get_loose_u16(payload: &PropertyValueNameToEnumMap<'_>, name: &str) -> Option<u16> {
    fn recurse(mut cursor: ZeroTrieSimpleAsciiCursor, mut rest: &[u8]) -> Option<usize> {
        if cursor.is_empty() {
            return None;
        }

        // Skip whitespace, underscore, hyphen in trie.
        for skip in [b'\t', b'\n', b'\x0C', b'\r', b' ', 0x0B, b'_', b'-'] {
            let mut skip_cursor = cursor.clone();
            skip_cursor.step(skip);
            if let Some(r) = recurse(skip_cursor, rest) {
                return Some(r);
            }
        }

        let ascii = loop {
            let Some((&a, r)) = rest.split_first() else {
                return cursor.take_value();
            };
            rest = r;

            // Skip whitespace, underscore, hyphen in input
            if !matches!(
                a,
                b'\t' | b'\n' | b'\x0C' | b'\r' | b' ' | 0x0B | b'_' | b'-'
            ) {
                break a;
            }
        };

        let mut other_case_cursor = cursor.clone();
        cursor.step(ascii);
        other_case_cursor.step(if ascii.is_ascii_lowercase() {
            ascii.to_ascii_uppercase()
        } else {
            ascii.to_ascii_lowercase()
        });
        // This uses the call stack as the DFS stack. The recursion will terminate as
        // rest's length is strictly shrinking. The call stack's depth is limited by
        // name.len().
        recurse(cursor, rest).or_else(|| recurse(other_case_cursor, rest))
    }

    recurse(payload.map.cursor(), name.as_bytes()).and_then(|i| i.try_into().ok())
}

/// A struct capable of looking up a property name from a value
/// Access its data by calling [`Self::as_borrowed()`] and using the methods on
/// [`PropertyNamesLongBorrowed`].
///
/// # Example
///
/// ```
/// use icu::properties::props::CanonicalCombiningClass;
/// use icu::properties::PropertyNamesLong;
///
/// let names = PropertyNamesLong::<CanonicalCombiningClass>::new();
/// assert_eq!(
///     names.get(CanonicalCombiningClass::KanaVoicing),
///     Some("Kana_Voicing")
/// );
/// assert_eq!(
///     names.get(CanonicalCombiningClass::AboveLeft),
///     Some("Above_Left")
/// );
/// ```
pub struct PropertyNamesLong<T: NamedEnumeratedProperty> {
    map: DataPayload<ErasedMarker<T::DataStructLong>>,
}

impl<T: NamedEnumeratedProperty> core::fmt::Debug for PropertyNamesLong<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PropertyNamesLong")
            // .field("map", &self.map)
            .finish()
    }
}

/// A borrowed wrapper around property value name-to-enum data, returned by
/// [`PropertyNamesLong::as_borrowed()`]. More efficient to query.
#[derive(Debug)]
pub struct PropertyNamesLongBorrowed<'a, T: NamedEnumeratedProperty> {
    map: &'a T::DataStructLongBorrowed<'a>,
}

impl<T: NamedEnumeratedProperty> Clone for PropertyNamesLongBorrowed<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: NamedEnumeratedProperty> Copy for PropertyNamesLongBorrowed<'_, T> {}

impl<T: NamedEnumeratedProperty> PropertyNamesLong<T> {
    /// Creates a new instance of `PropertyNamesLongBorrowed<T>`.
    ///
    /// ✨ *Enabled with the `compiled_data` Cargo feature.*
    ///
    /// [📚 Help choosing a constructor](icu_provider::constructors)
    #[cfg(feature = "compiled_data")]
    #[expect(clippy::new_ret_no_self)]
    pub fn new() -> PropertyNamesLongBorrowed<'static, T> {
        PropertyNamesLongBorrowed::new()
    }

    #[doc = icu_provider::gen_buffer_unstable_docs!(UNSTABLE, Self::new)]
    pub fn try_new_unstable(
        provider: &(impl DataProvider<T::DataMarkerLong> + ?Sized),
    ) -> Result<Self, DataError> {
        Ok(Self {
            map: provider.load(Default::default())?.payload.cast(),
        })
    }

    /// Construct a borrowed version of this type that can be queried.
    ///
    /// This avoids a potential small underlying cost per API call (like `get_static()`) by consolidating it
    /// up front.
    #[inline]
    pub fn as_borrowed(&self) -> PropertyNamesLongBorrowed<'_, T> {
        PropertyNamesLongBorrowed {
            map: T::nep_long_identity(self.map.get()),
        }
    }
}

impl<'a, T: NamedEnumeratedProperty> PropertyNamesLongBorrowed<'a, T> {
    /// Get the property name given a value
    ///
    /// # Example
    ///
    /// ```rust
    /// use icu::properties::props::CanonicalCombiningClass;
    /// use icu::properties::PropertyNamesLong;
    ///
    /// let lookup = PropertyNamesLong::<CanonicalCombiningClass>::new();
    /// assert_eq!(
    ///     lookup.get(CanonicalCombiningClass::KanaVoicing),
    ///     Some("Kana_Voicing")
    /// );
    /// assert_eq!(
    ///     lookup.get(CanonicalCombiningClass::AboveLeft),
    ///     Some("Above_Left")
    /// );
    /// ```
    #[inline]
    pub fn get(self, property: T) -> Option<&'a str> {
        self.map.get(property.to_u32())
    }
}

#[cfg(feature = "compiled_data")]
impl<T: NamedEnumeratedProperty> Default for PropertyNamesLongBorrowed<'static, T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: NamedEnumeratedProperty> PropertyNamesLongBorrowed<'static, T> {
    /// Creates a new instance of `PropertyNamesLongBorrowed<T>`.
    ///
    /// ✨ *Enabled with the `compiled_data` Cargo feature.*
    ///
    /// [📚 Help choosing a constructor](icu_provider::constructors)
    #[cfg(feature = "compiled_data")]
    pub fn new() -> Self {
        Self {
            map: T::SINGLETON_LONG,
        }
    }

    /// Cheaply converts a [`PropertyNamesLongBorrowed<'static>`] into a [`PropertyNamesLong`].
    ///
    /// Note: Due to branching and indirection, using [`PropertyNamesLong`] might inhibit some
    /// compile-time optimizations that are possible with [`PropertyNamesLongBorrowed`].
    ///
    /// This is currently not `const` unlike other `static_to_owned()` functions since it needs
    /// const traits to do that safely
    pub fn static_to_owned(self) -> PropertyNamesLong<T> {
        PropertyNamesLong {
            map: DataPayload::from_static_ref(T::nep_long_identity_static(self.map)),
        }
    }
}

/// A struct capable of looking up a property name from a value
/// Access its data by calling [`Self::as_borrowed()`] and using the methods on
/// [`PropertyNamesShortBorrowed`].
///
/// # Example
///
/// ```
/// use icu::properties::props::CanonicalCombiningClass;
/// use icu::properties::PropertyNamesShort;
///
/// let names = PropertyNamesShort::<CanonicalCombiningClass>::new();
/// assert_eq!(names.get(CanonicalCombiningClass::KanaVoicing), Some("KV"));
/// assert_eq!(names.get(CanonicalCombiningClass::AboveLeft), Some("AL"));
/// ```
pub struct PropertyNamesShort<T: NamedEnumeratedProperty> {
    map: DataPayload<ErasedMarker<T::DataStructShort>>,
}

impl<T: NamedEnumeratedProperty> core::fmt::Debug for PropertyNamesShort<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PropertyNamesShort")
            // .field("map", &self.map)
            .finish()
    }
}

/// A borrowed wrapper around property value name-to-enum data, returned by
/// [`PropertyNamesShort::as_borrowed()`]. More efficient to query.
#[derive(Debug)]
pub struct PropertyNamesShortBorrowed<'a, T: NamedEnumeratedProperty> {
    map: &'a T::DataStructShortBorrowed<'a>,
}

impl<T: NamedEnumeratedProperty> Clone for PropertyNamesShortBorrowed<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: NamedEnumeratedProperty> Copy for PropertyNamesShortBorrowed<'_, T> {}

impl<T: NamedEnumeratedProperty> PropertyNamesShort<T> {
    /// Creates a new instance of `PropertyNamesShortBorrowed<T>`.
    ///
    /// ✨ *Enabled with the `compiled_data` Cargo feature.*
    ///
    /// [📚 Help choosing a constructor](icu_provider::constructors)
    #[cfg(feature = "compiled_data")]
    #[expect(clippy::new_ret_no_self)]
    pub fn new() -> PropertyNamesShortBorrowed<'static, T> {
        PropertyNamesShortBorrowed::new()
    }

    #[doc = icu_provider::gen_buffer_unstable_docs!(UNSTABLE, Self::new)]
    pub fn try_new_unstable(
        provider: &(impl DataProvider<T::DataMarkerShort> + ?Sized),
    ) -> Result<Self, DataError> {
        Ok(Self {
            map: provider.load(Default::default())?.payload.cast(),
        })
    }

    /// Construct a borrowed version of this type that can be queried.
    ///
    /// This avoids a potential small underlying cost per API call (like `get_static()`) by consolidating it
    /// up front.
    #[inline]
    pub fn as_borrowed(&self) -> PropertyNamesShortBorrowed<'_, T> {
        PropertyNamesShortBorrowed {
            map: T::nep_short_identity(self.map.get()),
        }
    }
}

impl<'a, T: NamedEnumeratedProperty> PropertyNamesShortBorrowed<'a, T> {
    /// Get the property name given a value
    ///
    /// # Example
    ///
    /// ```rust
    /// use icu::properties::props::CanonicalCombiningClass;
    /// use icu::properties::PropertyNamesShort;
    ///
    /// let lookup = PropertyNamesShort::<CanonicalCombiningClass>::new();
    /// assert_eq!(lookup.get(CanonicalCombiningClass::KanaVoicing), Some("KV"));
    /// assert_eq!(lookup.get(CanonicalCombiningClass::AboveLeft), Some("AL"));
    /// ```
    #[inline]
    pub fn get(self, property: T) -> Option<&'a str> {
        self.map.get(property.to_u32())
    }
}

impl PropertyNamesShortBorrowed<'_, Script> {
    /// Gets the "name" of a script property as a `icu::locale::subtags::Script`.
    ///
    /// This method is available only on `PropertyNamesShortBorrowed<Script>`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use icu::locale::subtags::script;
    /// use icu::properties::props::Script;
    /// use icu::properties::PropertyNamesShort;
    ///
    /// let lookup = PropertyNamesShort::<Script>::new();
    /// assert_eq!(
    ///     lookup.get_locale_script(Script::Brahmi),
    ///     Some(script!("Brah"))
    /// );
    /// assert_eq!(
    ///     lookup.get_locale_script(Script::Hangul),
    ///     Some(script!("Hang"))
    /// );
    /// ```
    ///
    /// For the reverse direction, use property parsing as normal:
    /// ```
    /// use icu::locale::subtags::script;
    /// use icu::properties::props::Script;
    /// use icu::properties::PropertyParser;
    ///
    /// let parser = PropertyParser::<Script>::new();
    /// assert_eq!(
    ///     parser.get_strict(script!("Brah").as_str()),
    ///     Some(Script::Brahmi)
    /// );
    /// assert_eq!(
    ///     parser.get_strict(script!("Hang").as_str()),
    ///     Some(Script::Hangul)
    /// );
    /// ```
    #[inline]
    pub fn get_locale_script(self, property: Script) -> Option<icu_locale_core::subtags::Script> {
        let prop = usize::try_from(property.to_u32()).ok()?;
        self.map.map.get(prop).and_then(|o| o.0)
    }
}

#[cfg(feature = "compiled_data")]
impl<T: NamedEnumeratedProperty> Default for PropertyNamesShortBorrowed<'static, T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: NamedEnumeratedProperty> PropertyNamesShortBorrowed<'static, T> {
    /// Creates a new instance of `PropertyNamesShortBorrowed<T>`.
    ///
    /// ✨ *Enabled with the `compiled_data` Cargo feature.*
    ///
    /// [📚 Help choosing a constructor](icu_provider::constructors)
    #[cfg(feature = "compiled_data")]
    pub fn new() -> Self {
        Self {
            map: T::SINGLETON_SHORT,
        }
    }

    /// Cheaply converts a [`PropertyNamesShortBorrowed<'static>`] into a [`PropertyNamesShort`].
    ///
    /// Note: Due to branching and indirection, using [`PropertyNamesShort`] might inhibit some
    /// compile-time optimizations that are possible with [`PropertyNamesShortBorrowed`].
    ///
    /// This is currently not `const` unlike other `static_to_owned()` functions since it needs
    /// const traits to do that safely
    pub fn static_to_owned(self) -> PropertyNamesShort<T> {
        PropertyNamesShort {
            map: DataPayload::from_static_ref(T::nep_short_identity_static(self.map)),
        }
    }
}

/// A property whose value names can be parsed from strings.
pub trait ParseableEnumeratedProperty: crate::private::Sealed + TrieValue {
    #[doc(hidden)]
    type DataMarker: DataMarker<DataStruct = PropertyValueNameToEnumMap<'static>>;
    #[doc(hidden)]
    #[cfg(feature = "compiled_data")]
    const SINGLETON: &'static PropertyValueNameToEnumMap<'static>;
}

// Abstract over Linear/Sparse/Script representation
// This trait is implicitly sealed by not being exported.
pub trait PropertyEnumToValueNameLookup {
    fn get(&self, prop: u32) -> Option<&str>;
}

impl PropertyEnumToValueNameLookup for PropertyEnumToValueNameLinearMap<'_> {
    fn get(&self, prop: u32) -> Option<&str> {
        self.map.get(usize::try_from(prop).ok()?)
    }
}

#[cfg(feature = "alloc")]
impl PropertyEnumToValueNameLookup for PropertyEnumToValueNameSparseMap<'_> {
    fn get(&self, prop: u32) -> Option<&str> {
        self.map.get(&u16::try_from(prop).ok()?)
    }
}

impl PropertyEnumToValueNameLookup for PropertyScriptToIcuScriptMap<'_> {
    fn get(&self, prop: u32) -> Option<&str> {
        self.map
            .get_ule_ref(usize::try_from(prop).ok()?)
            .and_then(|no| no.as_ref())
            .map(|s| s.as_str())
    }
}

/// A property whose value names can be represented as strings.
pub trait NamedEnumeratedProperty: ParseableEnumeratedProperty {
    #[doc(hidden)]
    type DataStructLong: 'static
        + for<'a> Yokeable<'a, Output = Self::DataStructLongBorrowed<'a>>
        + PropertyEnumToValueNameLookup;
    #[doc(hidden)]
    type DataStructShort: 'static
        + for<'a> Yokeable<'a, Output = Self::DataStructShortBorrowed<'a>>
        + PropertyEnumToValueNameLookup;
    #[doc(hidden)]
    type DataStructLongBorrowed<'a>: PropertyEnumToValueNameLookup;
    #[doc(hidden)]
    type DataStructShortBorrowed<'a>: PropertyEnumToValueNameLookup;
    #[doc(hidden)]
    type DataMarkerLong: DataMarker<DataStruct = Self::DataStructLong>;
    #[doc(hidden)]
    type DataMarkerShort: DataMarker<DataStruct = Self::DataStructShort>;
    #[doc(hidden)]
    #[cfg(feature = "compiled_data")]
    const SINGLETON_LONG: &'static Self::DataStructLongBorrowed<'static>;
    #[doc(hidden)]
    #[cfg(feature = "compiled_data")]
    const SINGLETON_SHORT: &'static Self::DataStructShortBorrowed<'static>;

    // These wouldn't be necessary if Yoke used GATs (#6057)
    #[doc(hidden)]
    fn nep_long_identity<'a>(
        stat: &'a <Self::DataStructLong as Yokeable<'a>>::Output,
    ) -> &'a Self::DataStructLongBorrowed<'a>;
    #[doc(hidden)]
    fn nep_long_identity_static(
        stat: &'static Self::DataStructLongBorrowed<'static>,
    ) -> &'static Self::DataStructLong;

    #[doc(hidden)]
    fn nep_short_identity<'a>(
        stat: &'a <Self::DataStructShort as Yokeable<'a>>::Output,
    ) -> &'a Self::DataStructShortBorrowed<'a>;
    #[doc(hidden)]
    fn nep_short_identity_static(
        stat: &'static Self::DataStructShortBorrowed<'static>,
    ) -> &'static Self::DataStructShort;

    /// Convenience method for `PropertyParser::new().get_loose(s)`
    ///
    /// ✨ *Enabled with the `compiled_data` Cargo feature.*
    #[cfg(feature = "compiled_data")]
    fn try_from_str(s: &str) -> Option<Self> {
        PropertyParser::new().get_loose(s)
    }
    /// Convenience method for `PropertyNamesLong::new().get(*self).unwrap()`
    ///
    /// ✨ *Enabled with the `compiled_data` Cargo feature.*
    #[cfg(feature = "compiled_data")]
    fn long_name(&self) -> &'static str {
        PropertyNamesLong::new().get(*self).unwrap_or("unreachable")
    }
    /// Convenience method for `PropertyNamesShort::new().get(*self).unwrap()`
    ///
    /// ✨ *Enabled with the `compiled_data` Cargo feature.*
    #[cfg(feature = "compiled_data")]
    fn short_name(&self) -> &'static str {
        PropertyNamesShort::new()
            .get(*self)
            .unwrap_or("unreachable")
    }
}

macro_rules! impl_value_getter {
    (
        impl $ty:ident {
            $marker_n2e:ident / $singleton_n2e:ident;
            $(
                $(#[$meta:meta])*
                $data_struct_s:ident / $marker_e2sn:ident / $singleton_e2sn:ident;
                $data_struct_l:ident / $marker_e2ln:ident / $singleton_e2ln:ident;
            )?
        }
    ) => {
        impl ParseableEnumeratedProperty for $ty {
            type DataMarker = $marker_n2e;
            #[cfg(feature = "compiled_data")]
            const SINGLETON: &'static PropertyValueNameToEnumMap<'static> = crate::provider::Baked::$singleton_n2e;
        }

        $(
            $(#[$meta])*
            impl NamedEnumeratedProperty for $ty {
                type DataStructLong = $data_struct_l<'static>;
                type DataStructShort = $data_struct_s<'static>;
                type DataStructLongBorrowed<'a> = $data_struct_l<'a>;
                type DataStructShortBorrowed<'a> = $data_struct_s<'a>;
                type DataMarkerLong = crate::provider::$marker_e2ln;
                type DataMarkerShort = crate::provider::$marker_e2sn;
                #[cfg(feature = "compiled_data")]
                const SINGLETON_LONG: &'static Self::DataStructLong = crate::provider::Baked::$singleton_e2ln;
                #[cfg(feature = "compiled_data")]
                const SINGLETON_SHORT: &'static Self::DataStructShort = crate::provider::Baked::$singleton_e2sn;
                fn nep_long_identity<'a>(yoked: &'a $data_struct_l<'a>) -> &'a Self::DataStructLongBorrowed<'a> {
                    yoked
                }

                fn nep_long_identity_static(stat: &'static $data_struct_l<'static>) -> &'static $data_struct_l<'static> {
                    stat
                }


                fn nep_short_identity<'a>(yoked: &'a $data_struct_s<'a>) -> &'a Self::DataStructShortBorrowed<'a> {
                    yoked
                }
                fn nep_short_identity_static(stat: &'static $data_struct_s<'static>) -> &'static $data_struct_s<'static> {
                    stat
                }

            }


        )?
    };
}

impl_value_getter! {
    impl BidiClass {
        PropertyNameParseBidiClassV1 / SINGLETON_PROPERTY_NAME_PARSE_BIDI_CLASS_V1;
        PropertyEnumToValueNameLinearMap / PropertyNameShortBidiClassV1 / SINGLETON_PROPERTY_NAME_SHORT_BIDI_CLASS_V1;
        PropertyEnumToValueNameLinearMap / PropertyNameLongBidiClassV1 / SINGLETON_PROPERTY_NAME_LONG_BIDI_CLASS_V1;
    }
}

impl_value_getter! {
    impl GeneralCategory {
        PropertyNameParseGeneralCategoryV1 / SINGLETON_PROPERTY_NAME_PARSE_GENERAL_CATEGORY_V1;
        PropertyEnumToValueNameLinearMap / PropertyNameShortGeneralCategoryV1 / SINGLETON_PROPERTY_NAME_SHORT_GENERAL_CATEGORY_V1;
        PropertyEnumToValueNameLinearMap / PropertyNameLongGeneralCategoryV1 / SINGLETON_PROPERTY_NAME_LONG_GENERAL_CATEGORY_V1;
    }
}

impl_value_getter! {
    impl GeneralCategoryGroup {
        PropertyNameParseGeneralCategoryMaskV1 / SINGLETON_PROPERTY_NAME_PARSE_GENERAL_CATEGORY_MASK_V1;
    }
}

impl_value_getter! {
    impl Script {
        PropertyNameParseScriptV1 / SINGLETON_PROPERTY_NAME_PARSE_SCRIPT_V1;
        PropertyScriptToIcuScriptMap / PropertyNameShortScriptV1 / SINGLETON_PROPERTY_NAME_SHORT_SCRIPT_V1;
        PropertyEnumToValueNameLinearMap / PropertyNameLongScriptV1 / SINGLETON_PROPERTY_NAME_LONG_SCRIPT_V1;
    }
}

impl_value_getter! {
   impl HangulSyllableType {
        PropertyNameParseHangulSyllableTypeV1 / SINGLETON_PROPERTY_NAME_PARSE_HANGUL_SYLLABLE_TYPE_V1;
        PropertyEnumToValueNameLinearMap / PropertyNameShortHangulSyllableTypeV1 / SINGLETON_PROPERTY_NAME_SHORT_HANGUL_SYLLABLE_TYPE_V1;
        PropertyEnumToValueNameLinearMap / PropertyNameLongHangulSyllableTypeV1 / SINGLETON_PROPERTY_NAME_LONG_HANGUL_SYLLABLE_TYPE_V1;
    }
}

impl_value_getter! {
    impl EastAsianWidth {
        PropertyNameParseEastAsianWidthV1 / SINGLETON_PROPERTY_NAME_PARSE_EAST_ASIAN_WIDTH_V1;
        PropertyEnumToValueNameLinearMap / PropertyNameShortEastAsianWidthV1 / SINGLETON_PROPERTY_NAME_SHORT_EAST_ASIAN_WIDTH_V1;
        PropertyEnumToValueNameLinearMap / PropertyNameLongEastAsianWidthV1 / SINGLETON_PROPERTY_NAME_LONG_EAST_ASIAN_WIDTH_V1;
    }
}

impl_value_getter! {
    impl LineBreak {
        PropertyNameParseLineBreakV1 / SINGLETON_PROPERTY_NAME_PARSE_LINE_BREAK_V1;
        PropertyEnumToValueNameLinearMap / PropertyNameShortLineBreakV1 / SINGLETON_PROPERTY_NAME_SHORT_LINE_BREAK_V1;
        PropertyEnumToValueNameLinearMap / PropertyNameLongLineBreakV1 / SINGLETON_PROPERTY_NAME_LONG_LINE_BREAK_V1;
    }
}

impl_value_getter! {
    impl GraphemeClusterBreak {
        PropertyNameParseGraphemeClusterBreakV1 / SINGLETON_PROPERTY_NAME_PARSE_GRAPHEME_CLUSTER_BREAK_V1;
        PropertyEnumToValueNameLinearMap / PropertyNameShortGraphemeClusterBreakV1 / SINGLETON_PROPERTY_NAME_SHORT_GRAPHEME_CLUSTER_BREAK_V1;
        PropertyEnumToValueNameLinearMap / PropertyNameLongGraphemeClusterBreakV1 / SINGLETON_PROPERTY_NAME_LONG_GRAPHEME_CLUSTER_BREAK_V1;
    }
}

impl_value_getter! {
    impl WordBreak {
        PropertyNameParseWordBreakV1 / SINGLETON_PROPERTY_NAME_PARSE_WORD_BREAK_V1;
        PropertyEnumToValueNameLinearMap / PropertyNameShortWordBreakV1 / SINGLETON_PROPERTY_NAME_SHORT_WORD_BREAK_V1;
        PropertyEnumToValueNameLinearMap / PropertyNameLongWordBreakV1 / SINGLETON_PROPERTY_NAME_LONG_WORD_BREAK_V1;
    }
}

impl_value_getter! {
    impl SentenceBreak {
        PropertyNameParseSentenceBreakV1 / SINGLETON_PROPERTY_NAME_PARSE_SENTENCE_BREAK_V1;
        PropertyEnumToValueNameLinearMap / PropertyNameShortSentenceBreakV1 / SINGLETON_PROPERTY_NAME_SHORT_SENTENCE_BREAK_V1;
        PropertyEnumToValueNameLinearMap / PropertyNameLongSentenceBreakV1 / SINGLETON_PROPERTY_NAME_LONG_SENTENCE_BREAK_V1;
    }
}

impl_value_getter! {
    impl CanonicalCombiningClass {
        PropertyNameParseCanonicalCombiningClassV1 / SINGLETON_PROPERTY_NAME_PARSE_CANONICAL_COMBINING_CLASS_V1;
        #[cfg(feature = "alloc")]
        /// ✨ *Enabled with the `alloc` Cargo feature.*
        PropertyEnumToValueNameSparseMap / PropertyNameShortCanonicalCombiningClassV1 / SINGLETON_PROPERTY_NAME_SHORT_CANONICAL_COMBINING_CLASS_V1;
        PropertyEnumToValueNameSparseMap / PropertyNameLongCanonicalCombiningClassV1 / SINGLETON_PROPERTY_NAME_LONG_CANONICAL_COMBINING_CLASS_V1;
    }
}

impl_value_getter! {
    impl IndicSyllabicCategory {
        PropertyNameParseIndicSyllabicCategoryV1 / SINGLETON_PROPERTY_NAME_PARSE_INDIC_SYLLABIC_CATEGORY_V1;
        PropertyEnumToValueNameLinearMap / PropertyNameShortIndicSyllabicCategoryV1 / SINGLETON_PROPERTY_NAME_SHORT_INDIC_SYLLABIC_CATEGORY_V1;
        PropertyEnumToValueNameLinearMap / PropertyNameLongIndicSyllabicCategoryV1 / SINGLETON_PROPERTY_NAME_LONG_INDIC_SYLLABIC_CATEGORY_V1;
    }
}

impl_value_getter! {
    impl JoiningType {
        PropertyNameParseJoiningTypeV1 / SINGLETON_PROPERTY_NAME_PARSE_JOINING_TYPE_V1;
        PropertyEnumToValueNameLinearMap / PropertyNameShortJoiningTypeV1 / SINGLETON_PROPERTY_NAME_SHORT_JOINING_TYPE_V1;
        PropertyEnumToValueNameLinearMap / PropertyNameLongJoiningTypeV1 / SINGLETON_PROPERTY_NAME_LONG_JOINING_TYPE_V1;
    }
}

impl_value_getter! {
    impl VerticalOrientation {
        PropertyNameParseVerticalOrientationV1 / SINGLETON_PROPERTY_NAME_PARSE_VERTICAL_ORIENTATION_V1;
        PropertyEnumToValueNameLinearMap / PropertyNameShortVerticalOrientationV1 / SINGLETON_PROPERTY_NAME_SHORT_VERTICAL_ORIENTATION_V1;
        PropertyEnumToValueNameLinearMap / PropertyNameLongVerticalOrientationV1 / SINGLETON_PROPERTY_NAME_LONG_VERTICAL_ORIENTATION_V1;
    }
}
