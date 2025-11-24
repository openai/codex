#![allow(unknown_lints)]
use serde::{
    de::{DeserializeSeed, Deserializer, Error, SeqAccess, Visitor},
    ser::{Serialize, SerializeTupleStruct, Serializer},
};
use static_assertions::assert_impl_all;
use std::fmt::{Display, Write};

use crate::{
    signature_parser::SignatureParser, value::SignatureSeed, value_display_fmt, DynamicDeserialize,
    DynamicType, OwnedValue, Signature, Value,
};

/// Use this to efficiently build a [`Structure`].
///
/// [`Structure`]: struct.Structure.html
#[derive(Debug, Default, PartialEq)]
pub struct StructureBuilder<'a>(Vec<Value<'a>>);

assert_impl_all!(StructureBuilder<'_>: Send, Sync, Unpin);

impl<'a> StructureBuilder<'a> {
    /// Create a new `StructureBuilder`.
    ///
    /// Same as `StructureBuilder::default()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append `field` to `self`.
    ///
    /// This method returns `Self` so that you can use the builder pattern to create a complex
    /// structure.
    #[must_use]
    pub fn add_field<T>(self, field: T) -> Self
    where
        T: DynamicType + Into<Value<'a>>,
    {
        self.append_field(Value::new(field))
    }

    /// Append `field` to `self`.
    ///
    /// Identical to `add_field`, except the field must be in the form of a `Value`.
    #[must_use]
    pub fn append_field<'e: 'a>(mut self, field: Value<'e>) -> Self {
        self.0.push(field);

        self
    }

    /// Append `field` to `self`.
    ///
    /// Identical to `add_field`, except it makes changes in-place.
    pub fn push_field<T>(&mut self, field: T)
    where
        T: DynamicType + Into<Value<'a>>,
    {
        self.push_value(Value::new(field))
    }

    /// Append `field` to `self`.
    ///
    /// Identical to `append_field`, except it makes changes in-place.
    pub fn push_value<'e: 'a>(&mut self, field: Value<'e>) {
        self.0.push(field)
    }

    /// Build the `Structure`.
    ///
    /// [`Structure`]: struct.Structure.html
    pub fn build(self) -> Structure<'a> {
        let signature = create_signature_from_fields(&self.0);

        Structure {
            fields: self.0,
            signature,
        }
    }

    /// Same as `build` except Signature is provided.
    pub(crate) fn build_with_signature<'s: 'a>(self, signature: Signature<'s>) -> Structure<'a> {
        Structure {
            fields: self.0,
            signature,
        }
    }
}

/// Use this to deserialize a [`Structure`].
///
/// [`Structure`]: struct.Structure.html
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructureSeed<'a>(Signature<'a>);

assert_impl_all!(StructureSeed<'_>: Send, Sync, Unpin);

impl<'a> StructureSeed<'a> {
    /// Create a new `StructureSeed`
    ///
    /// The given signature must be a valid structure signature.
    #[must_use]
    pub fn new_unchecked(signature: Signature<'a>) -> Self {
        StructureSeed(signature)
    }
}

impl<'a> TryFrom<Signature<'a>> for StructureSeed<'a> {
    type Error = zvariant::Error;

    fn try_from(signature: Signature<'a>) -> Result<Self, zvariant::Error> {
        if signature.starts_with(zvariant::STRUCT_SIG_START_CHAR) {
            Ok(StructureSeed(signature))
        } else {
            Err(zvariant::Error::IncorrectType)
        }
    }
}

impl<'de> DeserializeSeed<'de> for StructureSeed<'de> {
    type Value = Structure<'de>;
    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(StructureVisitor { signature: self.0 })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StructureVisitor<'a> {
    signature: Signature<'a>,
}

impl<'de> Visitor<'de> for StructureVisitor<'de> {
    type Value = Structure<'de>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a Structure value")
    }

    fn visit_seq<V>(self, visitor: V) -> Result<Structure<'de>, V::Error>
    where
        V: SeqAccess<'de>,
    {
        SignatureSeed {
            signature: self.signature,
        }
        .visit_struct(visitor)
    }
}

/// A helper type to wrap structs in [`Value`].
///
/// API is provided to convert from, and to tuples.
///
/// [`Value`]: enum.Value.html
#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Structure<'a> {
    fields: Vec<Value<'a>>,
    signature: Signature<'a>,
}

assert_impl_all!(Structure<'_>: Send, Sync, Unpin);

impl<'a> Structure<'a> {
    /// Get a reference to all the fields of `self`.
    pub fn fields(&self) -> &[Value<'a>] {
        &self.fields
    }

    /// Converts `self` to a `Vec` containing all its fields.
    pub fn into_fields(self) -> Vec<Value<'a>> {
        self.fields
    }

    /// Get the signature of this `Structure`.
    ///
    /// NB: This method potentially allocates and copies. Use [`full_signature`] if you'd like to
    /// avoid that.
    ///
    /// [`full_signature`]: #method.full_signature
    pub fn signature(&self) -> Signature<'static> {
        self.signature.to_owned()
    }

    /// Get the signature of this `Structure`.
    pub fn full_signature(&self) -> &Signature<'_> {
        &self.signature
    }

    pub(crate) fn try_to_owned(&self) -> crate::Result<Structure<'static>> {
        Ok(Structure {
            fields: self
                .fields
                .iter()
                .map(|v| v.try_to_owned().map(Into::into))
                .collect::<crate::Result<_>>()?,
            signature: self.signature.to_owned(),
        })
    }

    /// Attempt to clone `self`.
    pub fn try_clone(&self) -> Result<Self, crate::Error> {
        let fields = self
            .fields
            .iter()
            .map(|v| v.try_clone())
            .collect::<crate::Result<Vec<_>>>()?;

        Ok(Self {
            fields,
            signature: self.signature.clone(),
        })
    }
}

impl Display for Structure<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        structure_display_fmt(self, f, true)
    }
}

pub(crate) fn structure_display_fmt(
    structure: &Structure<'_>,
    f: &mut std::fmt::Formatter<'_>,
    type_annotate: bool,
) -> std::fmt::Result {
    f.write_char('(')?;

    let fields = structure.fields();

    match fields.len() {
        0 => {}
        1 => {
            value_display_fmt(&fields[0], f, type_annotate)?;
            f.write_char(',')?;
        }
        _ => {
            for (i, field) in fields.iter().enumerate() {
                value_display_fmt(field, f, type_annotate)?;

                if i + 1 < fields.len() {
                    f.write_str(", ")?;
                }
            }
        }
    }

    f.write_char(')')
}

impl<'a> Default for Structure<'a> {
    fn default() -> Self {
        let signature = Signature::from_static_str_unchecked("()");

        Self {
            fields: vec![],
            signature,
        }
    }
}

impl<'a> DynamicType for Structure<'a> {
    fn dynamic_signature(&self) -> Signature<'_> {
        self.signature.as_ref()
    }
}

impl<'a> DynamicType for StructureSeed<'a> {
    fn dynamic_signature(&self) -> Signature<'_> {
        self.0.as_ref()
    }
}

impl<'a> DynamicDeserialize<'a> for Structure<'a> {
    type Deserializer = StructureSeed<'a>;

    fn deserializer_for_signature<S>(signature: S) -> zvariant::Result<Self::Deserializer>
    where
        S: TryInto<Signature<'a>>,
        S::Error: Into<zvariant::Error>,
    {
        let mut signature = signature.try_into().map_err(Into::into)?;
        if !signature.starts_with(zvariant::STRUCT_SIG_START_CHAR) {
            // This is certainly not a valid struct signature
            signature = format!("({signature})").try_into()?;
            return signature.try_into();
        }

        // The signature might be something like "(i)u(i)" - we need to parse it to check.
        let mut parser = SignatureParser::new(signature.as_ref());
        parser.parse_next_signature()?;
        if !parser.done() {
            // more than one element - we must wrap it
            signature = format!("({signature})").try_into()?;
        }
        signature.try_into()
    }
}

impl<'a> Serialize for Structure<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut structure =
            serializer.serialize_tuple_struct("zvariant::Structure", self.fields.len())?;
        for field in &self.fields {
            field.serialize_value_as_tuple_struct_field(&mut structure)?;
        }
        structure.end()
    }
}

macro_rules! tuple_impls {
    ($($len:expr => ($($n:tt $name:ident)+))+) => {
        $(
            impl<'a, $($name),+> From<($($name),+,)> for Structure<'a>
            where
                $($name: DynamicType + Into<Value<'a>>,)+
            {
                #[inline]
                fn from(value: ($($name),+,)) -> Self {
                    StructureBuilder::new()
                    $(
                        .add_field(value. $n)
                    )+
                    .build()
                }
            }

            impl<'a, E, $($name),+> TryFrom<Structure<'a>> for ($($name),+,)
            where
                $($name: TryFrom<Value<'a>, Error = E>,)+
                crate::Error: From<E>,

            {
                type Error = crate::Error;

                fn try_from(mut s: Structure<'a>) -> core::result::Result<Self, Self::Error> {
                    Ok((
                    $(
                         $name::try_from(s.fields.remove(0))?,
                    )+
                    ))
                }
            }

            impl<'a, E, $($name),+> TryFrom<Value<'a>> for ($($name),+,)
            where
                $($name: TryFrom<Value<'a>, Error = E>,)+
                crate::Error: From<E>,

            {
                type Error = crate::Error;

                fn try_from(v: Value<'a>) -> core::result::Result<Self, Self::Error> {
                    Self::try_from(Structure::try_from(v)?)
                }
            }

            impl<E, $($name),+> TryFrom<OwnedValue> for ($($name),+,)
            where
                $($name: TryFrom<Value<'static>, Error = E>,)+
                crate::Error: From<E>,

            {
                type Error = crate::Error;

                fn try_from(v: OwnedValue) -> core::result::Result<Self, Self::Error> {
                    Self::try_from(Value::from(v))
                }
            }
        )+
    }
}

tuple_impls! {
    1 => (0 T0)
    2 => (0 T0 1 T1)
    3 => (0 T0 1 T1 2 T2)
    4 => (0 T0 1 T1 2 T2 3 T3)
    5 => (0 T0 1 T1 2 T2 3 T3 4 T4)
    6 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5)
    7 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6)
    8 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7)
    9 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8)
    10 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8 9 T9)
    11 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8 9 T9 10 T10)
    12 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8 9 T9 10 T10 11 T11)
    13 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8 9 T9 10 T10 11 T11 12 T12)
    14 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8 9 T9 10 T10 11 T11 12 T12 13 T13)
    15 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8 9 T9 10 T10 11 T11 12 T12 13 T13 14 T14)
    16 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8 9 T9 10 T10 11 T11 12 T12 13 T13 14 T14 15 T15)
}

fn create_signature_from_fields(fields: &[Value<'_>]) -> Signature<'static> {
    let mut signature = String::with_capacity(255);
    signature.push('(');
    for field in fields {
        signature.push_str(&field.value_signature());
    }
    signature.push(')');

    Signature::from_string_unchecked(signature)
}

/// Owned [`Structure`]
#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct OwnedStructure(pub Structure<'static>);

/// Use this to deserialize an [`OwnedStructure`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedStructureSeed(Signature<'static>);

impl DynamicType for OwnedStructure {
    fn dynamic_signature(&self) -> Signature<'_> {
        self.0.dynamic_signature()
    }
}

impl DynamicType for OwnedStructureSeed {
    fn dynamic_signature(&self) -> Signature<'_> {
        self.0.clone()
    }
}

impl<'de> DynamicDeserialize<'de> for OwnedStructure {
    type Deserializer = OwnedStructureSeed;

    fn deserializer_for_signature<S>(signature: S) -> zvariant::Result<Self::Deserializer>
    where
        S: TryInto<Signature<'de>>,
        S::Error: Into<zvariant::Error>,
    {
        Structure::deserializer_for_signature(signature)
            .map(|StructureSeed(s)| OwnedStructureSeed(s.to_owned()))
    }
}

impl<'de> DeserializeSeed<'de> for OwnedStructureSeed {
    type Value = OwnedStructure;
    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        deserializer
            .deserialize_seq(StructureVisitor { signature: self.0 })
            .and_then(|s| match s.try_to_owned() {
                Ok(s) => Ok(OwnedStructure(s)),
                Err(e) => Err(D::Error::custom(e)),
            })
    }
}

impl Serialize for OwnedStructure {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.serialize(serializer)
    }
}
