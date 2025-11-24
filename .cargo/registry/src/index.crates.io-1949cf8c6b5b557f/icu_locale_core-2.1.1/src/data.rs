// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::extensions::unicode as unicode_ext;
use crate::subtags::{Language, Region, Script, Subtag, Variant};
#[cfg(feature = "alloc")]
use crate::ParseError;
use crate::{LanguageIdentifier, Locale};
use core::cmp::Ordering;
use core::default::Default;
use core::fmt;
use core::hash::Hash;
#[cfg(feature = "alloc")]
use core::str::FromStr;

/// A locale type optimized for use in fallbacking and the ICU4X data pipeline.
///
/// [`DataLocale`] contains less functionality than [`Locale`] but more than
/// [`LanguageIdentifier`] for better size and performance while still meeting
/// the needs of the ICU4X data pipeline.
///
/// You can create a [`DataLocale`] from a borrowed [`Locale`], which is more
/// efficient than cloning the [`Locale`], but less efficient than converting an owned
/// [`Locale`]:
///
/// ```
/// use icu_locale_core::locale;
/// use icu_provider::DataLocale;
///
/// let locale1 = locale!("en-u-ca-buddhist");
/// let data_locale = DataLocale::from(&locale1);
/// ```
///
/// [`DataLocale`] only supports `-u-sd` keywords, to reflect the current state of CLDR data
/// lookup and fallback. This may change in the future.
///
/// ```
/// use icu_locale_core::{locale, Locale};
/// use icu_provider::DataLocale;
///
/// let locale = "hi-IN-t-en-h0-hybrid-u-attr-ca-buddhist-sd-inas"
///     .parse::<Locale>()
///     .unwrap();
///
/// assert_eq!(
///     DataLocale::from(locale),
///     DataLocale::from(locale!("hi-IN-u-sd-inas"))
/// );
/// ```
#[derive(Clone, Copy, PartialEq, Hash, Eq)]
#[non_exhaustive]
pub struct DataLocale {
    /// Language subtag
    pub language: Language,
    /// Script subtag
    pub script: Option<Script>,
    /// Region subtag
    pub region: Option<Region>,
    /// Variant subtag
    pub variant: Option<Variant>,
    /// Subivision (-u-sd-) subtag
    pub subdivision: Option<Subtag>,
}

impl Default for DataLocale {
    fn default() -> Self {
        Self {
            language: Language::UNKNOWN,
            script: None,
            region: None,
            variant: None,
            subdivision: None,
        }
    }
}

impl DataLocale {
    /// `const` version of `Default::default`
    pub const fn default() -> Self {
        DataLocale {
            language: Language::UNKNOWN,
            script: None,
            region: None,
            variant: None,
            subdivision: None,
        }
    }
}

impl Default for &DataLocale {
    fn default() -> Self {
        static DEFAULT: DataLocale = DataLocale::default();
        &DEFAULT
    }
}

impl fmt::Debug for DataLocale {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "DataLocale{{{self}}}")
    }
}

impl_writeable_for_each_subtag_str_no_test!(DataLocale, selff, selff.script.is_none() && selff.region.is_none() && selff.variant.is_none() && selff.subdivision.is_none() => Some(selff.language.as_str()));

impl From<LanguageIdentifier> for DataLocale {
    fn from(langid: LanguageIdentifier) -> Self {
        Self::from(&langid)
    }
}

impl From<Locale> for DataLocale {
    fn from(locale: Locale) -> Self {
        Self::from(&locale)
    }
}

impl From<&LanguageIdentifier> for DataLocale {
    fn from(langid: &LanguageIdentifier) -> Self {
        Self {
            language: langid.language,
            script: langid.script,
            region: langid.region,
            variant: langid.variants.iter().copied().next(),
            subdivision: None,
        }
    }
}

impl From<&Locale> for DataLocale {
    fn from(locale: &Locale) -> Self {
        let mut r = Self::from(&locale.id);

        r.subdivision = locale
            .extensions
            .unicode
            .keywords
            .get(&unicode_ext::key!("sd"))
            .and_then(|v| v.as_single_subtag().copied());
        r
    }
}

/// ✨ *Enabled with the `alloc` Cargo feature.*
#[cfg(feature = "alloc")]
impl FromStr for DataLocale {
    type Err = ParseError;
    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from_str(s)
    }
}

impl DataLocale {
    #[inline]
    /// Parses a [`DataLocale`].
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    #[cfg(feature = "alloc")]
    pub fn try_from_str(s: &str) -> Result<Self, ParseError> {
        Self::try_from_utf8(s.as_bytes())
    }

    /// Parses a [`DataLocale`] from a UTF-8 byte slice.
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    #[cfg(feature = "alloc")]
    pub fn try_from_utf8(code_units: &[u8]) -> Result<Self, ParseError> {
        let locale = Locale::try_from_utf8(code_units)?;
        if locale.id.variants.len() > 1
            || !locale.extensions.transform.is_empty()
            || !locale.extensions.private.is_empty()
            || !locale.extensions.other.is_empty()
            || !locale.extensions.unicode.attributes.is_empty()
        {
            return Err(ParseError::InvalidExtension);
        }

        let unicode_extensions_count = locale.extensions.unicode.keywords.iter().count();

        if unicode_extensions_count != 0
            && (unicode_extensions_count != 1
                || !locale
                    .extensions
                    .unicode
                    .keywords
                    .contains_key(&unicode_ext::key!("sd")))
        {
            return Err(ParseError::InvalidExtension);
        }

        Ok(locale.into())
    }

    pub(crate) fn for_each_subtag_str<E, F>(&self, f: &mut F) -> Result<(), E>
    where
        F: FnMut(&str) -> Result<(), E>,
    {
        f(self.language.as_str())?;
        if let Some(ref script) = self.script {
            f(script.as_str())?;
        }
        if let Some(ref region) = self.region {
            f(region.as_str())?;
        }
        if let Some(ref single_variant) = self.variant {
            f(single_variant.as_str())?;
        }
        if let Some(ref subdivision) = self.subdivision {
            f("u")?;
            f("sd")?;
            f(subdivision.as_str())?;
        }
        Ok(())
    }

    fn as_tuple(
        &self,
    ) -> (
        Language,
        Option<Script>,
        Option<Region>,
        Option<Variant>,
        Option<Subtag>,
    ) {
        (
            self.language,
            self.script,
            self.region,
            self.variant,
            self.subdivision,
        )
    }

    /// Returns an ordering suitable for use in [`BTreeSet`].
    ///
    /// [`BTreeSet`]: alloc::collections::BTreeSet
    pub fn total_cmp(&self, other: &Self) -> Ordering {
        self.as_tuple().cmp(&other.as_tuple())
    }

    /// Compare this [`DataLocale`] with BCP-47 bytes.
    ///
    /// The return value is equivalent to what would happen if you first converted this
    /// [`DataLocale`] to a BCP-47 string and then performed a byte comparison.
    ///
    /// This function is case-sensitive and results in a *total order*, so it is appropriate for
    /// binary search. The only argument producing [`Ordering::Equal`] is `self.to_string()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use core::cmp::Ordering;
    /// use icu_provider::DataLocale;
    ///
    /// let bcp47_strings: &[&str] = &[
    ///     "ca",
    ///     "ca-ES",
    ///     "ca-ES-u-sd-esct",
    ///     "ca-ES-valencia",
    ///     "cat",
    ///     "pl-Latn-PL",
    ///     "und",
    ///     "und-fonipa",
    ///     "zh",
    /// ];
    ///
    /// for ab in bcp47_strings.windows(2) {
    ///     let a = ab[0];
    ///     let b = ab[1];
    ///     assert_eq!(a.cmp(b), Ordering::Less, "strings: {} < {}", a, b);
    ///     let a_loc: DataLocale = a.parse().unwrap();
    ///     assert_eq!(
    ///         a_loc.strict_cmp(a.as_bytes()),
    ///         Ordering::Equal,
    ///         "strict_cmp: {} == {}",
    ///         a_loc,
    ///         a
    ///     );
    ///     assert_eq!(
    ///         a_loc.strict_cmp(b.as_bytes()),
    ///         Ordering::Less,
    ///         "strict_cmp: {} < {}",
    ///         a_loc,
    ///         b
    ///     );
    ///     let b_loc: DataLocale = b.parse().unwrap();
    ///     assert_eq!(
    ///         b_loc.strict_cmp(b.as_bytes()),
    ///         Ordering::Equal,
    ///         "strict_cmp: {} == {}",
    ///         b_loc,
    ///         b
    ///     );
    ///     assert_eq!(
    ///         b_loc.strict_cmp(a.as_bytes()),
    ///         Ordering::Greater,
    ///         "strict_cmp: {} > {}",
    ///         b_loc,
    ///         a
    ///     );
    /// }
    /// ```
    ///
    /// Comparison against invalid strings:
    ///
    /// ```
    /// use icu_provider::DataLocale;
    ///
    /// let invalid_strings: &[&str] = &[
    ///     // Less than "ca-ES"
    ///     "CA",
    ///     "ar-x-gbp-FOO",
    ///     // Greater than "ca-AR"
    ///     "ca_ES",
    ///     "ca-ES-x-gbp-FOO",
    /// ];
    ///
    /// let data_locale = "ca-ES".parse::<DataLocale>().unwrap();
    ///
    /// for s in invalid_strings.iter() {
    ///     let expected_ordering = "ca-AR".cmp(s);
    ///     let actual_ordering = data_locale.strict_cmp(s.as_bytes());
    ///     assert_eq!(expected_ordering, actual_ordering, "{}", s);
    /// }
    /// ```
    pub fn strict_cmp(&self, other: &[u8]) -> Ordering {
        writeable::cmp_utf8(self, other)
    }

    /// Returns whether this [`DataLocale`] is `und` in the locale and extensions portion.
    ///
    /// # Examples
    ///
    /// ```
    /// use icu_provider::DataLocale;
    ///
    /// assert!("und".parse::<DataLocale>().unwrap().is_unknown());
    /// assert!(!"de-u-sd-denw".parse::<DataLocale>().unwrap().is_unknown());
    /// assert!(!"und-ES".parse::<DataLocale>().unwrap().is_unknown());
    /// ```
    pub fn is_unknown(&self) -> bool {
        self.language.is_unknown()
            && self.script.is_none()
            && self.region.is_none()
            && self.variant.is_none()
            && self.subdivision.is_none()
    }

    /// Converts this `DataLocale` into a [`Locale`].
    pub fn into_locale(self) -> Locale {
        Locale {
            id: LanguageIdentifier {
                language: self.language,
                script: self.script,
                region: self.region,
                variants: self
                    .variant
                    .map(crate::subtags::Variants::from_variant)
                    .unwrap_or_default(),
            },
            extensions: {
                let mut extensions = crate::extensions::Extensions::default();
                if let Some(sd) = self.subdivision {
                    extensions.unicode = unicode_ext::Unicode {
                        keywords: unicode_ext::Keywords::new_single(
                            unicode_ext::key!("sd"),
                            unicode_ext::Value::from_subtag(Some(sd)),
                        ),
                        ..Default::default()
                    }
                }
                extensions
            },
        }
    }
}

#[test]
fn test_data_locale_to_string() {
    struct TestCase {
        pub locale: &'static str,
        pub expected: &'static str,
    }

    for cas in [
        TestCase {
            locale: "und",
            expected: "und",
        },
        TestCase {
            locale: "und-u-sd-sdd",
            expected: "und-u-sd-sdd",
        },
        TestCase {
            locale: "en-ZA-u-sd-zaa",
            expected: "en-ZA-u-sd-zaa",
        },
    ] {
        let locale = cas.locale.parse::<DataLocale>().unwrap();
        writeable::assert_writeable_eq!(locale, cas.expected);
    }
}

#[test]
fn test_data_locale_from_string() {
    #[derive(Debug)]
    struct TestCase {
        pub input: &'static str,
        pub success: bool,
    }

    for cas in [
        TestCase {
            input: "und",
            success: true,
        },
        TestCase {
            input: "und-u-cu-gbp",
            success: false,
        },
        TestCase {
            input: "en-ZA-u-sd-zaa",
            success: true,
        },
        TestCase {
            input: "en...",
            success: false,
        },
    ] {
        let data_locale = match (DataLocale::from_str(cas.input), cas.success) {
            (Ok(l), true) => l,
            (Err(_), false) => {
                continue;
            }
            (Ok(_), false) => {
                panic!("DataLocale parsed but it was supposed to fail: {cas:?}");
            }
            (Err(_), true) => {
                panic!("DataLocale was supposed to parse but it failed: {cas:?}");
            }
        };
        writeable::assert_writeable_eq!(data_locale, cas.input);
    }
}
