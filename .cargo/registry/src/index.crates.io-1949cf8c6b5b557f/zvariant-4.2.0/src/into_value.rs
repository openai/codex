use std::{collections::HashMap, hash::BuildHasher};

#[cfg(feature = "gvariant")]
use crate::Maybe;
use crate::{Array, Dict, NoneValue, ObjectPath, Optional, Signature, Str, Structure, Type, Value};

#[cfg(unix)]
use crate::Fd;

//
// Conversions from encodable types to `Value`

macro_rules! into_value {
    ($from:ty, $kind:ident) => {
        impl<'a> From<$from> for Value<'a> {
            fn from(v: $from) -> Self {
                Value::$kind(v.into())
            }
        }
    };
}

macro_rules! into_value_from_ref {
    ($from:ty, $kind:ident) => {
        impl<'a> From<&'a $from> for Value<'a> {
            fn from(v: &'a $from) -> Self {
                Value::$kind(v.clone().into())
            }
        }
    };
}

macro_rules! into_value_from_both {
    ($from:ty, $kind:ident) => {
        into_value!($from, $kind);
        into_value_from_ref!($from, $kind);
    };
}

into_value_from_both!(u8, U8);
into_value_from_both!(i8, I16);
into_value_from_both!(bool, Bool);
into_value_from_both!(u16, U16);
into_value_from_both!(i16, I16);
into_value_from_both!(u32, U32);
into_value_from_both!(i32, I32);
into_value_from_both!(u64, U64);
into_value_from_both!(i64, I64);
into_value_from_both!(f32, F64);
into_value_from_both!(f64, F64);

into_value_from_both!(&'a str, Str);
into_value_from_both!(Str<'a>, Str);
into_value_from_both!(Signature<'a>, Signature);
into_value_from_both!(ObjectPath<'a>, ObjectPath);

macro_rules! try_into_value_from_ref {
    ($from:ty, $kind:ident) => {
        impl<'a> TryFrom<&'a $from> for Value<'a> {
            type Error = crate::Error;

            fn try_from(v: &'a $from) -> crate::Result<Self> {
                v.try_clone().map(Value::$kind)
            }
        }
    };
}

into_value!(Array<'a>, Array);
try_into_value_from_ref!(Array<'a>, Array);
into_value!(Dict<'a, 'a>, Dict);
try_into_value_from_ref!(Dict<'a, 'a>, Dict);
#[cfg(feature = "gvariant")]
into_value!(Maybe<'a>, Maybe);
#[cfg(feature = "gvariant")]
try_into_value_from_ref!(Maybe<'a>, Maybe);
#[cfg(unix)]
into_value!(Fd<'a>, Fd);
#[cfg(unix)]
try_into_value_from_ref!(Fd<'a>, Fd);

impl From<String> for Value<'_> {
    fn from(v: String) -> Self {
        Value::Str(crate::Str::from(v))
    }
}

impl<'v, 's: 'v, T> From<T> for Value<'v>
where
    T: Into<Structure<'s>>,
{
    fn from(v: T) -> Value<'v> {
        Value::Structure(v.into())
    }
}

impl<'v, V> From<&'v [V]> for Value<'v>
where
    &'v [V]: Into<Array<'v>>,
{
    fn from(v: &'v [V]) -> Value<'v> {
        Value::Array(v.into())
    }
}

impl<'v, V> From<Vec<V>> for Value<'v>
where
    Vec<V>: Into<Array<'v>>,
{
    fn from(v: Vec<V>) -> Value<'v> {
        Value::Array(v.into())
    }
}

impl<'v, V> From<&'v Vec<V>> for Value<'v>
where
    &'v Vec<V>: Into<Array<'v>>,
{
    fn from(v: &'v Vec<V>) -> Value<'v> {
        Value::Array(v.into())
    }
}

impl<'a, 'k, 'v, K, V, H> From<HashMap<K, V, H>> for Value<'a>
where
    'k: 'a,
    'v: 'a,
    K: Type + Into<Value<'k>> + std::hash::Hash + std::cmp::Eq,
    V: Type + Into<Value<'v>>,
    H: BuildHasher + Default,
{
    fn from(value: HashMap<K, V, H>) -> Self {
        Self::Dict(value.into())
    }
}

impl<'v> From<&'v String> for Value<'v> {
    fn from(v: &'v String) -> Value<'v> {
        Value::Str(v.into())
    }
}

impl<'v, V> From<Optional<V>> for Value<'v>
where
    V: Into<Value<'v>> + NoneValue<NoneType = V>,
{
    fn from(v: Optional<V>) -> Value<'v> {
        Option::<V>::from(v)
            .unwrap_or_else(|| V::null_value())
            .into()
    }
}

#[cfg(all(feature = "gvariant", not(feature = "option-as-array")))]
impl<'v, V> From<Option<V>> for Value<'v>
where
    Option<V>: Into<Maybe<'v>>,
{
    fn from(v: Option<V>) -> Value<'v> {
        Value::Maybe(v.into())
    }
}

#[cfg(feature = "option-as-array")]
impl<'v, V> From<Option<V>> for Value<'v>
where
    V: Into<Value<'v>> + Type,
{
    fn from(v: Option<V>) -> Value<'v> {
        let mut array = Array::new(V::signature());
        if let Some(v) = v {
            // We got the signature from the `Type` impl, so this should never panic.
            array.append(v.into()).expect("signature mismatch");
        }

        array.into()
    }
}
