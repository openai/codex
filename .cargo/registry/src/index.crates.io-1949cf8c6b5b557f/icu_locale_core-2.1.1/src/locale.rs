// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::parser::*;
use crate::subtags::Subtag;
use crate::{extensions, subtags, LanguageIdentifier};
#[cfg(feature = "alloc")]
use alloc::borrow::Cow;
use core::cmp::Ordering;
#[cfg(feature = "alloc")]
use core::str::FromStr;

/// A core struct representing a [`Unicode Locale Identifier`].
///
/// A locale is made of two parts:
///  * Unicode Language Identifier
///  * A set of Unicode Extensions
///
/// [`Locale`] exposes all of the same fields and methods as [`LanguageIdentifier`], and
/// on top of that is able to parse, manipulate and serialize unicode extension fields.
///
/// # Ordering
///
/// This type deliberately does not implement `Ord` or `PartialOrd` because there are
/// multiple possible orderings. Depending on your use case, two orderings are available:
///
/// 1. A string ordering, suitable for stable serialization: [`Locale::strict_cmp`]
/// 2. A struct ordering, suitable for use with a BTreeSet: [`Locale::total_cmp`]
///
/// See issue: <https://github.com/unicode-org/icu4x/issues/1215>
///
/// # Parsing
///
/// Unicode recognizes three levels of standard conformance for a locale:
///
///  * *well-formed* - syntactically correct
///  * *valid* - well-formed and only uses registered language subtags, extensions, keywords, types...
///  * *canonical* - valid and no deprecated codes or structure.
///
/// Any syntactically invalid subtags will cause the parsing to fail with an error.
///
/// This operation normalizes syntax to be well-formed. No legacy subtag replacements is performed.
/// For validation and canonicalization, see `LocaleCanonicalizer`.
///
/// ICU4X's Locale parsing does not allow for non-BCP-47-compatible locales [allowed by UTS 35 for backwards compatability][tr35-bcp].
/// Furthermore, it currently does not allow for language tags to have more than three characters.
///
/// # Serde
///
/// This type implements `serde::Serialize` and `serde::Deserialize` if the
/// `"serde"` Cargo feature is enabled on the crate.
///
/// The value will be serialized as a string and parsed when deserialized.
/// For tips on efficient storage and retrieval of locales, see [`crate::zerovec`].
///
/// # Examples
///
/// Simple example:
///
/// ```
/// use icu::locale::{
///     extensions::unicode::{key, value},
///     locale,
///     subtags::{language, region},
/// };
///
/// let loc = locale!("en-US-u-ca-buddhist");
///
/// assert_eq!(loc.id.language, language!("en"));
/// assert_eq!(loc.id.script, None);
/// assert_eq!(loc.id.region, Some(region!("US")));
/// assert_eq!(loc.id.variants.len(), 0);
/// assert_eq!(
///     loc.extensions.unicode.keywords.get(&key!("ca")),
///     Some(&value!("buddhist"))
/// );
/// ```
///
/// More complex example:
///
/// ```
/// use icu::locale::{subtags::*, Locale};
///
/// let loc: Locale = "eN-latn-Us-Valencia-u-hC-H12"
///     .parse()
///     .expect("Failed to parse.");
///
/// assert_eq!(loc.id.language, "en".parse::<Language>().unwrap());
/// assert_eq!(loc.id.script, "Latn".parse::<Script>().ok());
/// assert_eq!(loc.id.region, "US".parse::<Region>().ok());
/// assert_eq!(
///     loc.id.variants.first(),
///     "valencia".parse::<Variant>().ok().as_ref()
/// );
/// ```
///
/// [`Unicode Locale Identifier`]: https://unicode.org/reports/tr35/tr35.html#Unicode_locale_identifier
/// [tr35-bcp]: https://unicode.org/reports/tr35/#BCP_47_Conformance
#[derive(PartialEq, Eq, Clone, Hash)] // no Ord or PartialOrd: see docs
#[allow(clippy::exhaustive_structs)] // This struct is stable (and invoked by a macro)
pub struct Locale {
    /// The basic language/script/region components in the locale identifier along with any variants.
    pub id: LanguageIdentifier,
    /// Any extensions present in the locale identifier.
    pub extensions: extensions::Extensions,
}

#[test]
// Expected sizes are based on a 64-bit architecture
#[cfg(target_pointer_width = "64")]
fn test_sizes() {
    assert_eq!(core::mem::size_of::<subtags::Language>(), 3);
    assert_eq!(core::mem::size_of::<subtags::Script>(), 4);
    assert_eq!(core::mem::size_of::<subtags::Region>(), 3);
    assert_eq!(core::mem::size_of::<subtags::Variant>(), 8);
    assert_eq!(core::mem::size_of::<subtags::Variants>(), 16);
    assert_eq!(core::mem::size_of::<LanguageIdentifier>(), 32);

    assert_eq!(core::mem::size_of::<extensions::transform::Transform>(), 56);
    assert_eq!(core::mem::size_of::<Option<LanguageIdentifier>>(), 32);
    assert_eq!(core::mem::size_of::<extensions::transform::Fields>(), 24);

    assert_eq!(core::mem::size_of::<extensions::unicode::Attributes>(), 16);
    assert_eq!(core::mem::size_of::<extensions::unicode::Keywords>(), 24);
    assert_eq!(core::mem::size_of::<Vec<extensions::other::Other>>(), 24);
    assert_eq!(core::mem::size_of::<extensions::private::Private>(), 16);
    assert_eq!(core::mem::size_of::<extensions::Extensions>(), 136);

    assert_eq!(core::mem::size_of::<Locale>(), 168);
}

impl Locale {
    /// The unknown locale "und".
    pub const UNKNOWN: Self = crate::locale!("und");

    /// A constructor which takes a utf8 slice, parses it and
    /// produces a well-formed [`Locale`].
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::Locale;
    ///
    /// Locale::try_from_str("en-US-u-hc-h12").unwrap();
    /// ```
    #[inline]
    #[cfg(feature = "alloc")]
    pub fn try_from_str(s: &str) -> Result<Self, ParseError> {
        Self::try_from_utf8(s.as_bytes())
    }

    /// See [`Self::try_from_str`]
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    #[cfg(feature = "alloc")]
    pub fn try_from_utf8(code_units: &[u8]) -> Result<Self, ParseError> {
        parse_locale(code_units)
    }

    /// Normalize the locale (operating on UTF-8 formatted byte slices)
    ///
    /// This operation will normalize casing and the separator.
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::Locale;
    ///
    /// assert_eq!(
    ///     Locale::normalize_utf8(b"pL-latn-pl-U-HC-H12").as_deref(),
    ///     Ok("pl-Latn-PL-u-hc-h12")
    /// );
    /// ```
    #[cfg(feature = "alloc")]
    pub fn normalize_utf8(input: &[u8]) -> Result<Cow<'_, str>, ParseError> {
        let locale = Self::try_from_utf8(input)?;
        Ok(writeable::to_string_or_borrow(&locale, input))
    }

    /// Normalize the locale (operating on strings)
    ///
    /// This operation will normalize casing and the separator.
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::Locale;
    ///
    /// assert_eq!(
    ///     Locale::normalize("pL-latn-pl-U-HC-H12").as_deref(),
    ///     Ok("pl-Latn-PL-u-hc-h12")
    /// );
    /// ```
    #[cfg(feature = "alloc")]
    pub fn normalize(input: &str) -> Result<Cow<'_, str>, ParseError> {
        Self::normalize_utf8(input.as_bytes())
    }

    /// Compare this [`Locale`] with BCP-47 bytes.
    ///
    /// The return value is equivalent to what would happen if you first converted this
    /// [`Locale`] to a BCP-47 string and then performed a byte comparison.
    ///
    /// This function is case-sensitive and results in a *total order*, so it is appropriate for
    /// binary search. The only argument producing [`Ordering::Equal`] is `self.to_string()`.
    ///
    /// # Examples
    ///
    /// Sorting a list of locales with this method requires converting one of them to a string:
    ///
    /// ```
    /// use icu::locale::Locale;
    /// use std::cmp::Ordering;
    /// use writeable::Writeable;
    ///
    /// // Random input order:
    /// let bcp47_strings: &[&str] = &[
    ///     "und-u-ca-hebrew",
    ///     "ar-Latn",
    ///     "zh-Hant-TW",
    ///     "zh-TW",
    ///     "und-fonipa",
    ///     "zh-Hant",
    ///     "ar-SA",
    /// ];
    ///
    /// let mut locales = bcp47_strings
    ///     .iter()
    ///     .map(|s| s.parse().unwrap())
    ///     .collect::<Vec<Locale>>();
    /// locales.sort_by(|a, b| {
    ///     let b = b.write_to_string();
    ///     a.strict_cmp(b.as_bytes())
    /// });
    /// let strict_cmp_strings = locales
    ///     .iter()
    ///     .map(|l| l.to_string())
    ///     .collect::<Vec<String>>();
    ///
    /// // Output ordering, sorted alphabetically
    /// let expected_ordering: &[&str] = &[
    ///     "ar-Latn",
    ///     "ar-SA",
    ///     "und-fonipa",
    ///     "und-u-ca-hebrew",
    ///     "zh-Hant",
    ///     "zh-Hant-TW",
    ///     "zh-TW",
    /// ];
    ///
    /// assert_eq!(expected_ordering, strict_cmp_strings);
    /// ```
    pub fn strict_cmp(&self, other: &[u8]) -> Ordering {
        writeable::cmp_utf8(self, other)
    }

    #[expect(clippy::type_complexity)]
    pub(crate) fn as_tuple(
        &self,
    ) -> (
        (
            subtags::Language,
            Option<subtags::Script>,
            Option<subtags::Region>,
            &subtags::Variants,
        ),
        (
            (
                &extensions::unicode::Attributes,
                &extensions::unicode::Keywords,
            ),
            (
                Option<(
                    subtags::Language,
                    Option<subtags::Script>,
                    Option<subtags::Region>,
                    &subtags::Variants,
                )>,
                &extensions::transform::Fields,
            ),
            &extensions::private::Private,
            &[extensions::other::Other],
        ),
    ) {
        (self.id.as_tuple(), self.extensions.as_tuple())
    }

    /// Returns an ordering suitable for use in [`BTreeSet`].
    ///
    /// Unlike [`Locale::strict_cmp`], the ordering may or may not be equivalent
    /// to string ordering, and it may or may not be stable across ICU4X releases.
    ///
    /// # Examples
    ///
    /// This method returns a nonsensical ordering derived from the fields of the struct:
    ///
    /// ```
    /// use icu::locale::Locale;
    /// use std::cmp::Ordering;
    ///
    /// // Input strings, sorted alphabetically
    /// let bcp47_strings: &[&str] = &[
    ///     "ar-Latn",
    ///     "ar-SA",
    ///     "und-fonipa",
    ///     "und-u-ca-hebrew",
    ///     "zh-Hant",
    ///     "zh-Hant-TW",
    ///     "zh-TW",
    /// ];
    /// assert!(bcp47_strings.windows(2).all(|w| w[0] < w[1]));
    ///
    /// let mut locales = bcp47_strings
    ///     .iter()
    ///     .map(|s| s.parse().unwrap())
    ///     .collect::<Vec<Locale>>();
    /// locales.sort_by(Locale::total_cmp);
    /// let total_cmp_strings = locales
    ///     .iter()
    ///     .map(|l| l.to_string())
    ///     .collect::<Vec<String>>();
    ///
    /// // Output ordering, sorted arbitrarily
    /// let expected_ordering: &[&str] = &[
    ///     "ar-SA",
    ///     "ar-Latn",
    ///     "und-u-ca-hebrew",
    ///     "und-fonipa",
    ///     "zh-TW",
    ///     "zh-Hant",
    ///     "zh-Hant-TW",
    /// ];
    ///
    /// assert_eq!(expected_ordering, total_cmp_strings);
    /// ```
    ///
    /// Use a wrapper to add a [`Locale`] to a [`BTreeSet`]:
    ///
    /// ```no_run
    /// use icu::locale::Locale;
    /// use std::cmp::Ordering;
    /// use std::collections::BTreeSet;
    ///
    /// #[derive(PartialEq, Eq)]
    /// struct LocaleTotalOrd(Locale);
    ///
    /// impl Ord for LocaleTotalOrd {
    ///     fn cmp(&self, other: &Self) -> Ordering {
    ///         self.0.total_cmp(&other.0)
    ///     }
    /// }
    ///
    /// impl PartialOrd for LocaleTotalOrd {
    ///     fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    ///         Some(self.cmp(other))
    ///     }
    /// }
    ///
    /// let _: BTreeSet<LocaleTotalOrd> = unimplemented!();
    /// ```
    ///
    /// [`BTreeSet`]: alloc::collections::BTreeSet
    pub fn total_cmp(&self, other: &Self) -> Ordering {
        self.as_tuple().cmp(&other.as_tuple())
    }

    /// Compare this `Locale` with a potentially unnormalized BCP-47 string.
    ///
    /// The return value is equivalent to what would happen if you first parsed the
    /// BCP-47 string to a `Locale` and then performed a structural comparison.
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::Locale;
    ///
    /// let bcp47_strings: &[&str] = &[
    ///     "pl-LaTn-pL",
    ///     "uNd",
    ///     "UND-FONIPA",
    ///     "UnD-t-m0-TrUe",
    ///     "uNd-u-CA-Japanese",
    ///     "ZH",
    /// ];
    ///
    /// for a in bcp47_strings {
    ///     assert!(a.parse::<Locale>().unwrap().normalizing_eq(a));
    /// }
    /// ```
    #[cfg(feature = "alloc")]
    pub fn normalizing_eq(&self, other: &str) -> bool {
        macro_rules! subtag_matches {
            ($T:ty, $iter:ident, $expected:expr) => {
                $iter
                    .next()
                    .map(|b| <$T>::try_from_utf8(b) == Ok($expected))
                    .unwrap_or(false)
            };
        }

        let mut iter = SubtagIterator::new(other.as_bytes());
        if !subtag_matches!(subtags::Language, iter, self.id.language) {
            return false;
        }
        if let Some(ref script) = self.id.script {
            if !subtag_matches!(subtags::Script, iter, *script) {
                return false;
            }
        }
        if let Some(ref region) = self.id.region {
            if !subtag_matches!(subtags::Region, iter, *region) {
                return false;
            }
        }
        for variant in self.id.variants.iter() {
            if !subtag_matches!(subtags::Variant, iter, *variant) {
                return false;
            }
        }
        if !self.extensions.is_empty() {
            match extensions::Extensions::try_from_iter(&mut iter) {
                Ok(exts) => {
                    if self.extensions != exts {
                        return false;
                    }
                }
                Err(_) => {
                    return false;
                }
            }
        }
        iter.next().is_none()
    }

    #[doc(hidden)] // macro use
    #[expect(clippy::type_complexity)]
    pub const fn try_from_utf8_with_single_variant_single_keyword_unicode_extension(
        code_units: &[u8],
    ) -> Result<
        (
            subtags::Language,
            Option<subtags::Script>,
            Option<subtags::Region>,
            Option<subtags::Variant>,
            Option<(extensions::unicode::Key, Option<Subtag>)>,
        ),
        ParseError,
    > {
        parse_locale_with_single_variant_single_keyword_unicode_keyword_extension(
            code_units,
            ParserMode::Locale,
        )
    }

    pub(crate) fn for_each_subtag_str<E, F>(&self, f: &mut F) -> Result<(), E>
    where
        F: FnMut(&str) -> Result<(), E>,
    {
        self.id.for_each_subtag_str(f)?;
        self.extensions.for_each_subtag_str(f)?;
        Ok(())
    }
}

/// ✨ *Enabled with the `alloc` Cargo feature.*
#[cfg(feature = "alloc")]
impl FromStr for Locale {
    type Err = ParseError;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from_str(s)
    }
}

impl From<LanguageIdentifier> for Locale {
    fn from(id: LanguageIdentifier) -> Self {
        Self {
            id,
            extensions: extensions::Extensions::default(),
        }
    }
}

impl From<Locale> for LanguageIdentifier {
    fn from(loc: Locale) -> Self {
        loc.id
    }
}

impl core::fmt::Debug for Locale {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        writeable::Writeable::write_to(self, f)
    }
}

impl_writeable_for_each_subtag_str_no_test!(Locale, selff, selff.extensions.is_empty() => selff.id.writeable_borrow());

#[test]
fn test_writeable() {
    use writeable::assert_writeable_eq;
    assert_writeable_eq!(Locale::UNKNOWN, "und");
    assert_writeable_eq!("und-001".parse::<Locale>().unwrap(), "und-001");
    assert_writeable_eq!("und-Mymr".parse::<Locale>().unwrap(), "und-Mymr");
    assert_writeable_eq!("my-Mymr-MM".parse::<Locale>().unwrap(), "my-Mymr-MM");
    assert_writeable_eq!(
        "my-Mymr-MM-posix".parse::<Locale>().unwrap(),
        "my-Mymr-MM-posix",
    );
    assert_writeable_eq!(
        "zh-macos-posix".parse::<Locale>().unwrap(),
        "zh-macos-posix",
    );
    assert_writeable_eq!(
        "my-t-my-d0-zawgyi".parse::<Locale>().unwrap(),
        "my-t-my-d0-zawgyi",
    );
    assert_writeable_eq!(
        "ar-SA-u-ca-islamic-civil".parse::<Locale>().unwrap(),
        "ar-SA-u-ca-islamic-civil",
    );
    assert_writeable_eq!(
        "en-001-x-foo-bar".parse::<Locale>().unwrap(),
        "en-001-x-foo-bar",
    );
    assert_writeable_eq!("und-t-m0-true".parse::<Locale>().unwrap(), "und-t-m0-true",);
}

/// # Examples
///
/// ```
/// use icu::locale::Locale;
/// use icu::locale::{locale, subtags::language};
///
/// assert_eq!(Locale::from(language!("en")), locale!("en"));
/// ```
impl From<subtags::Language> for Locale {
    fn from(language: subtags::Language) -> Self {
        Self {
            id: language.into(),
            extensions: extensions::Extensions::new(),
        }
    }
}

/// # Examples
///
/// ```
/// use icu::locale::Locale;
/// use icu::locale::{locale, subtags::script};
///
/// assert_eq!(Locale::from(Some(script!("latn"))), locale!("und-Latn"));
/// ```
impl From<Option<subtags::Script>> for Locale {
    fn from(script: Option<subtags::Script>) -> Self {
        Self {
            id: script.into(),
            extensions: extensions::Extensions::new(),
        }
    }
}

/// # Examples
///
/// ```
/// use icu::locale::Locale;
/// use icu::locale::{locale, subtags::region};
///
/// assert_eq!(Locale::from(Some(region!("US"))), locale!("und-US"));
/// ```
impl From<Option<subtags::Region>> for Locale {
    fn from(region: Option<subtags::Region>) -> Self {
        Self {
            id: region.into(),
            extensions: extensions::Extensions::new(),
        }
    }
}

/// # Examples
///
/// ```
/// use icu::locale::Locale;
/// use icu::locale::{
///     locale,
///     subtags::{language, region, script},
/// };
///
/// assert_eq!(
///     Locale::from((
///         language!("en"),
///         Some(script!("Latn")),
///         Some(region!("US"))
///     )),
///     locale!("en-Latn-US")
/// );
/// ```
impl
    From<(
        subtags::Language,
        Option<subtags::Script>,
        Option<subtags::Region>,
    )> for Locale
{
    fn from(
        lsr: (
            subtags::Language,
            Option<subtags::Script>,
            Option<subtags::Region>,
        ),
    ) -> Self {
        Self {
            id: lsr.into(),
            extensions: extensions::Extensions::new(),
        }
    }
}
