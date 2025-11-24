#![allow(unknown_lints)]
use serde::{
    de::{DeserializeSeed, Deserializer, SeqAccess, Visitor},
    ser::{Serialize, SerializeSeq, Serializer},
};
use static_assertions::assert_impl_all;
use std::fmt::{Display, Write};

use crate::{
    value::{value_display_fmt, SignatureSeed},
    DynamicDeserialize, DynamicType, Error, Result, Signature, Type, Value,
};

/// A helper type to wrap arrays in a [`Value`].
///
/// API is provided to convert from, and to a [`Vec`].
///
/// [`Value`]: enum.Value.html#variant.Array
/// [`Vec`]: https://doc.rust-lang.org/std/vec/struct.Vec.html
#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord)]
pub struct Array<'a> {
    element_signature: Signature<'a>,
    elements: Vec<Value<'a>>,
    signature: Signature<'a>,
}

assert_impl_all!(Array<'_>: Send, Sync, Unpin);

impl<'a> Array<'a> {
    /// Create a new empty `Array`, given the signature of the elements.
    pub fn new(element_signature: Signature<'_>) -> Array<'_> {
        let signature = create_signature(&element_signature);
        Array {
            element_signature,
            elements: vec![],
            signature,
        }
    }

    pub(crate) fn new_full_signature(signature: Signature<'_>) -> Array<'_> {
        let element_signature = signature.slice(1..);
        Array {
            element_signature,
            elements: vec![],
            signature,
        }
    }

    /// Append `element`.
    ///
    /// # Errors
    ///
    /// if `element`'s signature doesn't match the element signature `self` was created for.
    pub fn append<'e: 'a>(&mut self, element: Value<'e>) -> Result<()> {
        check_child_value_signature!(self.element_signature, element.value_signature(), "element");

        self.elements.push(element);

        Ok(())
    }

    /// Get all the elements.
    pub fn inner(&self) -> &[Value<'a>] {
        &self.elements
    }

    /// Get the value at the given index.
    pub fn get<V>(&'a self, idx: usize) -> Result<Option<V>>
    where
        V: ?Sized + TryFrom<&'a Value<'a>>,
        <V as TryFrom<&'a Value<'a>>>::Error: Into<crate::Error>,
    {
        self.elements
            .get(idx)
            .map(|v| v.downcast_ref::<V>())
            .transpose()
            .map_err(Into::into)
    }

    /// Get the number of elements.
    pub fn len(&self) -> usize {
        self.elements.len()
    }

    pub fn is_empty(&self) -> bool {
        self.elements.len() == 0
    }

    /// Get the signature of this `Array`.
    ///
    /// NB: This method potentially allocates and copies. Use [`full_signature`] if you'd like to
    /// avoid that.
    ///
    /// [`full_signature`]: #method.full_signature
    pub fn signature(&self) -> Signature<'static> {
        self.signature.to_owned()
    }

    /// Get the signature of this `Array`.
    pub fn full_signature(&self) -> &Signature<'_> {
        &self.signature
    }

    /// Get the signature of the elements in the `Array`.
    pub fn element_signature(&self) -> &Signature<'_> {
        &self.element_signature
    }

    pub(crate) fn try_to_owned(&self) -> Result<Array<'static>> {
        Ok(Array {
            element_signature: self.element_signature.to_owned(),
            elements: self
                .elements
                .iter()
                .map(|v| v.try_to_owned().map(Into::into))
                .collect::<Result<_>>()?,
            signature: self.signature.to_owned(),
        })
    }

    /// Tries to clone the `Array`.
    pub fn try_clone(&self) -> crate::Result<Self> {
        let elements = self
            .elements
            .iter()
            .map(|v| v.try_clone())
            .collect::<crate::Result<Vec<_>>>()?;

        Ok(Self {
            element_signature: self.element_signature.clone(),
            elements,
            signature: self.signature.clone(),
        })
    }
}

impl Display for Array<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        array_display_fmt(self, f, true)
    }
}

pub(crate) fn array_display_fmt(
    array: &Array<'_>,
    f: &mut std::fmt::Formatter<'_>,
    type_annotate: bool,
) -> std::fmt::Result {
    // Print as string if it is a bytestring (i.e., first nul character is the last byte)
    if let [leading @ .., Value::U8(b'\0')] = array.as_ref() {
        if !leading.contains(&Value::U8(b'\0')) {
            let bytes = leading
                .iter()
                .map(|v| {
                    v.downcast_ref::<u8>()
                        .expect("item must have a signature of a byte")
                })
                .collect::<Vec<_>>();

            let string = String::from_utf8_lossy(&bytes);
            write!(f, "b{:?}", string.as_ref())?;

            return Ok(());
        }
    }

    if array.is_empty() {
        if type_annotate {
            write!(f, "@{} ", array.full_signature())?;
        }
        f.write_str("[]")?;
    } else {
        f.write_char('[')?;

        // Annotate only the first item as the rest will be of the same type.
        let mut type_annotate = type_annotate;

        for (i, item) in array.iter().enumerate() {
            value_display_fmt(item, f, type_annotate)?;
            type_annotate = false;

            if i + 1 < array.len() {
                f.write_str(", ")?;
            }
        }

        f.write_char(']')?;
    }

    Ok(())
}

/// Use this to deserialize an [Array].
pub struct ArraySeed<'a> {
    signature: Signature<'a>,
}

impl<'a> ArraySeed<'a> {
    /// Create a new empty `Array`, given the signature of the elements.
    pub fn new(element_signature: Signature<'_>) -> ArraySeed<'_> {
        let signature = create_signature(&element_signature);
        ArraySeed { signature }
    }

    pub(crate) fn new_full_signature(signature: Signature<'_>) -> ArraySeed<'_> {
        ArraySeed { signature }
    }
}

assert_impl_all!(ArraySeed<'_>: Send, Sync, Unpin);

impl<'a> DynamicType for Array<'a> {
    fn dynamic_signature(&self) -> Signature<'_> {
        self.signature.clone()
    }
}

impl<'a> DynamicType for ArraySeed<'a> {
    fn dynamic_signature(&self) -> Signature<'_> {
        self.signature.clone()
    }
}

impl<'a> DynamicDeserialize<'a> for Array<'a> {
    type Deserializer = ArraySeed<'a>;

    fn deserializer_for_signature<S>(signature: S) -> zvariant::Result<Self::Deserializer>
    where
        S: TryInto<Signature<'a>>,
        S::Error: Into<zvariant::Error>,
    {
        let signature = signature.try_into().map_err(Into::into)?;
        if signature.starts_with(zvariant::ARRAY_SIGNATURE_CHAR) {
            Ok(ArraySeed::new_full_signature(signature))
        } else {
            Err(zvariant::Error::SignatureMismatch(
                signature.to_owned(),
                "an array signature".to_owned(),
            ))
        }
    }
}

impl<'a> std::ops::Deref for Array<'a> {
    type Target = [Value<'a>];

    fn deref(&self) -> &Self::Target {
        self.inner()
    }
}

impl<'a, T> From<Vec<T>> for Array<'a>
where
    T: Type + Into<Value<'a>>,
{
    fn from(values: Vec<T>) -> Self {
        let element_signature = T::signature();
        let elements = values.into_iter().map(Value::new).collect();
        let signature = create_signature(&element_signature);

        Self {
            element_signature,
            elements,
            signature,
        }
    }
}

impl<'a, T> From<&[T]> for Array<'a>
where
    T: Type + Into<Value<'a>> + Clone,
{
    fn from(values: &[T]) -> Self {
        let element_signature = T::signature();
        let elements = values
            .iter()
            .map(|value| Value::new(value.clone()))
            .collect();
        let signature = create_signature(&element_signature);

        Self {
            element_signature,
            elements,
            signature,
        }
    }
}

impl<'a, T> From<&Vec<T>> for Array<'a>
where
    T: Type + Into<Value<'a>> + Clone,
{
    fn from(values: &Vec<T>) -> Self {
        Self::from(&values[..])
    }
}

impl<'a, T> TryFrom<Array<'a>> for Vec<T>
where
    T: TryFrom<Value<'a>>,
    T::Error: Into<crate::Error>,
{
    type Error = Error;

    fn try_from(v: Array<'a>) -> core::result::Result<Self, Self::Error> {
        // there is no try_map yet..
        let mut res = vec![];
        for e in v.elements.into_iter() {
            let value = if let Value::Value(v) = e {
                T::try_from(*v)
            } else {
                T::try_from(e)
            }
            .map_err(Into::into)?;

            res.push(value);
        }
        Ok(res)
    }
}

// TODO: this could be useful
// impl<'a, 'b, T> TryFrom<&'a Array<'b>> for Vec<T>

impl<'a> Serialize for Array<'a> {
    fn serialize<S>(&self, serializer: S) -> core::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(self.elements.len()))?;
        for element in &self.elements {
            element.serialize_value_as_seq_element(&mut seq)?;
        }

        seq.end()
    }
}

impl<'de> DeserializeSeed<'de> for ArraySeed<'de> {
    type Value = Array<'de>;
    fn deserialize<D>(self, deserializer: D) -> std::result::Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(ArrayVisitor {
            signature: self.signature,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ArrayVisitor<'a> {
    signature: Signature<'a>,
}

impl<'de> Visitor<'de> for ArrayVisitor<'de> {
    type Value = Array<'de>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("an Array value")
    }

    fn visit_seq<V>(self, visitor: V) -> std::result::Result<Array<'de>, V::Error>
    where
        V: SeqAccess<'de>,
    {
        SignatureSeed {
            signature: self.signature,
        }
        .visit_array(visitor)
    }
}

fn create_signature(element_signature: &Signature<'_>) -> Signature<'static> {
    Signature::from_string_unchecked(format!("a{element_signature}"))
}
