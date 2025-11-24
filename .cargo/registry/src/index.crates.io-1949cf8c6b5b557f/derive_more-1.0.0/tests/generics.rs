#![cfg_attr(not(feature = "std"), no_std)]
#![allow(dead_code)] // some code is tested for type checking only

use derive_more::{
    Add, AddAssign, Constructor, Deref, DerefMut, Display, Error, From, FromStr, Index,
    IndexMut, IntoIterator, Mul, MulAssign, Not, Sum, TryInto,
};

#[derive(
    From,
    FromStr,
    Display,
    Index,
    Not,
    Add,
    Mul,
    Sum,
    IndexMut,
    AddAssign,
    MulAssign,
    Deref,
    DerefMut,
    IntoIterator,
    Constructor
)]
#[deref(forward)]
#[deref_mut(forward)]
#[into_iterator(owned, ref, ref_mut)]
struct Wrapped<T: Clone>(T);

#[derive(Deref, DerefMut)]
struct Wrapped2<T: Clone>(T);

#[derive(From, Not, Add, Mul, AddAssign, MulAssign, Constructor, Sum)]
struct WrappedDouble<T: Clone, U: Clone>(T, U);

#[derive(From)]
#[from(forward)]
struct WrappedDouble2<T: Clone, U: Clone>(T, U);

#[cfg(nightly)]
#[derive(
    From,
    FromStr,
    Display,
    Index,
    Not,
    Add,
    Mul,
    Sum,
    IndexMut,
    AddAssign,
    MulAssign,
    Deref,
    DerefMut,
    IntoIterator,
    Constructor
)]
struct WrappedWithConst<T, const C: u32>(T);

#[derive(
    From,
    FromStr,
    Display,
    Index,
    Not,
    Add,
    Mul,
    IndexMut,
    AddAssign,
    MulAssign,
    Deref,
    DerefMut,
    IntoIterator,
    Constructor,
    Sum
)]
#[deref(forward)]
#[deref_mut(forward)]
#[into_iterator(owned, ref, ref_mut)]
struct Struct1<T: Clone> {
    t: T,
}

#[derive(Deref, DerefMut)]
struct Struct2<T: Clone> {
    t: T,
}

#[derive(From, Not, Add, Mul, AddAssign, MulAssign, Constructor, Sum)]
struct DoubleStruct<T: Clone, U: Clone> {
    t: T,
    u: U,
}

#[derive(From)]
#[from(forward)]
struct DoubleStruct2<T: Clone, U: Clone> {
    t: T,
    u: U,
}

#[derive(From, Not, Add)]
enum TupleEnum<T: Clone, U: Clone> {
    Tuple(T),
    DoubleTuple(T, U),
}

#[derive(From)]
#[from(forward)]
enum TupleEnum2<T: Clone, U: Clone, X: Clone> {
    DoubleTuple(T, U),
    TripleTuple(T, U, X),
}

#[derive(From, Not, Add)]
enum StructEnum<T: Clone, U: Clone> {
    Struct { t: T },
    DoubleStruct { t: T, u: U },
}

#[derive(From)]
#[from(forward)]
enum StructEnum2<T: Clone, U: Clone, X: Clone> {
    DoubleStruct { t: T, u: U },
    TripleStruct { t: T, u: U, x: X },
}

#[derive(Debug, Display, Error)]
enum Enum {}

#[derive(Debug, Display, Error)]
enum EnumGeneric<E> {
    Inner(E),
}

#[derive(Debug, Display, Error)]
enum EnumConst<const X: usize> {}

#[derive(Debug, Display, Error)]
enum EnumConstDefault<const X: usize = 42> {}

#[derive(Debug, Display, Error)]
enum EnumLifetime<'lt: 'static> {
    Inner(&'lt Enum),
}

#[derive(Debug, Display, Error)]
enum EnumConstGeneric<const X: usize, E> {
    Inner(E),
}

#[derive(Debug, Display, Error)]
enum EnumGenericConst<E, const X: usize> {
    Inner(E),
}

#[derive(Debug, Display, Error)]
enum EnumGenericConstDefault<E, const X: usize = 42> {
    Inner(E),
}

#[derive(Debug, Display, Error)]
enum EnumLifetimeGeneric<'lt: 'static, E> {
    Inner(&'lt E),
}

#[derive(Debug, Display, Error)]
enum EnumLifetimeConst<'lt: 'static, const X: usize> {
    Inner(&'lt EnumConst<X>),
}

#[derive(Debug, Display, Error)]
enum EnumLifetimeConstDefault<'lt: 'static, const X: usize = 42> {
    Inner(&'lt EnumConst<X>),
}

#[derive(Debug, Display, Error)]
enum EnumLifetimeConstGeneric<'lt: 'static, const X: usize, E> {
    Inner(&'lt E),
}

#[derive(Debug, Display, Error)]
enum EnumLifetimeGenericConst<'lt: 'static, E, const X: usize> {
    Inner(&'lt E),
}

#[derive(Debug, Display, Error)]
enum EnumLifetimeGenericConstDefault<'lt: 'static, E, const X: usize = 42> {
    Inner(&'lt E),
}

#[derive(Debug, Display, Error)]
struct Struct;

#[derive(Debug, Display, Error)]
struct StructGeneric<E> {
    inner: E,
}

#[derive(Debug, Display, Error)]
struct StructConst<const X: usize> {}

#[derive(Debug, Display, Error)]
struct StructConstDefault<const X: usize = 42> {}

#[derive(Debug, Display, Error)]
struct StructLifetime<'lt: 'static> {
    inner: &'lt Enum,
}

#[derive(Debug, Display, Error)]
struct StructConstGeneric<const X: usize, E> {
    inner: E,
}

#[derive(Debug, Display, Error)]
struct StructGenericConst<E, const X: usize> {
    inner: E,
}

#[derive(Debug, Display, Error)]
struct StructGenericConstDefault<E, const X: usize = 42> {
    inner: E,
}

#[derive(Debug, Display, Error)]
struct StructLifetimeGeneric<'lt: 'static, E> {
    inner: &'lt E,
}

#[derive(Debug, Display, Error)]
struct StructLifetimeConst<'lt: 'static, const X: usize> {
    inner: &'lt EnumConst<X>,
}

#[derive(Debug, Display, Error)]
struct StructLifetimeConstDefault<'lt: 'static, const X: usize = 42> {
    inner: &'lt EnumConst<X>,
}

#[derive(Debug, Display, Error)]
struct StructLifetimeConstGeneric<'lt: 'static, const X: usize, E> {
    inner: &'lt E,
}

#[derive(Debug, Display, Error)]
struct StructLifetimeGenericConst<'lt: 'static, E, const X: usize> {
    inner: &'lt E,
}

#[derive(Debug, Display, Error)]
struct StructLifetimeGenericConstDefault<'lt: 'static, E, const X: usize = 42> {
    inner: &'lt E,
}

#[derive(Debug, Display, Error)]
struct StructLifetimeGenericBoundsConstDefault<
    'lt: 'static,
    E: Clone,
    const X: usize = 42,
> {
    inner: &'lt E,
}

#[derive(Debug, Display)]
struct Wrapper<'a, const Y: usize, U>(&'a [U; Y]);

#[derive(Debug, Display, TryInto)]
enum Foo<'lt: 'static, T: Clone, const X: usize> {
    X(Wrapper<'lt, X, T>),
}
