#![cfg_attr(docsrs, doc(cfg(feature = "borsh")))]

use alloc::vec::Vec;
use core::hash::BuildHasher;
use core::hash::Hash;

use borsh::error::ERROR_ZST_FORBIDDEN;
use borsh::io::{Error, ErrorKind, Read, Result, Write};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::map::IndexMap;
use crate::set::IndexSet;

// NOTE: the real `#[deprecated]` attribute doesn't work for trait implementations,
// but we can get close by mimicking the message style for documentation.
/// <div class="stab deprecated"><span class="emoji">👎</span><span>Deprecated: use borsh's <code>indexmap</code> feature instead.</span></div>
impl<K, V, S> BorshSerialize for IndexMap<K, V, S>
where
    K: BorshSerialize,
    V: BorshSerialize,
{
    #[inline]
    fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
        check_zst::<K>()?;

        let iterator = self.iter();

        u32::try_from(iterator.len())
            .map_err(|_| ErrorKind::InvalidData)?
            .serialize(writer)?;

        for (key, value) in iterator {
            key.serialize(writer)?;
            value.serialize(writer)?;
        }

        Ok(())
    }
}

/// <div class="stab deprecated"><span class="emoji">👎</span><span>Deprecated: use borsh's <code>indexmap</code> feature instead.</span></div>
impl<K, V, S> BorshDeserialize for IndexMap<K, V, S>
where
    K: BorshDeserialize + Eq + Hash,
    V: BorshDeserialize,
    S: BuildHasher + Default,
{
    #[inline]
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        check_zst::<K>()?;
        let vec = <Vec<(K, V)>>::deserialize_reader(reader)?;
        Ok(vec.into_iter().collect::<IndexMap<K, V, S>>())
    }
}

/// <div class="stab deprecated"><span class="emoji">👎</span><span>Deprecated: use borsh's <code>indexmap</code> feature instead.</span></div>
impl<T, S> BorshSerialize for IndexSet<T, S>
where
    T: BorshSerialize,
{
    #[inline]
    fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
        check_zst::<T>()?;

        let iterator = self.iter();

        u32::try_from(iterator.len())
            .map_err(|_| ErrorKind::InvalidData)?
            .serialize(writer)?;

        for item in iterator {
            item.serialize(writer)?;
        }

        Ok(())
    }
}

/// <div class="stab deprecated"><span class="emoji">👎</span><span>Deprecated: use borsh's <code>indexmap</code> feature instead.</span></div>
impl<T, S> BorshDeserialize for IndexSet<T, S>
where
    T: BorshDeserialize + Eq + Hash,
    S: BuildHasher + Default,
{
    #[inline]
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        check_zst::<T>()?;
        let vec = <Vec<T>>::deserialize_reader(reader)?;
        Ok(vec.into_iter().collect::<IndexSet<T, S>>())
    }
}

fn check_zst<T>() -> Result<()> {
    if size_of::<T>() == 0 {
        return Err(Error::new(ErrorKind::InvalidData, ERROR_ZST_FORBIDDEN));
    }
    Ok(())
}

#[cfg(test)]
mod borsh_tests {
    use super::*;

    #[test]
    fn map_borsh_roundtrip() {
        let original_map: IndexMap<i32, i32> = {
            let mut map = IndexMap::new();
            map.insert(1, 2);
            map.insert(3, 4);
            map.insert(5, 6);
            map
        };
        let serialized_map = borsh::to_vec(&original_map).unwrap();
        let deserialized_map: IndexMap<i32, i32> =
            BorshDeserialize::try_from_slice(&serialized_map).unwrap();
        assert_eq!(original_map, deserialized_map);
    }

    #[test]
    fn set_borsh_roundtrip() {
        let original_map: IndexSet<i32> = [1, 2, 3, 4, 5, 6].into_iter().collect();
        let serialized_map = borsh::to_vec(&original_map).unwrap();
        let deserialized_map: IndexSet<i32> =
            BorshDeserialize::try_from_slice(&serialized_map).unwrap();
        assert_eq!(original_map, deserialized_map);
    }
}
