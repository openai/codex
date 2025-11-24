pub(crate) use self::macros::*;
use crate::{formats::Strictness, prelude::*};
#[cfg(feature = "hashbrown_0_14")]
use hashbrown_0_14::{HashMap as HashbrownMap014, HashSet as HashbrownSet014};
#[cfg(feature = "hashbrown_0_15")]
use hashbrown_0_15::{HashMap as HashbrownMap015, HashSet as HashbrownSet015};
#[cfg(feature = "indexmap_1")]
use indexmap_1::{IndexMap, IndexSet};
#[cfg(feature = "indexmap_2")]
use indexmap_2::{IndexMap as IndexMap2, IndexSet as IndexSet2};

///////////////////////////////////////////////////////////////////////////////
// Helper macro used internally

#[cfg(feature = "alloc")]
type BoxedSlice<T> = Box<[T]>;
type Slice<T> = [T];
type Ref<'a, T> = &'a T;
type RefMut<'a, T> = &'a mut T;

pub(crate) mod macros {
    // The unused_imports lint has false-positives around macros
    // https://github.com/rust-lang/rust/issues/78894
    #![allow(unused_imports)]

    macro_rules! foreach_map {
    ($m:ident) => {
        #[cfg(feature = "alloc")]
        $m!(BTreeMap<K, V>);
        #[cfg(feature = "std")]
        $m!(HashMap<K, V, H: Sized>);
        #[cfg(feature = "hashbrown_0_14")]
        $m!(HashbrownMap014<K, V, H: Sized>);
        #[cfg(feature = "hashbrown_0_15")]
        $m!(HashbrownMap015<K, V, H: Sized>);
        #[cfg(feature = "indexmap_1")]
        $m!(IndexMap<K, V, H: Sized>);
        #[cfg(feature = "indexmap_2")]
        $m!(IndexMap2<K, V, H: Sized>);
    };
}

    macro_rules! foreach_set {
    ($m:ident, $T:tt) => {
        #[cfg(feature = "alloc")]
        $m!(BTreeSet<$T>);
        #[cfg(feature = "std")]
        $m!(HashSet<$T, H: Sized>);
        #[cfg(feature = "hashbrown_0_14")]
        $m!(HashbrownSet014<$T, H: Sized>);
        #[cfg(feature = "hashbrown_0_15")]
        $m!(HashbrownSet015<$T, H: Sized>);
        #[cfg(feature = "indexmap_1")]
        $m!(IndexSet<$T, H: Sized>);
        #[cfg(feature = "indexmap_2")]
        $m!(IndexSet2<$T, H: Sized>);
    };
    ($m:ident) => {
        foreach_set!($m, T);
    };
}

    macro_rules! foreach_seq {
        ($m:ident, $T:tt) => {
            foreach_set!($m, $T);

            $m!(Slice<$T>);

            #[cfg(feature = "alloc")]
            $m!(BinaryHeap<$T>);
            #[cfg(feature = "alloc")]
            $m!(BoxedSlice<$T>);
            #[cfg(feature = "alloc")]
            $m!(LinkedList<$T>);
            #[cfg(feature = "alloc")]
            $m!(Vec<$T>);
            #[cfg(feature = "alloc")]
            $m!(VecDeque<$T>);
        };
        ($m:
            ident) => {
            foreach_seq!($m, T);
        };
    }

    // Make the macros available to the rest of the crate
    pub(crate) use foreach_map;
    pub(crate) use foreach_seq;
    pub(crate) use foreach_set;
}

///////////////////////////////////////////////////////////////////////////////
// region: Simple Wrapper types (e.g., Box, Option)

#[allow(unused_macros)]
macro_rules! pinned_wrapper {
    ($wrapper:ident $($lifetime:lifetime)?) => {
        impl<$($lifetime,)? T, U> SerializeAs<Pin<$wrapper<$($lifetime,)? T>>> for Pin<$wrapper<$($lifetime,)? U>>
        where
            U: SerializeAs<T>,
        {
            fn serialize_as<S>(source: &Pin<$wrapper<$($lifetime,)? T>>, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                SerializeAsWrap::<T, U>::new(source).serialize(serializer)
            }
        }
    };
}

impl<'a, T, U> SerializeAs<&'a T> for &'a U
where
    U: SerializeAs<T>,
    T: ?Sized,
    U: ?Sized,
{
    fn serialize_as<S>(source: &&'a T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        SerializeAsWrap::<T, U>::new(source).serialize(serializer)
    }
}

impl<'a, T, U> SerializeAs<&'a mut T> for &'a mut U
where
    U: SerializeAs<T>,
    T: ?Sized,
    U: ?Sized,
{
    fn serialize_as<S>(source: &&'a mut T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        SerializeAsWrap::<T, U>::new(source).serialize(serializer)
    }
}

pinned_wrapper!(Ref 'a);
pinned_wrapper!(RefMut 'a);

#[cfg(feature = "alloc")]
impl<T, U> SerializeAs<Box<T>> for Box<U>
where
    U: SerializeAs<T>,
{
    fn serialize_as<S>(source: &Box<T>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        SerializeAsWrap::<T, U>::new(source).serialize(serializer)
    }
}

#[cfg(feature = "alloc")]
pinned_wrapper!(Box);

impl<T, U> SerializeAs<Option<T>> for Option<U>
where
    U: SerializeAs<T>,
{
    fn serialize_as<S>(source: &Option<T>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match *source {
            Some(ref value) => serializer.serialize_some(&SerializeAsWrap::<T, U>::new(value)),
            None => serializer.serialize_none(),
        }
    }
}

impl<T, U> SerializeAs<Bound<T>> for Bound<U>
where
    U: SerializeAs<T>,
    T: Sized,
{
    fn serialize_as<S>(source: &Bound<T>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match source {
            Bound::Unbounded => Bound::Unbounded,
            Bound::Included(v) => Bound::Included(SerializeAsWrap::<T, U>::new(v)),
            Bound::Excluded(v) => Bound::Excluded(SerializeAsWrap::<T, U>::new(v)),
        }
        .serialize(serializer)
    }
}

#[cfg(feature = "alloc")]
impl<T, U> SerializeAs<Rc<T>> for Rc<U>
where
    U: SerializeAs<T>,
{
    fn serialize_as<S>(source: &Rc<T>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        SerializeAsWrap::<T, U>::new(source).serialize(serializer)
    }
}

#[cfg(feature = "alloc")]
pinned_wrapper!(Rc);

#[cfg(feature = "alloc")]
impl<T, U> SerializeAs<RcWeak<T>> for RcWeak<U>
where
    U: SerializeAs<T>,
{
    fn serialize_as<S>(source: &RcWeak<T>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        SerializeAsWrap::<Option<Rc<T>>, Option<Rc<U>>>::new(&source.upgrade())
            .serialize(serializer)
    }
}

#[cfg(all(feature = "alloc", target_has_atomic = "ptr"))]
impl<T, U> SerializeAs<Arc<T>> for Arc<U>
where
    U: SerializeAs<T>,
{
    fn serialize_as<S>(source: &Arc<T>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        SerializeAsWrap::<T, U>::new(source).serialize(serializer)
    }
}

#[cfg(all(feature = "alloc", target_has_atomic = "ptr"))]
pinned_wrapper!(Arc);

#[cfg(all(feature = "alloc", target_has_atomic = "ptr"))]
impl<T, U> SerializeAs<ArcWeak<T>> for ArcWeak<U>
where
    U: SerializeAs<T>,
{
    fn serialize_as<S>(source: &ArcWeak<T>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        SerializeAsWrap::<Option<Arc<T>>, Option<Arc<U>>>::new(&source.upgrade())
            .serialize(serializer)
    }
}

impl<T, U> SerializeAs<Cell<T>> for Cell<U>
where
    U: SerializeAs<T>,
    T: Copy,
{
    fn serialize_as<S>(source: &Cell<T>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        SerializeAsWrap::<T, U>::new(&source.get()).serialize(serializer)
    }
}

impl<T, U> SerializeAs<RefCell<T>> for RefCell<U>
where
    U: SerializeAs<T>,
{
    fn serialize_as<S>(source: &RefCell<T>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match source.try_borrow() {
            Ok(source) => SerializeAsWrap::<T, U>::new(&*source).serialize(serializer),
            Err(_) => Err(S::Error::custom("already mutably borrowed")),
        }
    }
}

#[cfg(feature = "std")]
impl<T, U> SerializeAs<Mutex<T>> for Mutex<U>
where
    U: SerializeAs<T>,
{
    fn serialize_as<S>(source: &Mutex<T>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match source.lock() {
            Ok(source) => SerializeAsWrap::<T, U>::new(&*source).serialize(serializer),
            Err(_) => Err(S::Error::custom("lock poison error while serializing")),
        }
    }
}

#[cfg(feature = "std")]
impl<T, U> SerializeAs<RwLock<T>> for RwLock<U>
where
    U: SerializeAs<T>,
{
    fn serialize_as<S>(source: &RwLock<T>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match source.read() {
            Ok(source) => SerializeAsWrap::<T, U>::new(&*source).serialize(serializer),
            Err(_) => Err(S::Error::custom("lock poison error while serializing")),
        }
    }
}

impl<T, TAs, E, EAs> SerializeAs<Result<T, E>> for Result<TAs, EAs>
where
    TAs: SerializeAs<T>,
    EAs: SerializeAs<E>,
{
    fn serialize_as<S>(source: &Result<T, E>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        source
            .as_ref()
            .map(SerializeAsWrap::<T, TAs>::new)
            .map_err(SerializeAsWrap::<E, EAs>::new)
            .serialize(serializer)
    }
}

impl<T, As, const N: usize> SerializeAs<[T; N]> for [As; N]
where
    As: SerializeAs<T>,
{
    fn serialize_as<S>(array: &[T; N], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut arr = serializer.serialize_tuple(N)?;
        for elem in array {
            arr.serialize_element(&SerializeAsWrap::<T, As>::new(elem))?;
        }
        arr.end()
    }
}

// endregion
///////////////////////////////////////////////////////////////////////////////
// region: More complex wrappers that are not just a single value

impl<Idx, IdxAs> SerializeAs<Range<Idx>> for Range<IdxAs>
where
    IdxAs: SerializeAs<Idx>,
{
    fn serialize_as<S>(value: &Range<Idx>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        Range {
            start: SerializeAsWrap::<Idx, IdxAs>::new(&value.start),
            end: SerializeAsWrap::<Idx, IdxAs>::new(&value.end),
        }
        .serialize(serializer)
    }
}

impl<Idx, IdxAs> SerializeAs<RangeFrom<Idx>> for RangeFrom<IdxAs>
where
    IdxAs: SerializeAs<Idx>,
{
    fn serialize_as<S>(value: &RangeFrom<Idx>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        RangeFrom {
            start: SerializeAsWrap::<Idx, IdxAs>::new(&value.start),
        }
        .serialize(serializer)
    }
}

impl<Idx, IdxAs> SerializeAs<RangeInclusive<Idx>> for RangeInclusive<IdxAs>
where
    IdxAs: SerializeAs<Idx>,
{
    fn serialize_as<S>(value: &RangeInclusive<Idx>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        RangeInclusive::new(
            SerializeAsWrap::<Idx, IdxAs>::new(value.start()),
            SerializeAsWrap::<Idx, IdxAs>::new(value.end()),
        )
        .serialize(serializer)
    }
}

impl<Idx, IdxAs> SerializeAs<RangeTo<Idx>> for RangeTo<IdxAs>
where
    IdxAs: SerializeAs<Idx>,
{
    fn serialize_as<S>(value: &RangeTo<Idx>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        RangeTo {
            end: SerializeAsWrap::<Idx, IdxAs>::new(&value.end),
        }
        .serialize(serializer)
    }
}

// endregion
///////////////////////////////////////////////////////////////////////////////
// region: Collection Types (e.g., Maps, Sets, Vec)

macro_rules! seq_impl {
    ($ty:ident < T $(: $tbound1:ident $(+ $tbound2:ident)*)* $(, $typaram:ident : $bound:ident )* >) => {
        impl<T, U $(, $typaram)*> SerializeAs<$ty<T $(, $typaram)*>> for $ty<U $(, $typaram)*>
        where
            U: SerializeAs<T>,
            $(T: ?Sized + $tbound1 $(+ $tbound2)*,)*
            $($typaram: ?Sized + $bound,)*
        {
            fn serialize_as<S>(source: &$ty<T $(, $typaram)*>, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.collect_seq(source.iter().map(|item| SerializeAsWrap::<T, U>::new(item)))
            }
        }
    }
}
foreach_seq!(seq_impl);

#[cfg(feature = "alloc")]
macro_rules! map_impl {
    ($ty:ident < K, V $(, $typaram:ident : $bound:ident)* >) => {
        impl<K, KU, V, VU $(, $typaram)*> SerializeAs<$ty<K, V $(, $typaram)*>> for $ty<KU, VU $(, $typaram)*>
        where
            KU: SerializeAs<K>,
            VU: SerializeAs<V>,
            $($typaram: ?Sized + $bound,)*
        {
            fn serialize_as<S>(source: &$ty<K, V $(, $typaram)*>, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.collect_map(source.iter().map(|(k, v)| (SerializeAsWrap::<K, KU>::new(k), SerializeAsWrap::<V, VU>::new(v))))
            }
        }
    }
}
foreach_map!(map_impl);

macro_rules! tuple_impl {
    ($len:literal $($n:tt $t:ident $tas:ident)+) => {
        impl<$($t, $tas,)+> SerializeAs<($($t,)+)> for ($($tas,)+)
        where
            $($tas: SerializeAs<$t>,)+
        {
            fn serialize_as<S>(tuple: &($($t,)+), serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                use serde::ser::SerializeTuple;
                let mut tup = serializer.serialize_tuple($len)?;
                $(
                    tup.serialize_element(&SerializeAsWrap::<$t, $tas>::new(&tuple.$n))?;
                )+
                tup.end()
            }
        }
    };
}

tuple_impl!(1 0 T0 As0);
tuple_impl!(2 0 T0 As0 1 T1 As1);
tuple_impl!(3 0 T0 As0 1 T1 As1 2 T2 As2);
tuple_impl!(4 0 T0 As0 1 T1 As1 2 T2 As2 3 T3 As3);
tuple_impl!(5 0 T0 As0 1 T1 As1 2 T2 As2 3 T3 As3 4 T4 As4);
tuple_impl!(6 0 T0 As0 1 T1 As1 2 T2 As2 3 T3 As3 4 T4 As4 5 T5 As5);
tuple_impl!(7 0 T0 As0 1 T1 As1 2 T2 As2 3 T3 As3 4 T4 As4 5 T5 As5 6 T6 As6);
tuple_impl!(8 0 T0 As0 1 T1 As1 2 T2 As2 3 T3 As3 4 T4 As4 5 T5 As5 6 T6 As6 7 T7 As7);
tuple_impl!(9 0 T0 As0 1 T1 As1 2 T2 As2 3 T3 As3 4 T4 As4 5 T5 As5 6 T6 As6 7 T7 As7 8 T8 As8);
tuple_impl!(10 0 T0 As0 1 T1 As1 2 T2 As2 3 T3 As3 4 T4 As4 5 T5 As5 6 T6 As6 7 T7 As7 8 T8 As8 9 T9 As9);
tuple_impl!(11 0 T0 As0 1 T1 As1 2 T2 As2 3 T3 As3 4 T4 As4 5 T5 As5 6 T6 As6 7 T7 As7 8 T8 As8 9 T9 As9 10 T10 As10);
tuple_impl!(12 0 T0 As0 1 T1 As1 2 T2 As2 3 T3 As3 4 T4 As4 5 T5 As5 6 T6 As6 7 T7 As7 8 T8 As8 9 T9 As9 10 T10 As10 11 T11 As11);
tuple_impl!(13 0 T0 As0 1 T1 As1 2 T2 As2 3 T3 As3 4 T4 As4 5 T5 As5 6 T6 As6 7 T7 As7 8 T8 As8 9 T9 As9 10 T10 As10 11 T11 As11 12 T12 As12);
tuple_impl!(14 0 T0 As0 1 T1 As1 2 T2 As2 3 T3 As3 4 T4 As4 5 T5 As5 6 T6 As6 7 T7 As7 8 T8 As8 9 T9 As9 10 T10 As10 11 T11 As11 12 T12 As12 13 T13 As13);
tuple_impl!(15 0 T0 As0 1 T1 As1 2 T2 As2 3 T3 As3 4 T4 As4 5 T5 As5 6 T6 As6 7 T7 As7 8 T8 As8 9 T9 As9 10 T10 As10 11 T11 As11 12 T12 As12 13 T13 As13 14 T14 As14);
tuple_impl!(16 0 T0 As0 1 T1 As1 2 T2 As2 3 T3 As3 4 T4 As4 5 T5 As5 6 T6 As6 7 T7 As7 8 T8 As8 9 T9 As9 10 T10 As10 11 T11 As11 12 T12 As12 13 T13 As13 14 T14 As14 15 T15 As15);

#[cfg(feature = "alloc")]
macro_rules! map_as_tuple_seq_intern {
    ($tyorig:ident < K, V $(, $typaram:ident : $bound:ident)* >, $ty:ident <(K, V)>) => {
        impl<K, KAs, V, VAs $(, $typaram)*> SerializeAs<$tyorig<K, V $(, $typaram)*>> for $ty<(KAs, VAs)>
        where
            KAs: SerializeAs<K>,
            VAs: SerializeAs<V>,
            $($typaram: ?Sized + $bound,)*
        {
            fn serialize_as<S>(source: &$tyorig<K, V $(, $typaram)*>, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.collect_seq(source.iter().map(|(k, v)| {
                    (
                        SerializeAsWrap::<K, KAs>::new(k),
                        SerializeAsWrap::<V, VAs>::new(v),
                    )
                }))
            }
        }
    };
}
#[cfg(feature = "alloc")]
macro_rules! map_as_tuple_seq {
    ($tyorig:ident < K, V $(, $typaram:ident : $bound:ident)* >) => {
        map_as_tuple_seq_intern!($tyorig<K, V $(, $typaram: $bound)* >, Seq<(K, V)>);
        #[cfg(feature = "alloc")]
        map_as_tuple_seq_intern!($tyorig<K, V $(, $typaram: $bound)* >, Vec<(K, V)>);
    }
}
foreach_map!(map_as_tuple_seq);

macro_rules! tuple_seq_as_map_impl_intern {
    ($tyorig:ident < (K, V) $(, $typaram:ident : $bound:ident)* >, $ty:ident <K, V>) => {
        #[allow(clippy::implicit_hasher)]
        impl<K, KAs, V, VAs $(, $typaram)*> SerializeAs<$tyorig<(K, V) $(, $typaram)*>> for $ty<KAs, VAs>
        where
            KAs: SerializeAs<K>,
            VAs: SerializeAs<V>,
            $($typaram: ?Sized + $bound,)*
        {
            fn serialize_as<S>(source: &$tyorig<(K, V) $(, $typaram)*>, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.collect_map(source.iter().map(|(k, v)| {
                    (
                        SerializeAsWrap::<K, KAs>::new(k),
                        SerializeAsWrap::<V, VAs>::new(v),
                    )
                }))
            }
        }
    };
}
macro_rules! tuple_seq_as_map_impl {
    ($tyorig:ident < (K, V) $(, $typaram:ident : $bound:ident)* >) => {
        tuple_seq_as_map_impl_intern!($tyorig<(K, V) $(, $typaram: $bound)* >, Map<K, V>);
        #[cfg(feature = "alloc")]
        tuple_seq_as_map_impl_intern!($tyorig<(K, V) $(, $typaram: $bound)* >, BTreeMap<K, V>);
        #[cfg(feature = "std")]
        tuple_seq_as_map_impl_intern!($tyorig<(K, V) $(, $typaram: $bound)* >, HashMap<K, V>);
    }
}
foreach_seq!(tuple_seq_as_map_impl, (K, V));
tuple_seq_as_map_impl!(Option<(K, V)>);

macro_rules! tuple_seq_as_map_arr {
    ($tyorig:ty, $ty:ident <K, V>) => {
        #[allow(clippy::implicit_hasher)]
        impl<K, KAs, V, VAs, const N: usize> SerializeAs<$tyorig> for $ty<KAs, VAs>
        where
            KAs: SerializeAs<K>,
            VAs: SerializeAs<V>,
        {
            fn serialize_as<S>(source: &$tyorig, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.collect_map(source.iter().map(|(k, v)| {
                    (
                        SerializeAsWrap::<K, KAs>::new(k),
                        SerializeAsWrap::<V, VAs>::new(v),
                    )
                }))
            }
        }
    };
}
tuple_seq_as_map_arr!([(K, V); N], Map<K, V>);
#[cfg(feature = "alloc")]
tuple_seq_as_map_arr!([(K, V); N], BTreeMap<K, V>);
#[cfg(feature = "std")]
tuple_seq_as_map_arr!([(K, V); N], HashMap<K, V>);

// endregion
///////////////////////////////////////////////////////////////////////////////
// region: Conversion types which cause different serialization behavior

impl<T> SerializeAs<T> for Same
where
    T: Serialize + ?Sized,
{
    fn serialize_as<S>(source: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        source.serialize(serializer)
    }
}

impl<T> SerializeAs<T> for DisplayFromStr
where
    T: Display,
{
    fn serialize_as<S>(source: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(source)
    }
}

impl<T, H, F> SerializeAs<T> for IfIsHumanReadable<H, F>
where
    T: ?Sized,
    H: SerializeAs<T>,
    F: SerializeAs<T>,
{
    fn serialize_as<S>(source: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if serializer.is_human_readable() {
            H::serialize_as(source, serializer)
        } else {
            F::serialize_as(source, serializer)
        }
    }
}

impl<T> SerializeAs<Option<T>> for NoneAsEmptyString
where
    T: Display,
{
    fn serialize_as<S>(source: &Option<T>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if let Some(value) = source {
            serializer.collect_str(value)
        } else {
            serializer.serialize_str("")
        }
    }
}

#[cfg(feature = "alloc")]
impl<T, TAs> SerializeAs<T> for DefaultOnError<TAs>
where
    TAs: SerializeAs<T>,
{
    fn serialize_as<S>(source: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        TAs::serialize_as(source, serializer)
    }
}

#[cfg(feature = "alloc")]
impl SerializeAs<Vec<u8>> for BytesOrString {
    fn serialize_as<S>(source: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        source.serialize(serializer)
    }
}

impl<SEPARATOR, I, T> SerializeAs<I> for StringWithSeparator<SEPARATOR, T>
where
    SEPARATOR: formats::Separator,
    for<'x> &'x I: IntoIterator<Item = &'x T>,
    T: Display,
    // This set of bounds is enough to make the function compile but has inference issues
    // making it unusable at the moment.
    // https://github.com/rust-lang/rust/issues/89196#issuecomment-932024770
    // for<'x> &'x I: IntoIterator,
    // for<'x> <&'x I as IntoIterator>::Item: Display,
{
    fn serialize_as<S>(source: &I, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        pub(crate) struct DisplayWithSeparator<'a, I, SEPARATOR>(&'a I, PhantomData<SEPARATOR>);

        impl<'a, I, SEPARATOR> DisplayWithSeparator<'a, I, SEPARATOR> {
            pub(crate) fn new(iter: &'a I) -> Self {
                Self(iter, PhantomData)
            }
        }

        impl<'a, I, SEPARATOR> Display for DisplayWithSeparator<'a, I, SEPARATOR>
        where
            SEPARATOR: formats::Separator,
            &'a I: IntoIterator,
            <&'a I as IntoIterator>::Item: Display,
        {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                let mut iter = self.0.into_iter();

                if let Some(first) = iter.next() {
                    first.fmt(f)?;
                }
                for elem in iter {
                    f.write_str(SEPARATOR::separator())?;
                    elem.fmt(f)?;
                }

                Ok(())
            }
        }

        serializer.collect_str(&DisplayWithSeparator::<I, SEPARATOR>::new(source))
    }
}

macro_rules! use_signed_duration {
    (
        $main_trait:ident $internal_trait:ident =>
        {
            $ty:ty =>
            $({
                $format:ty, $strictness:ty =>
                $($tbound:ident: $bound:ident $(,)?)*
            })*
        }
    ) => {
        $(
            impl<$($tbound,)*> SerializeAs<$ty> for $main_trait<$format, $strictness>
            where
                $($tbound: $bound,)*
            {
                fn serialize_as<S>(source: &$ty, serializer: S) -> Result<S::Ok, S::Error>
                where
                    S: Serializer,
                {
                    $internal_trait::<$format, $strictness>::serialize_as(
                        &DurationSigned::from(source),
                        serializer,
                    )
                }
            }
        )*
    };
    (
        $( $main_trait:ident $internal_trait:ident, )+ => $rest:tt
    ) => {
        $( use_signed_duration!($main_trait $internal_trait => $rest); )+
    };
}

use_signed_duration!(
    DurationSeconds DurationSeconds,
    DurationMilliSeconds DurationMilliSeconds,
    DurationMicroSeconds DurationMicroSeconds,
    DurationNanoSeconds DurationNanoSeconds,
    => {
        Duration =>
        {u64, STRICTNESS => STRICTNESS: Strictness}
        {f64, STRICTNESS => STRICTNESS: Strictness}
    }
);
#[cfg(feature = "alloc")]
use_signed_duration!(
    DurationSeconds DurationSeconds,
    DurationMilliSeconds DurationMilliSeconds,
    DurationMicroSeconds DurationMicroSeconds,
    DurationNanoSeconds DurationNanoSeconds,
    => {
        Duration =>
        {String, STRICTNESS => STRICTNESS: Strictness}
    }
);
use_signed_duration!(
    DurationSecondsWithFrac DurationSecondsWithFrac,
    DurationMilliSecondsWithFrac DurationMilliSecondsWithFrac,
    DurationMicroSecondsWithFrac DurationMicroSecondsWithFrac,
    DurationNanoSecondsWithFrac DurationNanoSecondsWithFrac,
    => {
        Duration =>
        {f64, STRICTNESS => STRICTNESS: Strictness}
    }
);
#[cfg(feature = "alloc")]
use_signed_duration!(
    DurationSecondsWithFrac DurationSecondsWithFrac,
    DurationMilliSecondsWithFrac DurationMilliSecondsWithFrac,
    DurationMicroSecondsWithFrac DurationMicroSecondsWithFrac,
    DurationNanoSecondsWithFrac DurationNanoSecondsWithFrac,
    => {
        Duration =>
        {String, STRICTNESS => STRICTNESS: Strictness}
    }
);

#[cfg(feature = "std")]
use_signed_duration!(
    TimestampSeconds DurationSeconds,
    TimestampMilliSeconds DurationMilliSeconds,
    TimestampMicroSeconds DurationMicroSeconds,
    TimestampNanoSeconds DurationNanoSeconds,
    => {
        SystemTime =>
        {i64, STRICTNESS => STRICTNESS: Strictness}
        {f64, STRICTNESS => STRICTNESS: Strictness}
        {String, STRICTNESS => STRICTNESS: Strictness}
    }
);
#[cfg(feature = "std")]
use_signed_duration!(
    TimestampSecondsWithFrac DurationSecondsWithFrac,
    TimestampMilliSecondsWithFrac DurationMilliSecondsWithFrac,
    TimestampMicroSecondsWithFrac DurationMicroSecondsWithFrac,
    TimestampNanoSecondsWithFrac DurationNanoSecondsWithFrac,
    => {
        SystemTime =>
        {f64, STRICTNESS => STRICTNESS: Strictness}
        {String, STRICTNESS => STRICTNESS: Strictness}
    }
);

impl<T, U> SerializeAs<T> for DefaultOnNull<U>
where
    U: SerializeAs<T>,
{
    fn serialize_as<S>(source: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_some(&SerializeAsWrap::<T, U>::new(source))
    }
}

impl SerializeAs<&[u8]> for Bytes {
    fn serialize_as<S>(bytes: &&[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(bytes)
    }
}

#[cfg(feature = "alloc")]
impl SerializeAs<Vec<u8>> for Bytes {
    fn serialize_as<S>(bytes: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(bytes)
    }
}

#[cfg(feature = "alloc")]
impl SerializeAs<Box<[u8]>> for Bytes {
    fn serialize_as<S>(bytes: &Box<[u8]>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(bytes)
    }
}

#[cfg(feature = "alloc")]
impl<'a> SerializeAs<Cow<'a, [u8]>> for Bytes {
    fn serialize_as<S>(bytes: &Cow<'a, [u8]>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(bytes)
    }
}

impl<const N: usize> SerializeAs<[u8; N]> for Bytes {
    fn serialize_as<S>(bytes: &[u8; N], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(bytes)
    }
}

impl<const N: usize> SerializeAs<&[u8; N]> for Bytes {
    fn serialize_as<S>(bytes: &&[u8; N], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(*bytes)
    }
}

#[cfg(feature = "alloc")]
impl<const N: usize> SerializeAs<Box<[u8; N]>> for Bytes {
    fn serialize_as<S>(bytes: &Box<[u8; N]>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(&**bytes)
    }
}

#[cfg(feature = "alloc")]
impl<'a, const N: usize> SerializeAs<Cow<'a, [u8; N]>> for Bytes {
    fn serialize_as<S>(bytes: &Cow<'a, [u8; N]>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(bytes.as_ref())
    }
}

#[cfg(feature = "alloc")]
impl<T, U> SerializeAs<Vec<T>> for OneOrMany<U, formats::PreferOne>
where
    U: SerializeAs<T>,
{
    fn serialize_as<S>(source: &Vec<T>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match source.len() {
            1 => SerializeAsWrap::<T, U>::new(source.iter().next().expect("Cannot be empty"))
                .serialize(serializer),
            _ => SerializeAsWrap::<Vec<T>, Vec<U>>::new(source).serialize(serializer),
        }
    }
}

#[cfg(feature = "alloc")]
impl<T, U> SerializeAs<Vec<T>> for OneOrMany<U, formats::PreferMany>
where
    U: SerializeAs<T>,
{
    fn serialize_as<S>(source: &Vec<T>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        SerializeAsWrap::<Vec<T>, Vec<U>>::new(source).serialize(serializer)
    }
}

#[cfg(feature = "alloc")]
impl<T, TAs1> SerializeAs<T> for PickFirst<(TAs1,)>
where
    TAs1: SerializeAs<T>,
{
    fn serialize_as<S>(source: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        SerializeAsWrap::<T, TAs1>::new(source).serialize(serializer)
    }
}

#[cfg(feature = "alloc")]
impl<T, TAs1, TAs2> SerializeAs<T> for PickFirst<(TAs1, TAs2)>
where
    TAs1: SerializeAs<T>,
{
    fn serialize_as<S>(source: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        SerializeAsWrap::<T, TAs1>::new(source).serialize(serializer)
    }
}

#[cfg(feature = "alloc")]
impl<T, TAs1, TAs2, TAs3> SerializeAs<T> for PickFirst<(TAs1, TAs2, TAs3)>
where
    TAs1: SerializeAs<T>,
{
    fn serialize_as<S>(source: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        SerializeAsWrap::<T, TAs1>::new(source).serialize(serializer)
    }
}

#[cfg(feature = "alloc")]
impl<T, TAs1, TAs2, TAs3, TAs4> SerializeAs<T> for PickFirst<(TAs1, TAs2, TAs3, TAs4)>
where
    TAs1: SerializeAs<T>,
{
    fn serialize_as<S>(source: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        SerializeAsWrap::<T, TAs1>::new(source).serialize(serializer)
    }
}

impl<T, U> SerializeAs<T> for FromInto<U>
where
    T: Into<U> + Clone,
    U: Serialize,
{
    fn serialize_as<S>(source: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        source.clone().into().serialize(serializer)
    }
}

impl<T, U> SerializeAs<T> for TryFromInto<U>
where
    T: TryInto<U> + Clone,
    <T as TryInto<U>>::Error: Display,
    U: Serialize,
{
    fn serialize_as<S>(source: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        source
            .clone()
            .try_into()
            .map_err(S::Error::custom)?
            .serialize(serializer)
    }
}

impl<T, U> SerializeAs<T> for FromIntoRef<U>
where
    for<'a> &'a T: Into<U>,
    U: Serialize,
{
    fn serialize_as<S>(source: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        source.into().serialize(serializer)
    }
}

impl<T, U> SerializeAs<T> for TryFromIntoRef<U>
where
    for<'a> &'a T: TryInto<U>,
    for<'a> <&'a T as TryInto<U>>::Error: Display,
    U: Serialize,
{
    fn serialize_as<S>(source: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        source
            .try_into()
            .map_err(S::Error::custom)?
            .serialize(serializer)
    }
}

#[cfg(feature = "alloc")]
impl<'a> SerializeAs<Cow<'a, str>> for BorrowCow {
    fn serialize_as<S>(source: &Cow<'a, str>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(source)
    }
}

#[cfg(feature = "alloc")]
impl<'a> SerializeAs<Cow<'a, [u8]>> for BorrowCow {
    fn serialize_as<S>(value: &Cow<'a, [u8]>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_seq(value.iter())
    }
}

#[cfg(feature = "alloc")]
impl<'a, const N: usize> SerializeAs<Cow<'a, [u8; N]>> for BorrowCow {
    fn serialize_as<S>(value: &Cow<'a, [u8; N]>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_seq(value.iter())
    }
}

impl<STRICTNESS: Strictness> SerializeAs<bool> for BoolFromInt<STRICTNESS> {
    fn serialize_as<S>(source: &bool, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u8(u8::from(*source))
    }
}

// endregion
