#[cfg(feature = "arbitrary")]
#[cfg_attr(docsrs, doc(cfg(feature = "arbitrary")))]
mod impl_arbitrary {
    use crate::{IndexMap, IndexSet};
    use arbitrary::{Arbitrary, Result, Unstructured};
    use core::hash::{BuildHasher, Hash};

    impl<'a, K, V, S> Arbitrary<'a> for IndexMap<K, V, S>
    where
        K: Arbitrary<'a> + Hash + Eq,
        V: Arbitrary<'a>,
        S: BuildHasher + Default,
    {
        fn arbitrary(u: &mut Unstructured<'a>) -> Result<Self> {
            u.arbitrary_iter()?.collect()
        }

        fn arbitrary_take_rest(u: Unstructured<'a>) -> Result<Self> {
            u.arbitrary_take_rest_iter()?.collect()
        }
    }

    impl<'a, T, S> Arbitrary<'a> for IndexSet<T, S>
    where
        T: Arbitrary<'a> + Hash + Eq,
        S: BuildHasher + Default,
    {
        fn arbitrary(u: &mut Unstructured<'a>) -> Result<Self> {
            u.arbitrary_iter()?.collect()
        }

        fn arbitrary_take_rest(u: Unstructured<'a>) -> Result<Self> {
            u.arbitrary_take_rest_iter()?.collect()
        }
    }
}

#[cfg(feature = "quickcheck")]
#[cfg_attr(docsrs, doc(cfg(feature = "quickcheck")))]
mod impl_quickcheck {
    use crate::{IndexMap, IndexSet};
    use alloc::boxed::Box;
    use alloc::vec::Vec;
    use core::hash::{BuildHasher, Hash};
    use quickcheck::{Arbitrary, Gen};

    impl<K, V, S> Arbitrary for IndexMap<K, V, S>
    where
        K: Arbitrary + Hash + Eq,
        V: Arbitrary,
        S: BuildHasher + Default + Clone + 'static,
    {
        fn arbitrary(g: &mut Gen) -> Self {
            Self::from_iter(Vec::arbitrary(g))
        }

        fn shrink(&self) -> Box<dyn Iterator<Item = Self>> {
            let vec = Vec::from_iter(self.clone());
            Box::new(vec.shrink().map(Self::from_iter))
        }
    }

    impl<T, S> Arbitrary for IndexSet<T, S>
    where
        T: Arbitrary + Hash + Eq,
        S: BuildHasher + Default + Clone + 'static,
    {
        fn arbitrary(g: &mut Gen) -> Self {
            Self::from_iter(Vec::arbitrary(g))
        }

        fn shrink(&self) -> Box<dyn Iterator<Item = Self>> {
            let vec = Vec::from_iter(self.clone());
            Box::new(vec.shrink().map(Self::from_iter))
        }
    }
}
