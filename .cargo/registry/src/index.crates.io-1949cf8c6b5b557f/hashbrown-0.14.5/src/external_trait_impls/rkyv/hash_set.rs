use crate::HashSet;
use core::{
    borrow::Borrow,
    hash::{BuildHasher, Hash},
};
use rkyv::{
    collections::hash_set::{ArchivedHashSet, HashSetResolver},
    ser::{ScratchSpace, Serializer},
    Archive, Deserialize, Fallible, Serialize,
};

impl<K: Archive + Hash + Eq, S> Archive for HashSet<K, S>
where
    K::Archived: Hash + Eq,
{
    type Archived = ArchivedHashSet<K::Archived>;
    type Resolver = HashSetResolver;

    #[inline]
    unsafe fn resolve(&self, pos: usize, resolver: Self::Resolver, out: *mut Self::Archived) {
        ArchivedHashSet::<K::Archived>::resolve_from_len(self.len(), pos, resolver, out);
    }
}

impl<K, S, RS> Serialize<S> for HashSet<K, RS>
where
    K::Archived: Hash + Eq,
    K: Serialize<S> + Hash + Eq,
    S: ScratchSpace + Serializer + ?Sized,
{
    #[inline]
    fn serialize(&self, serializer: &mut S) -> Result<Self::Resolver, S::Error> {
        unsafe { ArchivedHashSet::serialize_from_iter(self.iter(), serializer) }
    }
}

impl<K, D, S> Deserialize<HashSet<K, S>, D> for ArchivedHashSet<K::Archived>
where
    K: Archive + Hash + Eq,
    K::Archived: Deserialize<K, D> + Hash + Eq,
    D: Fallible + ?Sized,
    S: Default + BuildHasher,
{
    #[inline]
    fn deserialize(&self, deserializer: &mut D) -> Result<HashSet<K, S>, D::Error> {
        let mut result = HashSet::with_hasher(S::default());
        for k in self.iter() {
            result.insert(k.deserialize(deserializer)?);
        }
        Ok(result)
    }
}

impl<K: Hash + Eq + Borrow<AK>, AK: Hash + Eq, S: BuildHasher> PartialEq<HashSet<K, S>>
    for ArchivedHashSet<AK>
{
    #[inline]
    fn eq(&self, other: &HashSet<K, S>) -> bool {
        if self.len() != other.len() {
            false
        } else {
            self.iter().all(|key| other.get(key).is_some())
        }
    }
}

impl<K: Hash + Eq + Borrow<AK>, AK: Hash + Eq, S: BuildHasher> PartialEq<ArchivedHashSet<AK>>
    for HashSet<K, S>
{
    #[inline]
    fn eq(&self, other: &ArchivedHashSet<AK>) -> bool {
        other.eq(self)
    }
}

#[cfg(test)]
mod tests {
    use crate::HashSet;
    use alloc::string::String;
    use rkyv::{
        archived_root, check_archived_root,
        ser::{serializers::AllocSerializer, Serializer},
        Deserialize, Infallible,
    };

    #[test]
    fn index_set() {
        let mut value = HashSet::new();
        value.insert(String::from("foo"));
        value.insert(String::from("bar"));
        value.insert(String::from("baz"));
        value.insert(String::from("bat"));

        let mut serializer = AllocSerializer::<4096>::default();
        serializer.serialize_value(&value).unwrap();
        let result = serializer.into_serializer().into_inner();
        let archived = unsafe { archived_root::<HashSet<String>>(result.as_ref()) };

        assert_eq!(value.len(), archived.len());
        for k in value.iter() {
            let ak = archived.get(k.as_str()).unwrap();
            assert_eq!(k, ak);
        }

        let deserialized: HashSet<String> = archived.deserialize(&mut Infallible).unwrap();
        assert_eq!(value, deserialized);
    }

    #[test]
    fn validate_index_set() {
        let mut value = HashSet::new();
        value.insert(String::from("foo"));
        value.insert(String::from("bar"));
        value.insert(String::from("baz"));
        value.insert(String::from("bat"));

        let mut serializer = AllocSerializer::<4096>::default();
        serializer.serialize_value(&value).unwrap();
        let result = serializer.into_serializer().into_inner();
        check_archived_root::<HashSet<String>>(result.as_ref())
            .expect("failed to validate archived index set");
    }
}
