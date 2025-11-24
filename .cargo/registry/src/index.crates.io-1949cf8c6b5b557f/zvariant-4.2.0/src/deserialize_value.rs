use core::str;
use std::marker::PhantomData;

use serde::de::{Deserialize, Deserializer, SeqAccess, Visitor};
use static_assertions::assert_impl_all;

use crate::{Signature, Type, Value};

/// A wrapper to deserialize a value to `T: Type + Deserialize`.
///
/// When the type of a value is well-known, you may avoid the cost and complexity of wrapping to a
/// generic [`Value`] and instead use this wrapper.
///
/// ```
/// # use zvariant::{to_bytes, serialized::Context, DeserializeValue, SerializeValue, LE};
/// #
/// # let ctxt = Context::new_dbus(LE, 0);
/// # let array = [0, 1, 2];
/// # let v = SerializeValue(&array);
/// # let encoded = to_bytes(ctxt, &v).unwrap();
/// let decoded: DeserializeValue<[u8; 3]> = encoded.deserialize().unwrap().0;
/// # assert_eq!(decoded.0, array);
/// ```
///
/// [`Value`]: enum.Value.html
pub struct DeserializeValue<'de, T: Type + Deserialize<'de>>(
    pub T,
    std::marker::PhantomData<&'de T>,
);

assert_impl_all!(DeserializeValue<'_, i32>: Send, Sync, Unpin);

impl<'de, T: Type + Deserialize<'de>> Deserialize<'de> for DeserializeValue<'de, T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        const FIELDS: &[&str] = &["zvariant::Value::Signature", "zvariant::Value::Value"];
        Ok(DeserializeValue(
            deserializer.deserialize_struct(
                "zvariant::Value",
                FIELDS,
                DeserializeValueVisitor(PhantomData),
            )?,
            PhantomData,
        ))
    }
}

struct DeserializeValueVisitor<T>(PhantomData<T>);

impl<'de, T: Type + Deserialize<'de>> Visitor<'de> for DeserializeValueVisitor<T> {
    type Value = T;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("zvariant::Value")
    }

    fn visit_seq<V>(self, mut seq: V) -> Result<Self::Value, V::Error>
    where
        V: SeqAccess<'de>,
    {
        let sig: Signature<'_> = seq
            .next_element()?
            .ok_or_else(|| serde::de::Error::invalid_length(0, &self))?;
        if sig != T::signature() {
            return Err(serde::de::Error::invalid_value(
                serde::de::Unexpected::Str(&sig),
                &"the value signature",
            ));
        }

        seq.next_element()?
            .ok_or_else(|| serde::de::Error::invalid_length(1, &self))
    }
}

impl<'de, T: Type + Deserialize<'de>> Type for DeserializeValue<'de, T> {
    fn signature() -> Signature<'static> {
        Value::signature()
    }
}
