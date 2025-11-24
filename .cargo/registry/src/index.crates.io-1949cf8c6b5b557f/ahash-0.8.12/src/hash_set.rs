use crate::RandomState;
use std::collections::{hash_set, HashSet};
use std::fmt::{self, Debug};
use std::hash::{BuildHasher, Hash};
use std::iter::FromIterator;
use std::ops::{BitAnd, BitOr, BitXor, Deref, DerefMut, Sub};

#[cfg(feature = "serde")]
use serde::{
    de::{Deserialize, Deserializer},
    ser::{Serialize, Serializer},
};

/// A [`HashSet`](std::collections::HashSet) using [`RandomState`](crate::RandomState) to hash the items.
/// (Requires the `std` feature to be enabled.)
#[derive(Clone)]
pub struct AHashSet<T, S = RandomState>(HashSet<T, S>);

impl<T> From<HashSet<T, RandomState>> for AHashSet<T> {
    fn from(item: HashSet<T, RandomState>) -> Self {
        AHashSet(item)
    }
}

impl<T, const N: usize> From<[T; N]> for AHashSet<T>
where
    T: Eq + Hash,
{
    /// # Examples
    ///
    /// ```
    /// use ahash::AHashSet;
    ///
    /// let set1 = AHashSet::from([1, 2, 3, 4]);
    /// let set2: AHashSet<_> = [1, 2, 3, 4].into();
    /// assert_eq!(set1, set2);
    /// ```
    fn from(arr: [T; N]) -> Self {
        Self::from_iter(arr)
    }
}

impl<T> Into<HashSet<T, RandomState>> for AHashSet<T> {
    fn into(self) -> HashSet<T, RandomState> {
        self.0
    }
}

impl<T> AHashSet<T, RandomState> {
    /// This creates a hashset using [RandomState::new].
    /// See the documentation in [RandomSource] for notes about key strength.
    pub fn new() -> Self {
        AHashSet(HashSet::with_hasher(RandomState::new()))
    }

    /// This craetes a hashset with the specified capacity using [RandomState::new].
    /// See the documentation in [RandomSource] for notes about key strength.
    pub fn with_capacity(capacity: usize) -> Self {
        AHashSet(HashSet::with_capacity_and_hasher(capacity, RandomState::new()))
    }
}

impl<T, S> AHashSet<T, S>
where
    S: BuildHasher,
{
    pub fn with_hasher(hash_builder: S) -> Self {
        AHashSet(HashSet::with_hasher(hash_builder))
    }

    pub fn with_capacity_and_hasher(capacity: usize, hash_builder: S) -> Self {
        AHashSet(HashSet::with_capacity_and_hasher(capacity, hash_builder))
    }
}

impl<T, S> Deref for AHashSet<T, S> {
    type Target = HashSet<T, S>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T, S> DerefMut for AHashSet<T, S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T, S> PartialEq for AHashSet<T, S>
where
    T: Eq + Hash,
    S: BuildHasher,
{
    fn eq(&self, other: &AHashSet<T, S>) -> bool {
        self.0.eq(&other.0)
    }
}

impl<T, S> Eq for AHashSet<T, S>
where
    T: Eq + Hash,
    S: BuildHasher,
{
}

impl<T, S> BitOr<&AHashSet<T, S>> for &AHashSet<T, S>
where
    T: Eq + Hash + Clone,
    S: BuildHasher + Default,
{
    type Output = AHashSet<T, S>;

    /// Returns the union of `self` and `rhs` as a new `AHashSet<T, S>`.
    ///
    /// # Examples
    ///
    /// ```
    /// use ahash::AHashSet;
    ///
    /// let a: AHashSet<_> = vec![1, 2, 3].into_iter().collect();
    /// let b: AHashSet<_> = vec![3, 4, 5].into_iter().collect();
    ///
    /// let set = &a | &b;
    ///
    /// let mut i = 0;
    /// let expected = [1, 2, 3, 4, 5];
    /// for x in &set {
    ///     assert!(expected.contains(x));
    ///     i += 1;
    /// }
    /// assert_eq!(i, expected.len());
    /// ```
    fn bitor(self, rhs: &AHashSet<T, S>) -> AHashSet<T, S> {
        AHashSet(self.0.bitor(&rhs.0))
    }
}

impl<T, S> BitAnd<&AHashSet<T, S>> for &AHashSet<T, S>
where
    T: Eq + Hash + Clone,
    S: BuildHasher + Default,
{
    type Output = AHashSet<T, S>;

    /// Returns the intersection of `self` and `rhs` as a new `AHashSet<T, S>`.
    ///
    /// # Examples
    ///
    /// ```
    /// use ahash::AHashSet;
    ///
    /// let a: AHashSet<_> = vec![1, 2, 3].into_iter().collect();
    /// let b: AHashSet<_> = vec![2, 3, 4].into_iter().collect();
    ///
    /// let set = &a & &b;
    ///
    /// let mut i = 0;
    /// let expected = [2, 3];
    /// for x in &set {
    ///     assert!(expected.contains(x));
    ///     i += 1;
    /// }
    /// assert_eq!(i, expected.len());
    /// ```
    fn bitand(self, rhs: &AHashSet<T, S>) -> AHashSet<T, S> {
        AHashSet(self.0.bitand(&rhs.0))
    }
}

impl<T, S> BitXor<&AHashSet<T, S>> for &AHashSet<T, S>
where
    T: Eq + Hash + Clone,
    S: BuildHasher + Default,
{
    type Output = AHashSet<T, S>;

    /// Returns the symmetric difference of `self` and `rhs` as a new `AHashSet<T, S>`.
    ///
    /// # Examples
    ///
    /// ```
    /// use ahash::AHashSet;
    ///
    /// let a: AHashSet<_> = vec![1, 2, 3].into_iter().collect();
    /// let b: AHashSet<_> = vec![3, 4, 5].into_iter().collect();
    ///
    /// let set = &a ^ &b;
    ///
    /// let mut i = 0;
    /// let expected = [1, 2, 4, 5];
    /// for x in &set {
    ///     assert!(expected.contains(x));
    ///     i += 1;
    /// }
    /// assert_eq!(i, expected.len());
    /// ```
    fn bitxor(self, rhs: &AHashSet<T, S>) -> AHashSet<T, S> {
        AHashSet(self.0.bitxor(&rhs.0))
    }
}

impl<T, S> Sub<&AHashSet<T, S>> for &AHashSet<T, S>
where
    T: Eq + Hash + Clone,
    S: BuildHasher + Default,
{
    type Output = AHashSet<T, S>;

    /// Returns the difference of `self` and `rhs` as a new `AHashSet<T, S>`.
    ///
    /// # Examples
    ///
    /// ```
    /// use ahash::AHashSet;
    ///
    /// let a: AHashSet<_> = vec![1, 2, 3].into_iter().collect();
    /// let b: AHashSet<_> = vec![3, 4, 5].into_iter().collect();
    ///
    /// let set = &a - &b;
    ///
    /// let mut i = 0;
    /// let expected = [1, 2];
    /// for x in &set {
    ///     assert!(expected.contains(x));
    ///     i += 1;
    /// }
    /// assert_eq!(i, expected.len());
    /// ```
    fn sub(self, rhs: &AHashSet<T, S>) -> AHashSet<T, S> {
        AHashSet(self.0.sub(&rhs.0))
    }
}

impl<T, S> Debug for AHashSet<T, S>
where
    T: Debug,
    S: BuildHasher,
{
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(fmt)
    }
}

impl<T> FromIterator<T> for AHashSet<T, RandomState>
where
    T: Eq + Hash,
{
    /// This creates a hashset from the provided iterator using [RandomState::new].
    /// See the documentation in [RandomSource] for notes about key strength.
    #[inline]
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> AHashSet<T> {
        let mut inner = HashSet::with_hasher(RandomState::new());
        inner.extend(iter);
        AHashSet(inner)
    }
}

impl<'a, T, S> IntoIterator for &'a AHashSet<T, S> {
    type Item = &'a T;
    type IntoIter = hash_set::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        (&self.0).iter()
    }
}

impl<T, S> IntoIterator for AHashSet<T, S> {
    type Item = T;
    type IntoIter = hash_set::IntoIter<T>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<T, S> Extend<T> for AHashSet<T, S>
where
    T: Eq + Hash,
    S: BuildHasher,
{
    #[inline]
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        self.0.extend(iter)
    }
}

impl<'a, T, S> Extend<&'a T> for AHashSet<T, S>
where
    T: 'a + Eq + Hash + Copy,
    S: BuildHasher,
{
    #[inline]
    fn extend<I: IntoIterator<Item = &'a T>>(&mut self, iter: I) {
        self.0.extend(iter)
    }
}

/// NOTE: For safety this trait impl is only available available if either of the flags `runtime-rng` (on by default) or
/// `compile-time-rng` are enabled. This is to prevent weakly keyed maps from being accidentally created. Instead one of
/// constructors for [RandomState] must be used.
#[cfg(any(feature = "compile-time-rng", feature = "runtime-rng", feature = "no-rng"))]
impl<T> Default for AHashSet<T, RandomState> {
    /// Creates an empty `AHashSet<T, S>` with the `Default` value for the hasher.
    #[inline]
    fn default() -> AHashSet<T, RandomState> {
        AHashSet(HashSet::default())
    }
}

#[cfg(feature = "serde")]
impl<T> Serialize for AHashSet<T>
where
    T: Serialize + Eq + Hash,
{
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.deref().serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de, T> Deserialize<'de> for AHashSet<T>
where
    T: Deserialize<'de> + Eq + Hash,
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let hash_set = HashSet::deserialize(deserializer);
        hash_set.map(|hash_set| Self(hash_set))
    }

    fn deserialize_in_place<D: Deserializer<'de>>(deserializer: D, place: &mut Self) -> Result<(), D::Error> {
        HashSet::deserialize_in_place(deserializer, place)
    }
}

#[cfg(all(test, feature = "serde"))]
mod test {
    use super::*;

    #[test]
    fn test_serde() {
        let mut set = AHashSet::new();
        set.insert("for".to_string());
        set.insert("bar".to_string());
        let mut serialization = serde_json::to_string(&set).unwrap();
        let mut deserialization: AHashSet<String> = serde_json::from_str(&serialization).unwrap();
        assert_eq!(deserialization, set);

        set.insert("baz".to_string());
        serialization = serde_json::to_string(&set).unwrap();
        let mut deserializer = serde_json::Deserializer::from_str(&serialization);
        AHashSet::deserialize_in_place(&mut deserializer, &mut deserialization).unwrap();
        assert_eq!(deserialization, set);
    }
}
