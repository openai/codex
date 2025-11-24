use crate::lib::*;

use crate::de::{
    Deserialize, Deserializer, EnumAccess, Error, MapAccess, SeqAccess, Unexpected, VariantAccess,
    Visitor,
};
use crate::private::{self, InPlaceSeed};

#[cfg(any(feature = "std", feature = "alloc"))]
use crate::private::size_hint;

////////////////////////////////////////////////////////////////////////////////

struct UnitVisitor;

impl<'de> Visitor<'de> for UnitVisitor {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("unit")
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: Error,
    {
        Ok(())
    }
}

impl<'de> Deserialize<'de> for () {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_unit(UnitVisitor)
    }
}

#[cfg(feature = "unstable")]
#[cfg_attr(docsrs, doc(cfg(feature = "unstable")))]
impl<'de> Deserialize<'de> for ! {
    fn deserialize<D>(_deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Err(Error::custom("cannot deserialize `!`"))
    }
}

////////////////////////////////////////////////////////////////////////////////

struct BoolVisitor;

impl<'de> Visitor<'de> for BoolVisitor {
    type Value = bool;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a boolean")
    }

    fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
    where
        E: Error,
    {
        Ok(v)
    }
}

impl<'de> Deserialize<'de> for bool {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_bool(BoolVisitor)
    }
}

////////////////////////////////////////////////////////////////////////////////

macro_rules! impl_deserialize_num {
    ($primitive:ident, $nonzero:ident, $deserialize:ident $($method:ident!($($val:ident : $visit:ident)*);)*) => {
        impl_deserialize_num!($primitive, $deserialize $($method!($($val : $visit)*);)*);

        impl<'de> Deserialize<'de> for num::$nonzero {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct NonZeroVisitor;

                impl<'de> Visitor<'de> for NonZeroVisitor {
                    type Value = num::$nonzero;

                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        formatter.write_str(concat!("a nonzero ", stringify!($primitive)))
                    }

                    $($($method!(nonzero $primitive $val : $visit);)*)*
                }

                deserializer.$deserialize(NonZeroVisitor)
            }
        }

        #[cfg(not(no_core_num_saturating))]
        impl<'de> Deserialize<'de> for Saturating<$primitive> {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct SaturatingVisitor;

                impl<'de> Visitor<'de> for SaturatingVisitor {
                    type Value = Saturating<$primitive>;

                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        formatter.write_str("integer with support for saturating semantics")
                    }

                    $($($method!(saturating $primitive $val : $visit);)*)*
                }

                deserializer.$deserialize(SaturatingVisitor)
            }
        }
    };

    ($primitive:ident, $deserialize:ident $($method:ident!($($val:ident : $visit:ident)*);)*) => {
        impl<'de> Deserialize<'de> for $primitive {
            #[inline]
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct PrimitiveVisitor;

                impl<'de> Visitor<'de> for PrimitiveVisitor {
                    type Value = $primitive;

                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        formatter.write_str(stringify!($primitive))
                    }

                    $($($method!($val : $visit);)*)*
                }

                deserializer.$deserialize(PrimitiveVisitor)
            }
        }
    };
}

macro_rules! num_self {
    ($ty:ident : $visit:ident) => {
        #[inline]
        fn $visit<E>(self, v: $ty) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Ok(v)
        }
    };

    (nonzero $primitive:ident $ty:ident : $visit:ident) => {
        fn $visit<E>(self, v: $ty) -> Result<Self::Value, E>
        where
            E: Error,
        {
            if let Some(nonzero) = Self::Value::new(v) {
                Ok(nonzero)
            } else {
                Err(Error::invalid_value(Unexpected::Unsigned(0), &self))
            }
        }
    };

    (saturating $primitive:ident $ty:ident : $visit:ident) => {
        fn $visit<E>(self, v: $ty) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Ok(Saturating(v))
        }
    };
}

macro_rules! num_as_self {
    ($ty:ident : $visit:ident) => {
        #[inline]
        fn $visit<E>(self, v: $ty) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Ok(v as Self::Value)
        }
    };

    (nonzero $primitive:ident $ty:ident : $visit:ident) => {
        fn $visit<E>(self, v: $ty) -> Result<Self::Value, E>
        where
            E: Error,
        {
            if let Some(nonzero) = Self::Value::new(v as $primitive) {
                Ok(nonzero)
            } else {
                Err(Error::invalid_value(Unexpected::Unsigned(0), &self))
            }
        }
    };

    (saturating $primitive:ident $ty:ident : $visit:ident) => {
        fn $visit<E>(self, v: $ty) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Ok(Saturating(v as $primitive))
        }
    };
}

macro_rules! num_as_copysign_self {
    ($ty:ident : $visit:ident) => {
        #[inline]
        fn $visit<E>(self, v: $ty) -> Result<Self::Value, E>
        where
            E: Error,
        {
            #[cfg(not(feature = "std"))]
            {
                Ok(v as Self::Value)
            }

            #[cfg(feature = "std")]
            {
                // Preserve sign of NaN. The `as` produces a nondeterministic sign.
                let sign = if v.is_sign_positive() { 1.0 } else { -1.0 };
                Ok((v as Self::Value).copysign(sign))
            }
        }
    };
}

macro_rules! int_to_int {
    ($ty:ident : $visit:ident) => {
        #[inline]
        fn $visit<E>(self, v: $ty) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Self::Value::try_from(v as i64)
                .map_err(|_| Error::invalid_value(Unexpected::Signed(v as i64), &self))
        }
    };

    (nonzero $primitive:ident $ty:ident : $visit:ident) => {
        fn $visit<E>(self, v: $ty) -> Result<Self::Value, E>
        where
            E: Error,
        {
            if let Ok(v) = $primitive::try_from(v as i64) {
                if let Some(nonzero) = Self::Value::new(v) {
                    return Ok(nonzero);
                }
            }
            Err(Error::invalid_value(Unexpected::Signed(v as i64), &self))
        }
    };

    (saturating $primitive:ident $ty:ident : $visit:ident) => {
        fn $visit<E>(self, v: $ty) -> Result<Self::Value, E>
        where
            E: Error,
        {
            if (v as i64) < $primitive::MIN as i64 {
                Ok(Saturating($primitive::MIN))
            } else if ($primitive::MAX as i64) < v as i64 {
                Ok(Saturating($primitive::MAX))
            } else {
                Ok(Saturating(v as $primitive))
            }
        }
    };
}

macro_rules! int_to_uint {
    ($ty:ident : $visit:ident) => {
        #[inline]
        fn $visit<E>(self, v: $ty) -> Result<Self::Value, E>
        where
            E: Error,
        {
            if 0 <= v {
                #[allow(irrefutable_let_patterns)]
                if let Ok(v) = Self::Value::try_from(v as u64) {
                    return Ok(v as Self::Value);
                }
            }
            Err(Error::invalid_value(Unexpected::Signed(v as i64), &self))
        }
    };

    (nonzero $primitive:ident $ty:ident : $visit:ident) => {
        fn $visit<E>(self, v: $ty) -> Result<Self::Value, E>
        where
            E: Error,
        {
            if 0 < v {
                #[allow(irrefutable_let_patterns)]
                if let Ok(v) = $primitive::try_from(v as u64) {
                    if let Some(nonzero) = Self::Value::new(v) {
                        return Ok(nonzero);
                    }
                }
            }
            Err(Error::invalid_value(Unexpected::Signed(v as i64), &self))
        }
    };

    (saturating $primitive:ident $ty:ident : $visit:ident) => {
        fn $visit<E>(self, v: $ty) -> Result<Self::Value, E>
        where
            E: Error,
        {
            if v < 0 {
                Ok(Saturating(0))
            } else if ($primitive::MAX as u64) < v as u64 {
                Ok(Saturating($primitive::MAX))
            } else {
                Ok(Saturating(v as $primitive))
            }
        }
    };
}

macro_rules! uint_to_self {
    ($ty:ident : $visit:ident) => {
        #[inline]
        fn $visit<E>(self, v: $ty) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Self::Value::try_from(v as u64)
                .map_err(|_| Error::invalid_value(Unexpected::Unsigned(v as u64), &self))
        }
    };

    (nonzero $primitive:ident $ty:ident : $visit:ident) => {
        fn $visit<E>(self, v: $ty) -> Result<Self::Value, E>
        where
            E: Error,
        {
            if let Ok(v) = $primitive::try_from(v as u64) {
                if let Some(nonzero) = Self::Value::new(v) {
                    return Ok(nonzero);
                }
            }
            Err(Error::invalid_value(Unexpected::Unsigned(v as u64), &self))
        }
    };

    (saturating $primitive:ident $ty:ident : $visit:ident) => {
        fn $visit<E>(self, v: $ty) -> Result<Self::Value, E>
        where
            E: Error,
        {
            if let Ok(v) = $primitive::try_from(v as u64) {
                Ok(Saturating(v as $primitive))
            } else {
                Ok(Saturating($primitive::MAX))
            }
        }
    };
}

impl_deserialize_num! {
    i8, NonZeroI8, deserialize_i8
    num_self!(i8:visit_i8);
    int_to_int!(i16:visit_i16 i32:visit_i32 i64:visit_i64);
    uint_to_self!(u8:visit_u8 u16:visit_u16 u32:visit_u32 u64:visit_u64);
}

impl_deserialize_num! {
    i16, NonZeroI16, deserialize_i16
    num_self!(i16:visit_i16);
    num_as_self!(i8:visit_i8);
    int_to_int!(i32:visit_i32 i64:visit_i64);
    uint_to_self!(u8:visit_u8 u16:visit_u16 u32:visit_u32 u64:visit_u64);
}

impl_deserialize_num! {
    i32, NonZeroI32, deserialize_i32
    num_self!(i32:visit_i32);
    num_as_self!(i8:visit_i8 i16:visit_i16);
    int_to_int!(i64:visit_i64);
    uint_to_self!(u8:visit_u8 u16:visit_u16 u32:visit_u32 u64:visit_u64);
}

impl_deserialize_num! {
    i64, NonZeroI64, deserialize_i64
    num_self!(i64:visit_i64);
    num_as_self!(i8:visit_i8 i16:visit_i16 i32:visit_i32);
    uint_to_self!(u8:visit_u8 u16:visit_u16 u32:visit_u32 u64:visit_u64);
}

impl_deserialize_num! {
    isize, NonZeroIsize, deserialize_i64
    num_as_self!(i8:visit_i8 i16:visit_i16);
    int_to_int!(i32:visit_i32 i64:visit_i64);
    uint_to_self!(u8:visit_u8 u16:visit_u16 u32:visit_u32 u64:visit_u64);
}

impl_deserialize_num! {
    u8, NonZeroU8, deserialize_u8
    num_self!(u8:visit_u8);
    int_to_uint!(i8:visit_i8 i16:visit_i16 i32:visit_i32 i64:visit_i64);
    uint_to_self!(u16:visit_u16 u32:visit_u32 u64:visit_u64);
}

impl_deserialize_num! {
    u16, NonZeroU16, deserialize_u16
    num_self!(u16:visit_u16);
    num_as_self!(u8:visit_u8);
    int_to_uint!(i8:visit_i8 i16:visit_i16 i32:visit_i32 i64:visit_i64);
    uint_to_self!(u32:visit_u32 u64:visit_u64);
}

impl_deserialize_num! {
    u32, NonZeroU32, deserialize_u32
    num_self!(u32:visit_u32);
    num_as_self!(u8:visit_u8 u16:visit_u16);
    int_to_uint!(i8:visit_i8 i16:visit_i16 i32:visit_i32 i64:visit_i64);
    uint_to_self!(u64:visit_u64);
}

impl_deserialize_num! {
    u64, NonZeroU64, deserialize_u64
    num_self!(u64:visit_u64);
    num_as_self!(u8:visit_u8 u16:visit_u16 u32:visit_u32);
    int_to_uint!(i8:visit_i8 i16:visit_i16 i32:visit_i32 i64:visit_i64);
}

impl_deserialize_num! {
    usize, NonZeroUsize, deserialize_u64
    num_as_self!(u8:visit_u8 u16:visit_u16);
    int_to_uint!(i8:visit_i8 i16:visit_i16 i32:visit_i32 i64:visit_i64);
    uint_to_self!(u32:visit_u32 u64:visit_u64);
}

impl_deserialize_num! {
    f32, deserialize_f32
    num_self!(f32:visit_f32);
    num_as_copysign_self!(f64:visit_f64);
    num_as_self!(i8:visit_i8 i16:visit_i16 i32:visit_i32 i64:visit_i64);
    num_as_self!(u8:visit_u8 u16:visit_u16 u32:visit_u32 u64:visit_u64);
}

impl_deserialize_num! {
    f64, deserialize_f64
    num_self!(f64:visit_f64);
    num_as_copysign_self!(f32:visit_f32);
    num_as_self!(i8:visit_i8 i16:visit_i16 i32:visit_i32 i64:visit_i64);
    num_as_self!(u8:visit_u8 u16:visit_u16 u32:visit_u32 u64:visit_u64);
}

macro_rules! num_128 {
    ($ty:ident : $visit:ident) => {
        fn $visit<E>(self, v: $ty) -> Result<Self::Value, E>
        where
            E: Error,
        {
            if v as i128 >= Self::Value::MIN as i128 && v as u128 <= Self::Value::MAX as u128 {
                Ok(v as Self::Value)
            } else {
                Err(Error::invalid_value(
                    Unexpected::Other(stringify!($ty)),
                    &self,
                ))
            }
        }
    };

    (nonzero $primitive:ident $ty:ident : $visit:ident) => {
        fn $visit<E>(self, v: $ty) -> Result<Self::Value, E>
        where
            E: Error,
        {
            if v as i128 >= $primitive::MIN as i128 && v as u128 <= $primitive::MAX as u128 {
                if let Some(nonzero) = Self::Value::new(v as $primitive) {
                    Ok(nonzero)
                } else {
                    Err(Error::invalid_value(Unexpected::Unsigned(0), &self))
                }
            } else {
                Err(Error::invalid_value(
                    Unexpected::Other(stringify!($ty)),
                    &self,
                ))
            }
        }
    };

    (saturating $primitive:ident $ty:ident : $visit:ident) => {
        fn $visit<E>(self, v: $ty) -> Result<Self::Value, E>
        where
            E: Error,
        {
            if (v as i128) < $primitive::MIN as i128 {
                Ok(Saturating($primitive::MIN))
            } else if ($primitive::MAX as u128) < v as u128 {
                Ok(Saturating($primitive::MAX))
            } else {
                Ok(Saturating(v as $primitive))
            }
        }
    };
}

impl_deserialize_num! {
    i128, NonZeroI128, deserialize_i128
    num_self!(i128:visit_i128);
    num_as_self!(i8:visit_i8 i16:visit_i16 i32:visit_i32 i64:visit_i64);
    num_as_self!(u8:visit_u8 u16:visit_u16 u32:visit_u32 u64:visit_u64);
    num_128!(u128:visit_u128);
}

impl_deserialize_num! {
    u128, NonZeroU128, deserialize_u128
    num_self!(u128:visit_u128);
    num_as_self!(u8:visit_u8 u16:visit_u16 u32:visit_u32 u64:visit_u64);
    int_to_uint!(i8:visit_i8 i16:visit_i16 i32:visit_i32 i64:visit_i64);
    num_128!(i128:visit_i128);
}

////////////////////////////////////////////////////////////////////////////////

struct CharVisitor;

impl<'de> Visitor<'de> for CharVisitor {
    type Value = char;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a character")
    }

    #[inline]
    fn visit_char<E>(self, v: char) -> Result<Self::Value, E>
    where
        E: Error,
    {
        Ok(v)
    }

    #[inline]
    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: Error,
    {
        let mut iter = v.chars();
        match (iter.next(), iter.next()) {
            (Some(c), None) => Ok(c),
            _ => Err(Error::invalid_value(Unexpected::Str(v), &self)),
        }
    }
}

impl<'de> Deserialize<'de> for char {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_char(CharVisitor)
    }
}

////////////////////////////////////////////////////////////////////////////////

#[cfg(any(feature = "std", feature = "alloc"))]
struct StringVisitor;
#[cfg(any(feature = "std", feature = "alloc"))]
struct StringInPlaceVisitor<'a>(&'a mut String);

#[cfg(any(feature = "std", feature = "alloc"))]
impl<'de> Visitor<'de> for StringVisitor {
    type Value = String;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a string")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: Error,
    {
        Ok(v.to_owned())
    }

    fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
    where
        E: Error,
    {
        Ok(v)
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: Error,
    {
        match str::from_utf8(v) {
            Ok(s) => Ok(s.to_owned()),
            Err(_) => Err(Error::invalid_value(Unexpected::Bytes(v), &self)),
        }
    }

    fn visit_byte_buf<E>(self, v: Vec<u8>) -> Result<Self::Value, E>
    where
        E: Error,
    {
        match String::from_utf8(v) {
            Ok(s) => Ok(s),
            Err(e) => Err(Error::invalid_value(
                Unexpected::Bytes(&e.into_bytes()),
                &self,
            )),
        }
    }
}

#[cfg(any(feature = "std", feature = "alloc"))]
impl<'a, 'de> Visitor<'de> for StringInPlaceVisitor<'a> {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a string")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: Error,
    {
        self.0.clear();
        self.0.push_str(v);
        Ok(())
    }

    fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
    where
        E: Error,
    {
        *self.0 = v;
        Ok(())
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: Error,
    {
        match str::from_utf8(v) {
            Ok(s) => {
                self.0.clear();
                self.0.push_str(s);
                Ok(())
            }
            Err(_) => Err(Error::invalid_value(Unexpected::Bytes(v), &self)),
        }
    }

    fn visit_byte_buf<E>(self, v: Vec<u8>) -> Result<Self::Value, E>
    where
        E: Error,
    {
        match String::from_utf8(v) {
            Ok(s) => {
                *self.0 = s;
                Ok(())
            }
            Err(e) => Err(Error::invalid_value(
                Unexpected::Bytes(&e.into_bytes()),
                &self,
            )),
        }
    }
}

#[cfg(any(feature = "std", feature = "alloc"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
impl<'de> Deserialize<'de> for String {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_string(StringVisitor)
    }

    fn deserialize_in_place<D>(deserializer: D, place: &mut Self) -> Result<(), D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_string(StringInPlaceVisitor(place))
    }
}

////////////////////////////////////////////////////////////////////////////////

struct StrVisitor;

impl<'a> Visitor<'a> for StrVisitor {
    type Value = &'a str;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a borrowed string")
    }

    fn visit_borrowed_str<E>(self, v: &'a str) -> Result<Self::Value, E>
    where
        E: Error,
    {
        Ok(v) // so easy
    }

    fn visit_borrowed_bytes<E>(self, v: &'a [u8]) -> Result<Self::Value, E>
    where
        E: Error,
    {
        str::from_utf8(v).map_err(|_| Error::invalid_value(Unexpected::Bytes(v), &self))
    }
}

impl<'de: 'a, 'a> Deserialize<'de> for &'a str {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(StrVisitor)
    }
}

////////////////////////////////////////////////////////////////////////////////

struct BytesVisitor;

impl<'a> Visitor<'a> for BytesVisitor {
    type Value = &'a [u8];

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a borrowed byte array")
    }

    fn visit_borrowed_bytes<E>(self, v: &'a [u8]) -> Result<Self::Value, E>
    where
        E: Error,
    {
        Ok(v)
    }

    fn visit_borrowed_str<E>(self, v: &'a str) -> Result<Self::Value, E>
    where
        E: Error,
    {
        Ok(v.as_bytes())
    }
}

impl<'de: 'a, 'a> Deserialize<'de> for &'a [u8] {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_bytes(BytesVisitor)
    }
}

////////////////////////////////////////////////////////////////////////////////

#[cfg(any(feature = "std", all(not(no_core_cstr), feature = "alloc")))]
struct CStringVisitor;

#[cfg(any(feature = "std", all(not(no_core_cstr), feature = "alloc")))]
impl<'de> Visitor<'de> for CStringVisitor {
    type Value = CString;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("byte array")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let capacity = size_hint::cautious::<u8>(seq.size_hint());
        let mut values = Vec::<u8>::with_capacity(capacity);

        while let Some(value) = tri!(seq.next_element()) {
            values.push(value);
        }

        CString::new(values).map_err(Error::custom)
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: Error,
    {
        CString::new(v).map_err(Error::custom)
    }

    fn visit_byte_buf<E>(self, v: Vec<u8>) -> Result<Self::Value, E>
    where
        E: Error,
    {
        CString::new(v).map_err(Error::custom)
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: Error,
    {
        CString::new(v).map_err(Error::custom)
    }

    fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
    where
        E: Error,
    {
        CString::new(v).map_err(Error::custom)
    }
}

#[cfg(any(feature = "std", all(not(no_core_cstr), feature = "alloc")))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
impl<'de> Deserialize<'de> for CString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_byte_buf(CStringVisitor)
    }
}

macro_rules! forwarded_impl {
    (
        $(#[$attr:meta])*
        ($($id:ident),*), $ty:ty, $func:expr
    ) => {
        $(#[$attr])*
        impl<'de $(, $id : Deserialize<'de>,)*> Deserialize<'de> for $ty {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                Deserialize::deserialize(deserializer).map($func)
            }
        }
    }
}

forwarded_impl! {
    #[cfg(any(feature = "std", all(not(no_core_cstr), feature = "alloc")))]
    #[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
    (), Box<CStr>, CString::into_boxed_c_str
}

forwarded_impl! {
    (T), Reverse<T>, Reverse
}

////////////////////////////////////////////////////////////////////////////////

struct OptionVisitor<T> {
    marker: PhantomData<T>,
}

impl<'de, T> Visitor<'de> for OptionVisitor<T>
where
    T: Deserialize<'de>,
{
    type Value = Option<T>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("option")
    }

    #[inline]
    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: Error,
    {
        Ok(None)
    }

    #[inline]
    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: Error,
    {
        Ok(None)
    }

    #[inline]
    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        T::deserialize(deserializer).map(Some)
    }

    fn __private_visit_untagged_option<D>(self, deserializer: D) -> Result<Self::Value, ()>
    where
        D: Deserializer<'de>,
    {
        Ok(T::deserialize(deserializer).ok())
    }
}

impl<'de, T> Deserialize<'de> for Option<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_option(OptionVisitor {
            marker: PhantomData,
        })
    }

    // The Some variant's repr is opaque, so we can't play cute tricks with its
    // tag to have deserialize_in_place build the content in place unconditionally.
    //
    // FIXME: investigate whether branching on the old value being Some to
    // deserialize_in_place the value is profitable (probably data-dependent?)
}

////////////////////////////////////////////////////////////////////////////////

struct PhantomDataVisitor<T: ?Sized> {
    marker: PhantomData<T>,
}

impl<'de, T> Visitor<'de> for PhantomDataVisitor<T>
where
    T: ?Sized,
{
    type Value = PhantomData<T>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("unit")
    }

    #[inline]
    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: Error,
    {
        Ok(PhantomData)
    }
}

impl<'de, T> Deserialize<'de> for PhantomData<T>
where
    T: ?Sized,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let visitor = PhantomDataVisitor {
            marker: PhantomData,
        };
        deserializer.deserialize_unit_struct("PhantomData", visitor)
    }
}

////////////////////////////////////////////////////////////////////////////////

macro_rules! seq_impl {
    (
        $(#[$attr:meta])*
        $ty:ident <T $(: $tbound1:ident $(+ $tbound2:ident)*)* $(, $typaram:ident : $bound1:ident $(+ $bound2:ident)*)*>,
        $access:ident,
        $clear:expr,
        $with_capacity:expr,
        $reserve:expr,
        $insert:expr
    ) => {
        $(#[$attr])*
        impl<'de, T $(, $typaram)*> Deserialize<'de> for $ty<T $(, $typaram)*>
        where
            T: Deserialize<'de> $(+ $tbound1 $(+ $tbound2)*)*,
            $($typaram: $bound1 $(+ $bound2)*,)*
        {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct SeqVisitor<T $(, $typaram)*> {
                    marker: PhantomData<$ty<T $(, $typaram)*>>,
                }

                impl<'de, T $(, $typaram)*> Visitor<'de> for SeqVisitor<T $(, $typaram)*>
                where
                    T: Deserialize<'de> $(+ $tbound1 $(+ $tbound2)*)*,
                    $($typaram: $bound1 $(+ $bound2)*,)*
                {
                    type Value = $ty<T $(, $typaram)*>;

                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        formatter.write_str("a sequence")
                    }

                    #[inline]
                    fn visit_seq<A>(self, mut $access: A) -> Result<Self::Value, A::Error>
                    where
                        A: SeqAccess<'de>,
                    {
                        let mut values = $with_capacity;

                        while let Some(value) = tri!($access.next_element()) {
                            $insert(&mut values, value);
                        }

                        Ok(values)
                    }
                }

                let visitor = SeqVisitor { marker: PhantomData };
                deserializer.deserialize_seq(visitor)
            }

            fn deserialize_in_place<D>(deserializer: D, place: &mut Self) -> Result<(), D::Error>
            where
                D: Deserializer<'de>,
            {
                struct SeqInPlaceVisitor<'a, T: 'a $(, $typaram: 'a)*>(&'a mut $ty<T $(, $typaram)*>);

                impl<'a, 'de, T $(, $typaram)*> Visitor<'de> for SeqInPlaceVisitor<'a, T $(, $typaram)*>
                where
                    T: Deserialize<'de> $(+ $tbound1 $(+ $tbound2)*)*,
                    $($typaram: $bound1 $(+ $bound2)*,)*
                {
                    type Value = ();

                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        formatter.write_str("a sequence")
                    }

                    #[inline]
                    fn visit_seq<A>(mut self, mut $access: A) -> Result<Self::Value, A::Error>
                    where
                        A: SeqAccess<'de>,
                    {
                        $clear(&mut self.0);
                        $reserve(&mut self.0, size_hint::cautious::<T>($access.size_hint()));

                        // FIXME: try to overwrite old values here? (Vec, VecDeque, LinkedList)
                        while let Some(value) = tri!($access.next_element()) {
                            $insert(&mut self.0, value);
                        }

                        Ok(())
                    }
                }

                deserializer.deserialize_seq(SeqInPlaceVisitor(place))
            }
        }
    }
}

// Dummy impl of reserve
#[cfg(any(feature = "std", feature = "alloc"))]
fn nop_reserve<T>(_seq: T, _n: usize) {}

seq_impl!(
    #[cfg(any(feature = "std", feature = "alloc"))]
    #[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
    BinaryHeap<T: Ord>,
    seq,
    BinaryHeap::clear,
    BinaryHeap::with_capacity(size_hint::cautious::<T>(seq.size_hint())),
    BinaryHeap::reserve,
    BinaryHeap::push
);

seq_impl!(
    #[cfg(any(feature = "std", feature = "alloc"))]
    #[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
    BTreeSet<T: Eq + Ord>,
    seq,
    BTreeSet::clear,
    BTreeSet::new(),
    nop_reserve,
    BTreeSet::insert
);

seq_impl!(
    #[cfg(any(feature = "std", feature = "alloc"))]
    #[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
    LinkedList<T>,
    seq,
    LinkedList::clear,
    LinkedList::new(),
    nop_reserve,
    LinkedList::push_back
);

seq_impl!(
    #[cfg(feature = "std")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    HashSet<T: Eq + Hash, S: BuildHasher + Default>,
    seq,
    HashSet::clear,
    HashSet::with_capacity_and_hasher(size_hint::cautious::<T>(seq.size_hint()), S::default()),
    HashSet::reserve,
    HashSet::insert
);

seq_impl!(
    #[cfg(any(feature = "std", feature = "alloc"))]
    #[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
    VecDeque<T>,
    seq,
    VecDeque::clear,
    VecDeque::with_capacity(size_hint::cautious::<T>(seq.size_hint())),
    VecDeque::reserve,
    VecDeque::push_back
);

////////////////////////////////////////////////////////////////////////////////

#[cfg(any(feature = "std", feature = "alloc"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
impl<'de, T> Deserialize<'de> for Vec<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct VecVisitor<T> {
            marker: PhantomData<T>,
        }

        impl<'de, T> Visitor<'de> for VecVisitor<T>
        where
            T: Deserialize<'de>,
        {
            type Value = Vec<T>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a sequence")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let capacity = size_hint::cautious::<T>(seq.size_hint());
                let mut values = Vec::<T>::with_capacity(capacity);

                while let Some(value) = tri!(seq.next_element()) {
                    values.push(value);
                }

                Ok(values)
            }
        }

        let visitor = VecVisitor {
            marker: PhantomData,
        };
        deserializer.deserialize_seq(visitor)
    }

    fn deserialize_in_place<D>(deserializer: D, place: &mut Self) -> Result<(), D::Error>
    where
        D: Deserializer<'de>,
    {
        struct VecInPlaceVisitor<'a, T: 'a>(&'a mut Vec<T>);

        impl<'a, 'de, T> Visitor<'de> for VecInPlaceVisitor<'a, T>
        where
            T: Deserialize<'de>,
        {
            type Value = ();

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a sequence")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let hint = size_hint::cautious::<T>(seq.size_hint());
                if let Some(additional) = hint.checked_sub(self.0.len()) {
                    self.0.reserve(additional);
                }

                for i in 0..self.0.len() {
                    let next = {
                        let next_place = InPlaceSeed(&mut self.0[i]);
                        tri!(seq.next_element_seed(next_place))
                    };
                    if next.is_none() {
                        self.0.truncate(i);
                        return Ok(());
                    }
                }

                while let Some(value) = tri!(seq.next_element()) {
                    self.0.push(value);
                }

                Ok(())
            }
        }

        deserializer.deserialize_seq(VecInPlaceVisitor(place))
    }
}

////////////////////////////////////////////////////////////////////////////////

struct ArrayVisitor<A> {
    marker: PhantomData<A>,
}
struct ArrayInPlaceVisitor<'a, A: 'a>(&'a mut A);

impl<A> ArrayVisitor<A> {
    fn new() -> Self {
        ArrayVisitor {
            marker: PhantomData,
        }
    }
}

impl<'de, T> Visitor<'de> for ArrayVisitor<[T; 0]> {
    type Value = [T; 0];

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("an empty array")
    }

    #[inline]
    fn visit_seq<A>(self, _: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        Ok([])
    }
}

// Does not require T: Deserialize<'de>.
impl<'de, T> Deserialize<'de> for [T; 0] {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_tuple(0, ArrayVisitor::<[T; 0]>::new())
    }
}

macro_rules! array_impls {
    ($($len:expr => ($($n:tt)+))+) => {
        $(
            impl<'de, T> Visitor<'de> for ArrayVisitor<[T; $len]>
            where
                T: Deserialize<'de>,
            {
                type Value = [T; $len];

                fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                    formatter.write_str(concat!("an array of length ", $len))
                }

                #[inline]
                fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
                where
                    A: SeqAccess<'de>,
                {
                    Ok([$(
                        match tri!(seq.next_element()) {
                            Some(val) => val,
                            None => return Err(Error::invalid_length($n, &self)),
                        }
                    ),+])
                }
            }

            impl<'a, 'de, T> Visitor<'de> for ArrayInPlaceVisitor<'a, [T; $len]>
            where
                T: Deserialize<'de>,
            {
                type Value = ();

                fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                    formatter.write_str(concat!("an array of length ", $len))
                }

                #[inline]
                fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
                where
                    A: SeqAccess<'de>,
                {
                    let mut fail_idx = None;
                    for (idx, dest) in self.0[..].iter_mut().enumerate() {
                        if tri!(seq.next_element_seed(InPlaceSeed(dest))).is_none() {
                            fail_idx = Some(idx);
                            break;
                        }
                    }
                    if let Some(idx) = fail_idx {
                        return Err(Error::invalid_length(idx, &self));
                    }
                    Ok(())
                }
            }

            impl<'de, T> Deserialize<'de> for [T; $len]
            where
                T: Deserialize<'de>,
            {
                fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                where
                    D: Deserializer<'de>,
                {
                    deserializer.deserialize_tuple($len, ArrayVisitor::<[T; $len]>::new())
                }

                fn deserialize_in_place<D>(deserializer: D, place: &mut Self) -> Result<(), D::Error>
                where
                    D: Deserializer<'de>,
                {
                    deserializer.deserialize_tuple($len, ArrayInPlaceVisitor(place))
                }
            }
        )+
    }
}

array_impls! {
    1 => (0)
    2 => (0 1)
    3 => (0 1 2)
    4 => (0 1 2 3)
    5 => (0 1 2 3 4)
    6 => (0 1 2 3 4 5)
    7 => (0 1 2 3 4 5 6)
    8 => (0 1 2 3 4 5 6 7)
    9 => (0 1 2 3 4 5 6 7 8)
    10 => (0 1 2 3 4 5 6 7 8 9)
    11 => (0 1 2 3 4 5 6 7 8 9 10)
    12 => (0 1 2 3 4 5 6 7 8 9 10 11)
    13 => (0 1 2 3 4 5 6 7 8 9 10 11 12)
    14 => (0 1 2 3 4 5 6 7 8 9 10 11 12 13)
    15 => (0 1 2 3 4 5 6 7 8 9 10 11 12 13 14)
    16 => (0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15)
    17 => (0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16)
    18 => (0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17)
    19 => (0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18)
    20 => (0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19)
    21 => (0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20)
    22 => (0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21)
    23 => (0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22)
    24 => (0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23)
    25 => (0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24)
    26 => (0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25)
    27 => (0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26)
    28 => (0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27)
    29 => (0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28)
    30 => (0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29)
    31 => (0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30)
    32 => (0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30 31)
}

////////////////////////////////////////////////////////////////////////////////

macro_rules! tuple_impls {
    ($($len:tt => ($($n:tt $name:ident)+))+) => {
        $(
            #[cfg_attr(docsrs, doc(hidden))]
            impl<'de, $($name),+> Deserialize<'de> for ($($name,)+)
            where
                $($name: Deserialize<'de>,)+
            {
                tuple_impl_body!($len => ($($n $name)+));
            }
        )+
    };
}

macro_rules! tuple_impl_body {
    ($len:tt => ($($n:tt $name:ident)+)) => {
        #[inline]
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            struct TupleVisitor<$($name,)+> {
                marker: PhantomData<($($name,)+)>,
            }

            impl<'de, $($name: Deserialize<'de>),+> Visitor<'de> for TupleVisitor<$($name,)+> {
                type Value = ($($name,)+);

                fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                    formatter.write_str(concat!("a tuple of size ", $len))
                }

                #[inline]
                #[allow(non_snake_case)]
                fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
                where
                    A: SeqAccess<'de>,
                {
                    $(
                        let $name = match tri!(seq.next_element()) {
                            Some(value) => value,
                            None => return Err(Error::invalid_length($n, &self)),
                        };
                    )+

                    Ok(($($name,)+))
                }
            }

            deserializer.deserialize_tuple($len, TupleVisitor { marker: PhantomData })
        }

        #[inline]
        fn deserialize_in_place<D>(deserializer: D, place: &mut Self) -> Result<(), D::Error>
        where
            D: Deserializer<'de>,
        {
            struct TupleInPlaceVisitor<'a, $($name: 'a,)+>(&'a mut ($($name,)+));

            impl<'a, 'de, $($name: Deserialize<'de>),+> Visitor<'de> for TupleInPlaceVisitor<'a, $($name,)+> {
                type Value = ();

                fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                    formatter.write_str(concat!("a tuple of size ", $len))
                }

                #[inline]
                #[allow(non_snake_case)]
                fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
                where
                    A: SeqAccess<'de>,
                {
                    $(
                        if tri!(seq.next_element_seed(InPlaceSeed(&mut (self.0).$n))).is_none() {
                            return Err(Error::invalid_length($n, &self));
                        }
                    )+

                    Ok(())
                }
            }

            deserializer.deserialize_tuple($len, TupleInPlaceVisitor(place))
        }
    };
}

#[cfg_attr(docsrs, doc(fake_variadic))]
#[cfg_attr(
    docsrs,
    doc = "This trait is implemented for tuples up to 16 items long."
)]
impl<'de, T> Deserialize<'de> for (T,)
where
    T: Deserialize<'de>,
{
    tuple_impl_body!(1 => (0 T));
}

tuple_impls! {
    2  => (0 T0 1 T1)
    3  => (0 T0 1 T1 2 T2)
    4  => (0 T0 1 T1 2 T2 3 T3)
    5  => (0 T0 1 T1 2 T2 3 T3 4 T4)
    6  => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5)
    7  => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6)
    8  => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7)
    9  => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8)
    10 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8 9 T9)
    11 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8 9 T9 10 T10)
    12 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8 9 T9 10 T10 11 T11)
    13 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8 9 T9 10 T10 11 T11 12 T12)
    14 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8 9 T9 10 T10 11 T11 12 T12 13 T13)
    15 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8 9 T9 10 T10 11 T11 12 T12 13 T13 14 T14)
    16 => (0 T0 1 T1 2 T2 3 T3 4 T4 5 T5 6 T6 7 T7 8 T8 9 T9 10 T10 11 T11 12 T12 13 T13 14 T14 15 T15)
}

////////////////////////////////////////////////////////////////////////////////

macro_rules! map_impl {
    (
        $(#[$attr:meta])*
        $ty:ident <K $(: $kbound1:ident $(+ $kbound2:ident)*)*, V $(, $typaram:ident : $bound1:ident $(+ $bound2:ident)*)*>,
        $access:ident,
        $with_capacity:expr,
    ) => {
        $(#[$attr])*
        impl<'de, K, V $(, $typaram)*> Deserialize<'de> for $ty<K, V $(, $typaram)*>
        where
            K: Deserialize<'de> $(+ $kbound1 $(+ $kbound2)*)*,
            V: Deserialize<'de>,
            $($typaram: $bound1 $(+ $bound2)*),*
        {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct MapVisitor<K, V $(, $typaram)*> {
                    marker: PhantomData<$ty<K, V $(, $typaram)*>>,
                }

                impl<'de, K, V $(, $typaram)*> Visitor<'de> for MapVisitor<K, V $(, $typaram)*>
                where
                    K: Deserialize<'de> $(+ $kbound1 $(+ $kbound2)*)*,
                    V: Deserialize<'de>,
                    $($typaram: $bound1 $(+ $bound2)*),*
                {
                    type Value = $ty<K, V $(, $typaram)*>;

                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        formatter.write_str("a map")
                    }

                    #[inline]
                    fn visit_map<A>(self, mut $access: A) -> Result<Self::Value, A::Error>
                    where
                        A: MapAccess<'de>,
                    {
                        let mut values = $with_capacity;

                        while let Some((key, value)) = tri!($access.next_entry()) {
                            values.insert(key, value);
                        }

                        Ok(values)
                    }
                }

                let visitor = MapVisitor { marker: PhantomData };
                deserializer.deserialize_map(visitor)
            }
        }
    }
}

map_impl! {
    #[cfg(any(feature = "std", feature = "alloc"))]
    #[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
    BTreeMap<K: Ord, V>,
    map,
    BTreeMap::new(),
}

map_impl! {
    #[cfg(feature = "std")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    HashMap<K: Eq + Hash, V, S: BuildHasher + Default>,
    map,
    HashMap::with_capacity_and_hasher(size_hint::cautious::<(K, V)>(map.size_hint()), S::default()),
}

////////////////////////////////////////////////////////////////////////////////

#[cfg(any(feature = "std", not(no_core_net)))]
macro_rules! parse_ip_impl {
    ($ty:ty, $expecting:expr, $size:tt) => {
        impl<'de> Deserialize<'de> for $ty {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                if deserializer.is_human_readable() {
                    deserializer.deserialize_str(FromStrVisitor::new($expecting))
                } else {
                    <[u8; $size]>::deserialize(deserializer).map(<$ty>::from)
                }
            }
        }
    };
}

#[cfg(any(feature = "std", not(no_core_net)))]
macro_rules! variant_identifier {
    (
        $name_kind:ident ($($variant:ident; $bytes:expr; $index:expr),*)
        $expecting_message:expr,
        $variants_name:ident
    ) => {
        enum $name_kind {
            $($variant),*
        }

        static $variants_name: &[&str] = &[$(stringify!($variant)),*];

        impl<'de> Deserialize<'de> for $name_kind {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct KindVisitor;

                impl<'de> Visitor<'de> for KindVisitor {
                    type Value = $name_kind;

                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        formatter.write_str($expecting_message)
                    }

                    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
                    where
                        E: Error,
                    {
                        match value {
                            $(
                                $index => Ok($name_kind :: $variant),
                            )*
                            _ => Err(Error::invalid_value(Unexpected::Unsigned(value), &self),),
                        }
                    }

                    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
                    where
                        E: Error,
                    {
                        match value {
                            $(
                                stringify!($variant) => Ok($name_kind :: $variant),
                            )*
                            _ => Err(Error::unknown_variant(value, $variants_name)),
                        }
                    }

                    fn visit_bytes<E>(self, value: &[u8]) -> Result<Self::Value, E>
                    where
                        E: Error,
                    {
                        match value {
                            $(
                                $bytes => Ok($name_kind :: $variant),
                            )*
                            _ => {
                                match str::from_utf8(value) {
                                    Ok(value) => Err(Error::unknown_variant(value, $variants_name)),
                                    Err(_) => Err(Error::invalid_value(Unexpected::Bytes(value), &self)),
                                }
                            }
                        }
                    }
                }

                deserializer.deserialize_identifier(KindVisitor)
            }
        }
    }
}

#[cfg(any(feature = "std", not(no_core_net)))]
macro_rules! deserialize_enum {
    (
        $name:ident $name_kind:ident ($($variant:ident; $bytes:expr; $index:expr),*)
        $expecting_message:expr,
        $deserializer:expr
    ) => {
        variant_identifier! {
            $name_kind ($($variant; $bytes; $index),*)
            $expecting_message,
            VARIANTS
        }

        struct EnumVisitor;
        impl<'de> Visitor<'de> for EnumVisitor {
            type Value = $name;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str(concat!("a ", stringify!($name)))
            }


            fn visit_enum<A>(self, data: A) -> Result<Self::Value, A::Error>
            where
                A: EnumAccess<'de>,
            {
                match tri!(data.variant()) {
                    $(
                        ($name_kind :: $variant, v) => v.newtype_variant().map($name :: $variant),
                    )*
                }
            }
        }
        $deserializer.deserialize_enum(stringify!($name), VARIANTS, EnumVisitor)
    }
}

#[cfg(any(feature = "std", not(no_core_net)))]
impl<'de> Deserialize<'de> for net::IpAddr {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            deserializer.deserialize_str(FromStrVisitor::new("IP address"))
        } else {
            use crate::lib::net::IpAddr;
            deserialize_enum! {
                IpAddr IpAddrKind (V4; b"V4"; 0, V6; b"V6"; 1)
                "`V4` or `V6`",
                deserializer
            }
        }
    }
}

#[cfg(any(feature = "std", not(no_core_net)))]
parse_ip_impl!(net::Ipv4Addr, "IPv4 address", 4);

#[cfg(any(feature = "std", not(no_core_net)))]
parse_ip_impl!(net::Ipv6Addr, "IPv6 address", 16);

#[cfg(any(feature = "std", not(no_core_net)))]
macro_rules! parse_socket_impl {
    (
        $ty:ty, $expecting:tt,
        $new:expr,
    ) => {
        impl<'de> Deserialize<'de> for $ty {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                if deserializer.is_human_readable() {
                    deserializer.deserialize_str(FromStrVisitor::new($expecting))
                } else {
                    <(_, u16)>::deserialize(deserializer).map($new)
                }
            }
        }
    };
}

#[cfg(any(feature = "std", not(no_core_net)))]
impl<'de> Deserialize<'de> for net::SocketAddr {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            deserializer.deserialize_str(FromStrVisitor::new("socket address"))
        } else {
            use crate::lib::net::SocketAddr;
            deserialize_enum! {
                SocketAddr SocketAddrKind (V4; b"V4"; 0, V6; b"V6"; 1)
                "`V4` or `V6`",
                deserializer
            }
        }
    }
}

#[cfg(any(feature = "std", not(no_core_net)))]
parse_socket_impl! {
    net::SocketAddrV4, "IPv4 socket address",
    |(ip, port)| net::SocketAddrV4::new(ip, port),
}

#[cfg(any(feature = "std", not(no_core_net)))]
parse_socket_impl! {
    net::SocketAddrV6, "IPv6 socket address",
    |(ip, port)| net::SocketAddrV6::new(ip, port, 0, 0),
}

////////////////////////////////////////////////////////////////////////////////

#[cfg(feature = "std")]
struct PathVisitor;

#[cfg(feature = "std")]
impl<'a> Visitor<'a> for PathVisitor {
    type Value = &'a Path;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a borrowed path")
    }

    fn visit_borrowed_str<E>(self, v: &'a str) -> Result<Self::Value, E>
    where
        E: Error,
    {
        Ok(v.as_ref())
    }

    fn visit_borrowed_bytes<E>(self, v: &'a [u8]) -> Result<Self::Value, E>
    where
        E: Error,
    {
        str::from_utf8(v)
            .map(AsRef::as_ref)
            .map_err(|_| Error::invalid_value(Unexpected::Bytes(v), &self))
    }
}

#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
impl<'de: 'a, 'a> Deserialize<'de> for &'a Path {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(PathVisitor)
    }
}

#[cfg(feature = "std")]
struct PathBufVisitor;

#[cfg(feature = "std")]
impl<'de> Visitor<'de> for PathBufVisitor {
    type Value = PathBuf;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("path string")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: Error,
    {
        Ok(From::from(v))
    }

    fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
    where
        E: Error,
    {
        Ok(From::from(v))
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: Error,
    {
        str::from_utf8(v)
            .map(From::from)
            .map_err(|_| Error::invalid_value(Unexpected::Bytes(v), &self))
    }

    fn visit_byte_buf<E>(self, v: Vec<u8>) -> Result<Self::Value, E>
    where
        E: Error,
    {
        String::from_utf8(v)
            .map(From::from)
            .map_err(|e| Error::invalid_value(Unexpected::Bytes(&e.into_bytes()), &self))
    }
}

#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
impl<'de> Deserialize<'de> for PathBuf {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_string(PathBufVisitor)
    }
}

forwarded_impl! {
    #[cfg(feature = "std")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    (), Box<Path>, PathBuf::into_boxed_path
}

////////////////////////////////////////////////////////////////////////////////

// If this were outside of the serde crate, it would just use:
//
//    #[derive(Deserialize)]
//    #[serde(variant_identifier)]
#[cfg(all(feature = "std", any(unix, windows)))]
variant_identifier! {
    OsStringKind (Unix; b"Unix"; 0, Windows; b"Windows"; 1)
    "`Unix` or `Windows`",
    OSSTR_VARIANTS
}

#[cfg(all(feature = "std", any(unix, windows)))]
struct OsStringVisitor;

#[cfg(all(feature = "std", any(unix, windows)))]
impl<'de> Visitor<'de> for OsStringVisitor {
    type Value = OsString;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("os string")
    }

    #[cfg(unix)]
    fn visit_enum<A>(self, data: A) -> Result<Self::Value, A::Error>
    where
        A: EnumAccess<'de>,
    {
        use std::os::unix::ffi::OsStringExt;

        match tri!(data.variant()) {
            (OsStringKind::Unix, v) => v.newtype_variant().map(OsString::from_vec),
            (OsStringKind::Windows, _) => Err(Error::custom(
                "cannot deserialize Windows OS string on Unix",
            )),
        }
    }

    #[cfg(windows)]
    fn visit_enum<A>(self, data: A) -> Result<Self::Value, A::Error>
    where
        A: EnumAccess<'de>,
    {
        use std::os::windows::ffi::OsStringExt;

        match tri!(data.variant()) {
            (OsStringKind::Windows, v) => v
                .newtype_variant::<Vec<u16>>()
                .map(|vec| OsString::from_wide(&vec)),
            (OsStringKind::Unix, _) => Err(Error::custom(
                "cannot deserialize Unix OS string on Windows",
            )),
        }
    }
}

#[cfg(all(feature = "std", any(unix, windows)))]
#[cfg_attr(docsrs, doc(cfg(all(feature = "std", any(unix, windows)))))]
impl<'de> Deserialize<'de> for OsString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_enum("OsString", OSSTR_VARIANTS, OsStringVisitor)
    }
}

////////////////////////////////////////////////////////////////////////////////

forwarded_impl! {
    #[cfg(any(feature = "std", feature = "alloc"))]
    #[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
    (T), Box<T>, Box::new
}

forwarded_impl! {
    #[cfg(any(feature = "std", feature = "alloc"))]
    #[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
    (T), Box<[T]>, Vec::into_boxed_slice
}

forwarded_impl! {
    #[cfg(any(feature = "std", feature = "alloc"))]
    #[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
    (), Box<str>, String::into_boxed_str
}

forwarded_impl! {
    #[cfg(all(feature = "std", any(unix, windows)))]
    #[cfg_attr(docsrs, doc(cfg(all(feature = "std", any(unix, windows)))))]
    (), Box<OsStr>, OsString::into_boxed_os_str
}

#[cfg(any(feature = "std", feature = "alloc"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "alloc"))))]
impl<'de, 'a, T> Deserialize<'de> for Cow<'a, T>
where
    T: ?Sized + ToOwned,
    T::Owned: Deserialize<'de>,
{
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        T::Owned::deserialize(deserializer).map(Cow::Owned)
    }
}

////////////////////////////////////////////////////////////////////////////////

/// This impl requires the [`"rc"`] Cargo feature of Serde. The resulting
/// `Weak<T>` has a reference count of 0 and cannot be upgraded.
///
/// [`"rc"`]: https://serde.rs/feature-flags.html#-features-rc
#[cfg(all(feature = "rc", any(feature = "std", feature = "alloc")))]
#[cfg_attr(
    docsrs,
    doc(cfg(all(feature = "rc", any(feature = "std", feature = "alloc"))))
)]
impl<'de, T> Deserialize<'de> for RcWeak<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        tri!(Option::<T>::deserialize(deserializer));
        Ok(RcWeak::new())
    }
}

/// This impl requires the [`"rc"`] Cargo feature of Serde. The resulting
/// `Weak<T>` has a reference count of 0 and cannot be upgraded.
///
/// [`"rc"`]: https://serde.rs/feature-flags.html#-features-rc
#[cfg(all(feature = "rc", any(feature = "std", feature = "alloc")))]
#[cfg_attr(
    docsrs,
    doc(cfg(all(feature = "rc", any(feature = "std", feature = "alloc"))))
)]
impl<'de, T> Deserialize<'de> for ArcWeak<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        tri!(Option::<T>::deserialize(deserializer));
        Ok(ArcWeak::new())
    }
}

////////////////////////////////////////////////////////////////////////////////

macro_rules! box_forwarded_impl {
    (
        $(#[$attr:meta])*
        $t:ident
    ) => {
        $(#[$attr])*
        impl<'de, T> Deserialize<'de> for $t<T>
        where
            T: ?Sized,
            Box<T>: Deserialize<'de>,
        {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                Box::deserialize(deserializer).map(Into::into)
            }
        }
    };
}

box_forwarded_impl! {
    /// This impl requires the [`"rc"`] Cargo feature of Serde.
    ///
    /// Deserializing a data structure containing `Rc` will not attempt to
    /// deduplicate `Rc` references to the same data. Every deserialized `Rc`
    /// will end up with a strong count of 1.
    ///
    /// [`"rc"`]: https://serde.rs/feature-flags.html#-features-rc
    #[cfg(all(feature = "rc", any(feature = "std", feature = "alloc")))]
    #[cfg_attr(docsrs, doc(cfg(all(feature = "rc", any(feature = "std", feature = "alloc")))))]
    Rc
}

box_forwarded_impl! {
    /// This impl requires the [`"rc"`] Cargo feature of Serde.
    ///
    /// Deserializing a data structure containing `Arc` will not attempt to
    /// deduplicate `Arc` references to the same data. Every deserialized `Arc`
    /// will end up with a strong count of 1.
    ///
    /// [`"rc"`]: https://serde.rs/feature-flags.html#-features-rc
    #[cfg(all(feature = "rc", any(feature = "std", feature = "alloc")))]
    #[cfg_attr(docsrs, doc(cfg(all(feature = "rc", any(feature = "std", feature = "alloc")))))]
    Arc
}

////////////////////////////////////////////////////////////////////////////////

impl<'de, T> Deserialize<'de> for Cell<T>
where
    T: Deserialize<'de> + Copy,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        T::deserialize(deserializer).map(Cell::new)
    }
}

forwarded_impl! {
    (T), RefCell<T>, RefCell::new
}

forwarded_impl! {
    #[cfg(feature = "std")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    (T), Mutex<T>, Mutex::new
}

forwarded_impl! {
    #[cfg(feature = "std")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    (T), RwLock<T>, RwLock::new
}

////////////////////////////////////////////////////////////////////////////////

// This is a cleaned-up version of the impl generated by:
//
//     #[derive(Deserialize)]
//     #[serde(deny_unknown_fields)]
//     struct Duration {
//         secs: u64,
//         nanos: u32,
//     }
impl<'de> Deserialize<'de> for Duration {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // If this were outside of the serde crate, it would just use:
        //
        //    #[derive(Deserialize)]
        //    #[serde(field_identifier, rename_all = "lowercase")]
        enum Field {
            Secs,
            Nanos,
        }

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct FieldVisitor;

                impl<'de> Visitor<'de> for FieldVisitor {
                    type Value = Field;

                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        formatter.write_str("`secs` or `nanos`")
                    }

                    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
                    where
                        E: Error,
                    {
                        match value {
                            "secs" => Ok(Field::Secs),
                            "nanos" => Ok(Field::Nanos),
                            _ => Err(Error::unknown_field(value, FIELDS)),
                        }
                    }

                    fn visit_bytes<E>(self, value: &[u8]) -> Result<Self::Value, E>
                    where
                        E: Error,
                    {
                        match value {
                            b"secs" => Ok(Field::Secs),
                            b"nanos" => Ok(Field::Nanos),
                            _ => {
                                let value = private::string::from_utf8_lossy(value);
                                Err(Error::unknown_field(&*value, FIELDS))
                            }
                        }
                    }
                }

                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        fn check_overflow<E>(secs: u64, nanos: u32) -> Result<(), E>
        where
            E: Error,
        {
            static NANOS_PER_SEC: u32 = 1_000_000_000;
            match secs.checked_add((nanos / NANOS_PER_SEC) as u64) {
                Some(_) => Ok(()),
                None => Err(E::custom("overflow deserializing Duration")),
            }
        }

        struct DurationVisitor;

        impl<'de> Visitor<'de> for DurationVisitor {
            type Value = Duration;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct Duration")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let secs: u64 = match tri!(seq.next_element()) {
                    Some(value) => value,
                    None => {
                        return Err(Error::invalid_length(0, &self));
                    }
                };
                let nanos: u32 = match tri!(seq.next_element()) {
                    Some(value) => value,
                    None => {
                        return Err(Error::invalid_length(1, &self));
                    }
                };
                tri!(check_overflow(secs, nanos));
                Ok(Duration::new(secs, nanos))
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut secs: Option<u64> = None;
                let mut nanos: Option<u32> = None;
                while let Some(key) = tri!(map.next_key()) {
                    match key {
                        Field::Secs => {
                            if secs.is_some() {
                                return Err(<A::Error as Error>::duplicate_field("secs"));
                            }
                            secs = Some(tri!(map.next_value()));
                        }
                        Field::Nanos => {
                            if nanos.is_some() {
                                return Err(<A::Error as Error>::duplicate_field("nanos"));
                            }
                            nanos = Some(tri!(map.next_value()));
                        }
                    }
                }
                let secs = match secs {
                    Some(secs) => secs,
                    None => return Err(<A::Error as Error>::missing_field("secs")),
                };
                let nanos = match nanos {
                    Some(nanos) => nanos,
                    None => return Err(<A::Error as Error>::missing_field("nanos")),
                };
                tri!(check_overflow(secs, nanos));
                Ok(Duration::new(secs, nanos))
            }
        }

        const FIELDS: &[&str] = &["secs", "nanos"];
        deserializer.deserialize_struct("Duration", FIELDS, DurationVisitor)
    }
}

////////////////////////////////////////////////////////////////////////////////

#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
impl<'de> Deserialize<'de> for SystemTime {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Reuse duration
        enum Field {
            Secs,
            Nanos,
        }

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct FieldVisitor;

                impl<'de> Visitor<'de> for FieldVisitor {
                    type Value = Field;

                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        formatter.write_str("`secs_since_epoch` or `nanos_since_epoch`")
                    }

                    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
                    where
                        E: Error,
                    {
                        match value {
                            "secs_since_epoch" => Ok(Field::Secs),
                            "nanos_since_epoch" => Ok(Field::Nanos),
                            _ => Err(Error::unknown_field(value, FIELDS)),
                        }
                    }

                    fn visit_bytes<E>(self, value: &[u8]) -> Result<Self::Value, E>
                    where
                        E: Error,
                    {
                        match value {
                            b"secs_since_epoch" => Ok(Field::Secs),
                            b"nanos_since_epoch" => Ok(Field::Nanos),
                            _ => {
                                let value = String::from_utf8_lossy(value);
                                Err(Error::unknown_field(&value, FIELDS))
                            }
                        }
                    }
                }

                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        fn check_overflow<E>(secs: u64, nanos: u32) -> Result<(), E>
        where
            E: Error,
        {
            static NANOS_PER_SEC: u32 = 1_000_000_000;
            match secs.checked_add((nanos / NANOS_PER_SEC) as u64) {
                Some(_) => Ok(()),
                None => Err(E::custom("overflow deserializing SystemTime epoch offset")),
            }
        }

        struct DurationVisitor;

        impl<'de> Visitor<'de> for DurationVisitor {
            type Value = Duration;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct SystemTime")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let secs: u64 = match tri!(seq.next_element()) {
                    Some(value) => value,
                    None => {
                        return Err(Error::invalid_length(0, &self));
                    }
                };
                let nanos: u32 = match tri!(seq.next_element()) {
                    Some(value) => value,
                    None => {
                        return Err(Error::invalid_length(1, &self));
                    }
                };
                tri!(check_overflow(secs, nanos));
                Ok(Duration::new(secs, nanos))
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut secs: Option<u64> = None;
                let mut nanos: Option<u32> = None;
                while let Some(key) = tri!(map.next_key()) {
                    match key {
                        Field::Secs => {
                            if secs.is_some() {
                                return Err(<A::Error as Error>::duplicate_field(
                                    "secs_since_epoch",
                                ));
                            }
                            secs = Some(tri!(map.next_value()));
                        }
                        Field::Nanos => {
                            if nanos.is_some() {
                                return Err(<A::Error as Error>::duplicate_field(
                                    "nanos_since_epoch",
                                ));
                            }
                            nanos = Some(tri!(map.next_value()));
                        }
                    }
                }
                let secs = match secs {
                    Some(secs) => secs,
                    None => return Err(<A::Error as Error>::missing_field("secs_since_epoch")),
                };
                let nanos = match nanos {
                    Some(nanos) => nanos,
                    None => return Err(<A::Error as Error>::missing_field("nanos_since_epoch")),
                };
                tri!(check_overflow(secs, nanos));
                Ok(Duration::new(secs, nanos))
            }
        }

        const FIELDS: &[&str] = &["secs_since_epoch", "nanos_since_epoch"];
        let duration = tri!(deserializer.deserialize_struct("SystemTime", FIELDS, DurationVisitor));
        UNIX_EPOCH
            .checked_add(duration)
            .ok_or_else(|| D::Error::custom("overflow deserializing SystemTime"))
    }
}

////////////////////////////////////////////////////////////////////////////////

// Similar to:
//
//     #[derive(Deserialize)]
//     #[serde(deny_unknown_fields)]
//     struct Range<Idx> {
//         start: Idx,
//         end: Idx,
//     }
impl<'de, Idx> Deserialize<'de> for Range<Idx>
where
    Idx: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let (start, end) = tri!(deserializer.deserialize_struct(
            "Range",
            range::FIELDS,
            range::RangeVisitor {
                expecting: "struct Range",
                phantom: PhantomData,
            },
        ));
        Ok(start..end)
    }
}

impl<'de, Idx> Deserialize<'de> for RangeInclusive<Idx>
where
    Idx: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let (start, end) = tri!(deserializer.deserialize_struct(
            "RangeInclusive",
            range::FIELDS,
            range::RangeVisitor {
                expecting: "struct RangeInclusive",
                phantom: PhantomData,
            },
        ));
        Ok(RangeInclusive::new(start, end))
    }
}

mod range {
    use crate::lib::*;

    use crate::de::{Deserialize, Deserializer, Error, MapAccess, SeqAccess, Visitor};
    use crate::private;

    pub const FIELDS: &[&str] = &["start", "end"];

    // If this were outside of the serde crate, it would just use:
    //
    //    #[derive(Deserialize)]
    //    #[serde(field_identifier, rename_all = "lowercase")]
    enum Field {
        Start,
        End,
    }

    impl<'de> Deserialize<'de> for Field {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            struct FieldVisitor;

            impl<'de> Visitor<'de> for FieldVisitor {
                type Value = Field;

                fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                    formatter.write_str("`start` or `end`")
                }

                fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
                where
                    E: Error,
                {
                    match value {
                        "start" => Ok(Field::Start),
                        "end" => Ok(Field::End),
                        _ => Err(Error::unknown_field(value, FIELDS)),
                    }
                }

                fn visit_bytes<E>(self, value: &[u8]) -> Result<Self::Value, E>
                where
                    E: Error,
                {
                    match value {
                        b"start" => Ok(Field::Start),
                        b"end" => Ok(Field::End),
                        _ => {
                            let value = private::string::from_utf8_lossy(value);
                            Err(Error::unknown_field(&*value, FIELDS))
                        }
                    }
                }
            }

            deserializer.deserialize_identifier(FieldVisitor)
        }
    }

    pub struct RangeVisitor<Idx> {
        pub expecting: &'static str,
        pub phantom: PhantomData<Idx>,
    }

    impl<'de, Idx> Visitor<'de> for RangeVisitor<Idx>
    where
        Idx: Deserialize<'de>,
    {
        type Value = (Idx, Idx);

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str(self.expecting)
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let start: Idx = match tri!(seq.next_element()) {
                Some(value) => value,
                None => {
                    return Err(Error::invalid_length(0, &self));
                }
            };
            let end: Idx = match tri!(seq.next_element()) {
                Some(value) => value,
                None => {
                    return Err(Error::invalid_length(1, &self));
                }
            };
            Ok((start, end))
        }

        fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where
            A: MapAccess<'de>,
        {
            let mut start: Option<Idx> = None;
            let mut end: Option<Idx> = None;
            while let Some(key) = tri!(map.next_key()) {
                match key {
                    Field::Start => {
                        if start.is_some() {
                            return Err(<A::Error as Error>::duplicate_field("start"));
                        }
                        start = Some(tri!(map.next_value()));
                    }
                    Field::End => {
                        if end.is_some() {
                            return Err(<A::Error as Error>::duplicate_field("end"));
                        }
                        end = Some(tri!(map.next_value()));
                    }
                }
            }
            let start = match start {
                Some(start) => start,
                None => return Err(<A::Error as Error>::missing_field("start")),
            };
            let end = match end {
                Some(end) => end,
                None => return Err(<A::Error as Error>::missing_field("end")),
            };
            Ok((start, end))
        }
    }
}

////////////////////////////////////////////////////////////////////////////////

// Similar to:
//
//     #[derive(Deserialize)]
//     #[serde(deny_unknown_fields)]
//     struct RangeFrom<Idx> {
//         start: Idx,
//     }
impl<'de, Idx> Deserialize<'de> for RangeFrom<Idx>
where
    Idx: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let start = tri!(deserializer.deserialize_struct(
            "RangeFrom",
            range_from::FIELDS,
            range_from::RangeFromVisitor {
                expecting: "struct RangeFrom",
                phantom: PhantomData,
            },
        ));
        Ok(start..)
    }
}

mod range_from {
    use crate::lib::*;

    use crate::de::{Deserialize, Deserializer, Error, MapAccess, SeqAccess, Visitor};
    use crate::private;

    pub const FIELDS: &[&str] = &["start"];

    // If this were outside of the serde crate, it would just use:
    //
    //    #[derive(Deserialize)]
    //    #[serde(field_identifier, rename_all = "lowercase")]
    enum Field {
        Start,
    }

    impl<'de> Deserialize<'de> for Field {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            struct FieldVisitor;

            impl<'de> Visitor<'de> for FieldVisitor {
                type Value = Field;

                fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                    formatter.write_str("`start`")
                }

                fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
                where
                    E: Error,
                {
                    match value {
                        "start" => Ok(Field::Start),
                        _ => Err(Error::unknown_field(value, FIELDS)),
                    }
                }

                fn visit_bytes<E>(self, value: &[u8]) -> Result<Self::Value, E>
                where
                    E: Error,
                {
                    match value {
                        b"start" => Ok(Field::Start),
                        _ => {
                            let value = private::string::from_utf8_lossy(value);
                            Err(Error::unknown_field(&*value, FIELDS))
                        }
                    }
                }
            }

            deserializer.deserialize_identifier(FieldVisitor)
        }
    }

    pub struct RangeFromVisitor<Idx> {
        pub expecting: &'static str,
        pub phantom: PhantomData<Idx>,
    }

    impl<'de, Idx> Visitor<'de> for RangeFromVisitor<Idx>
    where
        Idx: Deserialize<'de>,
    {
        type Value = Idx;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str(self.expecting)
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let start: Idx = match tri!(seq.next_element()) {
                Some(value) => value,
                None => {
                    return Err(Error::invalid_length(0, &self));
                }
            };
            Ok(start)
        }

        fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where
            A: MapAccess<'de>,
        {
            let mut start: Option<Idx> = None;
            while let Some(key) = tri!(map.next_key()) {
                match key {
                    Field::Start => {
                        if start.is_some() {
                            return Err(<A::Error as Error>::duplicate_field("start"));
                        }
                        start = Some(tri!(map.next_value()));
                    }
                }
            }
            let start = match start {
                Some(start) => start,
                None => return Err(<A::Error as Error>::missing_field("start")),
            };
            Ok(start)
        }
    }
}

////////////////////////////////////////////////////////////////////////////////

// Similar to:
//
//     #[derive(Deserialize)]
//     #[serde(deny_unknown_fields)]
//     struct RangeTo<Idx> {
//         end: Idx,
//     }
impl<'de, Idx> Deserialize<'de> for RangeTo<Idx>
where
    Idx: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let end = tri!(deserializer.deserialize_struct(
            "RangeTo",
            range_to::FIELDS,
            range_to::RangeToVisitor {
                expecting: "struct RangeTo",
                phantom: PhantomData,
            },
        ));
        Ok(..end)
    }
}

mod range_to {
    use crate::lib::*;

    use crate::de::{Deserialize, Deserializer, Error, MapAccess, SeqAccess, Visitor};
    use crate::private;

    pub const FIELDS: &[&str] = &["end"];

    // If this were outside of the serde crate, it would just use:
    //
    //    #[derive(Deserialize)]
    //    #[serde(field_identifier, rename_all = "lowercase")]
    enum Field {
        End,
    }

    impl<'de> Deserialize<'de> for Field {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            struct FieldVisitor;

            impl<'de> Visitor<'de> for FieldVisitor {
                type Value = Field;

                fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                    formatter.write_str("`end`")
                }

                fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
                where
                    E: Error,
                {
                    match value {
                        "end" => Ok(Field::End),
                        _ => Err(Error::unknown_field(value, FIELDS)),
                    }
                }

                fn visit_bytes<E>(self, value: &[u8]) -> Result<Self::Value, E>
                where
                    E: Error,
                {
                    match value {
                        b"end" => Ok(Field::End),
                        _ => {
                            let value = private::string::from_utf8_lossy(value);
                            Err(Error::unknown_field(&*value, FIELDS))
                        }
                    }
                }
            }

            deserializer.deserialize_identifier(FieldVisitor)
        }
    }

    pub struct RangeToVisitor<Idx> {
        pub expecting: &'static str,
        pub phantom: PhantomData<Idx>,
    }

    impl<'de, Idx> Visitor<'de> for RangeToVisitor<Idx>
    where
        Idx: Deserialize<'de>,
    {
        type Value = Idx;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str(self.expecting)
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let end: Idx = match tri!(seq.next_element()) {
                Some(value) => value,
                None => {
                    return Err(Error::invalid_length(0, &self));
                }
            };
            Ok(end)
        }

        fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where
            A: MapAccess<'de>,
        {
            let mut end: Option<Idx> = None;
            while let Some(key) = tri!(map.next_key()) {
                match key {
                    Field::End => {
                        if end.is_some() {
                            return Err(<A::Error as Error>::duplicate_field("end"));
                        }
                        end = Some(tri!(map.next_value()));
                    }
                }
            }
            let end = match end {
                Some(end) => end,
                None => return Err(<A::Error as Error>::missing_field("end")),
            };
            Ok(end)
        }
    }
}

////////////////////////////////////////////////////////////////////////////////

impl<'de, T> Deserialize<'de> for Bound<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        enum Field {
            Unbounded,
            Included,
            Excluded,
        }

        impl<'de> Deserialize<'de> for Field {
            #[inline]
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct FieldVisitor;

                impl<'de> Visitor<'de> for FieldVisitor {
                    type Value = Field;

                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        formatter.write_str("`Unbounded`, `Included` or `Excluded`")
                    }

                    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
                    where
                        E: Error,
                    {
                        match value {
                            0 => Ok(Field::Unbounded),
                            1 => Ok(Field::Included),
                            2 => Ok(Field::Excluded),
                            _ => Err(Error::invalid_value(Unexpected::Unsigned(value), &self)),
                        }
                    }

                    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
                    where
                        E: Error,
                    {
                        match value {
                            "Unbounded" => Ok(Field::Unbounded),
                            "Included" => Ok(Field::Included),
                            "Excluded" => Ok(Field::Excluded),
                            _ => Err(Error::unknown_variant(value, VARIANTS)),
                        }
                    }

                    fn visit_bytes<E>(self, value: &[u8]) -> Result<Self::Value, E>
                    where
                        E: Error,
                    {
                        match value {
                            b"Unbounded" => Ok(Field::Unbounded),
                            b"Included" => Ok(Field::Included),
                            b"Excluded" => Ok(Field::Excluded),
                            _ => match str::from_utf8(value) {
                                Ok(value) => Err(Error::unknown_variant(value, VARIANTS)),
                                Err(_) => {
                                    Err(Error::invalid_value(Unexpected::Bytes(value), &self))
                                }
                            },
                        }
                    }
                }

                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        struct BoundVisitor<T>(PhantomData<Bound<T>>);

        impl<'de, T> Visitor<'de> for BoundVisitor<T>
        where
            T: Deserialize<'de>,
        {
            type Value = Bound<T>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("enum Bound")
            }

            fn visit_enum<A>(self, data: A) -> Result<Self::Value, A::Error>
            where
                A: EnumAccess<'de>,
            {
                match tri!(data.variant()) {
                    (Field::Unbounded, v) => v.unit_variant().map(|()| Bound::Unbounded),
                    (Field::Included, v) => v.newtype_variant().map(Bound::Included),
                    (Field::Excluded, v) => v.newtype_variant().map(Bound::Excluded),
                }
            }
        }

        const VARIANTS: &[&str] = &["Unbounded", "Included", "Excluded"];

        deserializer.deserialize_enum("Bound", VARIANTS, BoundVisitor(PhantomData))
    }
}

////////////////////////////////////////////////////////////////////////////////

#[cfg(feature = "result")]
#[cfg_attr(docsrs, doc(cfg(feature = "result")))]
impl<'de, T, E> Deserialize<'de> for Result<T, E>
where
    T: Deserialize<'de>,
    E: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // If this were outside of the serde crate, it would just use:
        //
        //    #[derive(Deserialize)]
        //    #[serde(variant_identifier)]
        enum Field {
            Ok,
            Err,
        }

        impl<'de> Deserialize<'de> for Field {
            #[inline]
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct FieldVisitor;

                impl<'de> Visitor<'de> for FieldVisitor {
                    type Value = Field;

                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        formatter.write_str("`Ok` or `Err`")
                    }

                    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
                    where
                        E: Error,
                    {
                        match value {
                            0 => Ok(Field::Ok),
                            1 => Ok(Field::Err),
                            _ => Err(Error::invalid_value(Unexpected::Unsigned(value), &self)),
                        }
                    }

                    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
                    where
                        E: Error,
                    {
                        match value {
                            "Ok" => Ok(Field::Ok),
                            "Err" => Ok(Field::Err),
                            _ => Err(Error::unknown_variant(value, VARIANTS)),
                        }
                    }

                    fn visit_bytes<E>(self, value: &[u8]) -> Result<Self::Value, E>
                    where
                        E: Error,
                    {
                        match value {
                            b"Ok" => Ok(Field::Ok),
                            b"Err" => Ok(Field::Err),
                            _ => match str::from_utf8(value) {
                                Ok(value) => Err(Error::unknown_variant(value, VARIANTS)),
                                Err(_) => {
                                    Err(Error::invalid_value(Unexpected::Bytes(value), &self))
                                }
                            },
                        }
                    }
                }

                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        struct ResultVisitor<T, E>(PhantomData<Result<T, E>>);

        impl<'de, T, E> Visitor<'de> for ResultVisitor<T, E>
        where
            T: Deserialize<'de>,
            E: Deserialize<'de>,
        {
            type Value = Result<T, E>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("enum Result")
            }

            fn visit_enum<A>(self, data: A) -> Result<Self::Value, A::Error>
            where
                A: EnumAccess<'de>,
            {
                match tri!(data.variant()) {
                    (Field::Ok, v) => v.newtype_variant().map(Ok),
                    (Field::Err, v) => v.newtype_variant().map(Err),
                }
            }
        }

        const VARIANTS: &[&str] = &["Ok", "Err"];

        deserializer.deserialize_enum("Result", VARIANTS, ResultVisitor(PhantomData))
    }
}

////////////////////////////////////////////////////////////////////////////////

impl<'de, T> Deserialize<'de> for Wrapping<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Deserialize::deserialize(deserializer).map(Wrapping)
    }
}

#[cfg(all(feature = "std", not(no_std_atomic)))]
macro_rules! atomic_impl {
    ($($ty:ident $size:expr)*) => {
        $(
            #[cfg(any(no_target_has_atomic, target_has_atomic = $size))]
            #[cfg_attr(docsrs, doc(cfg(all(feature = "std", target_has_atomic = $size))))]
            impl<'de> Deserialize<'de> for $ty {
                fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                where
                    D: Deserializer<'de>,
                {
                    Deserialize::deserialize(deserializer).map(Self::new)
                }
            }
        )*
    };
}

#[cfg(all(feature = "std", not(no_std_atomic)))]
atomic_impl! {
    AtomicBool "8"
    AtomicI8 "8"
    AtomicI16 "16"
    AtomicI32 "32"
    AtomicIsize "ptr"
    AtomicU8 "8"
    AtomicU16 "16"
    AtomicU32 "32"
    AtomicUsize "ptr"
}

#[cfg(all(feature = "std", not(no_std_atomic64)))]
atomic_impl! {
    AtomicI64 "64"
    AtomicU64 "64"
}

#[cfg(any(feature = "std", not(no_core_net)))]
struct FromStrVisitor<T> {
    expecting: &'static str,
    ty: PhantomData<T>,
}

#[cfg(any(feature = "std", not(no_core_net)))]
impl<T> FromStrVisitor<T> {
    fn new(expecting: &'static str) -> Self {
        FromStrVisitor {
            expecting,
            ty: PhantomData,
        }
    }
}

#[cfg(any(feature = "std", not(no_core_net)))]
impl<'de, T> Visitor<'de> for FromStrVisitor<T>
where
    T: str::FromStr,
    T::Err: fmt::Display,
{
    type Value = T;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(self.expecting)
    }

    fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
    where
        E: Error,
    {
        s.parse().map_err(Error::custom)
    }
}
