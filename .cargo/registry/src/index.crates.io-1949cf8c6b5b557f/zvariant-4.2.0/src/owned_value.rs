use serde::{Deserialize, Deserializer, Serialize};
use static_assertions::assert_impl_all;
use std::{collections::HashMap, hash::BuildHasher};

use crate::{
    Array, Dict, NoneValue, ObjectPath, Optional, OwnedObjectPath, OwnedSignature, Signature, Str,
    Structure, Type, Value,
};

#[cfg(unix)]
use crate::Fd;

#[cfg(feature = "gvariant")]
use crate::Maybe;

// FIXME: Replace with a generic impl<T: TryFrom<Value>> TryFrom<OwnedValue> for T?
// https://github.com/dbus2/zbus/issues/138

/// Owned [`Value`](enum.Value.html)
#[derive(Debug, PartialEq, Serialize, Type)]
pub struct OwnedValue(pub(crate) Value<'static>);

assert_impl_all!(OwnedValue: Send, Sync, Unpin);

impl OwnedValue {
    /// Attempt to clone the value.
    pub fn try_clone(&self) -> Result<Self, crate::Error> {
        self.0.try_clone().map(Self)
    }

    pub(crate) fn into_inner(self) -> Value<'static> {
        self.0
    }

    pub(crate) fn inner(&self) -> &Value<'_> {
        &self.0
    }
}

macro_rules! ov_try_from {
    ($to:ty) => {
        impl TryFrom<OwnedValue> for $to {
            type Error = crate::Error;

            fn try_from(v: OwnedValue) -> Result<Self, Self::Error> {
                <$to>::try_from(v.0)
            }
        }
    };
}

macro_rules! ov_try_from_ref {
    ($to:ty) => {
        impl<'a> TryFrom<&'a OwnedValue> for $to {
            type Error = crate::Error;

            fn try_from(v: &'a OwnedValue) -> Result<Self, Self::Error> {
                <$to>::try_from(&v.0)
            }
        }
    };
}

ov_try_from!(u8);
ov_try_from!(bool);
ov_try_from!(i16);
ov_try_from!(u16);
ov_try_from!(i32);
ov_try_from!(u32);
ov_try_from!(i64);
ov_try_from!(u64);
ov_try_from!(f64);
ov_try_from!(String);
ov_try_from!(Signature<'static>);
ov_try_from!(OwnedSignature);
ov_try_from!(ObjectPath<'static>);
ov_try_from!(OwnedObjectPath);
ov_try_from!(Array<'static>);
ov_try_from!(Dict<'static, 'static>);
#[cfg(feature = "gvariant")]
ov_try_from!(Maybe<'static>);
ov_try_from!(Str<'static>);
ov_try_from!(Structure<'static>);
#[cfg(unix)]
ov_try_from!(Fd<'static>);

ov_try_from_ref!(u8);
ov_try_from_ref!(bool);
ov_try_from_ref!(i16);
ov_try_from_ref!(u16);
ov_try_from_ref!(i32);
ov_try_from_ref!(u32);
ov_try_from_ref!(i64);
ov_try_from_ref!(u64);
ov_try_from_ref!(f64);
ov_try_from_ref!(&'a str);
ov_try_from_ref!(&'a Signature<'a>);
ov_try_from_ref!(&'a ObjectPath<'a>);
ov_try_from_ref!(&'a Array<'a>);
ov_try_from_ref!(&'a Dict<'a, 'a>);
ov_try_from_ref!(&'a Str<'a>);
ov_try_from_ref!(&'a Structure<'a>);
#[cfg(feature = "gvariant")]
ov_try_from_ref!(&'a Maybe<'a>);
#[cfg(unix)]
ov_try_from_ref!(&'a Fd<'a>);

impl<'a, T> TryFrom<OwnedValue> for Vec<T>
where
    T: TryFrom<Value<'a>>,
    T::Error: Into<crate::Error>,
{
    type Error = crate::Error;

    fn try_from(value: OwnedValue) -> Result<Self, Self::Error> {
        if let Value::Array(v) = value.0 {
            Self::try_from(v)
        } else {
            Err(crate::Error::IncorrectType)
        }
    }
}

#[cfg(feature = "enumflags2")]
impl<'a, F> TryFrom<OwnedValue> for enumflags2::BitFlags<F>
where
    F: enumflags2::BitFlag,
    F::Numeric: TryFrom<Value<'a>, Error = crate::Error>,
{
    type Error = crate::Error;

    fn try_from(value: OwnedValue) -> Result<Self, Self::Error> {
        Self::try_from(value.0)
    }
}

impl<'k, 'v, K, V, H> TryFrom<OwnedValue> for HashMap<K, V, H>
where
    K: crate::Basic + TryFrom<Value<'k>> + std::hash::Hash + std::cmp::Eq,
    V: TryFrom<Value<'v>>,
    H: BuildHasher + Default,
    K::Error: Into<crate::Error>,
    V::Error: Into<crate::Error>,
{
    type Error = crate::Error;

    fn try_from(value: OwnedValue) -> Result<Self, Self::Error> {
        if let Value::Dict(v) = value.0 {
            Self::try_from(v)
        } else {
            Err(crate::Error::IncorrectType)
        }
    }
}

impl<K, V, H> From<HashMap<K, V, H>> for OwnedValue
where
    K: Type + Into<Value<'static>> + std::hash::Hash + std::cmp::Eq,
    V: Type + Into<Value<'static>>,
    H: BuildHasher + Default,
{
    fn from(value: HashMap<K, V, H>) -> Self {
        Self(value.into())
    }
}

impl<'a, T> TryFrom<OwnedValue> for Optional<T>
where
    T: TryFrom<Value<'a>> + NoneValue + PartialEq<<T as NoneValue>::NoneType>,
    T::Error: Into<crate::Error>,
{
    type Error = crate::Error;

    fn try_from(value: OwnedValue) -> Result<Self, Self::Error> {
        Self::try_from(value.0)
    }
}

impl<V> From<Optional<V>> for OwnedValue
where
    V: Into<Value<'static>> + NoneValue<NoneType = V>,
{
    fn from(v: Optional<V>) -> OwnedValue {
        Self(Value::from(v))
    }
}

// tuple conversions in `structure` module for avoiding code-duplication.

impl<'a> TryFrom<Value<'a>> for OwnedValue {
    type Error = crate::Error;

    fn try_from(v: Value<'a>) -> crate::Result<Self> {
        // TODO: add into_owned, avoiding copy if already owned..
        v.try_to_owned()
    }
}

impl<'a> TryFrom<&Value<'a>> for OwnedValue {
    type Error = crate::Error;

    fn try_from(v: &Value<'a>) -> crate::Result<Self> {
        v.try_to_owned()
    }
}

macro_rules! to_value {
    ($from:ty, $variant:ident) => {
        impl<'a> From<$from> for OwnedValue {
            fn from(v: $from) -> Self {
                OwnedValue(<Value<'static>>::$variant(v.to_owned()))
            }
        }
    };
}

to_value!(u8, U8);
to_value!(bool, Bool);
to_value!(i16, I16);
to_value!(u16, U16);
to_value!(i32, I32);
to_value!(u32, U32);
to_value!(i64, I64);
to_value!(u64, U64);
to_value!(f64, F64);
to_value!(Str<'a>, Str);
to_value!(Signature<'a>, Signature);
to_value!(ObjectPath<'a>, ObjectPath);

macro_rules! try_to_value {
    ($from:ty) => {
        impl<'a> TryFrom<$from> for OwnedValue {
            type Error = crate::Error;

            fn try_from(v: $from) -> crate::Result<Self> {
                OwnedValue::try_from(<Value<'a>>::from(v))
            }
        }
    };
}

try_to_value!(Array<'a>);
try_to_value!(Dict<'a, 'a>);
#[cfg(feature = "gvariant")]
try_to_value!(Maybe<'a>);
try_to_value!(Structure<'a>);
#[cfg(unix)]
try_to_value!(Fd<'a>);

impl From<OwnedValue> for Value<'_> {
    fn from(v: OwnedValue) -> Self {
        v.into_inner()
    }
}

impl<'o> TryFrom<&'o OwnedValue> for Value<'o> {
    type Error = crate::Error;

    fn try_from(v: &'o OwnedValue) -> crate::Result<Value<'o>> {
        v.inner().try_clone()
    }
}

impl std::ops::Deref for OwnedValue {
    type Target = Value<'static>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'de> Deserialize<'de> for OwnedValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Value::deserialize(deserializer)
            .and_then(|v| v.try_to_owned().map_err(serde::de::Error::custom))
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, error::Error};

    use crate::{serialized::Context, to_bytes, OwnedValue, Value, LE};

    #[cfg(feature = "enumflags2")]
    #[test]
    fn bitflags() -> Result<(), Box<dyn Error>> {
        #[repr(u32)]
        #[enumflags2::bitflags]
        #[derive(Copy, Clone, Debug)]
        pub enum Flaggy {
            One = 0x1,
            Two = 0x2,
        }

        let v = Value::from(0x2u32);
        let ov: OwnedValue = v.try_into()?;
        assert_eq!(<enumflags2::BitFlags<Flaggy>>::try_from(ov)?, Flaggy::Two);
        Ok(())
    }

    #[test]
    fn from_value() -> Result<(), Box<dyn Error>> {
        let v = Value::from("hi!");
        let ov: OwnedValue = v.try_into()?;
        assert_eq!(<&str>::try_from(&ov)?, "hi!");
        Ok(())
    }

    #[test]
    fn serde() -> Result<(), Box<dyn Error>> {
        let ec = Context::new_dbus(LE, 0);
        let ov: OwnedValue = Value::from("hi!").try_into()?;
        let ser = to_bytes(ec, &ov)?;
        let (de, parsed): (Value<'_>, _) = ser.deserialize()?;
        assert_eq!(<&str>::try_from(&de)?, "hi!");
        assert_eq!(parsed, ser.len());
        Ok(())
    }

    #[test]
    fn map_conversion() -> Result<(), Box<dyn Error>> {
        let mut map = HashMap::<String, String>::new();
        map.insert("one".to_string(), "1".to_string());
        map.insert("two".to_string(), "2".to_string());
        let value = OwnedValue::from(map.clone());
        // Now convert back
        let map2 = <HashMap<String, String>>::try_from(value)?;
        assert_eq!(map, map2);

        Ok(())
    }
}
