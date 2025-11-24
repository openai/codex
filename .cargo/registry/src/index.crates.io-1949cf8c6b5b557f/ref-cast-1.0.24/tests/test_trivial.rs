#![allow(clippy::manual_non_exhaustive)]

use ref_cast::RefCast;
use std::marker::PhantomData;

type Marker = PhantomData<str>;

#[derive(RefCast)]
#[repr(transparent)]
pub struct ImplicitUnit {
    pub value: usize,
    _private: (),
}

#[derive(RefCast)]
#[repr(transparent)]
pub struct ImplicitPhantomData<T> {
    pub value: T,
    pub marker: PhantomData<T>,
}

#[derive(RefCast)]
#[repr(transparent)]
pub struct ExplicitTrivial {
    pub value: usize,
    #[trivial]
    pub marker: Marker,
}

#[derive(RefCast)]
#[repr(C)]
pub struct Override<U, V> {
    #[trivial]
    pub first: PhantomData<U>,
    pub second: PhantomData<V>,
}

#[derive(RefCast)]
#[repr(transparent)]
pub struct Unsized<'a> {
    pub marker: PhantomData<&'a str>,
    pub value: str,
}

#[test]
fn test_trivial() {
    ImplicitUnit::ref_cast(&0);
    ImplicitPhantomData::ref_cast(&0);
    ExplicitTrivial::ref_cast(&0);
    Override::<u8, i8>::ref_cast(&PhantomData::<i8>);
    Unsized::ref_cast("...");
}
