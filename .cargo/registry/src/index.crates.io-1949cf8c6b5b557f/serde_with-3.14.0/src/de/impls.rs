pub(crate) use self::macros::*;
use crate::{formats::*, prelude::*};
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

pub(crate) mod macros {
    // The unused_imports lint has false-positives around macros
    // https://github.com/rust-lang/rust/issues/78894
    #![allow(unused_imports)]

    macro_rules! foreach_map {
        ($m:ident) => {
            #[cfg(feature = "alloc")]
            $m!(BTreeMap<K: Ord, V>, (|_size| BTreeMap::new()));
            #[cfg(feature = "std")]
            $m!(
                HashMap<K: Eq + Hash, V, S: BuildHasher + Default>,
                (|size| HashMap::with_capacity_and_hasher(size, Default::default()))
            );
            #[cfg(feature = "hashbrown_0_14")]
            $m!(
                HashbrownMap014<K: Eq + Hash, V, S: BuildHasher + Default>,
                (|size| HashbrownMap014::with_capacity_and_hasher(size, Default::default()))
            );
            #[cfg(feature = "hashbrown_0_15")]
            $m!(
                HashbrownMap015<K: Eq + Hash, V, S: BuildHasher + Default>,
                (|size| HashbrownMap015::with_capacity_and_hasher(size, Default::default()))
            );
            #[cfg(feature = "indexmap_1")]
            $m!(
                IndexMap<K: Eq + Hash, V, S: BuildHasher + Default>,
                (|size| IndexMap::with_capacity_and_hasher(size, Default::default()))
            );
            #[cfg(feature = "indexmap_2")]
            $m!(
                IndexMap2<K: Eq + Hash, V, S: BuildHasher + Default>,
                (|size| IndexMap2::with_capacity_and_hasher(size, Default::default()))
            );
        };
    }

    macro_rules! foreach_set {
        ($m:ident) => {
            #[cfg(feature = "alloc")]
            $m!(BTreeSet<T: Ord>, (|_| BTreeSet::new()), insert);
            #[cfg(feature = "std")]
            $m!(
                HashSet<T: Eq + Hash, S: BuildHasher + Default>,
                (|size| HashSet::with_capacity_and_hasher(size, S::default())),
                insert
            );
            #[cfg(feature = "hashbrown_0_14")]
            $m!(
                HashbrownSet014<T: Eq + Hash, S: BuildHasher + Default>,
                (|size| HashbrownSet014::with_capacity_and_hasher(size, S::default())),
                insert
            );
            #[cfg(feature = "hashbrown_0_15")]
            $m!(
                HashbrownSet015<T: Eq + Hash, S: BuildHasher + Default>,
                (|size| HashbrownSet015::with_capacity_and_hasher(size, S::default())),
                insert
            );
            #[cfg(feature = "indexmap_1")]
            $m!(
                IndexSet<T: Eq + Hash, S: BuildHasher + Default>,
                (|size| IndexSet::with_capacity_and_hasher(size, S::default())),
                insert
            );
            #[cfg(feature = "indexmap_2")]
            $m!(
                IndexSet2<T: Eq + Hash, S: BuildHasher + Default>,
                (|size| IndexSet2::with_capacity_and_hasher(size, S::default())),
                insert
            );
        };
    }

    macro_rules! foreach_seq {
        ($m:ident) => {
            foreach_set!($m);

            #[cfg(feature = "alloc")]
            $m!(
                BinaryHeap<T: Ord>,
                (|size| BinaryHeap::with_capacity(size)),
                push
            );
            #[cfg(feature = "alloc")]
            $m!(BoxedSlice<T>, (|size| Vec::with_capacity(size)), push);
            #[cfg(feature = "alloc")]
            $m!(LinkedList<T>, (|_| LinkedList::new()), push_back);
            #[cfg(feature = "alloc")]
            $m!(Vec<T>, (|size| Vec::with_capacity(size)), push);
            #[cfg(feature = "alloc")]
            $m!(
                VecDeque<T>,
                (|size| VecDeque::with_capacity(size)),
                push_back
            );
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
    ($wrapper:ident) => {
        impl<'de, T, U> DeserializeAs<'de, Pin<$wrapper<T>>> for Pin<$wrapper<U>>
        where
            U: DeserializeAs<'de, T>,
        {
            fn deserialize_as<D>(deserializer: D) -> Result<Pin<$wrapper<T>>, D::Error>
            where
                D: Deserializer<'de>,
            {
                Ok($wrapper::pin(
                    DeserializeAsWrap::<T, U>::deserialize(deserializer)?.into_inner(),
                ))
            }
        }
    };
}

#[cfg(feature = "alloc")]
impl<'de, T, U> DeserializeAs<'de, Box<T>> for Box<U>
where
    U: DeserializeAs<'de, T>,
{
    fn deserialize_as<D>(deserializer: D) -> Result<Box<T>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Box::new(
            DeserializeAsWrap::<T, U>::deserialize(deserializer)?.into_inner(),
        ))
    }
}

#[cfg(feature = "alloc")]
pinned_wrapper!(Box);

impl<'de, T, U> DeserializeAs<'de, Option<T>> for Option<U>
where
    U: DeserializeAs<'de, T>,
{
    fn deserialize_as<D>(deserializer: D) -> Result<Option<T>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct OptionVisitor<T, U>(PhantomData<(T, U)>);

        impl<'de, T, U> Visitor<'de> for OptionVisitor<T, U>
        where
            U: DeserializeAs<'de, T>,
        {
            type Value = Option<T>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("option")
            }

            #[inline]
            fn visit_unit<E>(self) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                Ok(None)
            }

            #[inline]
            fn visit_none<E>(self) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                Ok(None)
            }

            #[inline]
            fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
            where
                D: Deserializer<'de>,
            {
                U::deserialize_as(deserializer).map(Some)
            }
        }

        deserializer.deserialize_option(OptionVisitor::<T, U>(PhantomData))
    }
}

impl<'de, T, U> DeserializeAs<'de, Bound<T>> for Bound<U>
where
    U: DeserializeAs<'de, T>,
{
    fn deserialize_as<D>(deserializer: D) -> Result<Bound<T>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(
            match Bound::<DeserializeAsWrap<T, U>>::deserialize(deserializer)? {
                Bound::Unbounded => Bound::Unbounded,
                Bound::Included(v) => Bound::Included(v.into_inner()),
                Bound::Excluded(v) => Bound::Excluded(v.into_inner()),
            },
        )
    }
}

#[cfg(feature = "alloc")]
impl<'de, T, U> DeserializeAs<'de, Rc<T>> for Rc<U>
where
    U: DeserializeAs<'de, T>,
{
    fn deserialize_as<D>(deserializer: D) -> Result<Rc<T>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Rc::new(
            DeserializeAsWrap::<T, U>::deserialize(deserializer)?.into_inner(),
        ))
    }
}

#[cfg(feature = "alloc")]
pinned_wrapper!(Rc);

#[cfg(feature = "alloc")]
impl<'de, T, U> DeserializeAs<'de, RcWeak<T>> for RcWeak<U>
where
    U: DeserializeAs<'de, T>,
{
    fn deserialize_as<D>(deserializer: D) -> Result<RcWeak<T>, D::Error>
    where
        D: Deserializer<'de>,
    {
        DeserializeAsWrap::<Option<Rc<T>>, Option<Rc<U>>>::deserialize(deserializer)?;
        Ok(RcWeak::new())
    }
}

#[cfg(all(feature = "alloc", target_has_atomic = "ptr"))]
impl<'de, T, U> DeserializeAs<'de, Arc<T>> for Arc<U>
where
    U: DeserializeAs<'de, T>,
{
    fn deserialize_as<D>(deserializer: D) -> Result<Arc<T>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Arc::new(
            DeserializeAsWrap::<T, U>::deserialize(deserializer)?.into_inner(),
        ))
    }
}

#[cfg(all(feature = "alloc", target_has_atomic = "ptr"))]
pinned_wrapper!(Arc);

#[cfg(all(feature = "alloc", target_has_atomic = "ptr"))]
impl<'de, T, U> DeserializeAs<'de, ArcWeak<T>> for ArcWeak<U>
where
    U: DeserializeAs<'de, T>,
{
    fn deserialize_as<D>(deserializer: D) -> Result<ArcWeak<T>, D::Error>
    where
        D: Deserializer<'de>,
    {
        DeserializeAsWrap::<Option<Arc<T>>, Option<Arc<U>>>::deserialize(deserializer)?;
        Ok(ArcWeak::new())
    }
}

impl<'de, T, U> DeserializeAs<'de, Cell<T>> for Cell<U>
where
    U: DeserializeAs<'de, T>,
{
    fn deserialize_as<D>(deserializer: D) -> Result<Cell<T>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Cell::new(
            DeserializeAsWrap::<T, U>::deserialize(deserializer)?.into_inner(),
        ))
    }
}

impl<'de, T, U> DeserializeAs<'de, RefCell<T>> for RefCell<U>
where
    U: DeserializeAs<'de, T>,
{
    fn deserialize_as<D>(deserializer: D) -> Result<RefCell<T>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(RefCell::new(
            DeserializeAsWrap::<T, U>::deserialize(deserializer)?.into_inner(),
        ))
    }
}

#[cfg(feature = "std")]
impl<'de, T, U> DeserializeAs<'de, Mutex<T>> for Mutex<U>
where
    U: DeserializeAs<'de, T>,
{
    fn deserialize_as<D>(deserializer: D) -> Result<Mutex<T>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Mutex::new(
            DeserializeAsWrap::<T, U>::deserialize(deserializer)?.into_inner(),
        ))
    }
}

#[cfg(feature = "std")]
impl<'de, T, U> DeserializeAs<'de, RwLock<T>> for RwLock<U>
where
    U: DeserializeAs<'de, T>,
{
    fn deserialize_as<D>(deserializer: D) -> Result<RwLock<T>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(RwLock::new(
            DeserializeAsWrap::<T, U>::deserialize(deserializer)?.into_inner(),
        ))
    }
}

impl<'de, T, TAs, E, EAs> DeserializeAs<'de, Result<T, E>> for Result<TAs, EAs>
where
    TAs: DeserializeAs<'de, T>,
    EAs: DeserializeAs<'de, E>,
{
    fn deserialize_as<D>(deserializer: D) -> Result<Result<T, E>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(
            match Result::<DeserializeAsWrap<T, TAs>, DeserializeAsWrap<E, EAs>>::deserialize(
                deserializer,
            )? {
                Ok(value) => Ok(value.into_inner()),
                Err(err) => Err(err.into_inner()),
            },
        )
    }
}

impl<'de, T, As, const N: usize> DeserializeAs<'de, [T; N]> for [As; N]
where
    As: DeserializeAs<'de, T>,
{
    fn deserialize_as<D>(deserializer: D) -> Result<[T; N], D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ArrayVisitor<T, const M: usize>(PhantomData<T>);

        impl<'de, T, As, const M: usize> Visitor<'de> for ArrayVisitor<DeserializeAsWrap<T, As>, M>
        where
            As: DeserializeAs<'de, T>,
        {
            type Value = [T; M];

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_fmt(format_args!("an array of size {M}"))
            }

            fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                utils::array_from_iterator(
                    utils::SeqIter::new(seq).map(
                        |res: Result<DeserializeAsWrap<T, As>, A::Error>| {
                            res.map(DeserializeAsWrap::into_inner)
                        },
                    ),
                    &self,
                )
            }
        }

        deserializer.deserialize_tuple(N, ArrayVisitor::<DeserializeAsWrap<T, As>, N>(PhantomData))
    }
}

// endregion
///////////////////////////////////////////////////////////////////////////////
// region: More complex wrappers that are not just a single value

impl<'de, Idx, IdxAs> DeserializeAs<'de, Range<Idx>> for Range<IdxAs>
where
    IdxAs: DeserializeAs<'de, Idx>,
{
    fn deserialize_as<D>(deserializer: D) -> Result<Range<Idx>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let Range::<DeserializeAsWrap<Idx, IdxAs>> { start, end } =
            Deserialize::deserialize(deserializer)?;

        Ok(Range {
            start: start.into_inner(),
            end: end.into_inner(),
        })
    }
}

impl<'de, Idx, IdxAs> DeserializeAs<'de, RangeFrom<Idx>> for RangeFrom<IdxAs>
where
    IdxAs: DeserializeAs<'de, Idx>,
{
    fn deserialize_as<D>(deserializer: D) -> Result<RangeFrom<Idx>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let RangeFrom::<DeserializeAsWrap<Idx, IdxAs>> { start } =
            Deserialize::deserialize(deserializer)?;

        Ok(RangeFrom {
            start: start.into_inner(),
        })
    }
}

impl<'de, Idx, IdxAs> DeserializeAs<'de, RangeInclusive<Idx>> for RangeInclusive<IdxAs>
where
    IdxAs: DeserializeAs<'de, Idx>,
{
    fn deserialize_as<D>(deserializer: D) -> Result<RangeInclusive<Idx>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let (start, end) =
            RangeInclusive::<DeserializeAsWrap<Idx, IdxAs>>::deserialize(deserializer)?
                .into_inner();

        Ok(RangeInclusive::new(start.into_inner(), end.into_inner()))
    }
}

impl<'de, Idx, IdxAs> DeserializeAs<'de, RangeTo<Idx>> for RangeTo<IdxAs>
where
    IdxAs: DeserializeAs<'de, Idx>,
{
    fn deserialize_as<D>(deserializer: D) -> Result<RangeTo<Idx>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let RangeTo::<DeserializeAsWrap<Idx, IdxAs>> { end } =
            Deserialize::deserialize(deserializer)?;

        Ok(RangeTo {
            end: end.into_inner(),
        })
    }
}

// endregion
///////////////////////////////////////////////////////////////////////////////
// region: Collection Types (e.g., Maps, Sets, Vec)

#[cfg(feature = "alloc")]
macro_rules! seq_impl {
    (
        $ty:ident < T $(: $tbound1:ident $(+ $tbound2:ident)*)? $(, $typaram:ident : $bound1:ident $(+ $bound2:ident)* )* >,
        $with_capacity:expr,
        $append:ident
    ) => {
        impl<'de, T, U $(, $typaram)*> DeserializeAs<'de, $ty<T $(, $typaram)*>> for $ty<U $(, $typaram)*>
        where
            U: DeserializeAs<'de, T>,
            $(T: $tbound1 $(+ $tbound2)*,)?
            $($typaram: $bound1 $(+ $bound2)*),*
        {
            fn deserialize_as<D>(deserializer: D) -> Result<$ty<T $(, $typaram)*>, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct SeqVisitor<T, U $(, $typaram)*> {
                    marker: PhantomData<(T, U $(, $typaram)*)>,
                }

                impl<'de, T, U $(, $typaram)*> Visitor<'de> for SeqVisitor<T, U $(, $typaram)*>
                where
                    U: DeserializeAs<'de, T>,
                    $(T: $tbound1 $(+ $tbound2)*,)?
                    $($typaram: $bound1 $(+ $bound2)*),*
                {
                    type Value = $ty<T $(, $typaram)*>;

                    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                        formatter.write_str("a sequence")
                    }

                    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
                    where
                        A: SeqAccess<'de>,
                    {
                        #[allow(clippy::redundant_closure_call)]
                        let mut values = ($with_capacity)(utils::size_hint_cautious::<T>(seq.size_hint()));

                        while let Some(value) = seq
                            .next_element()?
                            .map(|v: DeserializeAsWrap<T, U>| v.into_inner())
                        {
                            values.$append(value);
                        }

                        Ok(values.into())
                    }
                }

                let visitor = SeqVisitor::<T, U $(, $typaram)*> {
                    marker: PhantomData,
                };
                deserializer.deserialize_seq(visitor)
            }
        }
    };
}
foreach_seq!(seq_impl);

#[cfg(feature = "alloc")]
macro_rules! map_impl {
    (
        $ty:ident < K $(: $kbound1:ident $(+ $kbound2:ident)*)*, V $(, $typaram:ident : $bound1:ident $(+ $bound2:ident)*)* >,
        $with_capacity:expr
    ) => {
        impl<'de, K, V, KU, VU $(, $typaram)*> DeserializeAs<'de, $ty<K, V $(, $typaram)*>> for $ty<KU, VU $(, $typaram)*>
        where
            KU: DeserializeAs<'de, K>,
            VU: DeserializeAs<'de, V>,
            $(K: $kbound1 $(+ $kbound2)*,)*
            $($typaram: $bound1 $(+ $bound2)*),*
        {
            fn deserialize_as<D>(deserializer: D) -> Result<$ty<K, V $(, $typaram)*>, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct MapVisitor<K, V, KU, VU $(, $typaram)*>(PhantomData<(K, V, KU, VU $(, $typaram)*)>);

                impl<'de, K, V, KU, VU $(, $typaram)*> Visitor<'de> for MapVisitor<K, V, KU, VU $(, $typaram)*>
                where
                        KU: DeserializeAs<'de, K>,
                        VU: DeserializeAs<'de, V>,
                        $(K: $kbound1 $(+ $kbound2)*,)*
                        $($typaram: $bound1 $(+ $bound2)*),*
                {
                    type Value = $ty<K, V $(, $typaram)*>;

                    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                        formatter.write_str("a map")
                    }

                    #[inline]
                    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
                    where
                        A: MapAccess<'de>,
                    {
                        #[allow(clippy::redundant_closure_call)]
                        let mut values = ($with_capacity)(utils::size_hint_cautious::<(K, V)>(map.size_hint()));

                        while let Some((key, value)) = (map.next_entry())?.map(|(k, v): (DeserializeAsWrap::<K, KU>, DeserializeAsWrap::<V, VU>)| (k.into_inner(), v.into_inner())) {
                            values.insert(key, value);
                        }

                        Ok(values)
                    }
                }

                let visitor = MapVisitor::<K, V, KU, VU $(, $typaram)*> (PhantomData);
                deserializer.deserialize_map(visitor)
            }
        }
    }
}
foreach_map!(map_impl);

macro_rules! tuple_impl {
    ($len:literal $($n:tt $t:ident $tas:ident)+) => {
        impl<'de, $($t, $tas,)+> DeserializeAs<'de, ($($t,)+)> for ($($tas,)+)
        where
            $($tas: DeserializeAs<'de, $t>,)+
        {
            fn deserialize_as<D>(deserializer: D) -> Result<($($t,)+), D::Error>
            where
                D: Deserializer<'de>,
            {
                struct TupleVisitor<$($t,)+>(PhantomData<($($t,)+)>);

                impl<'de, $($t, $tas,)+> Visitor<'de>
                    for TupleVisitor<$(DeserializeAsWrap<$t, $tas>,)+>
                where
                    $($tas: DeserializeAs<'de, $t>,)+
                {
                    type Value = ($($t,)+);

                    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                        formatter.write_str(concat!("a tuple of size ", $len))
                    }

                    #[allow(non_snake_case)]
                    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
                    where
                        A: SeqAccess<'de>,
                    {
                        $(
                            let $t: DeserializeAsWrap<$t, $tas> = match seq.next_element()? {
                                Some(value) => value,
                                None => return Err(DeError::invalid_length($n, &self)),
                            };
                        )+

                        Ok(($($t.into_inner(),)+))
                    }
                }

                deserializer.deserialize_tuple(
                    $len,
                    TupleVisitor::<$(DeserializeAsWrap<$t, $tas>,)+>(PhantomData),
                )
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
    (
        $tyorig:ident < K $(: $kbound1:ident $(+ $kbound2:ident)*)?, V $(, $typaram:ident : $bound1:ident $(+ $bound2:ident)*)* >,
        $with_capacity:expr,
        $ty:ident <(KAs, VAs)>
    ) => {
        impl<'de, K, KAs, V, VAs $(, $typaram)*> DeserializeAs<'de, $tyorig<K, V $(, $typaram)*>> for $ty<(KAs, VAs)>
        where
            KAs: DeserializeAs<'de, K>,
            VAs: DeserializeAs<'de, V>,
            $(K: $kbound1 $(+ $kbound2)*,)?
            $($typaram: $bound1 $(+ $bound2)*,)*
        {
            fn deserialize_as<D>(deserializer: D) -> Result<$tyorig<K, V $(, $typaram)*>, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct SeqVisitor<K, KAs, V, VAs $(, $typaram)*> {
                    marker: PhantomData<(K, KAs, V, VAs $(, $typaram)*)>,
                }

                impl<'de, K, KAs, V, VAs $(, $typaram)*> Visitor<'de> for SeqVisitor<K, KAs, V, VAs $(, $typaram)*>
                where
                    KAs: DeserializeAs<'de, K>,
                    VAs: DeserializeAs<'de, V>,
                    $(K: $kbound1 $(+ $kbound2)*,)?
                    $($typaram: $bound1 $(+ $bound2)*,)*
                {
                    type Value = $tyorig<K, V $(, $typaram)*>;

                    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                        formatter.write_str("a sequence")
                    }

                    #[inline]
                    fn visit_seq<A>(self, access: A) -> Result<Self::Value, A::Error>
                    where
                        A: SeqAccess<'de>,
                    {
                        let iter = utils::SeqIter::new(access);
                        iter.map(|res| {
                            res.map(
                                |(k, v): (DeserializeAsWrap<K, KAs>, DeserializeAsWrap<V, VAs>)| {
                                    (k.into_inner(), v.into_inner())
                                },
                            )
                        })
                        .collect()
                    }
                }

                let visitor = SeqVisitor::<K, KAs, V, VAs $(, $typaram)*> {
                    marker: PhantomData,
                };
                deserializer.deserialize_seq(visitor)
            }
        }
    };
}
#[cfg(feature = "alloc")]
macro_rules! map_as_tuple_seq {
    (
        $tyorig:ident < K $(: $kbound1:ident $(+ $kbound2:ident)*)?, V $(, $typaram:ident : $bound1:ident $(+ $bound2:ident)*)* >,
        $with_capacity:expr
    ) => {
        map_as_tuple_seq_intern!($tyorig < K $(: $kbound1 $(+ $kbound2)*)? , V $(, $typaram : $bound1 $(+ $bound2)*)* >, $with_capacity, Seq<(KAs, VAs)>);
        #[cfg(feature = "alloc")]
        map_as_tuple_seq_intern!($tyorig < K $(: $kbound1 $(+ $kbound2)*)? , V $(, $typaram : $bound1 $(+ $bound2)*)* >, $with_capacity, Vec<(KAs, VAs)>);
    }
}
foreach_map!(map_as_tuple_seq);

#[cfg(feature = "alloc")]
macro_rules! tuple_seq_as_map_impl_intern {
    (
        $tyorig:ident < (K, V) $(: $($bound:ident $(+)?)+)? $(, $typaram:ident : $bound1:ident $(+ $bound2:ident)* )* >,
        $with_capacity:expr,
        $append:ident,
        $ty:ident <KAs, VAs>
    ) => {
        #[allow(clippy::implicit_hasher)]
        impl<'de, K, KAs, V, VAs $(, $typaram)*> DeserializeAs<'de, $tyorig < (K, V) $(, $typaram)* >> for $ty<KAs, VAs>
        where
            KAs: DeserializeAs<'de, K>,
            VAs: DeserializeAs<'de, V>,
            (K, V): $($($bound +)*)?,
            $($typaram: $bound1 $(+ $bound2)*,)*
        {
            fn deserialize_as<D>(deserializer: D) -> Result<$tyorig < (K, V) $(, $typaram)* >, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct MapVisitor<K, KAs, V, VAs $(, $typaram)*> {
                    marker: PhantomData<(K, KAs, V, VAs $(, $typaram)*)>,
                }

                impl<'de, K, KAs, V, VAs $(, $typaram)*> Visitor<'de> for MapVisitor<K, KAs, V, VAs $(, $typaram)*>
                where
                    KAs: DeserializeAs<'de, K>,
                    VAs: DeserializeAs<'de, V>,
                    (K, V): $($($bound +)*)?,
                    $($typaram: $bound1 $(+ $bound2)*,)*
                {
                    type Value = $tyorig < (K, V) $(, $typaram)* >;

                    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                        formatter.write_str("a map")
                    }

                    #[inline]
                    fn visit_map<A>(self, access: A) -> Result<Self::Value, A::Error>
                    where
                        A: MapAccess<'de>,
                    {
                        let iter = utils::MapIter::new(access);
                        iter.map(|res| {
                            res.map(
                                |(k, v): (DeserializeAsWrap<K, KAs>, DeserializeAsWrap<V, VAs>)| {
                                    (k.into_inner(), v.into_inner())
                                },
                            )
                        })
                        .collect()
                    }
                }

                let visitor = MapVisitor::<K, KAs, V, VAs $(, $typaram)*> {
                    marker: PhantomData,
                };
                deserializer.deserialize_map(visitor)
            }
        }
    }
}
#[cfg(feature = "alloc")]
macro_rules! tuple_seq_as_map_impl {
    (
        $tyorig:ident < T $(: $($bound:ident $(+)?)+)? $(, $typaram:ident : $bound1:ident $(+ $bound2:ident)* )* >,
        $with_capacity:expr,
        $append:ident
    ) => {
        tuple_seq_as_map_impl_intern!($tyorig < (K, V) $(: $($bound +)+)? $(, $typaram: $bound1 $(+ $bound2)*)*>, $with_capacity, $append, Map<KAs, VAs>);
        #[cfg(feature = "alloc")]
        tuple_seq_as_map_impl_intern!($tyorig < (K, V) $(: $($bound +)+)? $(, $typaram: $bound1 $(+ $bound2)*)*>, $with_capacity, $append, BTreeMap<KAs, VAs>);
        #[cfg(feature = "std")]
        tuple_seq_as_map_impl_intern!($tyorig < (K, V) $(: $($bound +)+)? $(, $typaram: $bound1 $(+ $bound2)*)*>, $with_capacity, $append, HashMap<KAs, VAs>);
    }
}
foreach_seq!(tuple_seq_as_map_impl);

// Option does not implement FromIterator directly, so we need a different implementation
#[cfg(feature = "alloc")]
macro_rules! tuple_seq_as_map_option_impl {
    ($ty:ident) => {
        #[allow(clippy::implicit_hasher)]
        impl<'de, K, KAs, V, VAs> DeserializeAs<'de, Option<(K, V)>> for $ty<KAs, VAs>
        where
            KAs: DeserializeAs<'de, K>,
            VAs: DeserializeAs<'de, V>,
        {
            fn deserialize_as<D>(deserializer: D) -> Result<Option<(K, V)>, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct MapVisitor<K, KAs, V, VAs> {
                    marker: PhantomData<(K, KAs, V, VAs)>,
                }

                impl<'de, K, KAs, V, VAs> Visitor<'de> for MapVisitor<K, KAs, V, VAs>
                where
                    KAs: DeserializeAs<'de, K>,
                    VAs: DeserializeAs<'de, V>,
                {
                    type Value = Option<(K, V)>;

                    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                        formatter.write_str("a map of size 1")
                    }

                    #[inline]
                    fn visit_map<A>(self, access: A) -> Result<Self::Value, A::Error>
                    where
                        A: MapAccess<'de>,
                    {
                        let iter = utils::MapIter::new(access);
                        iter.map(|res| {
                            res.map(
                                |(k, v): (DeserializeAsWrap<K, KAs>, DeserializeAsWrap<V, VAs>)| {
                                    (k.into_inner(), v.into_inner())
                                },
                            )
                        })
                        .next()
                        .transpose()
                    }
                }

                let visitor = MapVisitor::<K, KAs, V, VAs> {
                    marker: PhantomData,
                };
                deserializer.deserialize_map(visitor)
            }
        }
    };
}
#[cfg(feature = "alloc")]
tuple_seq_as_map_option_impl!(BTreeMap);
#[cfg(feature = "std")]
tuple_seq_as_map_option_impl!(HashMap);

macro_rules! tuple_seq_as_map_arr {
    ($ty:ident <KAs, VAs>) => {
        #[allow(clippy::implicit_hasher)]
        impl<'de, K, KAs, V, VAs, const N: usize> DeserializeAs<'de, [(K, V); N]> for $ty<KAs, VAs>
        where
            KAs: DeserializeAs<'de, K>,
            VAs: DeserializeAs<'de, V>,
        {
            fn deserialize_as<D>(deserializer: D) -> Result<[(K, V); N], D::Error>
            where
                D: Deserializer<'de>,
            {
                struct MapVisitor<K, KAs, V, VAs, const M: usize> {
                    marker: PhantomData<(K, KAs, V, VAs)>,
                }

                impl<'de, K, KAs, V, VAs, const M: usize> Visitor<'de> for MapVisitor<K, KAs, V, VAs, M>
                where
                    KAs: DeserializeAs<'de, K>,
                    VAs: DeserializeAs<'de, V>,
                {
                    type Value = [(K, V); M];

                    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                        formatter.write_fmt(format_args!("a map of length {}", M))
                    }

                    fn visit_map<A>(self, access: A) -> Result<Self::Value, A::Error>
                    where
                        A: MapAccess<'de>,
                    {
                        utils::array_from_iterator(utils::MapIter::new(access).map(
                            |res: Result<(DeserializeAsWrap<K, KAs>, DeserializeAsWrap<V, VAs>), A::Error>| {
                                res.map(|(k, v)| (k.into_inner(), v.into_inner()))
                            }
                        ), &self)
                    }
                }

                let visitor = MapVisitor::<K, KAs, V, VAs, N> {
                    marker: PhantomData,
                };
                deserializer.deserialize_map(visitor)
            }
        }
    }
}
tuple_seq_as_map_arr!(Map<KAs, VAs>);
#[cfg(feature = "alloc")]
tuple_seq_as_map_arr!(BTreeMap<KAs, VAs>);
#[cfg(feature = "std")]
tuple_seq_as_map_arr!(HashMap<KAs, VAs>);

// endregion
///////////////////////////////////////////////////////////////////////////////
// region: Conversion types which cause different serialization behavior

impl<'de, T: Deserialize<'de>> DeserializeAs<'de, T> for Same {
    fn deserialize_as<D>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
    {
        T::deserialize(deserializer)
    }
}

impl<'de, T> DeserializeAs<'de, T> for DisplayFromStr
where
    T: FromStr,
    T::Err: Display,
{
    fn deserialize_as<D>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Helper<S>(PhantomData<S>);
        impl<S> Visitor<'_> for Helper<S>
        where
            S: FromStr,
            <S as FromStr>::Err: Display,
        {
            type Value = S;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a string")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                value.parse::<Self::Value>().map_err(DeError::custom)
            }
        }

        deserializer.deserialize_str(Helper(PhantomData))
    }
}

impl<'de, T, H, F> DeserializeAs<'de, T> for IfIsHumanReadable<H, F>
where
    H: DeserializeAs<'de, T>,
    F: DeserializeAs<'de, T>,
{
    fn deserialize_as<D>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            H::deserialize_as(deserializer)
        } else {
            F::deserialize_as(deserializer)
        }
    }
}

impl<'de, Str> DeserializeAs<'de, Option<Str>> for NoneAsEmptyString
where
    Str: FromStr,
    Str::Err: Display,
{
    fn deserialize_as<D>(deserializer: D) -> Result<Option<Str>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct OptionStringEmptyNone<S>(PhantomData<S>);
        impl<S> Visitor<'_> for OptionStringEmptyNone<S>
        where
            S: FromStr,
            S::Err: Display,
        {
            type Value = Option<S>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a string")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                match value {
                    "" => Ok(None),
                    v => S::from_str(v).map(Some).map_err(DeError::custom),
                }
            }

            // handles the `null` case
            fn visit_unit<E>(self) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                Ok(None)
            }
        }

        deserializer.deserialize_any(OptionStringEmptyNone(PhantomData))
    }
}

#[cfg(feature = "alloc")]
impl<'de, T, TAs> DeserializeAs<'de, T> for DefaultOnError<TAs>
where
    TAs: DeserializeAs<'de, T>,
    T: Default,
{
    fn deserialize_as<D>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
    {
        let is_hr = deserializer.is_human_readable();
        let content: content::de::Content<'de> = match Deserialize::deserialize(deserializer) {
            Ok(content) => content,
            Err(_) => return Ok(Default::default()),
        };

        Ok(
            match <DeserializeAsWrap<T, TAs>>::deserialize(content::de::ContentDeserializer::<
                D::Error,
            >::new(content, is_hr))
            {
                Ok(elem) => elem.into_inner(),
                Err(_) => Default::default(),
            },
        )
    }
}

#[cfg(feature = "alloc")]
impl<'de> DeserializeAs<'de, Vec<u8>> for BytesOrString {
    fn deserialize_as<D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct BytesOrStringVisitor;
        impl<'de> Visitor<'de> for BytesOrStringVisitor {
            type Value = Vec<u8>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a list of bytes or a string")
            }

            fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E> {
                Ok(v.to_vec())
            }

            #[cfg(feature = "alloc")]
            fn visit_byte_buf<E>(self, v: Vec<u8>) -> Result<Self::Value, E> {
                Ok(v)
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E> {
                Ok(v.as_bytes().to_vec())
            }

            #[cfg(feature = "alloc")]
            fn visit_string<E>(self, v: String) -> Result<Self::Value, E> {
                Ok(v.into_bytes())
            }

            fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                utils::SeqIter::new(seq).collect()
            }
        }
        deserializer.deserialize_any(BytesOrStringVisitor)
    }
}

impl<'de, SEPARATOR, I, T> DeserializeAs<'de, I> for StringWithSeparator<SEPARATOR, T>
where
    SEPARATOR: Separator,
    I: FromIterator<T>,
    T: FromStr,
    T::Err: Display,
{
    fn deserialize_as<D>(deserializer: D) -> Result<I, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Helper<SEPARATOR, I, T>(PhantomData<(SEPARATOR, I, T)>);

        impl<SEPARATOR, I, T> Visitor<'_> for Helper<SEPARATOR, I, T>
        where
            SEPARATOR: Separator,
            I: FromIterator<T>,
            T: FromStr,
            T::Err: Display,
        {
            type Value = I;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a string")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                if value.is_empty() {
                    Ok(None.into_iter().collect())
                } else {
                    value
                        .split(SEPARATOR::separator())
                        .map(FromStr::from_str)
                        .collect::<Result<_, _>>()
                        .map_err(DeError::custom)
                }
            }
        }

        deserializer.deserialize_str(Helper::<SEPARATOR, I, T>(PhantomData))
    }
}

macro_rules! use_signed_duration {
    (
        $main_trait:ident $internal_trait:ident =>
        {
            $ty:ty; $converter:ident =>
            $({
                $format:ty, $strictness:ty =>
                $($tbound:ident: $bound:ident $(,)?)*
            })*
        }
    ) => {
        $(
            impl<'de, $($tbound,)*> DeserializeAs<'de, $ty> for $main_trait<$format, $strictness>
            where
                $($tbound: $bound,)*
            {
                fn deserialize_as<D>(deserializer: D) -> Result<$ty, D::Error>
                where
                    D: Deserializer<'de>,
                {
                    let dur: DurationSigned = $internal_trait::<$format, $strictness>::deserialize_as(deserializer)?;
                    dur.$converter::<D>()
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
        Duration; to_std_duration =>
        {u64, Strict =>}
        {FORMAT, Flexible => FORMAT: Format}
    }
);
#[cfg(feature = "alloc")]
use_signed_duration!(
    DurationSeconds DurationSeconds,
    DurationMilliSeconds DurationMilliSeconds,
    DurationMicroSeconds DurationMicroSeconds,
    DurationNanoSeconds DurationNanoSeconds,
    => {
        Duration; to_std_duration =>
        {String, Strict =>}
    }
);
#[cfg(feature = "std")]
use_signed_duration!(
    DurationSeconds DurationSeconds,
    DurationMilliSeconds DurationMilliSeconds,
    DurationMicroSeconds DurationMicroSeconds,
    DurationNanoSeconds DurationNanoSeconds,
    => {
        Duration; to_std_duration =>
        // round() only works on std
        {f64, Strict =>}
    }
);
use_signed_duration!(
    DurationSecondsWithFrac DurationSecondsWithFrac,
    DurationMilliSecondsWithFrac DurationMilliSecondsWithFrac,
    DurationMicroSecondsWithFrac DurationMicroSecondsWithFrac,
    DurationNanoSecondsWithFrac DurationNanoSecondsWithFrac,
    => {
        Duration; to_std_duration =>
        {f64, Strict =>}
        {FORMAT, Flexible => FORMAT: Format}
    }
);
#[cfg(feature = "alloc")]
use_signed_duration!(
    DurationSecondsWithFrac DurationSecondsWithFrac,
    DurationMilliSecondsWithFrac DurationMilliSecondsWithFrac,
    DurationMicroSecondsWithFrac DurationMicroSecondsWithFrac,
    DurationNanoSecondsWithFrac DurationNanoSecondsWithFrac,
    => {
        Duration; to_std_duration =>
        {String, Strict =>}
    }
);

#[cfg(feature = "std")]
use_signed_duration!(
    TimestampSeconds DurationSeconds,
    TimestampMilliSeconds DurationMilliSeconds,
    TimestampMicroSeconds DurationMicroSeconds,
    TimestampNanoSeconds DurationNanoSeconds,
    => {
        SystemTime; to_system_time =>
        {i64, Strict =>}
        {f64, Strict =>}
        {String, Strict =>}
        {FORMAT, Flexible => FORMAT: Format}
    }
);
#[cfg(feature = "std")]
use_signed_duration!(
    TimestampSecondsWithFrac DurationSecondsWithFrac,
    TimestampMilliSecondsWithFrac DurationMilliSecondsWithFrac,
    TimestampMicroSecondsWithFrac DurationMicroSecondsWithFrac,
    TimestampNanoSecondsWithFrac DurationNanoSecondsWithFrac,
    => {
        SystemTime; to_system_time =>
        {f64, Strict =>}
        {String, Strict =>}
        {FORMAT, Flexible => FORMAT: Format}
    }
);

impl<'de, T, U> DeserializeAs<'de, T> for DefaultOnNull<U>
where
    U: DeserializeAs<'de, T>,
    T: Default,
{
    fn deserialize_as<D>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Option::<U>::deserialize_as(deserializer)?.unwrap_or_default())
    }
}

impl<'de> DeserializeAs<'de, &'de [u8]> for Bytes {
    fn deserialize_as<D>(deserializer: D) -> Result<&'de [u8], D::Error>
    where
        D: Deserializer<'de>,
    {
        <&'de [u8]>::deserialize(deserializer)
    }
}

// serde_bytes implementation for ByteBuf
// https://github.com/serde-rs/bytes/blob/cbae606b9dc225fc094b031cc84eac9493da2058/src/bytebuf.rs#L196
//
// Implements:
// * visit_seq
// * visit_bytes
// * visit_byte_buf
// * visit_str
// * visit_string
#[cfg(feature = "alloc")]
impl<'de> DeserializeAs<'de, Vec<u8>> for Bytes {
    fn deserialize_as<D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct VecVisitor;

        impl<'de> Visitor<'de> for VecVisitor {
            type Value = Vec<u8>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a byte array")
            }

            fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                utils::SeqIter::new(seq).collect::<Result<_, _>>()
            }

            fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                Ok(v.to_vec())
            }

            fn visit_byte_buf<E>(self, v: Vec<u8>) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                Ok(v)
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                Ok(v.as_bytes().to_vec())
            }

            fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                Ok(v.into_bytes())
            }
        }

        deserializer.deserialize_byte_buf(VecVisitor)
    }
}

#[cfg(feature = "alloc")]
impl<'de> DeserializeAs<'de, Box<[u8]>> for Bytes {
    fn deserialize_as<D>(deserializer: D) -> Result<Box<[u8]>, D::Error>
    where
        D: Deserializer<'de>,
    {
        <Bytes as DeserializeAs<'de, Vec<u8>>>::deserialize_as(deserializer)
            .map(Vec::into_boxed_slice)
    }
}

// serde_bytes implementation for Cow<'a, [u8]>
// https://github.com/serde-rs/bytes/blob/cbae606b9dc225fc094b031cc84eac9493da2058/src/de.rs#L77
//
// Implements:
// * visit_borrowed_bytes
// * visit_borrowed_str
// * visit_bytes
// * visit_str
// * visit_byte_buf
// * visit_string
// * visit_seq
#[cfg(feature = "alloc")]
impl<'de> DeserializeAs<'de, Cow<'de, [u8]>> for Bytes {
    fn deserialize_as<D>(deserializer: D) -> Result<Cow<'de, [u8]>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct CowVisitor;

        impl<'de> Visitor<'de> for CowVisitor {
            type Value = Cow<'de, [u8]>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a byte array")
            }

            fn visit_borrowed_bytes<E>(self, v: &'de [u8]) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                Ok(Cow::Borrowed(v))
            }

            fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                Ok(Cow::Borrowed(v.as_bytes()))
            }

            fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                Ok(Cow::Owned(v.to_vec()))
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                Ok(Cow::Owned(v.as_bytes().to_vec()))
            }

            fn visit_byte_buf<E>(self, v: Vec<u8>) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                Ok(Cow::Owned(v))
            }

            fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                Ok(Cow::Owned(v.into_bytes()))
            }

            fn visit_seq<V>(self, seq: V) -> Result<Self::Value, V::Error>
            where
                V: SeqAccess<'de>,
            {
                Ok(Cow::Owned(
                    utils::SeqIter::new(seq).collect::<Result<_, _>>()?,
                ))
            }
        }

        deserializer.deserialize_bytes(CowVisitor)
    }
}

impl<'de, const N: usize> DeserializeAs<'de, [u8; N]> for Bytes {
    fn deserialize_as<D>(deserializer: D) -> Result<[u8; N], D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ArrayVisitor<const M: usize>;

        impl<'de, const M: usize> Visitor<'de> for ArrayVisitor<M> {
            type Value = [u8; M];

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_fmt(format_args!("an byte array of size {M}"))
            }

            fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                utils::array_from_iterator(utils::SeqIter::new(seq), &self)
            }

            fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                v.try_into()
                    .map_err(|_| DeError::invalid_length(v.len(), &self))
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                v.as_bytes()
                    .try_into()
                    .map_err(|_| DeError::invalid_length(v.len(), &self))
            }
        }

        deserializer.deserialize_bytes(ArrayVisitor::<N>)
    }
}

impl<'de, const N: usize> DeserializeAs<'de, &'de [u8; N]> for Bytes {
    fn deserialize_as<D>(deserializer: D) -> Result<&'de [u8; N], D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ArrayVisitor<const M: usize>;

        impl<'de, const M: usize> Visitor<'de> for ArrayVisitor<M> {
            type Value = &'de [u8; M];

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_fmt(format_args!("a borrowed byte array of size {M}"))
            }

            fn visit_borrowed_bytes<E>(self, v: &'de [u8]) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                v.try_into()
                    .map_err(|_| DeError::invalid_length(v.len(), &self))
            }

            fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                v.as_bytes()
                    .try_into()
                    .map_err(|_| DeError::invalid_length(v.len(), &self))
            }
        }

        deserializer.deserialize_bytes(ArrayVisitor::<N>)
    }
}

#[cfg(feature = "alloc")]
impl<'de, const N: usize> DeserializeAs<'de, Cow<'de, [u8; N]>> for Bytes {
    fn deserialize_as<D>(deserializer: D) -> Result<Cow<'de, [u8; N]>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct CowVisitor<const M: usize>;

        impl<'de, const M: usize> Visitor<'de> for CowVisitor<M> {
            type Value = Cow<'de, [u8; M]>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a byte array")
            }

            fn visit_borrowed_bytes<E>(self, v: &'de [u8]) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                Ok(Cow::Borrowed(
                    v.try_into()
                        .map_err(|_| DeError::invalid_length(v.len(), &self))?,
                ))
            }

            fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                Ok(Cow::Borrowed(
                    v.as_bytes()
                        .try_into()
                        .map_err(|_| DeError::invalid_length(v.len(), &self))?,
                ))
            }

            fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                Ok(Cow::Owned(
                    v.to_vec()
                        .try_into()
                        .map_err(|_| DeError::invalid_length(v.len(), &self))?,
                ))
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                Ok(Cow::Owned(
                    v.as_bytes()
                        .to_vec()
                        .try_into()
                        .map_err(|_| DeError::invalid_length(v.len(), &self))?,
                ))
            }

            fn visit_byte_buf<E>(self, v: Vec<u8>) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                let len = v.len();
                Ok(Cow::Owned(
                    v.try_into()
                        .map_err(|_| DeError::invalid_length(len, &self))?,
                ))
            }

            fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                let len = v.len();
                Ok(Cow::Owned(
                    v.into_bytes()
                        .try_into()
                        .map_err(|_| DeError::invalid_length(len, &self))?,
                ))
            }

            fn visit_seq<V>(self, seq: V) -> Result<Self::Value, V::Error>
            where
                V: SeqAccess<'de>,
            {
                Ok(Cow::Owned(utils::array_from_iterator(
                    utils::SeqIter::new(seq),
                    &self,
                )?))
            }
        }

        deserializer.deserialize_bytes(CowVisitor)
    }
}

#[cfg(feature = "alloc")]
impl<'de, const N: usize> DeserializeAs<'de, Box<[u8; N]>> for Bytes {
    fn deserialize_as<D>(deserializer: D) -> Result<Box<[u8; N]>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Bytes::deserialize_as(deserializer).map(Box::new)
    }
}

#[cfg(feature = "alloc")]
impl<'de, T, TAs, FORMAT> DeserializeAs<'de, Vec<T>> for OneOrMany<TAs, FORMAT>
where
    TAs: DeserializeAs<'de, T>,
    FORMAT: Format,
{
    fn deserialize_as<D>(deserializer: D) -> Result<Vec<T>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let is_hr = deserializer.is_human_readable();
        let content: content::de::Content<'de> = Deserialize::deserialize(deserializer)?;

        let one_err: D::Error = match <DeserializeAsWrap<T, TAs>>::deserialize(
            content::de::ContentRefDeserializer::new(&content, is_hr),
        ) {
            Ok(one) => return Ok(alloc::vec![one.into_inner()]),
            Err(err) => err,
        };
        let many_err: D::Error = match <DeserializeAsWrap<Vec<T>, Vec<TAs>>>::deserialize(
            content::de::ContentDeserializer::new(content, is_hr),
        ) {
            Ok(many) => return Ok(many.into_inner()),
            Err(err) => err,
        };
        Err(DeError::custom(format_args!(
            "OneOrMany could not deserialize any variant:\n  One: {one_err}\n  Many: {many_err}"
        )))
    }
}

#[cfg(feature = "alloc")]
impl<'de, T, TAs1> DeserializeAs<'de, T> for PickFirst<(TAs1,)>
where
    TAs1: DeserializeAs<'de, T>,
{
    fn deserialize_as<D>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(DeserializeAsWrap::<T, TAs1>::deserialize(deserializer)?.into_inner())
    }
}

#[cfg(feature = "alloc")]
impl<'de, T, TAs1, TAs2> DeserializeAs<'de, T> for PickFirst<(TAs1, TAs2)>
where
    TAs1: DeserializeAs<'de, T>,
    TAs2: DeserializeAs<'de, T>,
{
    fn deserialize_as<D>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
    {
        let is_hr = deserializer.is_human_readable();
        let content: content::de::Content<'de> = Deserialize::deserialize(deserializer)?;

        let first_err: D::Error = match <DeserializeAsWrap<T, TAs1>>::deserialize(
            content::de::ContentRefDeserializer::new(&content, is_hr),
        ) {
            Ok(first) => return Ok(first.into_inner()),
            Err(err) => err,
        };
        let second_err: D::Error = match <DeserializeAsWrap<T, TAs2>>::deserialize(
            content::de::ContentDeserializer::new(content, is_hr),
        ) {
            Ok(second) => return Ok(second.into_inner()),
            Err(err) => err,
        };
        Err(DeError::custom(format_args!(
            "PickFirst could not deserialize any variant:\n  First: {first_err}\n  Second: {second_err}"
        )))
    }
}

#[cfg(feature = "alloc")]
impl<'de, T, TAs1, TAs2, TAs3> DeserializeAs<'de, T> for PickFirst<(TAs1, TAs2, TAs3)>
where
    TAs1: DeserializeAs<'de, T>,
    TAs2: DeserializeAs<'de, T>,
    TAs3: DeserializeAs<'de, T>,
{
    fn deserialize_as<D>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
    {
        let is_hr = deserializer.is_human_readable();
        let content: content::de::Content<'de> = Deserialize::deserialize(deserializer)?;

        let first_err: D::Error = match <DeserializeAsWrap<T, TAs1>>::deserialize(
            content::de::ContentRefDeserializer::new(&content, is_hr),
        ) {
            Ok(first) => return Ok(first.into_inner()),
            Err(err) => err,
        };
        let second_err: D::Error = match <DeserializeAsWrap<T, TAs2>>::deserialize(
            content::de::ContentRefDeserializer::new(&content, is_hr),
        ) {
            Ok(second) => return Ok(second.into_inner()),
            Err(err) => err,
        };
        let third_err: D::Error = match <DeserializeAsWrap<T, TAs3>>::deserialize(
            content::de::ContentDeserializer::new(content, is_hr),
        ) {
            Ok(third) => return Ok(third.into_inner()),
            Err(err) => err,
        };
        Err(DeError::custom(format_args!(
            "PickFirst could not deserialize any variant:\n  First: {first_err}\n  Second: {second_err}\n  Third: {third_err}",
        )))
    }
}

#[cfg(feature = "alloc")]
impl<'de, T, TAs1, TAs2, TAs3, TAs4> DeserializeAs<'de, T> for PickFirst<(TAs1, TAs2, TAs3, TAs4)>
where
    TAs1: DeserializeAs<'de, T>,
    TAs2: DeserializeAs<'de, T>,
    TAs3: DeserializeAs<'de, T>,
    TAs4: DeserializeAs<'de, T>,
{
    fn deserialize_as<D>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
    {
        let is_hr = deserializer.is_human_readable();
        let content: content::de::Content<'de> = Deserialize::deserialize(deserializer)?;

        let first_err: D::Error = match <DeserializeAsWrap<T, TAs1>>::deserialize(
            content::de::ContentRefDeserializer::new(&content, is_hr),
        ) {
            Ok(first) => return Ok(first.into_inner()),
            Err(err) => err,
        };
        let second_err: D::Error = match <DeserializeAsWrap<T, TAs2>>::deserialize(
            content::de::ContentRefDeserializer::new(&content, is_hr),
        ) {
            Ok(second) => return Ok(second.into_inner()),
            Err(err) => err,
        };
        let third_err: D::Error = match <DeserializeAsWrap<T, TAs3>>::deserialize(
            content::de::ContentRefDeserializer::new(&content, is_hr),
        ) {
            Ok(third) => return Ok(third.into_inner()),
            Err(err) => err,
        };
        let fourth_err: D::Error = match <DeserializeAsWrap<T, TAs4>>::deserialize(
            content::de::ContentDeserializer::new(content, is_hr),
        ) {
            Ok(fourth) => return Ok(fourth.into_inner()),
            Err(err) => err,
        };
        Err(DeError::custom(format_args!(
            "PickFirst could not deserialize any variant:\n  First: {first_err}\n  Second: {second_err}\n  Third: {third_err}\n  Fourth: {fourth_err}",
        )))
    }
}

impl<'de, T, U> DeserializeAs<'de, T> for FromInto<U>
where
    U: Into<T>,
    U: Deserialize<'de>,
{
    fn deserialize_as<D>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(U::deserialize(deserializer)?.into())
    }
}

impl<'de, T, U> DeserializeAs<'de, T> for TryFromInto<U>
where
    U: TryInto<T>,
    <U as TryInto<T>>::Error: Display,
    U: Deserialize<'de>,
{
    fn deserialize_as<D>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
    {
        U::deserialize(deserializer)?
            .try_into()
            .map_err(DeError::custom)
    }
}

impl<'de, T, U> DeserializeAs<'de, T> for FromIntoRef<U>
where
    U: Into<T>,
    U: Deserialize<'de>,
{
    fn deserialize_as<D>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(U::deserialize(deserializer)?.into())
    }
}

impl<'de, T, U> DeserializeAs<'de, T> for TryFromIntoRef<U>
where
    U: TryInto<T>,
    <U as TryInto<T>>::Error: Display,
    U: Deserialize<'de>,
{
    fn deserialize_as<D>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
    {
        U::deserialize(deserializer)?
            .try_into()
            .map_err(DeError::custom)
    }
}

#[cfg(feature = "alloc")]
impl<'de> DeserializeAs<'de, Cow<'de, str>> for BorrowCow {
    fn deserialize_as<D>(deserializer: D) -> Result<Cow<'de, str>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct CowVisitor;

        impl<'de> Visitor<'de> for CowVisitor {
            type Value = Cow<'de, str>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an optionally borrowed string")
            }

            fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                Ok(Cow::Borrowed(v))
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                Ok(Cow::Owned(v.to_owned()))
            }

            fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                Ok(Cow::Owned(v))
            }
        }

        deserializer.deserialize_string(CowVisitor)
    }
}

#[cfg(feature = "alloc")]
impl<'de> DeserializeAs<'de, Cow<'de, [u8]>> for BorrowCow {
    fn deserialize_as<D>(deserializer: D) -> Result<Cow<'de, [u8]>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Bytes::deserialize_as(deserializer)
    }
}

#[cfg(feature = "alloc")]
impl<'de, const N: usize> DeserializeAs<'de, Cow<'de, [u8; N]>> for BorrowCow {
    fn deserialize_as<D>(deserializer: D) -> Result<Cow<'de, [u8; N]>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Bytes::deserialize_as(deserializer)
    }
}

impl<'de> DeserializeAs<'de, bool> for BoolFromInt<Strict> {
    fn deserialize_as<D>(deserializer: D) -> Result<bool, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct U8Visitor;
        impl Visitor<'_> for U8Visitor {
            type Value = bool;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an integer 0 or 1")
            }

            fn visit_u8<E>(self, v: u8) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                match v {
                    0 => Ok(false),
                    1 => Ok(true),
                    unexp => Err(DeError::invalid_value(
                        Unexpected::Unsigned(u64::from(unexp)),
                        &"0 or 1",
                    )),
                }
            }

            fn visit_i8<E>(self, v: i8) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                match v {
                    0 => Ok(false),
                    1 => Ok(true),
                    unexp => Err(DeError::invalid_value(
                        Unexpected::Signed(i64::from(unexp)),
                        &"0 or 1",
                    )),
                }
            }

            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                match v {
                    0 => Ok(false),
                    1 => Ok(true),
                    unexp => Err(DeError::invalid_value(
                        Unexpected::Unsigned(unexp),
                        &"0 or 1",
                    )),
                }
            }

            fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                match v {
                    0 => Ok(false),
                    1 => Ok(true),
                    unexp => Err(DeError::invalid_value(Unexpected::Signed(unexp), &"0 or 1")),
                }
            }

            fn visit_u128<E>(self, v: u128) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                match v {
                    0 => Ok(false),
                    1 => Ok(true),
                    unexp => {
                        let mut buf: [u8; 58] = [0u8; 58];
                        Err(DeError::invalid_value(
                            crate::utils::get_unexpected_u128(unexp, &mut buf),
                            &self,
                        ))
                    }
                }
            }

            fn visit_i128<E>(self, v: i128) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                match v {
                    0 => Ok(false),
                    1 => Ok(true),
                    unexp => {
                        let mut buf: [u8; 58] = [0u8; 58];
                        Err(DeError::invalid_value(
                            crate::utils::get_unexpected_i128(unexp, &mut buf),
                            &"0 or 1",
                        ))
                    }
                }
            }
        }

        deserializer.deserialize_u8(U8Visitor)
    }
}

impl<'de> DeserializeAs<'de, bool> for BoolFromInt<Flexible> {
    fn deserialize_as<D>(deserializer: D) -> Result<bool, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct U8Visitor;
        impl Visitor<'_> for U8Visitor {
            type Value = bool;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an integer")
            }

            fn visit_u8<E>(self, v: u8) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                Ok(v != 0)
            }

            fn visit_i8<E>(self, v: i8) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                Ok(v != 0)
            }

            fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                Ok(v != 0)
            }

            fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                Ok(v != 0)
            }

            fn visit_u128<E>(self, v: u128) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                Ok(v != 0)
            }

            fn visit_i128<E>(self, v: i128) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                Ok(v != 0)
            }
        }

        deserializer.deserialize_u8(U8Visitor)
    }
}

// endregion
