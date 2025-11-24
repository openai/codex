use std::borrow::Borrow;
use std::collections::hash_map::{IntoKeys, IntoValues};
use std::collections::{hash_map, HashMap};
use std::fmt::{self, Debug};
use std::hash::{BuildHasher, Hash};
use std::iter::FromIterator;
use std::ops::{Deref, DerefMut, Index};
use std::panic::UnwindSafe;

#[cfg(feature = "serde")]
use serde::{
    de::{Deserialize, Deserializer},
    ser::{Serialize, Serializer},
};

use crate::RandomState;

/// A [`HashMap`](std::collections::HashMap) using [`RandomState`](crate::RandomState) to hash the items.
/// (Requires the `std` feature to be enabled.)
#[derive(Clone)]
pub struct AHashMap<K, V, S = crate::RandomState>(HashMap<K, V, S>);

impl<K, V> From<HashMap<K, V, crate::RandomState>> for AHashMap<K, V> {
    fn from(item: HashMap<K, V, crate::RandomState>) -> Self {
        AHashMap(item)
    }
}

impl<K, V, const N: usize> From<[(K, V); N]> for AHashMap<K, V>
where
    K: Eq + Hash,
{
    /// # Examples
    ///
    /// ```
    /// use ahash::AHashMap;
    ///
    /// let map1 = AHashMap::from([(1, 2), (3, 4)]);
    /// let map2: AHashMap<_, _> = [(1, 2), (3, 4)].into();
    /// assert_eq!(map1, map2);
    /// ```
    fn from(arr: [(K, V); N]) -> Self {
        Self::from_iter(arr)
    }
}

impl<K, V> Into<HashMap<K, V, crate::RandomState>> for AHashMap<K, V> {
    fn into(self) -> HashMap<K, V, crate::RandomState> {
        self.0
    }
}

impl<K, V> AHashMap<K, V, RandomState> {
    /// This creates a hashmap using [RandomState::new] which obtains its keys from [RandomSource].
    /// See the documentation in [RandomSource] for notes about key strength.
    pub fn new() -> Self {
        AHashMap(HashMap::with_hasher(RandomState::new()))
    }

    /// This creates a hashmap with the specified capacity using [RandomState::new].
    /// See the documentation in [RandomSource] for notes about key strength.
    pub fn with_capacity(capacity: usize) -> Self {
        AHashMap(HashMap::with_capacity_and_hasher(capacity, RandomState::new()))
    }
}

impl<K, V, S> AHashMap<K, V, S>
where
    S: BuildHasher,
{
    pub fn with_hasher(hash_builder: S) -> Self {
        AHashMap(HashMap::with_hasher(hash_builder))
    }

    pub fn with_capacity_and_hasher(capacity: usize, hash_builder: S) -> Self {
        AHashMap(HashMap::with_capacity_and_hasher(capacity, hash_builder))
    }
}

impl<K, V, S> AHashMap<K, V, S>
where
    K: Hash + Eq,
    S: BuildHasher,
{
    /// Returns a reference to the value corresponding to the key.
    ///
    /// The key may be any borrowed form of the map's key type, but
    /// [`Hash`] and [`Eq`] on the borrowed form *must* match those for
    /// the key type.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::collections::HashMap;
    ///
    /// let mut map = HashMap::new();
    /// map.insert(1, "a");
    /// assert_eq!(map.get(&1), Some(&"a"));
    /// assert_eq!(map.get(&2), None);
    /// ```
    #[inline]
    pub fn get<Q: ?Sized>(&self, k: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.0.get(k)
    }

    /// Returns the key-value pair corresponding to the supplied key.
    ///
    /// The supplied key may be any borrowed form of the map's key type, but
    /// [`Hash`] and [`Eq`] on the borrowed form *must* match those for
    /// the key type.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::collections::HashMap;
    ///
    /// let mut map = HashMap::new();
    /// map.insert(1, "a");
    /// assert_eq!(map.get_key_value(&1), Some((&1, &"a")));
    /// assert_eq!(map.get_key_value(&2), None);
    /// ```
    #[inline]
    pub fn get_key_value<Q: ?Sized>(&self, k: &Q) -> Option<(&K, &V)>
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.0.get_key_value(k)
    }

    /// Returns a mutable reference to the value corresponding to the key.
    ///
    /// The key may be any borrowed form of the map's key type, but
    /// [`Hash`] and [`Eq`] on the borrowed form *must* match those for
    /// the key type.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::collections::HashMap;
    ///
    /// let mut map = HashMap::new();
    /// map.insert(1, "a");
    /// if let Some(x) = map.get_mut(&1) {
    ///     *x = "b";
    /// }
    /// assert_eq!(map[&1], "b");
    /// ```
    #[inline]
    pub fn get_mut<Q: ?Sized>(&mut self, k: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.0.get_mut(k)
    }

    /// Inserts a key-value pair into the map.
    ///
    /// If the map did not have this key present, [`None`] is returned.
    ///
    /// If the map did have this key present, the value is updated, and the old
    /// value is returned. The key is not updated, though; this matters for
    /// types that can be `==` without being identical. See the [module-level
    /// documentation] for more.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::collections::HashMap;
    ///
    /// let mut map = HashMap::new();
    /// assert_eq!(map.insert(37, "a"), None);
    /// assert_eq!(map.is_empty(), false);
    ///
    /// map.insert(37, "b");
    /// assert_eq!(map.insert(37, "c"), Some("b"));
    /// assert_eq!(map[&37], "c");
    /// ```
    #[inline]
    pub fn insert(&mut self, k: K, v: V) -> Option<V> {
        self.0.insert(k, v)
    }

    /// Creates a consuming iterator visiting all the keys in arbitrary order.
    /// The map cannot be used after calling this.
    /// The iterator element type is `K`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::collections::HashMap;
    ///
    /// let map = HashMap::from([
    ///     ("a", 1),
    ///     ("b", 2),
    ///     ("c", 3),
    /// ]);
    ///
    /// let mut vec: Vec<&str> = map.into_keys().collect();
    /// // The `IntoKeys` iterator produces keys in arbitrary order, so the
    /// // keys must be sorted to test them against a sorted array.
    /// vec.sort_unstable();
    /// assert_eq!(vec, ["a", "b", "c"]);
    /// ```
    ///
    /// # Performance
    ///
    /// In the current implementation, iterating over keys takes O(capacity) time
    /// instead of O(len) because it internally visits empty buckets too.
    #[inline]
    pub fn into_keys(self) -> IntoKeys<K, V> {
        self.0.into_keys()
    }

    /// Creates a consuming iterator visiting all the values in arbitrary order.
    /// The map cannot be used after calling this.
    /// The iterator element type is `V`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::collections::HashMap;
    ///
    /// let map = HashMap::from([
    ///     ("a", 1),
    ///     ("b", 2),
    ///     ("c", 3),
    /// ]);
    ///
    /// let mut vec: Vec<i32> = map.into_values().collect();
    /// // The `IntoValues` iterator produces values in arbitrary order, so
    /// // the values must be sorted to test them against a sorted array.
    /// vec.sort_unstable();
    /// assert_eq!(vec, [1, 2, 3]);
    /// ```
    ///
    /// # Performance
    ///
    /// In the current implementation, iterating over values takes O(capacity) time
    /// instead of O(len) because it internally visits empty buckets too.
    #[inline]
    pub fn into_values(self) -> IntoValues<K, V> {
        self.0.into_values()
    }

    /// Removes a key from the map, returning the value at the key if the key
    /// was previously in the map.
    ///
    /// The key may be any borrowed form of the map's key type, but
    /// [`Hash`] and [`Eq`] on the borrowed form *must* match those for
    /// the key type.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::collections::HashMap;
    ///
    /// let mut map = HashMap::new();
    /// map.insert(1, "a");
    /// assert_eq!(map.remove(&1), Some("a"));
    /// assert_eq!(map.remove(&1), None);
    /// ```
    #[inline]
    pub fn remove<Q: ?Sized>(&mut self, k: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.0.remove(k)
    }
}

impl<K, V, S> Deref for AHashMap<K, V, S> {
    type Target = HashMap<K, V, S>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<K, V, S> DerefMut for AHashMap<K, V, S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<K, V, S> UnwindSafe for AHashMap<K, V, S>
where
    K: UnwindSafe,
    V: UnwindSafe,
{
}

impl<K, V, S> PartialEq for AHashMap<K, V, S>
where
    K: Eq + Hash,
    V: PartialEq,
    S: BuildHasher,
{
    fn eq(&self, other: &AHashMap<K, V, S>) -> bool {
        self.0.eq(&other.0)
    }
}

impl<K, V, S> Eq for AHashMap<K, V, S>
where
    K: Eq + Hash,
    V: Eq,
    S: BuildHasher,
{
}

impl<K, Q: ?Sized, V, S> Index<&Q> for AHashMap<K, V, S>
where
    K: Eq + Hash + Borrow<Q>,
    Q: Eq + Hash,
    S: BuildHasher,
{
    type Output = V;

    /// Returns a reference to the value corresponding to the supplied key.
    ///
    /// # Panics
    ///
    /// Panics if the key is not present in the `HashMap`.
    #[inline]
    fn index(&self, key: &Q) -> &V {
        self.0.index(key)
    }
}

impl<K, V, S> Debug for AHashMap<K, V, S>
where
    K: Debug,
    V: Debug,
    S: BuildHasher,
{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(fmt)
    }
}

impl<K, V> FromIterator<(K, V)> for AHashMap<K, V, RandomState>
where
    K: Eq + Hash,
{
    /// This creates a hashmap from the provided iterator using [RandomState::new].
    /// See the documentation in [RandomSource] for notes about key strength.
    fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> Self {
        let mut inner = HashMap::with_hasher(RandomState::new());
        inner.extend(iter);
        AHashMap(inner)
    }
}

impl<'a, K, V, S> IntoIterator for &'a AHashMap<K, V, S> {
    type Item = (&'a K, &'a V);
    type IntoIter = hash_map::Iter<'a, K, V>;
    fn into_iter(self) -> Self::IntoIter {
        (&self.0).iter()
    }
}

impl<'a, K, V, S> IntoIterator for &'a mut AHashMap<K, V, S> {
    type Item = (&'a K, &'a mut V);
    type IntoIter = hash_map::IterMut<'a, K, V>;
    fn into_iter(self) -> Self::IntoIter {
        (&mut self.0).iter_mut()
    }
}

impl<K, V, S> IntoIterator for AHashMap<K, V, S> {
    type Item = (K, V);
    type IntoIter = hash_map::IntoIter<K, V>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<K, V, S> Extend<(K, V)> for AHashMap<K, V, S>
where
    K: Eq + Hash,
    S: BuildHasher,
{
    #[inline]
    fn extend<T: IntoIterator<Item = (K, V)>>(&mut self, iter: T) {
        self.0.extend(iter)
    }
}

impl<'a, K, V, S> Extend<(&'a K, &'a V)> for AHashMap<K, V, S>
where
    K: Eq + Hash + Copy + 'a,
    V: Copy + 'a,
    S: BuildHasher,
{
    #[inline]
    fn extend<T: IntoIterator<Item = (&'a K, &'a V)>>(&mut self, iter: T) {
        self.0.extend(iter)
    }
}

/// NOTE: For safety this trait impl is only available if either of the flags `runtime-rng` (on by default) or
/// `compile-time-rng` are enabled. This is to prevent weakly keyed maps from being accidentally created. Instead one of
/// constructors for [RandomState] must be used.
#[cfg(any(feature = "compile-time-rng", feature = "runtime-rng", feature = "no-rng"))]
impl<K, V> Default for AHashMap<K, V, RandomState> {
    #[inline]
    fn default() -> AHashMap<K, V, RandomState> {
        AHashMap(HashMap::default())
    }
}

#[cfg(feature = "serde")]
impl<K, V> Serialize for AHashMap<K, V>
where
    K: Serialize + Eq + Hash,
    V: Serialize,
{
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.deref().serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de, K, V> Deserialize<'de> for AHashMap<K, V>
where
    K: Deserialize<'de> + Eq + Hash,
    V: Deserialize<'de>,
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let hash_map = HashMap::deserialize(deserializer);
        hash_map.map(|hash_map| Self(hash_map))
    }

    fn deserialize_in_place<D: Deserializer<'de>>(deserializer: D, place: &mut Self) -> Result<(), D::Error> {
        use serde::de::{MapAccess, Visitor};

        struct MapInPlaceVisitor<'a, K: 'a, V: 'a>(&'a mut AHashMap<K, V>);

        impl<'a, 'de, K, V> Visitor<'de> for MapInPlaceVisitor<'a, K, V>
        where
            K: Deserialize<'de> + Eq + Hash,
            V: Deserialize<'de>,
        {
            type Value = ();

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a map")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                self.0.clear();
                self.0.reserve(map.size_hint().unwrap_or(0).min(4096));

                while let Some((key, value)) = map.next_entry()? {
                    self.0.insert(key, value);
                }

                Ok(())
            }
        }

        deserializer.deserialize_map(MapInPlaceVisitor(place))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_borrow() {
        let mut map: AHashMap<String, String> = AHashMap::new();
        map.insert("foo".to_string(), "Bar".to_string());
        map.insert("Bar".to_string(), map.get("foo").unwrap().to_owned());
    }

    #[cfg(feature = "serde")]
    #[test]
    fn test_serde() {
        let mut map = AHashMap::new();
        map.insert("for".to_string(), 0);
        map.insert("bar".to_string(), 1);
        let mut serialization = serde_json::to_string(&map).unwrap();
        let mut deserialization: AHashMap<String, u64> = serde_json::from_str(&serialization).unwrap();
        assert_eq!(deserialization, map);

        map.insert("baz".to_string(), 2);
        serialization = serde_json::to_string(&map).unwrap();
        let mut deserializer = serde_json::Deserializer::from_str(&serialization);
        AHashMap::deserialize_in_place(&mut deserializer, &mut deserialization).unwrap();
        assert_eq!(deserialization, map);
    }
}
