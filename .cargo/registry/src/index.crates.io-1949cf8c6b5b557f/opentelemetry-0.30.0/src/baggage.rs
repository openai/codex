//! Primitives for sending name/value data across system boundaries.
//!
//! Baggage is used to annotate telemetry, adding context and information to
//! metrics, traces, and logs. It is a set of name/value pairs describing
//! user-defined properties. Each name in Baggage is associated with exactly one
//! value.
//!
//! Main types in this module are:
//!
//! * [`Baggage`]: A set of name/value pairs describing user-defined properties.
//! * [`BaggageExt`]: Extensions for managing `Baggage` in a [`Context`].
//!
//! Baggage can be sent between systems using a baggage propagator in
//! accordance with the [W3C Baggage] specification.
//!
//! Note: Baggage is not automatically added to any telemetry. Users have to
//! explicitly add baggage entries to telemetry items.
//!
//!
//! [W3C Baggage]: https://w3c.github.io/baggage
use crate::{Context, Key, KeyValue, StringValue};
use std::collections::hash_map::Entry;
use std::collections::{hash_map, HashMap};
use std::fmt;
use std::sync::OnceLock;

static DEFAULT_BAGGAGE: OnceLock<Baggage> = OnceLock::new();

const MAX_KEY_VALUE_PAIRS: usize = 64;
const MAX_LEN_OF_ALL_PAIRS: usize = 8192;

// https://datatracker.ietf.org/doc/html/rfc7230#section-3.2.6
const INVALID_ASCII_KEY_CHARS: [u8; 17] = [
    b'(', b')', b',', b'/', b':', b';', b'<', b'=', b'>', b'?', b'@', b'[', b'\\', b']', b'{',
    b'}', b'"',
];

/// Returns the default baggage, ensuring it is initialized only once.
#[inline]
fn get_default_baggage() -> &'static Baggage {
    DEFAULT_BAGGAGE.get_or_init(Baggage::default)
}

/// A set of name/value pairs describing user-defined properties.
///
/// ### Baggage Names
///
/// * ASCII strings according to the token format, defined in [RFC2616, Section 2.2]
///
/// ### Baggage Values
///
/// * URL encoded UTF-8 strings.
///
/// ### Baggage Value Metadata
///
/// Additional metadata can be added to values in the form of a property set,
/// represented as semi-colon `;` delimited list of names and/or name/value pairs,
/// e.g. `;k1=v1;k2;k3=v3`.
///
/// ### Limits
///
/// * Maximum number of name/value pairs: `64`.
/// * Maximum total length of all name/value pairs: `8192`.
///
/// <https://www.w3.org/TR/baggage/#limits>
#[derive(Debug, Default)]
pub struct Baggage {
    inner: HashMap<Key, (StringValue, BaggageMetadata)>,
    kv_content_len: usize, // the length of key-value-metadata string in `inner`
}

impl Baggage {
    /// Creates an empty `Baggage`.
    pub fn new() -> Self {
        Baggage {
            inner: HashMap::default(),
            kv_content_len: 0,
        }
    }

    /// Returns a reference to the value associated with a given name
    ///
    /// # Examples
    ///
    /// ```
    /// use opentelemetry::{baggage::Baggage, StringValue};
    ///
    /// let mut baggage = Baggage::new();
    /// let _ = baggage.insert("my-name", "my-value");
    ///
    /// assert_eq!(baggage.get("my-name"), Some(&StringValue::from("my-value")))
    /// ```
    pub fn get<K: AsRef<str>>(&self, key: K) -> Option<&StringValue> {
        self.inner.get(key.as_ref()).map(|(value, _metadata)| value)
    }

    /// Returns a reference to the value and metadata associated with a given name
    ///
    /// # Examples
    /// ```
    /// use opentelemetry::{baggage::{Baggage, BaggageMetadata}, StringValue};
    ///
    /// let mut baggage = Baggage::new();
    /// let _ = baggage.insert("my-name", "my-value");
    ///
    /// // By default, the metadata is empty
    /// assert_eq!(baggage.get_with_metadata("my-name"), Some(&(StringValue::from("my-value"), BaggageMetadata::from(""))))
    /// ```
    pub fn get_with_metadata<K: AsRef<str>>(
        &self,
        key: K,
    ) -> Option<&(StringValue, BaggageMetadata)> {
        self.inner.get(key.as_ref())
    }

    /// Inserts a name/value pair into the baggage.
    ///
    /// If the name was not present, [`None`] is returned. If the name was present,
    /// the value is updated, and the old value is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use opentelemetry::{baggage::Baggage, StringValue};
    ///
    /// let mut baggage = Baggage::new();
    /// let _ = baggage.insert("my-name", "my-value");
    ///
    /// assert_eq!(baggage.get("my-name"), Some(&StringValue::from("my-value")))
    /// ```
    pub fn insert<K, V>(&mut self, key: K, value: V) -> Option<StringValue>
    where
        K: Into<Key>,
        V: Into<StringValue>,
    {
        self.insert_with_metadata(key, value, BaggageMetadata::default())
            .map(|pair| pair.0)
    }

    /// Inserts a name/value(+metadata) pair into the baggage.
    ///
    /// Same with `insert`, if the name was not present, [`None`] will be returned.
    /// If the name is present, the old value and metadata will be returned.
    ///
    /// Also checks for [limits](https://w3c.github.io/baggage/#limits).
    ///
    /// # Examples
    ///
    /// ```
    /// use opentelemetry::{baggage::{Baggage, BaggageMetadata}, StringValue};
    ///
    /// let mut baggage = Baggage::new();
    /// let _ = baggage.insert_with_metadata("my-name", "my-value", "test");
    ///
    /// assert_eq!(baggage.get_with_metadata("my-name"), Some(&(StringValue::from("my-value"), BaggageMetadata::from("test"))))
    /// ```
    pub fn insert_with_metadata<K, V, S>(
        &mut self,
        key: K,
        value: V,
        metadata: S,
    ) -> Option<(StringValue, BaggageMetadata)>
    where
        K: Into<Key>,
        V: Into<StringValue>,
        S: Into<BaggageMetadata>,
    {
        let (key, value, metadata) = (key.into(), value.into(), metadata.into());
        let entries_count = self.inner.len();
        match self.inner.entry(key) {
            Entry::Occupied(mut occupied_entry) => {
                let key_str = occupied_entry.key().as_str();
                let entry_content_len =
                    key_value_metadata_bytes_size(key_str, value.as_str(), metadata.as_str());
                let prev_content_len = key_value_metadata_bytes_size(
                    key_str,
                    occupied_entry.get().0.as_str(),
                    occupied_entry.get().1.as_str(),
                );
                let new_content_len = self.kv_content_len + entry_content_len - prev_content_len;
                if new_content_len > MAX_LEN_OF_ALL_PAIRS {
                    return None;
                }
                self.kv_content_len = new_content_len;
                Some(occupied_entry.insert((value, metadata)))
            }
            Entry::Vacant(vacant_entry) => {
                let key_str = vacant_entry.key().as_str();
                if !Self::is_key_valid(key_str.as_bytes()) {
                    return None;
                }
                if entries_count == MAX_KEY_VALUE_PAIRS {
                    return None;
                }
                let entry_content_len =
                    key_value_metadata_bytes_size(key_str, value.as_str(), metadata.as_str());
                let new_content_len = self.kv_content_len + entry_content_len;
                if new_content_len > MAX_LEN_OF_ALL_PAIRS {
                    return None;
                }
                self.kv_content_len = new_content_len;
                vacant_entry.insert((value, metadata));
                None
            }
        }
    }

    /// Removes a name from the baggage, returning the value
    /// corresponding to the name if the pair was previously in the map.
    pub fn remove<K: AsRef<str>>(&mut self, key: K) -> Option<(StringValue, BaggageMetadata)> {
        self.inner.remove(key.as_ref())
    }

    /// Returns the number of attributes for this baggage
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns `true` if the baggage contains no items.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Gets an iterator over the baggage items, in any order.
    pub fn iter(&self) -> Iter<'_> {
        self.into_iter()
    }

    fn is_key_valid(key: &[u8]) -> bool {
        !key.is_empty()
            && key
                .iter()
                .all(|b| b.is_ascii_graphic() && !INVALID_ASCII_KEY_CHARS.contains(b))
    }
}

/// Get the number of bytes for one key-value pair
fn key_value_metadata_bytes_size(key: &str, value: &str, metadata: &str) -> usize {
    key.len() + value.len() + metadata.len()
}

/// An iterator over the entries of a [`Baggage`].
#[derive(Debug)]
pub struct Iter<'a>(hash_map::Iter<'a, Key, (StringValue, BaggageMetadata)>);

impl<'a> Iterator for Iter<'a> {
    type Item = (&'a Key, &'a (StringValue, BaggageMetadata));

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl<'a> IntoIterator for &'a Baggage {
    type Item = (&'a Key, &'a (StringValue, BaggageMetadata));
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        Iter(self.inner.iter())
    }
}

impl FromIterator<(Key, (StringValue, BaggageMetadata))> for Baggage {
    fn from_iter<I: IntoIterator<Item = (Key, (StringValue, BaggageMetadata))>>(iter: I) -> Self {
        let mut baggage = Baggage::default();
        for (key, (value, metadata)) in iter.into_iter() {
            baggage.insert_with_metadata(key, value, metadata);
        }
        baggage
    }
}

impl FromIterator<KeyValue> for Baggage {
    fn from_iter<I: IntoIterator<Item = KeyValue>>(iter: I) -> Self {
        let mut baggage = Baggage::default();
        for kv in iter.into_iter() {
            baggage.insert(kv.key, kv.value);
        }
        baggage
    }
}

impl FromIterator<KeyValueMetadata> for Baggage {
    fn from_iter<I: IntoIterator<Item = KeyValueMetadata>>(iter: I) -> Self {
        let mut baggage = Baggage::default();
        for kvm in iter.into_iter() {
            baggage.insert_with_metadata(kvm.key, kvm.value, kvm.metadata);
        }
        baggage
    }
}

impl<I> From<I> for Baggage
where
    I: IntoIterator,
    I::Item: Into<KeyValueMetadata>,
{
    fn from(value: I) -> Self {
        value.into_iter().map(Into::into).collect()
    }
}

fn encode(s: &str) -> String {
    let mut encoded_string = String::with_capacity(s.len());

    for byte in s.as_bytes() {
        match *byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'.' | b'-' | b'_' | b'~' => {
                encoded_string.push(*byte as char)
            }
            b' ' => encoded_string.push_str("%20"),
            _ => encoded_string.push_str(&format!("%{:02X}", byte)),
        }
    }
    encoded_string
}

impl fmt::Display for Baggage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, (k, v)) in self.into_iter().enumerate() {
            write!(f, "{}={}", k, encode(v.0.as_str()))?;
            if !v.1.as_str().is_empty() {
                write!(f, ";{}", v.1)?;
            }

            if i < self.len() - 1 {
                write!(f, ",")?;
            }
        }

        Ok(())
    }
}

/// Methods for sorting and retrieving baggage data in a context.
pub trait BaggageExt {
    /// Returns a clone of the given context with the included name/value pairs.
    ///
    /// # Examples
    ///
    /// ```
    /// use opentelemetry::{baggage::{Baggage, BaggageExt}, Context, KeyValue, StringValue};
    ///
    /// // Explicit `Baggage` creation
    /// let mut baggage = Baggage::new();
    /// let _ = baggage.insert("my-name", "my-value");
    ///
    /// let cx = Context::map_current(|cx| {
    ///     cx.with_baggage(baggage)
    /// });
    ///
    /// // Passing an iterator
    /// let cx = Context::map_current(|cx| {
    ///     cx.with_baggage([KeyValue::new("my-name", "my-value")])
    /// });
    ///
    /// assert_eq!(
    ///     cx.baggage().get("my-name"),
    ///     Some(&StringValue::from("my-value")),
    /// )
    /// ```
    fn with_baggage<T: Into<Baggage>>(&self, baggage: T) -> Self;

    /// Returns a clone of the current context with the included name/value pairs.
    ///
    /// # Examples
    ///
    /// ```
    /// use opentelemetry::{baggage::{Baggage, BaggageExt}, Context, StringValue};
    ///
    /// let mut baggage = Baggage::new();
    /// let _ = baggage.insert("my-name", "my-value");
    ///
    /// let cx = Context::current_with_baggage(baggage);
    ///
    /// assert_eq!(
    ///     cx.baggage().get("my-name"),
    ///     Some(&StringValue::from("my-value")),
    /// )
    /// ```
    fn current_with_baggage<T: Into<Baggage>>(baggage: T) -> Self;

    /// Returns a clone of the given context with no baggage.
    ///
    /// # Examples
    ///
    /// ```
    /// use opentelemetry::{baggage::BaggageExt, Context};
    ///
    /// let cx = Context::map_current(|cx| cx.with_cleared_baggage());
    ///
    /// assert_eq!(cx.baggage().len(), 0);
    /// ```
    fn with_cleared_baggage(&self) -> Self;

    /// Returns a reference to this context's baggage, or the default
    /// empty baggage if none has been set.
    fn baggage(&self) -> &Baggage;
}

/// Solely used to store `Baggage` in the `Context` without allowing direct access
#[derive(Debug)]
struct BaggageContextValue(Baggage);

impl BaggageExt for Context {
    fn with_baggage<T: Into<Baggage>>(&self, baggage: T) -> Self {
        self.with_value(BaggageContextValue(baggage.into()))
    }

    fn current_with_baggage<T: Into<Baggage>>(baggage: T) -> Self {
        Context::map_current(|cx| cx.with_baggage(baggage))
    }

    fn with_cleared_baggage(&self) -> Self {
        self.with_value(Baggage::new())
    }

    fn baggage(&self) -> &Baggage {
        self.get::<BaggageContextValue>()
            .map_or(get_default_baggage(), |b| &b.0)
    }
}

/// An optional property set that can be added to [`Baggage`] values.
///
/// `BaggageMetadata` can be added to values in the form of a property set,
/// represented as semi-colon `;` delimited list of names and/or name/value
/// pairs, e.g. `;k1=v1;k2;k3=v3`.
#[derive(Clone, Debug, PartialOrd, PartialEq, Eq, Default)]
pub struct BaggageMetadata(String);

impl BaggageMetadata {
    /// Return underlying string
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl From<String> for BaggageMetadata {
    fn from(s: String) -> BaggageMetadata {
        BaggageMetadata(s.trim().to_string())
    }
}

impl From<&str> for BaggageMetadata {
    fn from(s: &str) -> Self {
        BaggageMetadata(s.trim().to_string())
    }
}

impl fmt::Display for BaggageMetadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Ok(write!(f, "{}", self.as_str())?)
    }
}

/// [`Baggage`] name/value pairs with their associated metadata.
#[derive(Clone, Debug, PartialEq)]
pub struct KeyValueMetadata {
    /// Dimension or event key
    pub(crate) key: Key,
    /// Dimension or event value
    pub(crate) value: StringValue,
    /// Metadata associate with this key value pair
    pub(crate) metadata: BaggageMetadata,
}

impl KeyValueMetadata {
    /// Create a new `KeyValue` pair with metadata
    pub fn new<K, V, S>(key: K, value: V, metadata: S) -> Self
    where
        K: Into<Key>,
        V: Into<StringValue>,
        S: Into<BaggageMetadata>,
    {
        KeyValueMetadata {
            key: key.into(),
            value: value.into(),
            metadata: metadata.into(),
        }
    }
}

impl From<KeyValue> for KeyValueMetadata {
    fn from(kv: KeyValue) -> Self {
        KeyValueMetadata {
            key: kv.key,
            value: kv.value.into(),
            metadata: BaggageMetadata::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::StringValue;

    use super::*;

    #[test]
    fn insert_non_ascii_key() {
        let mut baggage = Baggage::new();
        baggage.insert("🚫", "not ascii key");
        assert_eq!(baggage.len(), 0, "did not insert invalid key");
    }

    #[test]
    fn test_ascii_values() {
        let string1 = "test_ 123";
        let string2 = "Hello123";
        let string3 = "This & That = More";
        let string4 = "Unicode: 😊";
        let string5 = "Non-ASCII: áéíóú";
        let string6 = "Unsafe: ~!@#$%^&*()_+{}[];:'\\\"<>?,./";
        let string7: &str = "🚀Unicode:";
        let string8 = "ΑΒΓ";

        assert_eq!(encode(string1), "test_%20123");
        assert_eq!(encode(string2), "Hello123");
        assert_eq!(encode(string3), "This%20%26%20That%20%3D%20More");
        assert_eq!(encode(string4), "Unicode%3A%20%F0%9F%98%8A");
        assert_eq!(
            encode(string5),
            "Non-ASCII%3A%20%C3%A1%C3%A9%C3%AD%C3%B3%C3%BA"
        );
        assert_eq!(encode(string6), "Unsafe%3A%20~%21%40%23%24%25%5E%26%2A%28%29_%2B%7B%7D%5B%5D%3B%3A%27%5C%22%3C%3E%3F%2C.%2F");
        assert_eq!(encode(string7), "%F0%9F%9A%80Unicode%3A");
        assert_eq!(encode(string8), "%CE%91%CE%92%CE%93");
    }

    #[test]
    fn insert_too_much_baggage() {
        // too many key pairs
        let over_limit = MAX_KEY_VALUE_PAIRS + 1;
        let mut data = Vec::with_capacity(over_limit);
        for i in 0..over_limit {
            data.push(KeyValue::new(format!("key{i}"), format!("key{i}")))
        }
        let baggage = data.into_iter().collect::<Baggage>();
        assert_eq!(baggage.len(), MAX_KEY_VALUE_PAIRS)
    }

    #[test]
    fn insert_pairs_length_exceed() {
        let mut data = vec![];
        for letter in vec!['a', 'b', 'c', 'd'].into_iter() {
            data.push(KeyValue::new(
                (0..MAX_LEN_OF_ALL_PAIRS / 3)
                    .map(|_| letter)
                    .collect::<String>(),
                "",
            ));
        }
        let baggage = data.into_iter().collect::<Baggage>();
        assert_eq!(baggage.len(), 3)
    }

    #[test]
    fn serialize_baggage_as_string() {
        // Empty baggage
        let b = Baggage::default();
        assert_eq!("", b.to_string());

        // "single member empty value no properties"
        let mut b = Baggage::default();
        b.insert("foo", StringValue::from(""));
        assert_eq!("foo=", b.to_string());

        // "single member no properties"
        let mut b = Baggage::default();
        b.insert("foo", StringValue::from("1"));
        assert_eq!("foo=1", b.to_string());

        // "URL encoded value"
        let mut b = Baggage::default();
        b.insert("foo", StringValue::from("1=1"));
        assert_eq!("foo=1%3D1", b.to_string());

        // "single member empty value with properties"
        let mut b = Baggage::default();
        b.insert_with_metadata(
            "foo",
            StringValue::from(""),
            BaggageMetadata::from("red;state=on"),
        );
        assert_eq!("foo=;red;state=on", b.to_string());

        // "single member with properties"
        let mut b = Baggage::default();
        b.insert_with_metadata("foo", StringValue::from("1"), "red;state=on;z=z=z");
        assert_eq!("foo=1;red;state=on;z=z=z", b.to_string());

        // "two members with properties"
        let mut b = Baggage::default();
        b.insert_with_metadata("foo", StringValue::from("1"), "red;state=on");
        b.insert_with_metadata("bar", StringValue::from("2"), "yellow");
        assert!(b.to_string().contains("bar=2;yellow"));
        assert!(b.to_string().contains("foo=1;red;state=on"));
    }

    #[test]
    fn replace_existing_key() {
        let half_minus2: StringValue = (0..MAX_LEN_OF_ALL_PAIRS / 2 - 2)
            .map(|_| 'x')
            .collect::<String>()
            .into();

        let mut b = Baggage::default();
        b.insert("a", half_minus2.clone()); // +1 for key
        b.insert("b", half_minus2); // +1 for key
        b.insert("c", StringValue::from(".")); // total of 2 bytes
        assert!(b.get("a").is_some());
        assert!(b.get("b").is_some());
        assert!(b.get("c").is_some());
        assert!(b.insert("c", StringValue::from("..")).is_none()); // exceeds MAX_LEN_OF_ALL_PAIRS
        assert_eq!(b.insert("c", StringValue::from("!")).unwrap(), ".".into()); // replaces existing
    }

    #[test]
    fn test_crud_operations() {
        let mut baggage = Baggage::default();
        assert!(baggage.is_empty());

        // create
        baggage.insert("foo", "1");
        assert_eq!(baggage.len(), 1);

        // get
        assert_eq!(baggage.get("foo"), Some(&StringValue::from("1")));

        // update
        baggage.insert("foo", "2");
        assert_eq!(baggage.get("foo"), Some(&StringValue::from("2")));

        // delete
        baggage.remove("foo");
        assert!(baggage.is_empty());
    }

    #[test]
    fn test_insert_invalid_key() {
        let mut baggage = Baggage::default();

        // empty
        baggage.insert("", "1");
        assert!(baggage.is_empty());

        // non-ascii
        baggage.insert("Grüße", "1");
        assert!(baggage.is_empty());

        // invalid ascii chars
        baggage.insert("(example)", "1");
        assert!(baggage.is_empty());
    }
}
