use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use crate::hir;

/// An inclusive range of codepoints from a generated file (hence the static
/// lifetime).
type Range = &'static [(char, char)];

/// An error that occurs when dealing with Unicode.
///
/// We don't impl the Error trait here because these always get converted
/// into other public errors. (This error type isn't exported.)
#[derive(Debug)]
pub enum Error {
    PropertyNotFound,
    PropertyValueNotFound,
    // Not used when unicode-perl is enabled.
    #[allow(dead_code)]
    PerlClassNotFound,
}

/// An error that occurs when Unicode-aware simple case folding fails.
///
/// This error can occur when the case mapping tables necessary for Unicode
/// aware case folding are unavailable. This only occurs when the
/// `unicode-case` feature is disabled. (The feature is enabled by default.)
#[derive(Debug)]
pub struct CaseFoldError(());

#[cfg(feature = "std")]
impl std::error::Error for CaseFoldError {}

impl core::fmt::Display for CaseFoldError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "Unicode-aware case folding is not available \
             (probably because the unicode-case feature is not enabled)"
        )
    }
}

/// An error that occurs when the Unicode-aware `\w` class is unavailable.
///
/// This error can occur when the data tables necessary for the Unicode aware
/// Perl character class `\w` are unavailable. This only occurs when the
/// `unicode-perl` feature is disabled. (The feature is enabled by default.)
#[derive(Debug)]
pub struct UnicodeWordError(());

#[cfg(feature = "std")]
impl std::error::Error for UnicodeWordError {}

impl core::fmt::Display for UnicodeWordError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "Unicode-aware \\w class is not available \
             (probably because the unicode-perl feature is not enabled)"
        )
    }
}

/// A state oriented traverser of the simple case folding table.
///
/// A case folder can be constructed via `SimpleCaseFolder::new()`, which will
/// return an error if the underlying case folding table is unavailable.
///
/// After construction, it is expected that callers will use
/// `SimpleCaseFolder::mapping` by calling it with codepoints in strictly
/// increasing order. For example, calling it on `b` and then on `a` is illegal
/// and will result in a panic.
///
/// The main idea of this type is that it tries hard to make mapping lookups
/// fast by exploiting the structure of the underlying table, and the ordering
/// assumption enables this.
#[derive(Debug)]
pub struct SimpleCaseFolder {
    /// The simple case fold table. It's a sorted association list, where the
    /// keys are Unicode scalar values and the values are the corresponding
    /// equivalence class (not including the key) of the "simple" case folded
    /// Unicode scalar values.
    table: &'static [(char, &'static [char])],
    /// The last codepoint that was used for a lookup.
    last: Option<char>,
    /// The index to the entry in `table` corresponding to the smallest key `k`
    /// such that `k > k0`, where `k0` is the most recent key lookup. Note that
    /// in particular, `k0` may not be in the table!
    next: usize,
}

impl SimpleCaseFolder {
    /// Create a new simple case folder, returning an error if the underlying
    /// case folding table is unavailable.
    pub fn new() -> Result<SimpleCaseFolder, CaseFoldError> {
        #[cfg(not(feature = "unicode-case"))]
        {
            Err(CaseFoldError(()))
        }
        #[cfg(feature = "unicode-case")]
        {
            Ok(SimpleCaseFolder {
                table: crate::unicode_tables::case_folding_simple::CASE_FOLDING_SIMPLE,
                last: None,
                next: 0,
            })
        }
    }

    /// Return the equivalence class of case folded codepoints for the given
    /// codepoint. The equivalence class returned never includes the codepoint
    /// given. If the given codepoint has no case folded codepoints (i.e.,
    /// no entry in the underlying case folding table), then this returns an
    /// empty slice.
    ///
    /// # Panics
    ///
    /// This panics when called with a `c` that is less than or equal to the
    /// previous call. In other words, callers need to use this method with
    /// strictly increasing values of `c`.
    pub fn mapping(&mut self, c: char) -> &'static [char] {
        if let Some(last) = self.last {
            assert!(
                last < c,
                "got codepoint U+{:X} which occurs before \
                 last codepoint U+{:X}",
                u32::from(c),
                u32::from(last),
            );
        }
        self.last = Some(c);
        if self.next >= self.table.len() {
            return &[];
        }
        let (k, v) = self.table[self.next];
        if k == c {
            self.next += 1;
            return v;
        }
        match self.get(c) {
            Err(i) => {
                self.next = i;
                &[]
            }
            Ok(i) => {
                // Since we require lookups to proceed
                // in order, anything we find should be
                // after whatever we thought might be
                // next. Otherwise, the caller is either
                // going out of order or we would have
                // found our next key at 'self.next'.
                assert!(i > self.next);
                self.next = i + 1;
                self.table[i].1
            }
        }
    }

    /// Returns true if and only if the given range overlaps with any region
    /// of the underlying case folding table. That is, when true, there exists
    /// at least one codepoint in the inclusive range `[start, end]` that has
    /// a non-trivial equivalence class of case folded codepoints. Conversely,
    /// when this returns false, all codepoints in the range `[start, end]`
    /// correspond to the trivial equivalence class of case folded codepoints,
    /// i.e., itself.
    ///
    /// This is useful to call before iterating over the codepoints in the
    /// range and looking up the mapping for each. If you know none of the
    /// mappings will return anything, then you might be able to skip doing it
    /// altogether.
    ///
    /// # Panics
    ///
    /// This panics when `end < start`.
    pub fn overlaps(&self, start: char, end: char) -> bool {
        use core::cmp::Ordering;

        assert!(start <= end);
        self.table
            .binary_search_by(|&(c, _)| {
                if start <= c && c <= end {
                    Ordering::Equal
                } else if c > end {
                    Ordering::Greater
                } else {
                    Ordering::Less
                }
            })
            .is_ok()
    }

    /// Returns the index at which `c` occurs in the simple case fold table. If
    /// `c` does not occur, then this returns an `i` such that `table[i-1].0 <
    /// c` and `table[i].0 > c`.
    fn get(&self, c: char) -> Result<usize, usize> {
        self.table.binary_search_by_key(&c, |&(c1, _)| c1)
    }
}

/// A query for finding a character class defined by Unicode. This supports
/// either use of a property name directly, or lookup by property value. The
/// former generally refers to Binary properties (see UTS#44, Table 8), but
/// as a special exception (see UTS#18, Section 1.2) both general categories
/// (an enumeration) and scripts (a catalog) are supported as if each of their
/// possible values were a binary property.
///
/// In all circumstances, property names and values are normalized and
/// canonicalized. That is, `GC == gc == GeneralCategory == general_category`.
///
/// The lifetime `'a` refers to the shorter of the lifetimes of property name
/// and property value.
#[derive(Debug)]
pub enum ClassQuery<'a> {
    /// Return a class corresponding to a Unicode binary property, named by
    /// a single letter.
    OneLetter(char),
    /// Return a class corresponding to a Unicode binary property.
    ///
    /// Note that, by special exception (see UTS#18, Section 1.2), both
    /// general category values and script values are permitted here as if
    /// they were a binary property.
    Binary(&'a str),
    /// Return a class corresponding to all codepoints whose property
    /// (identified by `property_name`) corresponds to the given value
    /// (identified by `property_value`).
    ByValue {
        /// A property name.
        property_name: &'a str,
        /// A property value.
        property_value: &'a str,
    },
}

impl<'a> ClassQuery<'a> {
    fn canonicalize(&self) -> Result<CanonicalClassQuery, Error> {
        match *self {
            ClassQuery::OneLetter(c) => self.canonical_binary(&c.to_string()),
            ClassQuery::Binary(name) => self.canonical_binary(name),
            ClassQuery::ByValue { property_name, property_value } => {
                let property_name = symbolic_name_normalize(property_name);
                let property_value = symbolic_name_normalize(property_value);

                let canon_name = match canonical_prop(&property_name)? {
                    None => return Err(Error::PropertyNotFound),
                    Some(canon_name) => canon_name,
                };
                Ok(match canon_name {
                    "General_Category" => {
                        let canon = match canonical_gencat(&property_value)? {
                            None => return Err(Error::PropertyValueNotFound),
                            Some(canon) => canon,
                        };
                        CanonicalClassQuery::GeneralCategory(canon)
                    }
                    "Script" => {
                        let canon = match canonical_script(&property_value)? {
                            None => return Err(Error::PropertyValueNotFound),
                            Some(canon) => canon,
                        };
                        CanonicalClassQuery::Script(canon)
                    }
                    _ => {
                        let vals = match property_values(canon_name)? {
                            None => return Err(Error::PropertyValueNotFound),
                            Some(vals) => vals,
                        };
                        let canon_val =
                            match canonical_value(vals, &property_value) {
                                None => {
                                    return Err(Error::PropertyValueNotFound)
                                }
                                Some(canon_val) => canon_val,
                            };
                        CanonicalClassQuery::ByValue {
                            property_name: canon_name,
                            property_value: canon_val,
                        }
                    }
                })
            }
        }
    }

    fn canonical_binary(
        &self,
        name: &str,
    ) -> Result<CanonicalClassQuery, Error> {
        let norm = symbolic_name_normalize(name);

        // This is a special case where 'cf' refers to the 'Format' general
        // category, but where the 'cf' abbreviation is also an abbreviation
        // for the 'Case_Folding' property. But we want to treat it as
        // a general category. (Currently, we don't even support the
        // 'Case_Folding' property. But if we do in the future, users will be
        // required to spell it out.)
        //
        // Also 'sc' refers to the 'Currency_Symbol' general category, but is
        // also the abbreviation for the 'Script' property. So we avoid calling
        // 'canonical_prop' for it too, which would erroneously normalize it
        // to 'Script'.
        //
        // Another case: 'lc' is an abbreviation for the 'Cased_Letter'
        // general category, but is also an abbreviation for the 'Lowercase_Mapping'
        // property. We don't currently support the latter, so as with 'cf'
        // above, we treat 'lc' as 'Cased_Letter'.
        if norm != "cf" && norm != "sc" && norm != "lc" {
            if let Some(canon) = canonical_prop(&norm)? {
                return Ok(CanonicalClassQuery::Binary(canon));
            }
        }
        if let Some(canon) = canonical_gencat(&norm)? {
            return Ok(CanonicalClassQuery::GeneralCategory(canon));
        }
        if let Some(canon) = canonical_script(&norm)? {
            return Ok(CanonicalClassQuery::Script(canon));
        }
        Err(Error::PropertyNotFound)
    }
}

/// Like ClassQuery, but its parameters have been canonicalized. This also
/// differentiates binary properties from flattened general categories and
/// scripts.
#[derive(Debug, Eq, PartialEq)]
enum CanonicalClassQuery {
    /// The canonical binary property name.
    Binary(&'static str),
    /// The canonical general category name.
    GeneralCategory(&'static str),
    /// The canonical script name.
    Script(&'static str),
    /// An arbitrary association between property and value, both of which
    /// have been canonicalized.
    ///
    /// Note that by construction, the property name of ByValue will never
    /// be General_Category or Script. Those two cases are subsumed by the
    /// eponymous variants.
    ByValue {
        /// The canonical property name.
        property_name: &'static str,
        /// The canonical property value.
        property_value: &'static str,
    },
}

/// Looks up a Unicode class given a query. If one doesn't exist, then
/// `None` is returned.
pub fn class(query: ClassQuery<'_>) -> Result<hir::ClassUnicode, Error> {
    use self::CanonicalClassQuery::*;

    match query.canonicalize()? {
        Binary(name) => bool_property(name),
        GeneralCategory(name) => gencat(name),
        Script(name) => script(name),
        ByValue { property_name: "Age", property_value } => {
            let mut class = hir::ClassUnicode::empty();
            for set in ages(property_value)? {
                class.union(&hir_class(set));
            }
            Ok(class)
        }
        ByValue { property_name: "Script_Extensions", property_value } => {
            script_extension(property_value)
        }
        ByValue {
            property_name: "Grapheme_Cluster_Break",
            property_value,
        } => gcb(property_value),
        ByValue { property_name: "Sentence_Break", property_value } => {
            sb(property_value)
        }
        ByValue { property_name: "Word_Break", property_value } => {
            wb(property_value)
        }
        _ => {
            // What else should we support?
            Err(Error::PropertyNotFound)
        }
    }
}

/// Returns a Unicode aware class for \w.
///
/// This returns an error if the data is not available for \w.
pub fn perl_word() -> Result<hir::ClassUnicode, Error> {
    #[cfg(not(feature = "unicode-perl"))]
    fn imp() -> Result<hir::ClassUnicode, Error> {
        Err(Error::PerlClassNotFound)
    }

    #[cfg(feature = "unicode-perl")]
    fn imp() -> Result<hir::ClassUnicode, Error> {
        use crate::unicode_tables::perl_word::PERL_WORD;
        Ok(hir_class(PERL_WORD))
    }

    imp()
}

/// Returns a Unicode aware class for \s.
///
/// This returns an error if the data is not available for \s.
pub fn perl_space() -> Result<hir::ClassUnicode, Error> {
    #[cfg(not(any(feature = "unicode-perl", feature = "unicode-bool")))]
    fn imp() -> Result<hir::ClassUnicode, Error> {
        Err(Error::PerlClassNotFound)
    }

    #[cfg(all(feature = "unicode-perl", not(feature = "unicode-bool")))]
    fn imp() -> Result<hir::ClassUnicode, Error> {
        use crate::unicode_tables::perl_space::WHITE_SPACE;
        Ok(hir_class(WHITE_SPACE))
    }

    #[cfg(feature = "unicode-bool")]
    fn imp() -> Result<hir::ClassUnicode, Error> {
        use crate::unicode_tables::property_bool::WHITE_SPACE;
        Ok(hir_class(WHITE_SPACE))
    }

    imp()
}

/// Returns a Unicode aware class for \d.
///
/// This returns an error if the data is not available for \d.
pub fn perl_digit() -> Result<hir::ClassUnicode, Error> {
    #[cfg(not(any(feature = "unicode-perl", feature = "unicode-gencat")))]
    fn imp() -> Result<hir::ClassUnicode, Error> {
        Err(Error::PerlClassNotFound)
    }

    #[cfg(all(feature = "unicode-perl", not(feature = "unicode-gencat")))]
    fn imp() -> Result<hir::ClassUnicode, Error> {
        use crate::unicode_tables::perl_decimal::DECIMAL_NUMBER;
        Ok(hir_class(DECIMAL_NUMBER))
    }

    #[cfg(feature = "unicode-gencat")]
    fn imp() -> Result<hir::ClassUnicode, Error> {
        use crate::unicode_tables::general_category::DECIMAL_NUMBER;
        Ok(hir_class(DECIMAL_NUMBER))
    }

    imp()
}

/// Build a Unicode HIR class from a sequence of Unicode scalar value ranges.
pub fn hir_class(ranges: &[(char, char)]) -> hir::ClassUnicode {
    let hir_ranges: Vec<hir::ClassUnicodeRange> = ranges
        .iter()
        .map(|&(s, e)| hir::ClassUnicodeRange::new(s, e))
        .collect();
    hir::ClassUnicode::new(hir_ranges)
}

/// Returns true only if the given codepoint is in the `\w` character class.
///
/// If the `unicode-perl` feature is not enabled, then this returns an error.
pub fn is_word_character(c: char) -> Result<bool, UnicodeWordError> {
    #[cfg(not(feature = "unicode-perl"))]
    fn imp(_: char) -> Result<bool, UnicodeWordError> {
        Err(UnicodeWordError(()))
    }

    #[cfg(feature = "unicode-perl")]
    fn imp(c: char) -> Result<bool, UnicodeWordError> {
        use crate::{is_word_byte, unicode_tables::perl_word::PERL_WORD};

        if u8::try_from(c).map_or(false, is_word_byte) {
            return Ok(true);
        }
        Ok(PERL_WORD
            .binary_search_by(|&(start, end)| {
                use core::cmp::Ordering;

                if start <= c && c <= end {
                    Ordering::Equal
                } else if start > c {
                    Ordering::Greater
                } else {
                    Ordering::Less
                }
            })
            .is_ok())
    }

    imp(c)
}

/// A mapping of property values for a specific property.
///
/// The first element of each tuple is a normalized property value while the
/// second element of each tuple is the corresponding canonical property
/// value.
type PropertyValues = &'static [(&'static str, &'static str)];

fn canonical_gencat(
    normalized_value: &str,
) -> Result<Option<&'static str>, Error> {
    Ok(match normalized_value {
        "any" => Some("Any"),
        "assigned" => Some("Assigned"),
        "ascii" => Some("ASCII"),
        _ => {
            let gencats = property_values("General_Category")?.unwrap();
            canonical_value(gencats, normalized_value)
        }
    })
}

fn canonical_script(
    normalized_value: &str,
) -> Result<Option<&'static str>, Error> {
    let scripts = property_values("Script")?.unwrap();
    Ok(canonical_value(scripts, normalized_value))
}

/// Find the canonical property name for the given normalized property name.
///
/// If no such property exists, then `None` is returned.
///
/// The normalized property name must have been normalized according to
/// UAX44 LM3, which can be done using `symbolic_name_normalize`.
///
/// If the property names data is not available, then an error is returned.
fn canonical_prop(
    normalized_name: &str,
) -> Result<Option<&'static str>, Error> {
    #[cfg(not(any(
        feature = "unicode-age",
        feature = "unicode-bool",
        feature = "unicode-gencat",
        feature = "unicode-perl",
        feature = "unicode-script",
        feature = "unicode-segment",
    )))]
    fn imp(_: &str) -> Result<Option<&'static str>, Error> {
        Err(Error::PropertyNotFound)
    }

    #[cfg(any(
        feature = "unicode-age",
        feature = "unicode-bool",
        feature = "unicode-gencat",
        feature = "unicode-perl",
        feature = "unicode-script",
        feature = "unicode-segment",
    ))]
    fn imp(name: &str) -> Result<Option<&'static str>, Error> {
        use crate::unicode_tables::property_names::PROPERTY_NAMES;

        Ok(PROPERTY_NAMES
            .binary_search_by_key(&name, |&(n, _)| n)
            .ok()
            .map(|i| PROPERTY_NAMES[i].1))
    }

    imp(normalized_name)
}

/// Find the canonical property value for the given normalized property
/// value.
///
/// The given property values should correspond to the values for the property
/// under question, which can be found using `property_values`.
///
/// If no such property value exists, then `None` is returned.
///
/// The normalized property value must have been normalized according to
/// UAX44 LM3, which can be done using `symbolic_name_normalize`.
fn canonical_value(
    vals: PropertyValues,
    normalized_value: &str,
) -> Option<&'static str> {
    vals.binary_search_by_key(&normalized_value, |&(n, _)| n)
        .ok()
        .map(|i| vals[i].1)
}

/// Return the table of property values for the given property name.
///
/// If the property values data is not available, then an error is returned.
fn property_values(
    canonical_property_name: &'static str,
) -> Result<Option<PropertyValues>, Error> {
    #[cfg(not(any(
        feature = "unicode-age",
        feature = "unicode-bool",
        feature = "unicode-gencat",
        feature = "unicode-perl",
        feature = "unicode-script",
        feature = "unicode-segment",
    )))]
    fn imp(_: &'static str) -> Result<Option<PropertyValues>, Error> {
        Err(Error::PropertyValueNotFound)
    }

    #[cfg(any(
        feature = "unicode-age",
        feature = "unicode-bool",
        feature = "unicode-gencat",
        feature = "unicode-perl",
        feature = "unicode-script",
        feature = "unicode-segment",
    ))]
    fn imp(name: &'static str) -> Result<Option<PropertyValues>, Error> {
        use crate::unicode_tables::property_values::PROPERTY_VALUES;

        Ok(PROPERTY_VALUES
            .binary_search_by_key(&name, |&(n, _)| n)
            .ok()
            .map(|i| PROPERTY_VALUES[i].1))
    }

    imp(canonical_property_name)
}

// This is only used in some cases, but small enough to just let it be dead
// instead of figuring out (and maintaining) the right set of features.
#[allow(dead_code)]
fn property_set(
    name_map: &'static [(&'static str, Range)],
    canonical: &'static str,
) -> Option<Range> {
    name_map
        .binary_search_by_key(&canonical, |x| x.0)
        .ok()
        .map(|i| name_map[i].1)
}

/// Returns an iterator over Unicode Age sets. Each item corresponds to a set
/// of codepoints that were added in a particular revision of Unicode. The
/// iterator yields items in chronological order.
///
/// If the given age value isn't valid or if the data isn't available, then an
/// error is returned instead.
fn ages(canonical_age: &str) -> Result<impl Iterator<Item = Range>, Error> {
    #[cfg(not(feature = "unicode-age"))]
    fn imp(_: &str) -> Result<impl Iterator<Item = Range>, Error> {
        use core::option::IntoIter;
        Err::<IntoIter<Range>, _>(Error::PropertyNotFound)
    }

    #[cfg(feature = "unicode-age")]
    fn imp(canonical_age: &str) -> Result<impl Iterator<Item = Range>, Error> {
        use crate::unicode_tables::age;

        const AGES: &[(&str, Range)] = &[
            ("V1_1", age::V1_1),
            ("V2_0", age::V2_0),
            ("V2_1", age::V2_1),
            ("V3_0", age::V3_0),
            ("V3_1", age::V3_1),
            ("V3_2", age::V3_2),
            ("V4_0", age::V4_0),
            ("V4_1", age::V4_1),
            ("V5_0", age::V5_0),
            ("V5_1", age::V5_1),
            ("V5_2", age::V5_2),
            ("V6_0", age::V6_0),
            ("V6_1", age::V6_1),
            ("V6_2", age::V6_2),
            ("V6_3", age::V6_3),
            ("V7_0", age::V7_0),
            ("V8_0", age::V8_0),
            ("V9_0", age::V9_0),
            ("V10_0", age::V10_0),
            ("V11_0", age::V11_0),
            ("V12_0", age::V12_0),
            ("V12_1", age::V12_1),
            ("V13_0", age::V13_0),
            ("V14_0", age::V14_0),
            ("V15_0", age::V15_0),
            ("V15_1", age::V15_1),
            ("V16_0", age::V16_0),
        ];
        assert_eq!(AGES.len(), age::BY_NAME.len(), "ages are out of sync");

        let pos = AGES.iter().position(|&(age, _)| canonical_age == age);
        match pos {
            None => Err(Error::PropertyValueNotFound),
            Some(i) => Ok(AGES[..=i].iter().map(|&(_, classes)| classes)),
        }
    }

    imp(canonical_age)
}

/// Returns the Unicode HIR class corresponding to the given general category.
///
/// Name canonicalization is assumed to be performed by the caller.
///
/// If the given general category could not be found, or if the general
/// category data is not available, then an error is returned.
fn gencat(canonical_name: &'static str) -> Result<hir::ClassUnicode, Error> {
    #[cfg(not(feature = "unicode-gencat"))]
    fn imp(_: &'static str) -> Result<hir::ClassUnicode, Error> {
        Err(Error::PropertyNotFound)
    }

    #[cfg(feature = "unicode-gencat")]
    fn imp(name: &'static str) -> Result<hir::ClassUnicode, Error> {
        use crate::unicode_tables::general_category::BY_NAME;
        match name {
            "ASCII" => Ok(hir_class(&[('\0', '\x7F')])),
            "Any" => Ok(hir_class(&[('\0', '\u{10FFFF}')])),
            "Assigned" => {
                let mut cls = gencat("Unassigned")?;
                cls.negate();
                Ok(cls)
            }
            name => property_set(BY_NAME, name)
                .map(hir_class)
                .ok_or(Error::PropertyValueNotFound),
        }
    }

    match canonical_name {
        "Decimal_Number" => perl_digit(),
        name => imp(name),
    }
}

/// Returns the Unicode HIR class corresponding to the given script.
///
/// Name canonicalization is assumed to be performed by the caller.
///
/// If the given script could not be found, or if the script data is not
/// available, then an error is returned.
fn script(canonical_name: &'static str) -> Result<hir::ClassUnicode, Error> {
    #[cfg(not(feature = "unicode-script"))]
    fn imp(_: &'static str) -> Result<hir::ClassUnicode, Error> {
        Err(Error::PropertyNotFound)
    }

    #[cfg(feature = "unicode-script")]
    fn imp(name: &'static str) -> Result<hir::ClassUnicode, Error> {
        use crate::unicode_tables::script::BY_NAME;
        property_set(BY_NAME, name)
            .map(hir_class)
            .ok_or(Error::PropertyValueNotFound)
    }

    imp(canonical_name)
}

/// Returns the Unicode HIR class corresponding to the given script extension.
///
/// Name canonicalization is assumed to be performed by the caller.
///
/// If the given script extension could not be found, or if the script data is
/// not available, then an error is returned.
fn script_extension(
    canonical_name: &'static str,
) -> Result<hir::ClassUnicode, Error> {
    #[cfg(not(feature = "unicode-script"))]
    fn imp(_: &'static str) -> Result<hir::ClassUnicode, Error> {
        Err(Error::PropertyNotFound)
    }

    #[cfg(feature = "unicode-script")]
    fn imp(name: &'static str) -> Result<hir::ClassUnicode, Error> {
        use crate::unicode_tables::script_extension::BY_NAME;
        property_set(BY_NAME, name)
            .map(hir_class)
            .ok_or(Error::PropertyValueNotFound)
    }

    imp(canonical_name)
}

/// Returns the Unicode HIR class corresponding to the given Unicode boolean
/// property.
///
/// Name canonicalization is assumed to be performed by the caller.
///
/// If the given boolean property could not be found, or if the boolean
/// property data is not available, then an error is returned.
fn bool_property(
    canonical_name: &'static str,
) -> Result<hir::ClassUnicode, Error> {
    #[cfg(not(feature = "unicode-bool"))]
    fn imp(_: &'static str) -> Result<hir::ClassUnicode, Error> {
        Err(Error::PropertyNotFound)
    }

    #[cfg(feature = "unicode-bool")]
    fn imp(name: &'static str) -> Result<hir::ClassUnicode, Error> {
        use crate::unicode_tables::property_bool::BY_NAME;
        property_set(BY_NAME, name)
            .map(hir_class)
            .ok_or(Error::PropertyNotFound)
    }

    match canonical_name {
        "Decimal_Number" => perl_digit(),
        "White_Space" => perl_space(),
        name => imp(name),
    }
}

/// Returns the Unicode HIR class corresponding to the given grapheme cluster
/// break property.
///
/// Name canonicalization is assumed to be performed by the caller.
///
/// If the given property could not be found, or if the corresponding data is
/// not available, then an error is returned.
fn gcb(canonical_name: &'static str) -> Result<hir::ClassUnicode, Error> {
    #[cfg(not(feature = "unicode-segment"))]
    fn imp(_: &'static str) -> Result<hir::ClassUnicode, Error> {
        Err(Error::PropertyNotFound)
    }

    #[cfg(feature = "unicode-segment")]
    fn imp(name: &'static str) -> Result<hir::ClassUnicode, Error> {
        use crate::unicode_tables::grapheme_cluster_break::BY_NAME;
        property_set(BY_NAME, name)
            .map(hir_class)
            .ok_or(Error::PropertyValueNotFound)
    }

    imp(canonical_name)
}

/// Returns the Unicode HIR class corresponding to the given word break
/// property.
///
/// Name canonicalization is assumed to be performed by the caller.
///
/// If the given property could not be found, or if the corresponding data is
/// not available, then an error is returned.
fn wb(canonical_name: &'static str) -> Result<hir::ClassUnicode, Error> {
    #[cfg(not(feature = "unicode-segment"))]
    fn imp(_: &'static str) -> Result<hir::ClassUnicode, Error> {
        Err(Error::PropertyNotFound)
    }

    #[cfg(feature = "unicode-segment")]
    fn imp(name: &'static str) -> Result<hir::ClassUnicode, Error> {
        use crate::unicode_tables::word_break::BY_NAME;
        property_set(BY_NAME, name)
            .map(hir_class)
            .ok_or(Error::PropertyValueNotFound)
    }

    imp(canonical_name)
}

/// Returns the Unicode HIR class corresponding to the given sentence
/// break property.
///
/// Name canonicalization is assumed to be performed by the caller.
///
/// If the given property could not be found, or if the corresponding data is
/// not available, then an error is returned.
fn sb(canonical_name: &'static str) -> Result<hir::ClassUnicode, Error> {
    #[cfg(not(feature = "unicode-segment"))]
    fn imp(_: &'static str) -> Result<hir::ClassUnicode, Error> {
        Err(Error::PropertyNotFound)
    }

    #[cfg(feature = "unicode-segment")]
    fn imp(name: &'static str) -> Result<hir::ClassUnicode, Error> {
        use crate::unicode_tables::sentence_break::BY_NAME;
        property_set(BY_NAME, name)
            .map(hir_class)
            .ok_or(Error::PropertyValueNotFound)
    }

    imp(canonical_name)
}

/// Like symbolic_name_normalize_bytes, but operates on a string.
fn symbolic_name_normalize(x: &str) -> String {
    let mut tmp = x.as_bytes().to_vec();
    let len = symbolic_name_normalize_bytes(&mut tmp).len();
    tmp.truncate(len);
    // This should always succeed because `symbolic_name_normalize_bytes`
    // guarantees that `&tmp[..len]` is always valid UTF-8.
    //
    // N.B. We could avoid the additional UTF-8 check here, but it's unlikely
    // to be worth skipping the additional safety check. A benchmark must
    // justify it first.
    String::from_utf8(tmp).unwrap()
}

/// Normalize the given symbolic name in place according to UAX44-LM3.
///
/// A "symbolic name" typically corresponds to property names and property
/// value aliases. Note, though, that it should not be applied to property
/// string values.
///
/// The slice returned is guaranteed to be valid UTF-8 for all possible values
/// of `slice`.
///
/// See: https://unicode.org/reports/tr44/#UAX44-LM3
fn symbolic_name_normalize_bytes(slice: &mut [u8]) -> &mut [u8] {
    // I couldn't find a place in the standard that specified that property
    // names/aliases had a particular structure (unlike character names), but
    // we assume that it's ASCII only and drop anything that isn't ASCII.
    let mut start = 0;
    let mut starts_with_is = false;
    if slice.len() >= 2 {
        // Ignore any "is" prefix.
        starts_with_is = slice[0..2] == b"is"[..]
            || slice[0..2] == b"IS"[..]
            || slice[0..2] == b"iS"[..]
            || slice[0..2] == b"Is"[..];
        if starts_with_is {
            start = 2;
        }
    }
    let mut next_write = 0;
    for i in start..slice.len() {
        // VALIDITY ARGUMENT: To guarantee that the resulting slice is valid
        // UTF-8, we ensure that the slice contains only ASCII bytes. In
        // particular, we drop every non-ASCII byte from the normalized string.
        let b = slice[i];
        if b == b' ' || b == b'_' || b == b'-' {
            continue;
        } else if b'A' <= b && b <= b'Z' {
            slice[next_write] = b + (b'a' - b'A');
            next_write += 1;
        } else if b <= 0x7F {
            slice[next_write] = b;
            next_write += 1;
        }
    }
    // Special case: ISO_Comment has a 'isc' abbreviation. Since we generally
    // ignore 'is' prefixes, the 'isc' abbreviation gets caught in the cross
    // fire and ends up creating an alias for 'c' to 'ISO_Comment', but it
    // is actually an alias for the 'Other' general category.
    if starts_with_is && next_write == 1 && slice[0] == b'c' {
        slice[0] = b'i';
        slice[1] = b's';
        slice[2] = b'c';
        next_write = 3;
    }
    &mut slice[..next_write]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "unicode-case")]
    fn simple_fold_ok(c: char) -> impl Iterator<Item = char> {
        SimpleCaseFolder::new().unwrap().mapping(c).iter().copied()
    }

    #[cfg(feature = "unicode-case")]
    fn contains_case_map(start: char, end: char) -> bool {
        SimpleCaseFolder::new().unwrap().overlaps(start, end)
    }

    #[test]
    #[cfg(feature = "unicode-case")]
    fn simple_fold_k() {
        let xs: Vec<char> = simple_fold_ok('k').collect();
        assert_eq!(xs, alloc::vec!['K', 'K']);

        let xs: Vec<char> = simple_fold_ok('K').collect();
        assert_eq!(xs, alloc::vec!['k', 'K']);

        let xs: Vec<char> = simple_fold_ok('K').collect();
        assert_eq!(xs, alloc::vec!['K', 'k']);
    }

    #[test]
    #[cfg(feature = "unicode-case")]
    fn simple_fold_a() {
        let xs: Vec<char> = simple_fold_ok('a').collect();
        assert_eq!(xs, alloc::vec!['A']);

        let xs: Vec<char> = simple_fold_ok('A').collect();
        assert_eq!(xs, alloc::vec!['a']);
    }

    #[test]
    #[cfg(not(feature = "unicode-case"))]
    fn simple_fold_disabled() {
        assert!(SimpleCaseFolder::new().is_err());
    }

    #[test]
    #[cfg(feature = "unicode-case")]
    fn range_contains() {
        assert!(contains_case_map('A', 'A'));
        assert!(contains_case_map('Z', 'Z'));
        assert!(contains_case_map('A', 'Z'));
        assert!(contains_case_map('@', 'A'));
        assert!(contains_case_map('Z', '['));
        assert!(contains_case_map('☃', 'Ⰰ'));

        assert!(!contains_case_map('[', '['));
        assert!(!contains_case_map('[', '`'));

        assert!(!contains_case_map('☃', '☃'));
    }

    #[test]
    #[cfg(feature = "unicode-gencat")]
    fn regression_466() {
        use super::{CanonicalClassQuery, ClassQuery};

        let q = ClassQuery::OneLetter('C');
        assert_eq!(
            q.canonicalize().unwrap(),
            CanonicalClassQuery::GeneralCategory("Other")
        );
    }

    #[test]
    fn sym_normalize() {
        let sym_norm = symbolic_name_normalize;

        assert_eq!(sym_norm("Line_Break"), "linebreak");
        assert_eq!(sym_norm("Line-break"), "linebreak");
        assert_eq!(sym_norm("linebreak"), "linebreak");
        assert_eq!(sym_norm("BA"), "ba");
        assert_eq!(sym_norm("ba"), "ba");
        assert_eq!(sym_norm("Greek"), "greek");
        assert_eq!(sym_norm("isGreek"), "greek");
        assert_eq!(sym_norm("IS_Greek"), "greek");
        assert_eq!(sym_norm("isc"), "isc");
        assert_eq!(sym_norm("is c"), "isc");
        assert_eq!(sym_norm("is_c"), "isc");
    }

    #[test]
    fn valid_utf8_symbolic() {
        let mut x = b"abc\xFFxyz".to_vec();
        let y = symbolic_name_normalize_bytes(&mut x);
        assert_eq!(y, b"abcxyz");
    }
}
