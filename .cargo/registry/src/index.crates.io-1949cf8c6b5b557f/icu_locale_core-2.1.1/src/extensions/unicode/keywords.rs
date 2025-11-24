// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use core::borrow::Borrow;
use core::cmp::Ordering;
#[cfg(feature = "alloc")]
use core::iter::FromIterator;
#[cfg(feature = "alloc")]
use core::str::FromStr;
use litemap::LiteMap;

use super::Key;
use super::Value;
#[cfg(feature = "alloc")]
use crate::parser::ParseError;
#[cfg(feature = "alloc")]
use crate::parser::SubtagIterator;
use crate::shortvec::ShortBoxSlice;

/// A list of [`Key`]-[`Value`] pairs representing functional information
/// about locale's internationalization preferences.
///
/// Here are examples of fields used in Unicode:
/// - `hc` - Hour Cycle (`h11`, `h12`, `h23`, `h24`)
/// - `ca` - Calendar (`buddhist`, `gregory`, ...)
/// - `fw` - First Day Of the Week (`sun`, `mon`, `sat`, ...)
///
/// You can find the full list in [`Unicode BCP 47 U Extension`] section of LDML.
///
/// [`Unicode BCP 47 U Extension`]: https://unicode.org/reports/tr35/tr35.html#Key_And_Type_Definitions_
///
/// # Examples
///
/// Manually build up a [`Keywords`] object:
///
/// ```
/// use icu::locale::extensions::unicode::{key, value, Keywords};
///
/// let keywords = [(key!("hc"), value!("h23"))]
///     .into_iter()
///     .collect::<Keywords>();
///
/// assert_eq!(&keywords.to_string(), "hc-h23");
/// ```
///
/// Access a [`Keywords`] object from a [`Locale`]:
///
/// ```
/// use icu::locale::{
///     extensions::unicode::{key, value},
///     Locale,
/// };
///
/// let loc: Locale = "und-u-hc-h23-kc-true".parse().expect("Valid BCP-47");
///
/// assert_eq!(loc.extensions.unicode.keywords.get(&key!("ca")), None);
/// assert_eq!(
///     loc.extensions.unicode.keywords.get(&key!("hc")),
///     Some(&value!("h23"))
/// );
/// assert_eq!(
///     loc.extensions.unicode.keywords.get(&key!("kc")),
///     Some(&value!("true"))
/// );
///
/// assert_eq!(loc.extensions.unicode.keywords.to_string(), "hc-h23-kc");
/// ```
///
/// [`Locale`]: crate::Locale
#[derive(Clone, PartialEq, Eq, Debug, Default, Hash, PartialOrd, Ord)]
pub struct Keywords(LiteMap<Key, Value, ShortBoxSlice<(Key, Value)>>);

impl Keywords {
    /// Returns a new empty list of key-value pairs. Same as [`default()`](Default::default()), but is `const`.
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::extensions::unicode::Keywords;
    ///
    /// assert_eq!(Keywords::new(), Keywords::default());
    /// ```
    #[inline]
    pub const fn new() -> Self {
        Self(LiteMap::new())
    }

    /// Create a new list of key-value pairs having exactly one pair, callable in a `const` context.
    #[inline]
    pub const fn new_single(key: Key, value: Value) -> Self {
        Self(LiteMap::from_sorted_store_unchecked(
            ShortBoxSlice::new_single((key, value)),
        ))
    }

    /// A constructor which takes a str slice, parses it and
    /// produces a well-formed [`Keywords`].
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
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
        let mut iter = SubtagIterator::new(code_units);
        Self::try_from_iter(&mut iter)
    }

    /// Returns `true` if there are no keywords.
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::locale;
    /// use icu::locale::Locale;
    ///
    /// let loc1 = Locale::try_from_str("und-t-h0-hybrid").unwrap();
    /// let loc2 = locale!("und-u-ca-buddhist");
    ///
    /// assert!(loc1.extensions.unicode.keywords.is_empty());
    /// assert!(!loc2.extensions.unicode.keywords.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns `true` if the list contains a [`Value`] for the specified [`Key`].
    ///
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::extensions::unicode::{key, value, Keywords};
    ///
    /// let keywords = [(key!("ca"), value!("gregory"))]
    ///     .into_iter()
    ///     .collect::<Keywords>();
    ///
    /// assert!(&keywords.contains_key(&key!("ca")));
    /// ```
    pub fn contains_key<Q>(&self, key: &Q) -> bool
    where
        Key: Borrow<Q>,
        Q: Ord,
    {
        self.0.contains_key(key)
    }

    /// Returns a reference to the [`Value`] corresponding to the [`Key`].
    ///
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::extensions::unicode::{key, value, Keywords};
    ///
    /// let keywords = [(key!("ca"), value!("buddhist"))]
    ///     .into_iter()
    ///     .collect::<Keywords>();
    ///
    /// assert_eq!(keywords.get(&key!("ca")), Some(&value!("buddhist")));
    /// ```
    pub fn get<Q>(&self, key: &Q) -> Option<&Value>
    where
        Key: Borrow<Q>,
        Q: Ord,
    {
        self.0.get(key)
    }

    /// Returns a mutable reference to the [`Value`] corresponding to the [`Key`].
    ///
    /// Returns `None` if the key doesn't exist or if the key has no value.
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::extensions::unicode::{key, value, Keywords};
    ///
    /// let mut keywords = [(key!("ca"), value!("buddhist"))]
    ///     .into_iter()
    ///     .collect::<Keywords>();
    ///
    /// if let Some(value) = keywords.get_mut(&key!("ca")) {
    ///     *value = value!("gregory");
    /// }
    /// assert_eq!(keywords.get(&key!("ca")), Some(&value!("gregory")));
    /// ```
    #[cfg(feature = "alloc")]
    pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut Value>
    where
        Key: Borrow<Q>,
        Q: Ord,
    {
        self.0.get_mut(key)
    }

    /// Sets the specified keyword, returning the old value if it already existed.
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::extensions::unicode::{key, value};
    /// use icu::locale::Locale;
    ///
    /// let mut loc: Locale = "und-u-hello-ca-buddhist-hc-h12"
    ///     .parse()
    ///     .expect("valid BCP-47 identifier");
    /// let old_value = loc
    ///     .extensions
    ///     .unicode
    ///     .keywords
    ///     .set(key!("ca"), value!("japanese"));
    ///
    /// assert_eq!(old_value, Some(value!("buddhist")));
    /// assert_eq!(loc, "und-u-hello-ca-japanese-hc-h12".parse().unwrap());
    /// ```
    #[cfg(feature = "alloc")]
    pub fn set(&mut self, key: Key, value: Value) -> Option<Value> {
        self.0.insert(key, value)
    }

    /// Removes the specified keyword, returning the old value if it existed.
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::extensions::unicode::key;
    /// use icu::locale::Locale;
    ///
    /// let mut loc: Locale = "und-u-hello-ca-buddhist-hc-h12"
    ///     .parse()
    ///     .expect("valid BCP-47 identifier");
    /// loc.extensions.unicode.keywords.remove(key!("ca"));
    /// assert_eq!(loc, "und-u-hello-hc-h12".parse().unwrap());
    /// ```
    #[cfg(feature = "alloc")]
    pub fn remove<Q: Borrow<Key>>(&mut self, key: Q) -> Option<Value> {
        self.0.remove(key.borrow())
    }

    /// Clears all Unicode extension keywords, leaving Unicode attributes.
    ///
    /// Returns the old Unicode extension keywords.
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::Locale;
    ///
    /// let mut loc: Locale = "und-u-hello-ca-buddhist-hc-h12".parse().unwrap();
    /// loc.extensions.unicode.keywords.clear();
    /// assert_eq!(loc, "und-u-hello".parse().unwrap());
    /// ```
    pub fn clear(&mut self) -> Self {
        core::mem::take(self)
    }

    /// Retains a subset of keywords as specified by the predicate function.
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::extensions::unicode::key;
    /// use icu::locale::Locale;
    ///
    /// let mut loc: Locale = "und-u-ca-buddhist-hc-h12-ms-metric".parse().unwrap();
    ///
    /// loc.extensions
    ///     .unicode
    ///     .keywords
    ///     .retain_by_key(|&k| k == key!("hc"));
    /// assert_eq!(loc, "und-u-hc-h12".parse().unwrap());
    ///
    /// loc.extensions
    ///     .unicode
    ///     .keywords
    ///     .retain_by_key(|&k| k == key!("ms"));
    /// assert_eq!(loc, Locale::UNKNOWN);
    /// ```
    #[cfg(feature = "alloc")]
    pub fn retain_by_key<F>(&mut self, mut predicate: F)
    where
        F: FnMut(&Key) -> bool,
    {
        self.0.retain(|k, _| predicate(k))
    }

    /// Compare this [`Keywords`] with BCP-47 bytes.
    ///
    /// The return value is equivalent to what would happen if you first converted this
    /// [`Keywords`] to a BCP-47 string and then performed a byte comparison.
    ///
    /// This function is case-sensitive and results in a *total order*, so it is appropriate for
    /// binary search. The only argument producing [`Ordering::Equal`] is `self.to_string()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::Locale;
    /// use std::cmp::Ordering;
    ///
    /// let bcp47_strings: &[&str] =
    ///     &["ca-hebrew", "ca-japanese", "ca-japanese-nu-latn", "nu-latn"];
    ///
    /// for ab in bcp47_strings.windows(2) {
    ///     let a = ab[0];
    ///     let b = ab[1];
    ///     assert!(a.cmp(b) == Ordering::Less);
    ///     let a_kwds = format!("und-u-{}", a)
    ///         .parse::<Locale>()
    ///         .unwrap()
    ///         .extensions
    ///         .unicode
    ///         .keywords;
    ///     assert!(a_kwds.strict_cmp(a.as_bytes()) == Ordering::Equal);
    ///     assert!(a_kwds.strict_cmp(b.as_bytes()) == Ordering::Less);
    /// }
    /// ```
    pub fn strict_cmp(&self, other: &[u8]) -> Ordering {
        writeable::cmp_utf8(self, other)
    }

    #[cfg(feature = "alloc")]
    pub(crate) fn try_from_iter(iter: &mut SubtagIterator) -> Result<Self, ParseError> {
        let mut keywords = LiteMap::new();

        let mut current_keyword = None;
        let mut current_value = ShortBoxSlice::new();

        while let Some(subtag) = iter.peek() {
            let slen = subtag.len();
            if slen == 2 {
                if let Some(kw) = current_keyword.take() {
                    keywords.try_insert(kw, Value::from_short_slice_unchecked(current_value));
                    current_value = ShortBoxSlice::new();
                }
                current_keyword = Some(Key::try_from_utf8(subtag)?);
            } else if current_keyword.is_some() {
                match Value::parse_subtag_from_utf8(subtag) {
                    Ok(Some(t)) => current_value.push(t),
                    Ok(None) => {}
                    Err(_) => break,
                }
            } else {
                break;
            }
            iter.next();
        }

        if let Some(kw) = current_keyword.take() {
            keywords.try_insert(kw, Value::from_short_slice_unchecked(current_value));
        }

        Ok(keywords.into())
    }

    /// Produce an ordered iterator over key-value pairs
    pub fn iter(&self) -> impl Iterator<Item = (&Key, &Value)> {
        self.0.iter()
    }

    pub(crate) fn for_each_subtag_str<E, F>(&self, f: &mut F) -> Result<(), E>
    where
        F: FnMut(&str) -> Result<(), E>,
    {
        for (k, v) in self.0.iter() {
            f(k.as_str())?;
            v.for_each_subtag_str(f)?;
        }
        Ok(())
    }

    /// Extends the `Keywords` with values from  another `Keywords`.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::locale::extensions::unicode::Keywords;
    ///
    /// let mut kw: Keywords = "ab-cd-ca-buddhist".parse().unwrap();
    /// let kw2: Keywords = "ca-gregory-hc-h12".parse().unwrap();
    ///
    /// kw.extend_from_keywords(kw2);
    ///
    /// assert_eq!(kw, "ab-cd-ca-gregory-hc-h12".parse().unwrap());
    /// ```
    #[cfg(feature = "alloc")]
    pub fn extend_from_keywords(&mut self, other: Keywords) {
        for (key, value) in other.0 {
            self.0.insert(key, value);
        }
    }

    /// This needs to be its own method to help with type inference in helpers.rs
    #[cfg(test)]
    pub(crate) fn from_tuple_vec(v: Vec<(Key, Value)>) -> Self {
        v.into_iter().collect()
    }
}

impl From<LiteMap<Key, Value, ShortBoxSlice<(Key, Value)>>> for Keywords {
    fn from(map: LiteMap<Key, Value, ShortBoxSlice<(Key, Value)>>) -> Self {
        Self(map)
    }
}

/// ✨ *Enabled with the `alloc` Cargo feature.*
#[cfg(feature = "alloc")]
impl FromIterator<(Key, Value)> for Keywords {
    fn from_iter<I: IntoIterator<Item = (Key, Value)>>(iter: I) -> Self {
        LiteMap::from_iter(iter).into()
    }
}

/// ✨ *Enabled with the `alloc` Cargo feature.*
#[cfg(feature = "alloc")]
impl FromStr for Keywords {
    type Err = ParseError;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from_str(s)
    }
}

impl_writeable_for_key_value!(Keywords, "ca", "islamic-civil", "mm", "mm");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keywords_fromstr() {
        let kw: Keywords = "hc-h12".parse().expect("Failed to parse Keywords");
        assert_eq!(kw.to_string(), "hc-h12");
    }
}
