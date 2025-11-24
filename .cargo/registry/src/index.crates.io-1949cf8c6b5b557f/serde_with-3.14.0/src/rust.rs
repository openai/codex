//! De/Serialization for Rust's builtin and std types

use crate::prelude::*;

/// Makes a distinction between a missing, unset, or existing value
///
/// Some serialization formats make a distinction between missing fields, fields with a `null`
/// value, and existing values. One such format is JSON. By default it is not easily possible to
/// differentiate between a missing value and a field which is `null`, as they deserialize to the
/// same value. This helper changes it, by using an `Option<Option<T>>` to deserialize into.
///
/// * `None`: Represents a missing value.
/// * `Some(None)`: Represents a `null` value.
/// * `Some(Some(value))`: Represents an existing value.
///
/// Note: This cannot be made compatible to `serde_as`, since skipping of values is only available on the field level.
/// A hypothetical `DoubleOption<T>` with a `SerializeAs` implementation would allow writing something like this.
/// This cannot work, since there is no way to tell the `Vec` to skip the inner `DoubleOption` if it is `None`.
///
/// ```rust
/// # #[cfg(any())] {
/// # struct Foobar {
/// #[serde_as(as = "Vec<DoubleOption<_>>")]
/// data: Vec<Option<Option<i32>>>,
/// # }
/// # }
/// ```
///
/// # Examples
///
/// ```rust
/// # use serde::{Deserialize, Serialize};
/// #
/// # #[derive(Debug, PartialEq, Eq)]
/// #[derive(Deserialize, Serialize)]
/// struct Doc {
///     #[serde(
///         default,                                    // <- important for deserialization
///         skip_serializing_if = "Option::is_none",    // <- important for serialization
///         with = "::serde_with::rust::double_option",
///     )]
///     a: Option<Option<u8>>,
/// }
/// // Missing Value
/// let s = r#"{}"#;
/// assert_eq!(Doc { a: None }, serde_json::from_str(s).unwrap());
/// assert_eq!(s, serde_json::to_string(&Doc { a: None }).unwrap());
///
/// // Unset Value
/// let s = r#"{"a":null}"#;
/// assert_eq!(Doc { a: Some(None) }, serde_json::from_str(s).unwrap());
/// assert_eq!(s, serde_json::to_string(&Doc { a: Some(None) }).unwrap());
///
/// // Existing Value
/// let s = r#"{"a":5}"#;
/// assert_eq!(Doc { a: Some(Some(5)) }, serde_json::from_str(s).unwrap());
/// assert_eq!(s, serde_json::to_string(&Doc { a: Some(Some(5)) }).unwrap());
/// ```
#[allow(clippy::option_option)]
pub mod double_option {
    use super::*;

    /// Deserialize potentially non-existing optional value
    pub fn deserialize<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
    where
        T: Deserialize<'de>,
        D: Deserializer<'de>,
    {
        Deserialize::deserialize(deserializer).map(Some)
    }

    /// Serialize optional value
    pub fn serialize<S, T>(values: &Option<Option<T>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: Serialize,
    {
        match values {
            None => serializer.serialize_unit(),
            Some(None) => serializer.serialize_none(),
            Some(Some(v)) => serializer.serialize_some(&v),
        }
    }
}

/// Serialize inner value if [`Some`]`(T)`. If [`None`], serialize the unit struct `()`.
///
/// When used in conjunction with `skip_serializing_if = "Option::is_none"` and
/// `default`, you can build an optional value by skipping if it is [`None`], or serializing its
/// inner value if [`Some`]`(T)`.
///
/// Not all serialization formats easily support optional values.
/// While JSON uses the [`Option`] type to represent optional values and only serializes the inner
/// part of the [`Some`]`()`, other serialization formats, such as [RON][], choose to serialize the
/// [`Some`] around a value.
/// This helper helps building a truly optional value for such serializers.
///
/// [RON]: https://github.com/ron-rs/ron
///
/// # Example
///
/// ```rust
/// # use serde::{Deserialize, Serialize};
/// #
/// # #[derive(Debug, Eq, PartialEq)]
/// #[derive(Deserialize, Serialize)]
/// struct Doc {
///     mandatory: usize,
///     #[serde(
///         default,                                    // <- important for deserialization
///         skip_serializing_if = "Option::is_none",    // <- important for serialization
///         with = "::serde_with::rust::unwrap_or_skip",
///     )]
///     optional: Option<usize>,
/// }
///
/// // Transparently add/remove Some() wrapper
/// # let pretty_config = ron::ser::PrettyConfig::new().new_line("\n");
/// let s = r#"(
///     mandatory: 1,
///     optional: 2,
/// )"#;
/// let v = Doc {
///     mandatory: 1,
///     optional: Some(2),
/// };
/// assert_eq!(v, ron::de::from_str(s).unwrap());
/// assert_eq!(s, ron::ser::to_string_pretty(&v, pretty_config).unwrap());
///
/// // Missing values are deserialized as `None`
/// // while `None` values are skipped during serialization.
/// # let pretty_config = ron::ser::PrettyConfig::new().new_line("\n");
/// let s = r#"(
///     mandatory: 1,
/// )"#;
/// let v = Doc {
///     mandatory: 1,
///     optional: None,
/// };
/// assert_eq!(v, ron::de::from_str(s).unwrap());
/// assert_eq!(s, ron::ser::to_string_pretty(&v, pretty_config).unwrap());
/// ```
pub mod unwrap_or_skip {
    use super::*;

    /// Deserialize value wrapped in Some(T)
    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
    where
        D: Deserializer<'de>,
        T: Deserialize<'de>,
    {
        T::deserialize(deserializer).map(Some)
    }

    /// Serialize value if Some(T), unit struct if None
    pub fn serialize<T, S>(option: &Option<T>, serializer: S) -> Result<S::Ok, S::Error>
    where
        T: Serialize,
        S: Serializer,
    {
        if let Some(value) = option {
            value.serialize(serializer)
        } else {
            ().serialize(serializer)
        }
    }
}

/// Ensure no duplicate values exist in a set.
///
/// By default serde has a last-value-wins implementation, if duplicate values for a set exist.
/// Sometimes it is desirable to know when such an event happens, as the first value is overwritten
/// and it can indicate an error in the serialized data.
///
/// This helper returns an error if two identical values exist in a set.
///
/// The implementation supports both the [`HashSet`] and the [`BTreeSet`] from the standard library.
///
/// # Converting to `serde_as`
///
/// The same functionality can be more clearly expressed using the `serde_as` macro and [`SetPreventDuplicates`].
/// The `_` is a placeholder which works for any type which implements [`Serialize`]/[`Deserialize`].
///
/// ```rust
/// # #[cfg(any())] {
/// #[serde_as]
/// #[derive(Deserialize, Serialize)]
/// struct A {
///     #[serde_as(as = "SetPreventDuplicates<_, _>")]
///     s: HashSet<usize>,
/// }
/// # }
/// ```
///
/// [`HashSet`]: std::collections::HashSet
/// [`BTreeSet`]: std::collections::HashSet
///
/// # Example
///
/// ```rust
/// # use std::collections::HashSet;
/// # use serde::Deserialize;
/// #
/// # #[derive(Debug, Eq, PartialEq)]
/// #[derive(Deserialize)]
/// struct Doc {
///     #[serde(with = "::serde_with::rust::sets_duplicate_value_is_error")]
///     set: HashSet<usize>,
/// }
///
/// // Sets are serialized normally,
/// let s = r#"{"set": [1, 2, 3, 4]}"#;
/// let v = Doc {
///     set: HashSet::from_iter(vec![1, 2, 3, 4]),
/// };
/// assert_eq!(v, serde_json::from_str(s).unwrap());
///
/// // but create an error if duplicate values, like the `1`, exist.
/// let s = r#"{"set": [1, 2, 3, 4, 1]}"#;
/// let res: Result<Doc, _> = serde_json::from_str(s);
/// assert!(res.is_err());
/// ```
#[cfg(feature = "alloc")]
pub mod sets_duplicate_value_is_error {
    use super::*;
    use crate::duplicate_key_impls::PreventDuplicateInsertsSet;

    /// Deserialize a set and return an error on duplicate values
    pub fn deserialize<'de, D, T, V>(deserializer: D) -> Result<T, D::Error>
    where
        T: PreventDuplicateInsertsSet<V>,
        V: Deserialize<'de>,
        D: Deserializer<'de>,
    {
        struct SeqVisitor<T, V> {
            marker: PhantomData<T>,
            set_item_type: PhantomData<V>,
        }

        impl<'de, T, V> Visitor<'de> for SeqVisitor<T, V>
        where
            T: PreventDuplicateInsertsSet<V>,
            V: Deserialize<'de>,
        {
            type Value = T;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a sequence")
            }

            #[inline]
            fn visit_seq<A>(self, mut access: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut values = Self::Value::new(access.size_hint());

                while let Some(value) = access.next_element()? {
                    if !values.insert(value) {
                        return Err(DeError::custom("invalid entry: found duplicate value"));
                    };
                }

                Ok(values)
            }
        }

        let visitor = SeqVisitor {
            marker: PhantomData,
            set_item_type: PhantomData,
        };
        deserializer.deserialize_seq(visitor)
    }

    /// Serialize the set with the default serializer
    pub fn serialize<T, S>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        T: Serialize,
        S: Serializer,
    {
        value.serialize(serializer)
    }
}

/// Ensure no duplicate keys exist in a map.
///
/// By default serde has a last-value-wins implementation, if duplicate keys for a map exist.
/// Sometimes it is desirable to know when such an event happens, as the first value is overwritten
/// and it can indicate an error in the serialized data.
///
/// This helper returns an error if two identical keys exist in a map.
///
/// The implementation supports both the [`HashMap`] and the [`BTreeMap`] from the standard library.
///
/// # Converting to `serde_as`
///
/// The same functionality can be more clearly expressed using the `serde_as` macro and [`MapPreventDuplicates`].
/// The `_` is a placeholder which works for any type which implements [`Serialize`]/[`Deserialize`].
///
/// ```rust
/// # #[cfg(any())] {
/// #[serde_as]
/// #[derive(Deserialize, Serialize)]
/// struct A {
///     #[serde_as(as = "MapPreventDuplicates<_, _>")]
///     s: HashMap<usize, usize>,
/// }
/// # }
/// ```
///
/// [`HashMap`]: std::collections::HashMap
/// [`BTreeMap`]: std::collections::HashMap
///
/// # Example
///
/// ```rust
/// # use serde::Deserialize;
/// # use std::collections::HashMap;
/// #
/// # #[derive(Debug, Eq, PartialEq)]
/// #[derive(Deserialize)]
/// struct Doc {
///     #[serde(with = "::serde_with::rust::maps_duplicate_key_is_error")]
///     map: HashMap<usize, usize>,
/// }
///
/// // Maps are serialized normally,
/// let s = r#"{"map": {"1": 1, "2": 2, "3": 3}}"#;
/// let mut v = Doc {
///     map: HashMap::new(),
/// };
/// v.map.insert(1, 1);
/// v.map.insert(2, 2);
/// v.map.insert(3, 3);
/// assert_eq!(v, serde_json::from_str(s).unwrap());
///
/// // but create an error if duplicate keys, like the `1`, exist.
/// let s = r#"{"map": {"1": 1, "2": 2, "1": 3}}"#;
/// let res: Result<Doc, _> = serde_json::from_str(s);
/// assert!(res.is_err());
/// ```
#[cfg(feature = "alloc")]
pub mod maps_duplicate_key_is_error {
    use super::*;
    use crate::duplicate_key_impls::PreventDuplicateInsertsMap;

    /// Deserialize a map and return an error on duplicate keys
    pub fn deserialize<'de, D, T, K, V>(deserializer: D) -> Result<T, D::Error>
    where
        T: PreventDuplicateInsertsMap<K, V>,
        K: Deserialize<'de>,
        V: Deserialize<'de>,
        D: Deserializer<'de>,
    {
        struct MapVisitor<T, K, V> {
            marker: PhantomData<T>,
            map_key_type: PhantomData<K>,
            map_value_type: PhantomData<V>,
        }

        impl<'de, T, K, V> Visitor<'de> for MapVisitor<T, K, V>
        where
            T: PreventDuplicateInsertsMap<K, V>,
            K: Deserialize<'de>,
            V: Deserialize<'de>,
        {
            type Value = T;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a map")
            }

            #[inline]
            fn visit_map<A>(self, mut access: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut values = Self::Value::new(access.size_hint());

                while let Some((key, value)) = access.next_entry()? {
                    if !values.insert(key, value) {
                        return Err(DeError::custom("invalid entry: found duplicate key"));
                    };
                }

                Ok(values)
            }
        }

        let visitor = MapVisitor {
            marker: PhantomData,
            map_key_type: PhantomData,
            map_value_type: PhantomData,
        };
        deserializer.deserialize_map(visitor)
    }

    /// Serialize the map with the default serializer
    pub fn serialize<T, S>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        T: Serialize,
        S: Serializer,
    {
        value.serialize(serializer)
    }
}

/// Ensure that the last value is taken, if duplicate values exist
///
/// By default serde has a first-value-wins implementation, if duplicate keys for a set exist.
/// Sometimes the opposite strategy is desired. This helper implements a first-value-wins strategy.
///
/// The implementation supports both the [`HashSet`] and the [`BTreeSet`] from the standard library.
///
/// # Converting to `serde_as`
///
/// The same functionality can be more clearly expressed using the `serde_as` macro and [`SetLastValueWins`].
/// The `_` is a placeholder which works for any type which implements [`Serialize`]/[`Deserialize`].
///
/// ```rust
/// # #[cfg(any())] {
/// #[serde_as]
/// #[derive(Deserialize, Serialize)]
/// struct A {
///     #[serde_as(as = "SetLastValueWins<_, _>")]
///     s: HashSet<usize>,
/// }
/// # }
/// ```
///
/// [`HashSet`]: std::collections::HashSet
/// [`BTreeSet`]: std::collections::HashSet
#[cfg(feature = "alloc")]
pub mod sets_last_value_wins {
    use super::*;
    use crate::duplicate_key_impls::DuplicateInsertsLastWinsSet;

    /// Deserialize a set and keep the last of equal values
    pub fn deserialize<'de, D, T, V>(deserializer: D) -> Result<T, D::Error>
    where
        T: DuplicateInsertsLastWinsSet<V>,
        V: Deserialize<'de>,
        D: Deserializer<'de>,
    {
        struct SeqVisitor<T, V> {
            marker: PhantomData<T>,
            set_item_type: PhantomData<V>,
        }

        impl<'de, T, V> Visitor<'de> for SeqVisitor<T, V>
        where
            T: DuplicateInsertsLastWinsSet<V>,
            V: Deserialize<'de>,
        {
            type Value = T;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a sequence")
            }

            #[inline]
            fn visit_seq<A>(self, mut access: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut values = Self::Value::new(access.size_hint());

                while let Some(value) = access.next_element()? {
                    values.replace(value);
                }

                Ok(values)
            }
        }

        let visitor = SeqVisitor {
            marker: PhantomData,
            set_item_type: PhantomData,
        };
        deserializer.deserialize_seq(visitor)
    }

    /// Serialize the set with the default serializer
    pub fn serialize<T, S>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        T: Serialize,
        S: Serializer,
    {
        value.serialize(serializer)
    }
}

/// Ensure that the first key is taken, if duplicate keys exist
///
/// By default serde has a last-key-wins implementation, if duplicate keys for a map exist.
/// Sometimes the opposite strategy is desired. This helper implements a first-key-wins strategy.
///
/// The implementation supports both the [`HashMap`] and the [`BTreeMap`] from the standard library.
///
/// [`HashMap`]: std::collections::HashMap
/// [`BTreeMap`]: std::collections::HashMap
///
/// # Converting to `serde_as`
///
/// The same functionality can be more clearly expressed using the `serde_as` macro and [`MapFirstKeyWins`].
/// The `_` is a placeholder which works for any type which implements [`Serialize`]/[`Deserialize`].
///
/// ```rust
/// # #[cfg(any())] {
/// #[serde_as]
/// #[derive(Deserialize, Serialize)]
/// struct A {
///     #[serde_as(as = "MapFirstKeyWins<_, _>")]
///     s: HashMap<usize, usize>,
/// }
/// # }
/// ```
///
/// # Example
///
/// ```rust
/// # use serde::Deserialize;
/// # use std::collections::HashMap;
/// #
/// # #[derive(Debug, Eq, PartialEq)]
/// #[derive(Deserialize)]
/// struct Doc {
///     #[serde(with = "::serde_with::rust::maps_first_key_wins")]
///     map: HashMap<usize, usize>,
/// }
///
/// // Maps are serialized normally,
/// let s = r#"{"map": {"1": 1, "2": 2, "3": 3}}"#;
/// let mut v = Doc {
///     map: HashMap::new(),
/// };
/// v.map.insert(1, 1);
/// v.map.insert(2, 2);
/// v.map.insert(3, 3);
/// assert_eq!(v, serde_json::from_str(s).unwrap());
///
/// // but create an error if duplicate keys, like the `1`, exist.
/// let s = r#"{"map": {"1": 1, "2": 2, "1": 3}}"#;
/// let mut v = Doc {
///     map: HashMap::new(),
/// };
/// v.map.insert(1, 1);
/// v.map.insert(2, 2);
/// assert_eq!(v, serde_json::from_str(s).unwrap());
/// ```
#[cfg(feature = "alloc")]
pub mod maps_first_key_wins {
    use super::*;
    use crate::duplicate_key_impls::DuplicateInsertsFirstWinsMap;

    /// Deserialize a map and return an error on duplicate keys
    pub fn deserialize<'de, D, T, K, V>(deserializer: D) -> Result<T, D::Error>
    where
        T: DuplicateInsertsFirstWinsMap<K, V>,
        K: Deserialize<'de>,
        V: Deserialize<'de>,
        D: Deserializer<'de>,
    {
        struct MapVisitor<T, K, V> {
            marker: PhantomData<T>,
            map_key_type: PhantomData<K>,
            map_value_type: PhantomData<V>,
        }

        impl<'de, T, K, V> Visitor<'de> for MapVisitor<T, K, V>
        where
            T: DuplicateInsertsFirstWinsMap<K, V>,
            K: Deserialize<'de>,
            V: Deserialize<'de>,
        {
            type Value = T;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a map")
            }

            #[inline]
            fn visit_map<A>(self, mut access: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut values = Self::Value::new(access.size_hint());

                while let Some((key, value)) = access.next_entry()? {
                    values.insert(key, value);
                }

                Ok(values)
            }
        }

        let visitor = MapVisitor {
            marker: PhantomData,
            map_key_type: PhantomData,
            map_value_type: PhantomData,
        };
        deserializer.deserialize_map(visitor)
    }

    /// Serialize the map with the default serializer
    pub fn serialize<T, S>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        T: Serialize,
        S: Serializer,
    {
        value.serialize(serializer)
    }
}

/// Deserialize any value, ignore it, and return the default value for the type being deserialized.
///
/// This function can be used in two different ways:
///
/// 1. It is useful for instance to create an enum with a catch-all variant that will accept any incoming data.
/// 2. [`untagged`] enum representations do not allow the `other` annotation as the fallback enum variant.
///    With this function you can emulate an `other` variant, which can deserialize any data carrying enum.
///
/// **Note:** Using this function will prevent deserializing data-less enum variants.
/// If this is a problem depends on the data format.
/// For example, deserializing `"Bar"` as an enum in JSON would fail, since it carries no data.
///
/// # Examples
///
/// ## Deserializing a heterogeneous collection of XML nodes
///
/// When [`serde-xml-rs`] deserializes an XML tag to an enum, it always maps the tag
/// name to the enum variant name, and the tag attributes and children to the enum contents.
/// This means that in order for an enum variant to accept any XML tag, it both has to use
/// `#[serde(other)]` to accept any tag name, and `#[serde(deserialize_with = "deserialize_ignore_any")]`
/// to accept any attributes and children.
///
/// ```rust
/// # use serde::Deserialize;
/// use serde_with::rust::deserialize_ignore_any;
///
/// # #[derive(Debug, PartialEq)]
/// #[derive(Deserialize)]
/// struct Root {
///     values: Vec<Item>,
/// }
///
/// # #[derive(Debug, PartialEq)]
/// #[derive(Deserialize)]
/// #[serde(rename_all = "lowercase")]
/// enum Item {
///     Foo(String),
///     Bar(String),
///     #[serde(other, deserialize_with = "deserialize_ignore_any")]
///     Other,
/// }
///
/// // Deserialize this XML
/// # let items: Root = serde_xml_rs::from_str(
/// r#"<?xml version="1.0" encoding="UTF-8"?>
/// <Root>
///     <values>
///         <foo>a</foo>
///     </values>
///     <values>
///         <bar>b</bar>
///     </values>
///     <values>
///         <foo>c</foo>
///     </values>
///     <values>
///         <unknown>d</unknown>
///     </values>
/// </Root>"#
/// # ).unwrap();
///
/// // into these Items
/// # let expected =
/// Root {
///     values: vec![
///         Item::Foo(String::from("a")),
///         Item::Bar(String::from("b")),
///         Item::Foo(String::from("c")),
///         Item::Other,
///     ]
/// }
/// # ;
/// # assert_eq!(expected, items);
/// ```
///
/// ## Simulating an `other` enum variant in an `untagged` enum
///
/// ```rust
/// # use serde::Deserialize;
/// # use serde_json::json;
/// use serde_with::rust::deserialize_ignore_any;
///
/// # #[derive(Debug, PartialEq)]
/// #[derive(Deserialize)]
/// #[serde(untagged)]
/// enum Item {
///     Foo{x: u8},
///     #[serde(deserialize_with = "deserialize_ignore_any")]
///     Other,
/// }
///
/// // Deserialize this JSON
/// # let items: Vec<Item> = serde_json::from_value(
/// json!([
///     {"y": 1},
///     {"x": 1},
/// ])
/// # ).unwrap();
///
/// // into these Items
/// # let expected =
/// vec![Item::Other, Item::Foo{x: 1}]
/// # ;
/// # assert_eq!(expected, items);
/// ```
///
/// [`serde-xml-rs`]: https://docs.rs/serde-xml-rs
/// [`untagged`]: https://serde.rs/enum-representations.html#untagged
pub fn deserialize_ignore_any<'de, D: Deserializer<'de>, T: Default>(
    deserializer: D,
) -> Result<T, D::Error> {
    IgnoredAny::deserialize(deserializer).map(|_| T::default())
}
