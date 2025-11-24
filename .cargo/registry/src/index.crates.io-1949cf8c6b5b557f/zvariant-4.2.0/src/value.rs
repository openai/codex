use core::{
    cmp::Ordering,
    fmt::{Display, Write},
    hash::{Hash, Hasher},
    marker::PhantomData,
    mem::discriminant,
    str,
};

use serde::{
    de::{
        Deserialize, DeserializeSeed, Deserializer, Error, MapAccess, SeqAccess, Unexpected,
        Visitor,
    },
    ser::{
        Serialize, SerializeMap, SerializeSeq, SerializeStruct, SerializeTupleStruct, Serializer,
    },
};
use static_assertions::assert_impl_all;

use crate::{
    array_display_fmt, dict_display_fmt, signature_parser::SignatureParser, structure_display_fmt,
    utils::*, Array, Basic, Dict, DynamicType, ObjectPath, OwnedValue, Signature, Str, Structure,
    StructureBuilder, Type,
};
#[cfg(feature = "gvariant")]
use crate::{maybe_display_fmt, Maybe};

#[cfg(unix)]
use crate::Fd;

/// A generic container, in the form of an enum that holds exactly one value of any of the other
/// types.
///
/// Note that this type corresponds to the `VARIANT` data type defined by the [D-Bus specification]
/// and as such, its encoding is not the same as that of the enclosed value.
///
/// # Examples
///
/// ```
/// use zvariant::{to_bytes, serialized::Context, Value, LE};
///
/// // Create a Value from an i16
/// let v = Value::new(i16::max_value());
///
/// // Encode it
/// let ctxt = Context::new_dbus(LE, 0);
/// let encoding = to_bytes(ctxt, &v).unwrap();
///
/// // Decode it back
/// let v: Value = encoding.deserialize().unwrap().0;
///
/// // Check everything is as expected
/// assert_eq!(i16::try_from(&v).unwrap(), i16::max_value());
/// ```
///
/// Now let's try a more complicated example:
///
/// ```
/// use zvariant::{to_bytes, serialized::Context, LE};
/// use zvariant::{Structure, Value, Str};
///
/// // Create a Value from a tuple this time
/// let v = Value::new((i16::max_value(), "hello", true));
///
/// // Same drill as previous example
/// let ctxt = Context::new_dbus(LE, 0);
/// let encoding = to_bytes(ctxt, &v).unwrap();
/// let v: Value = encoding.deserialize().unwrap().0;
///
/// // Check everything is as expected
/// let s = Structure::try_from(v).unwrap();
/// assert_eq!(
///     <(i16, Str, bool)>::try_from(s).unwrap(),
///     (i16::max_value(), Str::from("hello"), true),
/// );
/// ```
///
/// [D-Bus specification]: https://dbus.freedesktop.org/doc/dbus-specification.html#container-types
#[derive(Debug, PartialEq, PartialOrd)]
pub enum Value<'a> {
    // Simple types
    U8(u8),
    Bool(bool),
    I16(i16),
    U16(u16),
    I32(i32),
    U32(u32),
    I64(i64),
    U64(u64),
    F64(f64),
    Str(Str<'a>),
    Signature(Signature<'a>),
    ObjectPath(ObjectPath<'a>),
    Value(Box<Value<'a>>),

    // Container types
    Array(Array<'a>),
    Dict(Dict<'a, 'a>),
    Structure(Structure<'a>),
    #[cfg(feature = "gvariant")]
    Maybe(Maybe<'a>),

    #[cfg(unix)]
    Fd(Fd<'a>),
}

impl Hash for Value<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        discriminant(self).hash(state);
        match self {
            Self::U8(inner) => inner.hash(state),
            Self::Bool(inner) => inner.hash(state),
            Self::I16(inner) => inner.hash(state),
            Self::U16(inner) => inner.hash(state),
            Self::I32(inner) => inner.hash(state),
            Self::U32(inner) => inner.hash(state),
            Self::I64(inner) => inner.hash(state),
            Self::U64(inner) => inner.hash(state),
            // To hold the +0.0 == -0.0 => hash(+0.0) == hash(-0.0) property.
            // See https://doc.rust-lang.org/beta/std/hash/trait.Hash.html#hash-and-eq
            Self::F64(inner) if *inner == 0. => 0f64.to_le_bytes().hash(state),
            Self::F64(inner) => inner.to_le_bytes().hash(state),
            Self::Str(inner) => inner.hash(state),
            Self::Signature(inner) => inner.hash(state),
            Self::ObjectPath(inner) => inner.hash(state),
            Self::Value(inner) => inner.hash(state),
            Self::Array(inner) => inner.hash(state),
            Self::Dict(inner) => inner.hash(state),
            Self::Structure(inner) => inner.hash(state),
            #[cfg(feature = "gvariant")]
            Self::Maybe(inner) => inner.hash(state),
            #[cfg(unix)]
            Self::Fd(inner) => inner.hash(state),
        }
    }
}

impl Eq for Value<'_> {}

impl Ord for Value<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other)
            .unwrap_or_else(|| match (self, other) {
                (Self::F64(lhs), Self::F64(rhs)) => lhs.total_cmp(rhs),
                // `partial_cmp` returns `Some(_)` if either the discriminants are different
                // or if both the left hand side and right hand side is `Self::F64(_)`,
                // because `f64` is the only type in this enum, that does not implement `Ord`.
                // This `match`-arm is therefore unreachable.
                _ => unreachable!(),
            })
    }
}

assert_impl_all!(Value<'_>: Send, Sync, Unpin);

macro_rules! serialize_value {
    ($self:ident $serializer:ident.$method:ident $($first_arg:expr)*) => {
        match $self {
            Value::U8(value) => $serializer.$method($($first_arg,)* value),
            Value::Bool(value) => $serializer.$method($($first_arg,)* value),
            Value::I16(value) => $serializer.$method($($first_arg,)* value),
            Value::U16(value) => $serializer.$method($($first_arg,)* value),
            Value::I32(value) => $serializer.$method($($first_arg,)* value),
            Value::U32(value) => $serializer.$method($($first_arg,)* value),
            Value::I64(value) => $serializer.$method($($first_arg,)* value),
            Value::U64(value) => $serializer.$method($($first_arg,)* value),
            Value::F64(value) => $serializer.$method($($first_arg,)* value),
            Value::Str(value) => $serializer.$method($($first_arg,)* value),
            Value::Signature(value) => $serializer.$method($($first_arg,)* value),
            Value::ObjectPath(value) => $serializer.$method($($first_arg,)* value),
            Value::Value(value) => $serializer.$method($($first_arg,)* value),

            // Container types
            Value::Array(value) => $serializer.$method($($first_arg,)* value),
            Value::Dict(value) => $serializer.$method($($first_arg,)* value),
            Value::Structure(value) => $serializer.$method($($first_arg,)* value),
            #[cfg(feature = "gvariant")]
            Value::Maybe(value) => $serializer.$method($($first_arg,)* value),

            #[cfg(unix)]
            Value::Fd(value) => $serializer.$method($($first_arg,)* value),
        }
    }
}

impl<'a> Value<'a> {
    /// Make a [`Value`] for a given value.
    ///
    /// In general, you can use [`Into`] trait on basic types, except
    /// when you explicitly need to wrap [`Value`] itself, in which
    /// case this constructor comes handy.
    ///
    /// # Examples
    ///
    /// ```
    /// use zvariant::Value;
    ///
    /// let s = Value::new("hello");
    /// let u: Value = 51.into();
    /// assert_ne!(s, u);
    /// ```
    ///
    /// [`Value`]: enum.Value.html
    /// [`Into`]: https://doc.rust-lang.org/std/convert/trait.Into.html
    pub fn new<T>(value: T) -> Self
    where
        T: Into<Self> + DynamicType,
    {
        // With specialization, we wouldn't have this
        if value.dynamic_signature() == VARIANT_SIGNATURE_STR {
            Self::Value(Box::new(value.into()))
        } else {
            value.into()
        }
    }

    /// Try to create an owned version of `self`.
    ///
    /// # Errors
    ///
    /// This method can currently only fail on Unix platforms for [`Value::Fd`] variant. This
    /// happens when the current process exceeds the maximum number of open file descriptors.
    pub fn try_to_owned(&self) -> crate::Result<OwnedValue> {
        Ok(OwnedValue(match self {
            Value::U8(v) => Value::U8(*v),
            Value::Bool(v) => Value::Bool(*v),
            Value::I16(v) => Value::I16(*v),
            Value::U16(v) => Value::U16(*v),
            Value::I32(v) => Value::I32(*v),
            Value::U32(v) => Value::U32(*v),
            Value::I64(v) => Value::I64(*v),
            Value::U64(v) => Value::U64(*v),
            Value::F64(v) => Value::F64(*v),
            Value::Str(v) => Value::Str(v.to_owned()),
            Value::Signature(v) => Value::Signature(v.to_owned()),
            Value::ObjectPath(v) => Value::ObjectPath(v.to_owned()),
            Value::Value(v) => {
                let o = OwnedValue::try_from(&**v)?;
                Value::Value(Box::new(o.into_inner()))
            }

            Value::Array(v) => Value::Array(v.try_to_owned()?),
            Value::Dict(v) => Value::Dict(v.try_to_owned()?),
            Value::Structure(v) => Value::Structure(v.try_to_owned()?),
            #[cfg(feature = "gvariant")]
            Value::Maybe(v) => Value::Maybe(v.try_to_owned()?),
            #[cfg(unix)]
            Value::Fd(v) => Value::Fd(v.try_to_owned()?),
        }))
    }

    /// Get the signature of the enclosed value.
    pub fn value_signature(&self) -> Signature<'_> {
        match self {
            Value::U8(_) => u8::signature(),
            Value::Bool(_) => bool::signature(),
            Value::I16(_) => i16::signature(),
            Value::U16(_) => u16::signature(),
            Value::I32(_) => i32::signature(),
            Value::U32(_) => u32::signature(),
            Value::I64(_) => i64::signature(),
            Value::U64(_) => u64::signature(),
            Value::F64(_) => f64::signature(),
            Value::Str(_) => <&str>::signature(),
            Value::Signature(_) => Signature::signature(),
            Value::ObjectPath(_) => ObjectPath::signature(),
            Value::Value(_) => Signature::from_static_str_unchecked("v"),

            // Container types
            Value::Array(value) => value.full_signature().as_ref(),
            Value::Dict(value) => value.full_signature().as_ref(),
            Value::Structure(value) => value.full_signature().as_ref(),
            #[cfg(feature = "gvariant")]
            Value::Maybe(value) => value.full_signature().as_ref(),

            #[cfg(unix)]
            Value::Fd(_) => Fd::signature(),
        }
    }

    /// Try to clone the value.
    ///
    /// # Errors
    ///
    /// This method can currently only fail on Unix platforms for [`Value::Fd`] variant containing
    /// an [`Fd::Owned`] variant. This happens when the current process exceeds the maximum number
    /// of open file descriptors.
    pub fn try_clone(&self) -> crate::Result<Self> {
        Ok(match self {
            Value::U8(v) => Value::U8(*v),
            Value::Bool(v) => Value::Bool(*v),
            Value::I16(v) => Value::I16(*v),
            Value::U16(v) => Value::U16(*v),
            Value::I32(v) => Value::I32(*v),
            Value::U32(v) => Value::U32(*v),
            Value::I64(v) => Value::I64(*v),
            Value::U64(v) => Value::U64(*v),
            Value::F64(v) => Value::F64(*v),
            Value::Str(v) => Value::Str(v.clone()),
            Value::Signature(v) => Value::Signature(v.clone()),
            Value::ObjectPath(v) => Value::ObjectPath(v.clone()),
            Value::Value(v) => Value::Value(Box::new(v.try_clone()?)),
            Value::Array(v) => Value::Array(v.try_clone()?),
            Value::Dict(v) => Value::Dict(v.try_clone()?),
            Value::Structure(v) => Value::Structure(v.try_clone()?),
            #[cfg(feature = "gvariant")]
            Value::Maybe(v) => Value::Maybe(v.try_clone()?),
            #[cfg(unix)]
            Value::Fd(v) => Value::Fd(v.try_clone()?),
        })
    }

    pub(crate) fn serialize_value_as_struct_field<S>(
        &self,
        name: &'static str,
        serializer: &mut S,
    ) -> Result<(), S::Error>
    where
        S: SerializeStruct,
    {
        serialize_value!(self serializer.serialize_field name)
    }

    pub(crate) fn serialize_value_as_tuple_struct_field<S>(
        &self,
        serializer: &mut S,
    ) -> Result<(), S::Error>
    where
        S: SerializeTupleStruct,
    {
        serialize_value!(self serializer.serialize_field)
    }

    // Really crappy that we need to do this separately for struct and seq cases. :(
    pub(crate) fn serialize_value_as_seq_element<S>(
        &self,
        serializer: &mut S,
    ) -> Result<(), S::Error>
    where
        S: SerializeSeq,
    {
        serialize_value!(self serializer.serialize_element)
    }

    pub(crate) fn serialize_value_as_dict_key<S>(&self, serializer: &mut S) -> Result<(), S::Error>
    where
        S: SerializeMap,
    {
        serialize_value!(self serializer.serialize_key)
    }

    pub(crate) fn serialize_value_as_dict_value<S>(
        &self,
        serializer: &mut S,
    ) -> Result<(), S::Error>
    where
        S: SerializeMap,
    {
        serialize_value!(self serializer.serialize_value)
    }

    #[cfg(feature = "gvariant")]
    pub(crate) fn serialize_value_as_some<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_value!(self serializer.serialize_some)
    }

    /// Try to get the underlying type `T`.
    ///
    /// Note that [`TryFrom<Value>`] is implemented for various types, and it's usually best to use
    /// that instead. However, in generic code where you also want to unwrap [`Value::Value`],
    /// you should use this function (because [`TryFrom<Value>`] can not be implemented for `Value`
    /// itself as [`From<Value>`] is implicitly implemented for `Value`).
    ///
    /// # Examples
    ///
    /// ```
    /// use zvariant::{Error, Result, Value};
    ///
    /// fn value_vec_to_type_vec<'a, T>(values: Vec<Value<'a>>) -> Result<Vec<T>>
    /// where
    ///     T: TryFrom<Value<'a>>,
    ///     <T as TryFrom<Value<'a>>>::Error: Into<Error>,
    /// {
    ///     let mut res = vec![];
    ///     for value in values.into_iter() {
    ///         res.push(value.downcast()?);
    ///     }
    ///
    ///     Ok(res)
    /// }
    ///
    /// // Let's try u32 values first
    /// let v = vec![Value::U32(42), Value::U32(43)];
    /// let v = value_vec_to_type_vec::<u32>(v).unwrap();
    /// assert_eq!(v[0], 42);
    /// assert_eq!(v[1], 43);
    ///
    /// // Now try Value values
    /// let v = vec![Value::new(Value::U32(42)), Value::new(Value::U32(43))];
    /// let v = value_vec_to_type_vec::<Value>(v).unwrap();
    /// assert_eq!(v[0], Value::U32(42));
    /// assert_eq!(v[1], Value::U32(43));
    /// ```
    ///
    /// [`Value::Value`]: enum.Value.html#variant.Value
    /// [`TryFrom<Value>`]: https://doc.rust-lang.org/std/convert/trait.TryFrom.html
    /// [`From<Value>`]: https://doc.rust-lang.org/std/convert/trait.From.html
    pub fn downcast<T>(self) -> Result<T, crate::Error>
    where
        T: ?Sized + TryFrom<Value<'a>>,
        <T as TryFrom<Value<'a>>>::Error: Into<crate::Error>,
    {
        if let Value::Value(v) = self {
            T::try_from(*v)
        } else {
            T::try_from(self)
        }
        .map_err(Into::into)
    }

    /// Try to get the underlying type `T`.
    ///
    /// Same as [`downcast`] except it doesn't consume `self` and hence requires
    /// `T: TryFrom<&Value<_>>`.
    ///
    /// # Examples
    ///
    /// ```
    /// use zvariant::{Error, Result, Value};
    ///
    /// fn value_vec_to_type_vec<'a, T>(values: &'a Vec<Value<'a>>) -> Result<Vec<&'a T>>
    /// where
    ///     &'a T: TryFrom<&'a Value<'a>>,
    ///     <&'a T as TryFrom<&'a Value<'a>>>::Error: Into<Error>,
    /// {
    ///     let mut res = vec![];
    ///     for value in values.into_iter() {
    ///         res.push(value.downcast_ref()?);
    ///     }
    ///
    ///     Ok(res)
    /// }
    ///
    /// // Let's try u32 values first
    /// let v = vec![Value::U32(42), Value::U32(43)];
    /// let v = value_vec_to_type_vec::<u32>(&v).unwrap();
    /// assert_eq!(*v[0], 42);
    /// assert_eq!(*v[1], 43);
    ///
    /// // Now try Value values
    /// let v = vec![Value::new(Value::U32(42)), Value::new(Value::U32(43))];
    /// let v = value_vec_to_type_vec::<Value>(&v).unwrap();
    /// assert_eq!(*v[0], Value::U32(42));
    /// assert_eq!(*v[1], Value::U32(43));
    /// ```
    ///
    /// [`downcast`]: enum.Value.html#method.downcast
    pub fn downcast_ref<T>(&'a self) -> Result<T, crate::Error>
    where
        T: ?Sized + TryFrom<&'a Value<'a>>,
        <T as TryFrom<&'a Value<'a>>>::Error: Into<crate::Error>,
    {
        if let Value::Value(v) = self {
            <T>::try_from(v)
        } else {
            <T>::try_from(self)
        }
        .map_err(Into::into)
    }
}

impl Display for Value<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        value_display_fmt(self, f, true)
    }
}

/// Implemented based on https://gitlab.gnome.org/GNOME/glib/-/blob/e1d47f0b0d0893ac9171e24cc7bf635495376546/glib/gvariant.c#L2213
pub(crate) fn value_display_fmt(
    value: &Value<'_>,
    f: &mut std::fmt::Formatter<'_>,
    type_annotate: bool,
) -> std::fmt::Result {
    match value {
        Value::U8(num) => {
            if type_annotate {
                f.write_str("byte ")?;
            }
            write!(f, "0x{:02x}", num)
        }
        Value::Bool(boolean) => {
            write!(f, "{}", boolean)
        }
        Value::I16(num) => {
            if type_annotate {
                f.write_str("int16 ")?;
            }
            write!(f, "{}", num)
        }
        Value::U16(num) => {
            if type_annotate {
                f.write_str("uint16 ")?;
            }
            write!(f, "{}", num)
        }
        Value::I32(num) => {
            // Never annotate this type because it is the default for numbers
            write!(f, "{}", num)
        }
        Value::U32(num) => {
            if type_annotate {
                f.write_str("uint32 ")?;
            }
            write!(f, "{}", num)
        }
        Value::I64(num) => {
            if type_annotate {
                f.write_str("int64 ")?;
            }
            write!(f, "{}", num)
        }
        Value::U64(num) => {
            if type_annotate {
                f.write_str("uint64 ")?;
            }
            write!(f, "{}", num)
        }
        Value::F64(num) => {
            if num.fract() == 0. {
                // Add a dot to make it clear that this is a float
                write!(f, "{}.", num)
            } else {
                write!(f, "{}", num)
            }
        }
        Value::Str(string) => {
            write!(f, "{:?}", string.as_str())
        }
        Value::Signature(val) => {
            if type_annotate {
                f.write_str("signature ")?;
            }
            write!(f, "{:?}", val.as_str())
        }
        Value::ObjectPath(val) => {
            if type_annotate {
                f.write_str("objectpath ")?;
            }
            write!(f, "{:?}", val.as_str())
        }
        Value::Value(child) => {
            f.write_char('<')?;

            // Always annotate types in nested variants, because they are (by nature) of
            // variable type.
            value_display_fmt(child, f, true)?;

            f.write_char('>')?;
            Ok(())
        }
        Value::Array(array) => array_display_fmt(array, f, type_annotate),
        Value::Dict(dict) => dict_display_fmt(dict, f, type_annotate),
        Value::Structure(structure) => structure_display_fmt(structure, f, type_annotate),
        #[cfg(feature = "gvariant")]
        Value::Maybe(maybe) => maybe_display_fmt(maybe, f, type_annotate),
        #[cfg(unix)]
        Value::Fd(handle) => {
            if type_annotate {
                f.write_str("handle ")?;
            }
            write!(f, "{}", handle)
        }
    }
}

impl<'a> Serialize for Value<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Serializer implementation needs to ensure padding isn't added for Value.
        let mut structure = serializer.serialize_struct("zvariant::Value", 2)?;

        let signature = self.value_signature();
        structure.serialize_field("zvariant::Value::Signature", &signature)?;

        self.serialize_value_as_struct_field("zvariant::Value::Value", &mut structure)?;

        structure.end()
    }
}

impl<'de: 'a, 'a> Deserialize<'de> for Value<'a> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let visitor = ValueVisitor;

        deserializer.deserialize_any(visitor)
    }
}

// Note that the Visitor implementations don't check for validity of the
// signature. That's left to the Deserialize implementation of Signature
// itself.

struct ValueVisitor;

impl<'de> Visitor<'de> for ValueVisitor {
    type Value = Value<'de>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a Value")
    }

    fn visit_seq<V>(self, mut visitor: V) -> Result<Value<'de>, V::Error>
    where
        V: SeqAccess<'de>,
    {
        let signature = visitor.next_element::<Signature<'_>>()?.ok_or_else(|| {
            Error::invalid_value(Unexpected::Other("nothing"), &"a Value signature")
        })?;
        let seed = ValueSeed::<Value<'_>> {
            signature,
            phantom: PhantomData,
        };

        visitor
            .next_element_seed(seed)?
            .ok_or_else(|| Error::invalid_value(Unexpected::Other("nothing"), &"a Value value"))
    }

    fn visit_map<V>(self, mut visitor: V) -> Result<Value<'de>, V::Error>
    where
        V: MapAccess<'de>,
    {
        let (_, signature) = visitor
            .next_entry::<&str, Signature<'_>>()?
            .ok_or_else(|| {
                Error::invalid_value(Unexpected::Other("nothing"), &"a Value signature")
            })?;
        let _ = visitor.next_key::<&str>()?;

        let seed = ValueSeed::<Value<'_>> {
            signature,
            phantom: PhantomData,
        };
        visitor.next_value_seed(seed)
    }
}

pub(crate) struct SignatureSeed<'de> {
    pub signature: Signature<'de>,
}

impl<'de> SignatureSeed<'de> {
    pub(crate) fn visit_array<V>(self, mut visitor: V) -> Result<Array<'de>, V::Error>
    where
        V: SeqAccess<'de>,
    {
        let element_signature = self.signature.slice(1..);
        let mut array = Array::new_full_signature(self.signature.clone());

        while let Some(elem) = visitor.next_element_seed(ValueSeed::<Value<'_>> {
            signature: element_signature.clone(),
            phantom: PhantomData,
        })? {
            elem.value_signature();
            array.append(elem).map_err(Error::custom)?;
        }

        Ok(array)
    }

    pub(crate) fn visit_struct<V>(self, mut visitor: V) -> Result<Structure<'de>, V::Error>
    where
        V: SeqAccess<'de>,
    {
        let mut i = 1;
        let signature_end = self.signature.len() - 1;
        let mut builder = StructureBuilder::new();
        while i < signature_end {
            let fields_signature = self.signature.slice(i..signature_end);
            let parser = SignatureParser::new(fields_signature.as_ref());
            let len = parser.next_signature().map_err(Error::custom)?.len();
            let field_signature = fields_signature.slice(0..len);
            i += field_signature.len();

            if let Some(field) = visitor.next_element_seed(ValueSeed::<Value<'_>> {
                signature: field_signature,
                phantom: PhantomData,
            })? {
                builder = builder.append_field(field);
            }
        }
        Ok(builder.build_with_signature(self.signature))
    }
}

impl<'de, T> From<ValueSeed<'de, T>> for SignatureSeed<'de> {
    fn from(seed: ValueSeed<'de, T>) -> Self {
        SignatureSeed {
            signature: seed.signature,
        }
    }
}

struct ValueSeed<'de, T> {
    signature: Signature<'de>,
    phantom: PhantomData<T>,
}

impl<'de, T> ValueSeed<'de, T>
where
    T: Deserialize<'de>,
{
    #[inline]
    fn visit_array<V>(self, visitor: V) -> Result<Value<'de>, V::Error>
    where
        V: SeqAccess<'de>,
    {
        SignatureSeed::from(self)
            .visit_array(visitor)
            .map(Value::Array)
    }

    #[inline]
    fn visit_struct<V>(self, visitor: V) -> Result<Value<'de>, V::Error>
    where
        V: SeqAccess<'de>,
    {
        SignatureSeed::from(self)
            .visit_struct(visitor)
            .map(Value::Structure)
    }

    #[inline]
    fn visit_variant<V>(self, visitor: V) -> Result<Value<'de>, V::Error>
    where
        V: SeqAccess<'de>,
    {
        ValueVisitor
            .visit_seq(visitor)
            .map(|v| Value::Value(Box::new(v)))
    }
}

macro_rules! value_seed_basic_method {
    ($name:ident, $type:ty) => {
        #[inline]
        fn $name<E>(self, value: $type) -> Result<Value<'de>, E>
        where
            E: serde::de::Error,
        {
            Ok(value.into())
        }
    };
}

macro_rules! value_seed_str_method {
    ($name:ident, $type:ty, $constructor:ident) => {
        fn $name<E>(self, value: $type) -> Result<Value<'de>, E>
        where
            E: serde::de::Error,
        {
            match self.signature.as_str() {
                <&str>::SIGNATURE_STR => Ok(Value::Str(Str::from(value))),
                Signature::SIGNATURE_STR => Ok(Value::Signature(Signature::$constructor(value))),
                ObjectPath::SIGNATURE_STR => Ok(Value::ObjectPath(ObjectPath::$constructor(value))),
                _ => {
                    let expected = format!(
                        "`{}`, `{}` or `{}`",
                        <&str>::SIGNATURE_STR,
                        Signature::SIGNATURE_STR,
                        ObjectPath::SIGNATURE_STR,
                    );
                    Err(Error::invalid_type(
                        Unexpected::Str(self.signature.as_str()),
                        &expected.as_str(),
                    ))
                }
            }
        }
    };
}

impl<'de, T> Visitor<'de> for ValueSeed<'de, T>
where
    T: Deserialize<'de>,
{
    type Value = Value<'de>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a Value value")
    }

    value_seed_basic_method!(visit_bool, bool);
    value_seed_basic_method!(visit_i16, i16);
    value_seed_basic_method!(visit_i64, i64);
    value_seed_basic_method!(visit_u8, u8);
    value_seed_basic_method!(visit_u16, u16);
    value_seed_basic_method!(visit_u32, u32);
    value_seed_basic_method!(visit_u64, u64);
    value_seed_basic_method!(visit_f64, f64);

    fn visit_i32<E>(self, value: i32) -> Result<Value<'de>, E>
    where
        E: serde::de::Error,
    {
        let v = match self.signature.as_bytes().first().ok_or_else(|| {
            Error::invalid_value(
                Unexpected::Other("nothing"),
                &"i32 or fd signature character",
            )
        })? {
            #[cfg(unix)]
            b'h' => {
                // SAFETY: The `'de` lifetimes will ensure the borrow won't outlive the raw FD.
                let fd = unsafe { std::os::fd::BorrowedFd::borrow_raw(value) };
                Fd::Borrowed(fd).into()
            }
            _ => value.into(),
        };

        Ok(v)
    }

    #[inline]
    fn visit_str<E>(self, value: &str) -> Result<Value<'de>, E>
    where
        E: serde::de::Error,
    {
        self.visit_string(String::from(value))
    }

    value_seed_str_method!(visit_borrowed_str, &'de str, from_str_unchecked);

    fn visit_seq<V>(self, visitor: V) -> Result<Value<'de>, V::Error>
    where
        V: SeqAccess<'de>,
    {
        match self.signature.as_bytes().first().ok_or_else(|| {
            Error::invalid_value(
                Unexpected::Other("nothing"),
                &"Array or Struct signature character",
            )
        })? {
            // For some reason rustc doesn't like us using ARRAY_SIGNATURE_CHAR const
            b'a' => self.visit_array(visitor),
            b'(' => self.visit_struct(visitor),
            b'v' => self.visit_variant(visitor),
            b => Err(Error::invalid_value(
                Unexpected::Char(*b as char),
                &"a Value signature",
            )),
        }
    }

    fn visit_map<V>(self, mut visitor: V) -> Result<Value<'de>, V::Error>
    where
        V: MapAccess<'de>,
    {
        if self.signature.len() < 5 {
            return Err(serde::de::Error::invalid_length(
                self.signature.len(),
                &">= 5 characters in dict entry signature",
            ));
        }
        let key_signature = self.signature.slice(2..3);
        let signature_end = self.signature.len() - 1;
        let value_signature = self.signature.slice(3..signature_end);
        let mut dict = Dict::new_full_signature(self.signature.clone());

        while let Some((key, value)) = visitor.next_entry_seed(
            ValueSeed::<Value<'_>> {
                signature: key_signature.clone(),
                phantom: PhantomData,
            },
            ValueSeed::<Value<'_>> {
                signature: value_signature.clone(),
                phantom: PhantomData,
            },
        )? {
            dict.append(key, value).map_err(Error::custom)?;
        }

        Ok(Value::Dict(dict))
    }

    #[cfg(feature = "gvariant")]
    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        let visitor = ValueSeed::<T> {
            signature: self.signature.slice(1..),
            phantom: PhantomData,
        };

        deserializer
            .deserialize_any(visitor)
            .map(|v| Value::Maybe(Maybe::just_full_signature(v, self.signature)))
    }

    #[cfg(not(feature = "gvariant"))]
    fn visit_some<D>(self, _deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        panic!("`Maybe` type is only supported for GVariant format but it's disabled");
    }

    #[cfg(feature = "gvariant")]
    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: Error,
    {
        let value = Maybe::nothing_full_signature(self.signature);

        Ok(Value::Maybe(value))
    }

    #[cfg(not(feature = "gvariant"))]
    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: Error,
    {
        panic!("`Maybe` type is only supported for GVariant format but it's disabled");
    }
}

impl<'de, T> DeserializeSeed<'de> for ValueSeed<'de, T>
where
    T: Deserialize<'de>,
{
    type Value = Value<'de>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(self)
    }
}

impl<'a> Type for Value<'a> {
    fn signature() -> Signature<'static> {
        Signature::from_static_str_unchecked(VARIANT_SIGNATURE_STR)
    }
}

impl<'a> TryFrom<&Value<'a>> for Value<'a> {
    type Error = crate::Error;

    fn try_from(value: &Value<'a>) -> crate::Result<Value<'a>> {
        value.try_clone()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn value_display() {
        assert_eq!(
            Value::new((
                255_u8,
                true,
                -1_i16,
                65535_u16,
                -1,
                1_u32,
                -9223372036854775808_i64,
                18446744073709551615_u64,
                (-1., 1.0, 11000000000., 1.1e-10)
            ))
            .to_string(),
            "(byte 0xff, true, int16 -1, uint16 65535, -1, uint32 1, \
                int64 -9223372036854775808, uint64 18446744073709551615, \
                (-1., 1., 11000000000., 0.00000000011))"
        );

        assert_eq!(
            Value::new(vec![
                "", " ", "a", r#"""#, "'", "a'b", "a'\"b", "\\", "\n'\"",
            ])
            .to_string(),
            r#"["", " ", "a", "\"", "'", "a'b", "a'\"b", "\\", "\n'\""]"#
        );
        assert_eq!(
            Value::new(vec![
                "\x07\x08\x09\x0A\x0B\x0C\x0D",
                "\x7F",
                char::from_u32(0xD8000).unwrap().to_string().as_str()
            ])
            .to_string(),
            r#"["\u{7}\u{8}\t\n\u{b}\u{c}\r", "\u{7f}", "\u{d8000}"]"#
        );

        assert_eq!(
            Value::new((
                vec![
                    Signature::from_static_str("").unwrap(),
                    Signature::from_static_str("(ysa{sd})").unwrap(),
                ],
                vec![
                    ObjectPath::from_static_str("/").unwrap(),
                    ObjectPath::from_static_str("/a/very/looooooooooooooooooooooooo0000o0ng/path")
                        .unwrap(),
                ],
                vec![
                    Value::new(0_u8),
                    Value::new((Value::new(51), Value::new(Value::new(1_u32)))),
                ]
            ))
            .to_string(),
            "([signature \"\", \"(ysa{sd})\"], \
                [objectpath \"/\", \"/a/very/looooooooooooooooooooooooo0000o0ng/path\"], \
                [<byte 0x00>, <(<51>, <<uint32 1>>)>])"
        );

        assert_eq!(Value::new(vec![] as Vec<Vec<i64>>).to_string(), "@aax []");
        assert_eq!(
            Value::new(vec![
                vec![0_i16, 1_i16],
                vec![2_i16, 3_i16],
                vec![4_i16, 5_i16]
            ])
            .to_string(),
            "[[int16 0, 1], [2, 3], [4, 5]]"
        );
        assert_eq!(
            Value::new(vec![
                b"Hello".to_vec(),
                b"Hell\0o".to_vec(),
                b"H\0ello\0".to_vec(),
                b"Hello\0".to_vec(),
                b"\0".to_vec(),
                b" \0".to_vec(),
                b"'\0".to_vec(),
                b"\n'\"\0".to_vec(),
                b"\\\0".to_vec(),
            ])
            .to_string(),
            "[[byte 0x48, 0x65, 0x6c, 0x6c, 0x6f], \
                [0x48, 0x65, 0x6c, 0x6c, 0x00, 0x6f], \
                [0x48, 0x00, 0x65, 0x6c, 0x6c, 0x6f, 0x00], \
                b\"Hello\", b\"\", b\" \", b\"'\", b\"\\n'\\\"\", b\"\\\\\"]"
        );

        assert_eq!(
            Value::new(HashMap::<bool, bool>::new()).to_string(),
            "@a{bb} {}"
        );
        assert_eq!(
            Value::new(vec![(true, 0_i64)].into_iter().collect::<HashMap<_, _>>()).to_string(),
            "{true: int64 0}",
        );
        // The order of the entries may vary
        let val = Value::new(
            vec![(32_u16, 64_i64), (100_u16, 200_i64)]
                .into_iter()
                .collect::<HashMap<_, _>>(),
        )
        .to_string();
        assert!(val.starts_with('{'));
        assert!(val.ends_with('}'));
        assert_eq!(val.matches("uint16").count(), 1);
        assert_eq!(val.matches("int64").count(), 1);

        let items_str = val.split(", ").collect::<Vec<_>>();
        assert_eq!(items_str.len(), 2);
        assert!(items_str
            .iter()
            .any(|str| str.contains("32") && str.contains(": ") && str.contains("64")));
        assert!(items_str
            .iter()
            .any(|str| str.contains("100") && str.contains(": ") && str.contains("200")));

        assert_eq!(Value::new(Structure::default()).to_string(), "()");
        assert_eq!(
            Value::new(((true,), (true, false), (true, true, false))).to_string(),
            "((true,), (true, false), (true, true, false))"
        );

        #[cfg(any(feature = "gvariant", feature = "option-as-array"))]
        {
            #[cfg(unix)]
            use std::os::fd::BorrowedFd;

            #[cfg(all(feature = "gvariant", not(feature = "option-as-array")))]
            let s = "((@mn 0, @mmn 0, @mmmn 0), \
                (@mn nothing, @mmn just nothing, @mmmn just just nothing), \
                (@mmn nothing, @mmmn just nothing))";
            #[cfg(feature = "option-as-array")]
            let s = "(([int16 0], [[int16 0]], [[[int16 0]]]), \
                (@an [], [@an []], [[@an []]]), \
                (@aan [], [@aan []]))";
            assert_eq!(
                Value::new((
                    (Some(0_i16), Some(Some(0_i16)), Some(Some(Some(0_i16))),),
                    (None::<i16>, Some(None::<i16>), Some(Some(None::<i16>)),),
                    (None::<Option<i16>>, Some(None::<Option<i16>>)),
                ))
                .to_string(),
                s,
            );

            #[cfg(unix)]
            assert_eq!(
                Value::new(vec![
                    Fd::from(unsafe { BorrowedFd::borrow_raw(0) }),
                    Fd::from(unsafe { BorrowedFd::borrow_raw(-100) })
                ])
                .to_string(),
                "[handle 0, -100]"
            );

            #[cfg(all(feature = "gvariant", not(feature = "option-as-array")))]
            let s = "(@mb nothing, @mb nothing, \
                @ma{sv} {\"size\": <(800, 600)>}, \
                [<1>, <{\"dimension\": <([2.4, 1.], \
                @mmn 200, <(byte 0x03, \"Hello!\")>)>}>], \
                7777, objectpath \"/\", 8888)";
            #[cfg(feature = "option-as-array")]
            let s = "(@ab [], @ab [], [{\"size\": <(800, 600)>}], \
                [<1>, <{\"dimension\": <([2.4, 1.], [[int16 200]], \
                <(byte 0x03, \"Hello!\")>)>}>], 7777, objectpath \"/\", 8888)";
            assert_eq!(
                Value::new((
                    None::<bool>,
                    None::<bool>,
                    Some(
                        vec![("size", Value::new((800, 600)))]
                            .into_iter()
                            .collect::<HashMap<_, _>>()
                    ),
                    vec![
                        Value::new(1),
                        Value::new(
                            vec![(
                                "dimension",
                                Value::new((
                                    vec![2.4, 1.],
                                    Some(Some(200_i16)),
                                    Value::new((3_u8, "Hello!"))
                                ))
                            )]
                            .into_iter()
                            .collect::<HashMap<_, _>>()
                        )
                    ],
                    7777,
                    ObjectPath::from_static_str("/").unwrap(),
                    8888
                ))
                .to_string(),
                s,
            );
        }
    }
}
