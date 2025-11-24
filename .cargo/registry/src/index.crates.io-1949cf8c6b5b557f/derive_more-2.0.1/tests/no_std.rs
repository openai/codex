#![no_std]
#![allow(dead_code)] // some code is tested for type checking only

use derive_more::with_trait::{
    Add, AddAssign, Constructor, Deref, DerefMut, Display, From, FromStr, Index,
    IndexMut, Into, IntoIterator, Mul, MulAssign, Not, Sum, TryInto,
};

#[derive(
    AddAssign,
    MulAssign,
    Add,
    Mul,
    Not,
    Index,
    Display,
    FromStr,
    Into,
    From,
    IndexMut,
    Sum,
    Deref,
    DerefMut,
    Constructor
)]
#[into(owned, ref, ref_mut)]
struct MyInts(u64);

#[derive(Deref, DerefMut)]
#[deref(forward)]
#[deref_mut(forward)]
struct MyBoxedInt<'a>(&'a mut u64);

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

#[derive(From, Not, Add, Mul, AddAssign, Constructor, Sum)]
struct WrappedDouble<T: Clone, U: Clone>(T, U);

#[derive(Add, Not, TryInto)]
#[try_into(owned, ref, ref_mut)]
enum MixedInts {
    SmallInt(i32),
    BigInt(i64),
    TwoSmallInts(i32, i32),
    NamedSmallInts { x: i32, y: i32 },
    UnsignedOne(u32),
    UnsignedTwo(u32),
}

#[derive(Not, Add)]
enum EnumWithUnit {
    SmallInt(i32),
    Unit,
}

#[rustversion::nightly]
mod error {
    use derive_more::with_trait::{Display, Error, From};

    #[derive(Default, Debug, Display, Error)]
    struct Simple;

    #[derive(Default, Debug, Display, Error)]
    struct WithSource {
        source: Simple,
    }
    #[derive(Default, Debug, Display, Error)]
    struct WithExplicitSource {
        #[error(source)]
        explicit_source: Simple,
    }

    #[derive(Default, Debug, Display, Error)]
    struct Tuple(Simple);

    #[derive(Default, Debug, Display, Error)]
    struct WithoutSource(#[error(not(source))] i32);
    #[derive(Debug, Display, Error, From)]
    enum CompoundError {
        Simple,
        WithSource {
            source: Simple,
        },
        WithExplicitSource {
            #[error(source)]
            explicit_source: WithSource,
        },
        Tuple(WithExplicitSource),
        WithoutSource(#[error(not(source))] Tuple),
    }

    #[test]
    fn assert() {
        assert!(Simple.source().is_none());
        assert!(WithSource::default().source().is_some());
        assert!(WithExplicitSource::default().source().is_some());
        assert!(Tuple::default().source().is_some());
        assert!(Tuple::default().source().is_some());
        assert!(WithoutSource::default().source().is_none());
        assert!(CompoundError::Simple.source().is_none());
        assert!(CompoundError::from(Simple).source().is_some());
        assert!(CompoundError::from(WithSource::default())
            .source()
            .is_some());
        assert!(CompoundError::from(WithExplicitSource::default())
            .source()
            .is_some());
        assert!(CompoundError::from(Tuple::default()).source().is_none());
    }
}
