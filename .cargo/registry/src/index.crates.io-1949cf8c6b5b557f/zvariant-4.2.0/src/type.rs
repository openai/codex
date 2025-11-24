use crate::{utils::*, Signature};
use serde::de::{Deserialize, DeserializeSeed};
use std::{
    marker::PhantomData,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    path::{Path, PathBuf},
    rc::Rc,
    sync::{Arc, Mutex, RwLock},
    time::Duration,
};

/// Trait implemented by all serializable types.
///
/// This very simple trait provides the signature for the implementing type. Since the [D-Bus type
/// system] relies on these signatures, our [serialization and deserialization] API requires this
/// trait in addition to [`Serialize`] and [`Deserialize`], respectively.
///
/// Implementation is provided for all the [basic types] and blanket implementations for common
/// container types, such as, arrays, slices, tuples, [`Vec`] and [`HashMap`]. For easy
/// implementation for custom types, use `Type` derive macro from [zvariant_derive] crate.
///
/// If your type's signature cannot be determined statically, you should implement the
/// [DynamicType] trait instead, which is otherwise automatically implemented if you implement this
/// trait.
///
/// [D-Bus type system]: https://dbus.freedesktop.org/doc/dbus-specification.html#type-system
/// [serialization and deserialization]: index.html#functions
/// [`Serialize`]: https://docs.serde.rs/serde/trait.Serialize.html
/// [`Deserialize`]: https://docs.serde.rs/serde/de/trait.Deserialize.html
/// [basic types]: trait.Basic.html
/// [`Vec`]: https://doc.rust-lang.org/std/vec/struct.Vec.html
/// [`HashMap`]: https://doc.rust-lang.org/std/collections/struct.HashMap.html
/// [zvariant_derive]: https://docs.rs/zvariant_derive/latest/zvariant_derive/
pub trait Type {
    /// Get the signature for the implementing type.
    ///
    /// # Example
    ///
    /// ```
    /// use std::collections::HashMap;
    /// use zvariant::Type;
    ///
    /// assert_eq!(u32::signature(), "u");
    /// assert_eq!(String::signature(), "s");
    /// assert_eq!(<(u32, &str, u64)>::signature(), "(ust)");
    /// assert_eq!(<(u32, &str, &[u64])>::signature(), "(usat)");
    /// assert_eq!(<HashMap<u8, &str>>::signature(), "a{ys}");
    /// ```
    fn signature() -> Signature<'static>;
}

/// Types with dynamic signatures.
///
/// Prefer implementing [Type] if possible, but if the actual signature of your type cannot be
/// determined until runtime, you can implement this type to support serialization.  You should
/// also implement [DynamicDeserialize] for deserialization.
pub trait DynamicType {
    /// Get the signature for the implementing type.
    ///
    /// See [Type::signature] for details.
    fn dynamic_signature(&self) -> Signature<'_>;
}

/// Types that deserialize based on dynamic signatures.
///
/// Prefer implementing [Type] and [Deserialize] if possible, but if the actual signature of your
/// type cannot be determined until runtime, you should implement this type to support
/// deserialization given a signature.
pub trait DynamicDeserialize<'de>: DynamicType {
    /// A [DeserializeSeed] implementation for this type.
    type Deserializer: DeserializeSeed<'de, Value = Self> + DynamicType;

    /// Get a deserializer compatible with this signature.
    fn deserializer_for_signature<S>(signature: S) -> zvariant::Result<Self::Deserializer>
    where
        S: TryInto<Signature<'de>>,
        S::Error: Into<zvariant::Error>;
}

impl<T> DynamicType for T
where
    T: Type + ?Sized,
{
    fn dynamic_signature(&self) -> Signature<'_> {
        <T as Type>::signature()
    }
}

impl<T> Type for PhantomData<T>
where
    T: Type + ?Sized,
{
    fn signature() -> Signature<'static> {
        T::signature()
    }
}

impl<'de, T> DynamicDeserialize<'de> for T
where
    T: Type + ?Sized + Deserialize<'de>,
{
    type Deserializer = PhantomData<T>;

    fn deserializer_for_signature<S>(signature: S) -> zvariant::Result<Self::Deserializer>
    where
        S: TryInto<Signature<'de>>,
        S::Error: Into<zvariant::Error>,
    {
        let mut expected = <T as Type>::signature();
        let original = signature.try_into().map_err(Into::into)?;

        if original == expected {
            return Ok(PhantomData);
        }

        let mut signature = original.as_ref();
        while expected.len() < signature.len()
            && signature.starts_with(STRUCT_SIG_START_CHAR)
            && signature.ends_with(STRUCT_SIG_END_CHAR)
        {
            signature = signature.slice(1..signature.len() - 1);
        }

        while signature.len() < expected.len()
            && expected.starts_with(STRUCT_SIG_START_CHAR)
            && expected.ends_with(STRUCT_SIG_END_CHAR)
        {
            expected = expected.slice(1..expected.len() - 1);
        }

        if signature == expected {
            Ok(PhantomData)
        } else {
            let expected = <T as Type>::signature();
            Err(zvariant::Error::SignatureMismatch(
                original.to_owned(),
                format!("`{expected}`"),
            ))
        }
    }
}

macro_rules! array_type {
    ($arr:ty) => {
        impl<T> Type for $arr
        where
            T: Type,
        {
            #[inline]
            fn signature() -> Signature<'static> {
                Signature::from_string_unchecked(format!("a{}", T::signature()))
            }
        }
    };
}

array_type!([T]);
array_type!(Vec<T>);

impl<T, S> Type for std::collections::HashSet<T, S>
where
    T: Type + Eq + Hash,
    S: BuildHasher,
{
    #[inline]
    fn signature() -> Signature<'static> {
        <[T]>::signature()
    }
}

#[cfg(feature = "arrayvec")]
impl<T, const CAP: usize> Type for arrayvec::ArrayVec<T, CAP>
where
    T: Type,
{
    #[inline]
    fn signature() -> Signature<'static> {
        <[T]>::signature()
    }
}

#[cfg(feature = "arrayvec")]
impl<const CAP: usize> Type for arrayvec::ArrayString<CAP> {
    #[inline]
    fn signature() -> Signature<'static> {
        <&str>::signature()
    }
}

#[cfg(feature = "heapless")]
impl<T, const CAP: usize> Type for heapless::Vec<T, CAP>
where
    T: Type,
{
    #[inline]
    fn signature() -> Signature<'static> {
        <[T]>::signature()
    }
}

#[cfg(feature = "heapless")]
impl<const CAP: usize> Type for heapless::String<CAP> {
    #[inline]
    fn signature() -> Signature<'static> {
        <&str>::signature()
    }
}

// Empty type deserves empty signature
impl Type for () {
    #[inline]
    fn signature() -> Signature<'static> {
        Signature::from_static_str_unchecked("")
    }
}

macro_rules! deref_impl {
    (
        $type:ty,
        <$($desc:tt)+
    ) => {
        impl <$($desc)+ {
            #[inline]
            fn signature() -> Signature<'static> {
                <$type>::signature()
            }
        }
    };
}

deref_impl!(T, <T: ?Sized + Type> Type for &T);
deref_impl!(T, <T: ?Sized + Type> Type for &mut T);
deref_impl!(T, <T: ?Sized + Type + ToOwned> Type for Cow<'_, T>);
deref_impl!(T, <T: ?Sized + Type> Type for Arc<T>);
deref_impl!(T, <T: ?Sized + Type> Type for Mutex<T>);
deref_impl!(T, <T: ?Sized + Type> Type for RwLock<T>);
deref_impl!(T, <T: ?Sized + Type> Type for Box<T>);
deref_impl!(T, <T: ?Sized + Type> Type for Rc<T>);

#[cfg(all(feature = "gvariant", not(feature = "option-as-array")))]
impl<T> Type for Option<T>
where
    T: Type,
{
    #[inline]
    fn signature() -> Signature<'static> {
        Signature::from_string_unchecked(format!("m{}", T::signature()))
    }
}

#[cfg(feature = "option-as-array")]
impl<T> Type for Option<T>
where
    T: Type,
{
    #[inline]
    fn signature() -> Signature<'static> {
        Signature::from_string_unchecked(format!("a{}", T::signature()))
    }
}

////////////////////////////////////////////////////////////////////////////////

macro_rules! tuple_impls {
    ($($len:expr => ($($n:tt $name:ident)+))+) => {
        $(
            impl<$($name),+> Type for ($($name,)+)
            where
                $($name: Type,)+
            {
                fn signature() -> Signature<'static> {
                    let mut sig = String::with_capacity(255);
                    sig.push(STRUCT_SIG_START_CHAR);
                    $(
                        sig.push_str($name::signature().as_str());
                    )+
                    sig.push(STRUCT_SIG_END_CHAR);

                    Signature::from_string_unchecked(sig)
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

////////////////////////////////////////////////////////////////////////////////

// Arrays are serialized as tuples/structs by Serde so we treat them as such too even though
// it's very strange. Slices and arrayvec::ArrayVec can be used anyway so I guess it's no big
// deal.
impl<T, const N: usize> Type for [T; N]
where
    T: Type,
{
    #[allow(clippy::reversed_empty_ranges)]
    fn signature() -> Signature<'static> {
        let mut sig = String::with_capacity(255);
        sig.push(STRUCT_SIG_START_CHAR);
        for _ in 0..N {
            sig.push_str(T::signature().as_str());
        }
        sig.push(STRUCT_SIG_END_CHAR);

        Signature::from_string_unchecked(sig)
    }
}

////////////////////////////////////////////////////////////////////////////////

use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap},
    hash::{BuildHasher, Hash},
    time::SystemTime,
};

macro_rules! map_impl {
    ($ty:ident < K $(: $kbound1:ident $(+ $kbound2:ident)*)*, V $(, $typaram:ident : $bound:ident)* >) => {
        impl<K, V $(, $typaram)*> Type for $ty<K, V $(, $typaram)*>
        where
            K: Type $(+ $kbound1 $(+ $kbound2)*)*,
            V: Type,
            $($typaram: $bound,)*
        {
            #[inline]
            fn signature() -> Signature<'static> {
                Signature::from_string_unchecked(format!("a{{{}{}}}", K::signature(), V::signature()))
            }
        }
    }
}

map_impl!(BTreeMap<K: Ord, V>);
map_impl!(HashMap<K: Eq + Hash, V, H: BuildHasher>);

impl Type for Duration {
    fn signature() -> Signature<'static> {
        <(u64, u32)>::signature()
    }
}

impl Type for SystemTime {
    #[inline]
    fn signature() -> Signature<'static> {
        <(
            // seconds
            u64,
            // nano
            u32,
        )>::signature()
    }
}

impl Type for Ipv4Addr {
    #[inline]
    fn signature() -> Signature<'static> {
        <[u8; 4]>::signature()
    }
}

impl Type for Ipv6Addr {
    #[inline]
    fn signature() -> Signature<'static> {
        <[u8; 16]>::signature()
    }
}

impl Type for IpAddr {
    #[inline]
    fn signature() -> Signature<'static> {
        <(u32, &[u8])>::signature()
    }
}

// BitFlags
#[cfg(feature = "enumflags2")]
impl<F> Type for enumflags2::BitFlags<F>
where
    F: Type + enumflags2::BitFlag,
{
    #[inline]
    fn signature() -> Signature<'static> {
        F::signature()
    }
}

#[cfg(feature = "serde_bytes")]
impl Type for serde_bytes::Bytes {
    fn signature() -> Signature<'static> {
        Signature::from_static_str_unchecked("ay")
    }
}

#[cfg(feature = "serde_bytes")]
impl Type for serde_bytes::ByteBuf {
    fn signature() -> Signature<'static> {
        Signature::from_static_str_unchecked("ay")
    }
}

#[allow(unused)]
macro_rules! static_str_type {
    ($ty:ty) => {
        impl Type for $ty {
            fn signature() -> Signature<'static> {
                <&str>::signature()
            }
        }
    };
}

static_str_type!(Path);
static_str_type!(PathBuf);

#[cfg(feature = "uuid")]
impl Type for uuid::Uuid {
    fn signature() -> Signature<'static> {
        Signature::from_static_str_unchecked("ay")
    }
}

#[cfg(feature = "url")]
static_str_type!(url::Url);

// FIXME: Ignoring the `serde-human-readable` feature of `time` crate in these impls:
// https://github.com/time-rs/time/blob/f9398b9598757508ca3815694f23203843e0011b/src/serde/mod.rs#L110
#[cfg(feature = "time")]
impl Type for time::Date {
    fn signature() -> Signature<'static> {
        // Serialized as a (year, ordinal) tuple:
        // https://github.com/time-rs/time/blob/f9398b9598757508ca3815694f23203843e0011b/src/serde/mod.rs#L92
        <(i32, u16)>::signature()
    }
}

#[cfg(feature = "time")]
impl Type for time::Duration {
    fn signature() -> Signature<'static> {
        // Serialized as a (whole seconds, nanoseconds) tuple:
        // https://github.com/time-rs/time/blob/f9398b9598757508ca3815694f23203843e0011b/src/serde/mod.rs#L119
        <(i64, i32)>::signature()
    }
}

#[cfg(feature = "time")]
impl Type for time::OffsetDateTime {
    fn signature() -> Signature<'static> {
        // Serialized as a tuple:
        // https://github.com/time-rs/time/blob/f9398b9598757508ca3815694f23203843e0011b/src/serde/mod.rs#L155
        <(
            // year
            i32,
            // ordinal
            u16,
            // hour
            u8,
            // minute
            u8,
            // second
            u8,
            // nanosecond
            u32,
            // offset.whole_hours
            i8,
            // offset.minutes_past_hour
            i8,
            // offset.seconds_past_minute
            i8,
        )>::signature()
    }
}

#[cfg(feature = "time")]
impl Type for time::PrimitiveDateTime {
    fn signature() -> Signature<'static> {
        // Serialized as a tuple:
        // https://github.com/time-rs/time/blob/f9398b9598757508ca3815694f23203843e0011b/src/serde/mod.rs#L200
        <(
            // year
            i32,
            // ordinal
            u16,
            // hour
            u8,
            // minute
            u8,
            // second
            u8,
            // nanosecond
            u32,
        )>::signature()
    }
}

#[cfg(feature = "time")]
impl Type for time::Time {
    fn signature() -> Signature<'static> {
        // Serialized as a tuple:
        // https://github.com/time-rs/time/blob/f9398b9598757508ca3815694f23203843e0011b/src/serde/mod.rs#L246
        <(
            // hour
            u8,
            // minute
            u8,
            // second
            u8,
            // nanosecond
            u32,
        )>::signature()
    }
}

#[cfg(feature = "time")]
impl Type for time::UtcOffset {
    fn signature() -> Signature<'static> {
        // Serialized as a (whole hours, minutes past hour, seconds past minute) tuple:
        // https://github.com/time-rs/time/blob/f9398b9598757508ca3815694f23203843e0011b/src/serde/mod.rs#L282
        <(i8, i8, i8)>::signature()
    }
}

#[cfg(feature = "time")]
impl Type for time::Weekday {
    fn signature() -> Signature<'static> {
        // Serialized as number from Monday:
        // https://github.com/time-rs/time/blob/f9398b9598757508ca3815694f23203843e0011b/src/serde/mod.rs#L312
        u8::signature()
    }
}

#[cfg(feature = "time")]
impl Type for time::Month {
    fn signature() -> Signature<'static> {
        // Serialized as month number:
        // https://github.com/time-rs/time/blob/f9398b9598757508ca3815694f23203843e0011b/src/serde/mod.rs#L337
        u8::signature()
    }
}

#[cfg(feature = "chrono")]
impl<Tz: chrono::TimeZone> Type for chrono::DateTime<Tz> {
    fn signature() -> Signature<'static> {
        <&str>::signature()
    }
}

#[cfg(feature = "chrono")]
static_str_type!(chrono::NaiveDateTime);
#[cfg(feature = "chrono")]
static_str_type!(chrono::NaiveTime);

// TODO: Blanket implementation for more types: https://github.com/serde-rs/serde/blob/master/serde/src/ser/impls.rs
