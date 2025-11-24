mod size_hint {
    use core::cmp;

    /// This presumably exists to prevent denial of service attacks.
    ///
    /// Original discussion: https://github.com/serde-rs/serde/issues/1114.
    #[cfg_attr(feature = "inline-more", inline)]
    pub(super) fn cautious(hint: Option<usize>) -> usize {
        cmp::min(hint.unwrap_or(0), 4096)
    }
}

mod map {
    use core::fmt;
    use core::hash::{BuildHasher, Hash};
    use core::marker::PhantomData;
    use serde::de::{Deserialize, Deserializer, MapAccess, Visitor};
    use serde::ser::{Serialize, Serializer};

    use crate::hash_map::HashMap;

    use super::size_hint;

    impl<K, V, H> Serialize for HashMap<K, V, H>
    where
        K: Serialize + Eq + Hash,
        V: Serialize,
        H: BuildHasher,
    {
        #[cfg_attr(feature = "inline-more", inline)]
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            serializer.collect_map(self)
        }
    }

    impl<'de, K, V, S> Deserialize<'de> for HashMap<K, V, S>
    where
        K: Deserialize<'de> + Eq + Hash,
        V: Deserialize<'de>,
        S: BuildHasher + Default,
    {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            struct MapVisitor<K, V, S> {
                marker: PhantomData<HashMap<K, V, S>>,
            }

            impl<'de, K, V, S> Visitor<'de> for MapVisitor<K, V, S>
            where
                K: Deserialize<'de> + Eq + Hash,
                V: Deserialize<'de>,
                S: BuildHasher + Default,
            {
                type Value = HashMap<K, V, S>;

                fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                    formatter.write_str("a map")
                }

                #[cfg_attr(feature = "inline-more", inline)]
                fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
                where
                    A: MapAccess<'de>,
                {
                    let mut values = HashMap::with_capacity_and_hasher(
                        size_hint::cautious(map.size_hint()),
                        S::default(),
                    );

                    while let Some((key, value)) = map.next_entry()? {
                        values.insert(key, value);
                    }

                    Ok(values)
                }
            }

            let visitor = MapVisitor {
                marker: PhantomData,
            };
            deserializer.deserialize_map(visitor)
        }
    }
}

mod set {
    use core::fmt;
    use core::hash::{BuildHasher, Hash};
    use core::marker::PhantomData;
    use serde::de::{Deserialize, Deserializer, SeqAccess, Visitor};
    use serde::ser::{Serialize, Serializer};

    use crate::hash_set::HashSet;

    use super::size_hint;

    impl<T, H> Serialize for HashSet<T, H>
    where
        T: Serialize + Eq + Hash,
        H: BuildHasher,
    {
        #[cfg_attr(feature = "inline-more", inline)]
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            serializer.collect_seq(self)
        }
    }

    impl<'de, T, S> Deserialize<'de> for HashSet<T, S>
    where
        T: Deserialize<'de> + Eq + Hash,
        S: BuildHasher + Default,
    {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            struct SeqVisitor<T, S> {
                marker: PhantomData<HashSet<T, S>>,
            }

            impl<'de, T, S> Visitor<'de> for SeqVisitor<T, S>
            where
                T: Deserialize<'de> + Eq + Hash,
                S: BuildHasher + Default,
            {
                type Value = HashSet<T, S>;

                fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                    formatter.write_str("a sequence")
                }

                #[cfg_attr(feature = "inline-more", inline)]
                fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
                where
                    A: SeqAccess<'de>,
                {
                    let mut values = HashSet::with_capacity_and_hasher(
                        size_hint::cautious(seq.size_hint()),
                        S::default(),
                    );

                    while let Some(value) = seq.next_element()? {
                        values.insert(value);
                    }

                    Ok(values)
                }
            }

            let visitor = SeqVisitor {
                marker: PhantomData,
            };
            deserializer.deserialize_seq(visitor)
        }

        #[allow(clippy::missing_errors_doc)]
        fn deserialize_in_place<D>(deserializer: D, place: &mut Self) -> Result<(), D::Error>
        where
            D: Deserializer<'de>,
        {
            struct SeqInPlaceVisitor<'a, T, S>(&'a mut HashSet<T, S>);

            impl<'a, 'de, T, S> Visitor<'de> for SeqInPlaceVisitor<'a, T, S>
            where
                T: Deserialize<'de> + Eq + Hash,
                S: BuildHasher + Default,
            {
                type Value = ();

                fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                    formatter.write_str("a sequence")
                }

                #[cfg_attr(feature = "inline-more", inline)]
                fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
                where
                    A: SeqAccess<'de>,
                {
                    self.0.clear();
                    self.0.reserve(size_hint::cautious(seq.size_hint()));

                    while let Some(value) = seq.next_element()? {
                        self.0.insert(value);
                    }

                    Ok(())
                }
            }

            deserializer.deserialize_seq(SeqInPlaceVisitor(place))
        }
    }
}
