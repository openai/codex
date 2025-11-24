use core::borrow::Borrow;

/// Key equivalence trait.
///
/// This trait allows hash table lookup to be customized.
/// It has one blanket implementation that uses the regular `Borrow` solution,
/// just like `HashMap` and `BTreeMap` do, so that you can pass `&str` to lookup
/// into a map with `String` keys and so on.
///
/// # Contract
///
/// The implementor **must** hash like `K`, if it is hashable.
pub trait Equivalent<K: ?Sized> {
    /// Compare self to `key` and return `true` if they are equal.
    fn equivalent(&self, key: &K) -> bool;
}

impl<Q: ?Sized, K: ?Sized> Equivalent<K> for Q
where
    Q: Eq,
    K: Borrow<Q>,
{
    #[inline]
    fn equivalent(&self, key: &K) -> bool {
        *self == *key.borrow()
    }
}
