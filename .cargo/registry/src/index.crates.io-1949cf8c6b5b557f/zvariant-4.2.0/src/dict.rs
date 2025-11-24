use std::{
    collections::{BTreeMap, HashMap},
    fmt::{Display, Write},
    hash::{BuildHasher, Hash},
};

use serde::ser::{Serialize, SerializeMap, Serializer};
use static_assertions::assert_impl_all;

use crate::{value_display_fmt, Basic, DynamicType, Error, Signature, Type, Value};

/// A helper type to wrap dictionaries in a [`Value`].
///
/// API is provided to convert from, and to a [`HashMap`].
///
/// [`Value`]: enum.Value.html#variant.Dict
/// [`HashMap`]: https://doc.rust-lang.org/std/collections/struct.HashMap.html
#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord)]
pub struct Dict<'k, 'v> {
    map: BTreeMap<Value<'k>, Value<'v>>,
    key_signature: Signature<'k>,
    value_signature: Signature<'v>,
    // should use a separate lifetime or everything should use the same but API break.
    signature: Signature<'k>,
}

assert_impl_all!(Dict<'_, '_>: Send, Sync, Unpin);

impl<'k, 'v> Dict<'k, 'v> {
    /// Create a new empty `Dict`, given the signature of the keys and values.
    pub fn new(key_signature: Signature<'k>, value_signature: Signature<'v>) -> Self {
        let signature = create_signature(&key_signature, &value_signature);

        Self {
            map: BTreeMap::new(),
            key_signature,
            value_signature,
            signature,
        }
    }

    /// Append `key` and `value` as a new entry.
    ///
    /// # Errors
    ///
    /// * if [`key.value_signature()`] doesn't match the key signature `self` was created for.
    /// * if [`value.value_signature()`] doesn't match the value signature `self` was created for.
    ///
    /// [`key.value_signature()`]: enum.Value.html#method.value_signature
    /// [`value.value_signature()`]: enum.Value.html#method.value_signature
    pub fn append<'kv: 'k, 'vv: 'v>(
        &mut self,
        key: Value<'kv>,
        value: Value<'vv>,
    ) -> Result<(), Error> {
        check_child_value_signature!(self.key_signature, key.value_signature(), "key");
        check_child_value_signature!(self.value_signature, value.value_signature(), "value");

        self.map.insert(key, value);

        Ok(())
    }

    /// Add a new entry.
    pub fn add<K, V>(&mut self, key: K, value: V) -> Result<(), Error>
    where
        K: Basic + Into<Value<'k>> + Ord,
        V: Into<Value<'v>> + DynamicType,
    {
        check_child_value_signature!(self.key_signature, K::signature(), "key");
        check_child_value_signature!(self.value_signature, value.dynamic_signature(), "value");

        self.map.insert(Value::new(key), Value::new(value));

        Ok(())
    }

    /// Get the value for the given key.
    pub fn get<'d, K, V>(&'d self, key: &'k K) -> Result<Option<V>, Error>
    where
        'd: 'k + 'v,
        &'k K: TryInto<Value<'k>>,
        <&'k K as TryInto<Value<'k>>>::Error: Into<crate::Error>,
        V: TryFrom<&'v Value<'v>>,
        <V as TryFrom<&'v Value<'v>>>::Error: Into<crate::Error>,
    {
        let key: Value<'_> = key.try_into().map_err(Into::into)?;

        self.map.get(&key).map(|v| v.downcast_ref()).transpose()
    }

    /// Get the signature of this `Dict`.
    ///
    /// NB: This method potentially allocates and copies. Use [`full_signature`] if you'd like to
    /// avoid that.
    ///
    /// [`full_signature`]: #method.full_signature
    pub fn signature(&self) -> Signature<'static> {
        self.signature.to_owned()
    }

    /// Get the signature of this `Dict`.
    pub fn full_signature(&self) -> &Signature<'_> {
        &self.signature
    }

    pub(crate) fn try_to_owned(&self) -> crate::Result<Dict<'static, 'static>> {
        Ok(Dict {
            key_signature: self.key_signature.to_owned(),
            value_signature: self.value_signature.to_owned(),
            signature: self.signature.to_owned(),
            map: self
                .map
                .iter()
                .map(|(k, v)| {
                    Ok((
                        k.try_to_owned().map(Into::into)?,
                        v.try_to_owned().map(Into::into)?,
                    ))
                })
                .collect::<crate::Result<_>>()?,
        })
    }

    /// Try to clone the `Dict`.
    pub fn try_clone(&self) -> Result<Self, Error> {
        let entries = self
            .map
            .iter()
            .map(|(k, v)| Ok((k.try_clone()?, v.try_clone()?)))
            .collect::<Result<_, crate::Error>>()?;

        Ok(Self {
            map: entries,
            key_signature: self.key_signature.clone(),
            value_signature: self.value_signature.clone(),
            signature: self.signature.clone(),
        })
    }

    /// Create a new empty `Dict`, given the complete signature.
    pub(crate) fn new_full_signature<'s: 'k + 'v>(signature: Signature<'s>) -> Self {
        let key_signature = signature.slice(2..3);
        let value_signature = signature.slice(3..signature.len() - 1);

        Self {
            map: BTreeMap::new(),
            key_signature,
            value_signature,
            signature,
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Value<'k>, &Value<'v>)> {
        self.map.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&Value<'k>, &mut Value<'v>)> {
        self.map.iter_mut()
    }

    // TODO: Provide more API like https://docs.rs/toml/0.5.5/toml/map/struct.Map.html
}

impl Display for Dict<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        dict_display_fmt(self, f, true)
    }
}

impl<'k, 'v> IntoIterator for Dict<'k, 'v> {
    type Item = (Value<'k>, Value<'v>);
    type IntoIter = <BTreeMap<Value<'k>, Value<'v>> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.map.into_iter()
    }
}

pub(crate) fn dict_display_fmt(
    dict: &Dict<'_, '_>,
    f: &mut std::fmt::Formatter<'_>,
    type_annotate: bool,
) -> std::fmt::Result {
    if dict.map.is_empty() {
        if type_annotate {
            write!(f, "@{} ", dict.full_signature())?;
        }
        f.write_str("{}")?;
    } else {
        f.write_char('{')?;

        // Annotate only the first entry as the rest will be of the same type.
        let mut type_annotate = type_annotate;

        for (i, (key, value)) in dict.map.iter().enumerate() {
            value_display_fmt(key, f, type_annotate)?;
            f.write_str(": ")?;
            value_display_fmt(value, f, type_annotate)?;
            type_annotate = false;

            if i + 1 < dict.map.len() {
                f.write_str(", ")?;
            }
        }

        f.write_char('}')?;
    }

    Ok(())
}

impl<'k, 'v> Serialize for Dict<'k, 'v> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.map.len()))?;
        for (key, value) in self.map.iter() {
            key.serialize_value_as_dict_key(&mut map)?;
            value.serialize_value_as_dict_value(&mut map)?;
        }

        map.end()
    }
}

// Conversion of Dict to Map types
macro_rules! from_dict {
    ($ty:ident <K $(: $kbound1:ident $(+ $kbound2:ident)*)*, V $(, $typaram:ident)*>) => {
        impl<'k, 'v, K, V $(, $typaram)*> TryFrom<Dict<'k, 'v>> for $ty<K, V $(, $typaram)*>
        where
            K: Basic + TryFrom<Value<'k>> $(+ $kbound1 $(+ $kbound2)*)*,
            V: TryFrom<Value<'v>>,
            K::Error: Into<crate::Error>,
            V::Error: Into<crate::Error>,
            $($typaram: BuildHasher + Default,)*
        {
            type Error = Error;

            fn try_from(v: Dict<'k, 'v>) -> Result<Self, Self::Error> {
                v.map.into_iter().map(|(key, value)| {
                    let key = if let Value::Value(v) = key {
                        K::try_from(*v)
                    } else {
                        K::try_from(key)
                    }
                    .map_err(Into::into)?;

                    let value = if let Value::Value(v) = value {
                        V::try_from(*v)
                    } else {
                        V::try_from(value)
                    }
                    .map_err(Into::into)?;

                    Ok((key, value))
                }).collect::<Result<_, _>>()
            }
        }
    };
}
from_dict!(HashMap<K: Eq + Hash, V, H>);
from_dict!(BTreeMap<K: Ord, V>);

// TODO: this could be useful
// impl<'d, 'k, 'v, K, V, H> TryFrom<&'d Dict<'k, 'v>> for HashMap<&'k K, &'v V, H>

// Conversion of Hashmap to Dict
macro_rules! to_dict {
    ($ty:ident <K $(: $kbound1:ident $(+ $kbound2:ident)*)*, V $(, $typaram:ident)*>) => {
        impl<'k, 'v, K, V $(, $typaram)*> From<$ty<K, V $(, $typaram)*>> for Dict<'k, 'v>
        where
            K: Type + Into<Value<'k>>,
            V: Type + Into<Value<'v>>,
            $($typaram: BuildHasher,)*
        {
            fn from(value: $ty<K, V $(, $typaram)*>) -> Self {
                let entries = value
                    .into_iter()
                    .map(|(key, value)| (Value::new(key), Value::new(value)))
                    .collect();
                let key_signature = K::signature();
                let value_signature = V::signature();
                let signature = create_signature(&key_signature, &value_signature);

                Self {
                    map: entries,
                    key_signature,
                    value_signature,
                    signature,
                }
            }
        }
    };
}
to_dict!(HashMap<K: Eq + Hash, V, H>);
to_dict!(BTreeMap<K: Ord, V>);

fn create_signature(
    key_signature: &Signature<'_>,
    value_signature: &Signature<'_>,
) -> Signature<'static> {
    Signature::from_string_unchecked(format!("a{{{key_signature}{value_signature}}}",))
}
