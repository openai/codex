use std::{collections, hash, ops::DerefMut, sync};

/// Trait implemented by types which can be cleared in place, retaining any
/// allocated memory.
///
/// This is essentially a generalization of methods on standard library
/// collection types, including as [`Vec::clear`], [`String::clear`], and
/// [`HashMap::clear`]. These methods drop all data stored in the collection,
/// but retain the collection's heap allocation for future use. Types such as
/// `BTreeMap`, whose `clear` methods drops allocations, should not
/// implement this trait.
///
/// When implemented for types which do not own a heap allocation, `Clear`
/// should reset the type in place if possible. If the type has an empty state
/// or stores `Option`s, those values should be reset to the empty state. For
/// "plain old data" types, which hold no pointers to other data and do not have
/// an empty or initial state, it's okay for a `Clear` implementation to be a
/// no-op. In that case, it essentially serves as a marker indicating that the
/// type may be reused to store new data.
///
/// [`Vec::clear`]: https://doc.rust-lang.org/stable/std/vec/struct.Vec.html#method.clear
/// [`String::clear`]: https://doc.rust-lang.org/stable/std/string/struct.String.html#method.clear
/// [`HashMap::clear`]: https://doc.rust-lang.org/stable/std/collections/struct.HashMap.html#method.clear
pub trait Clear {
    /// Clear all data in `self`, retaining the allocated capacithy.
    fn clear(&mut self);
}

impl<T> Clear for Option<T> {
    fn clear(&mut self) {
        let _ = self.take();
    }
}

impl<T> Clear for Box<T>
where
    T: Clear,
{
    #[inline]
    fn clear(&mut self) {
        self.deref_mut().clear()
    }
}

impl<T> Clear for Vec<T> {
    #[inline]
    fn clear(&mut self) {
        Vec::clear(self)
    }
}

impl<K, V, S> Clear for collections::HashMap<K, V, S>
where
    K: hash::Hash + Eq,
    S: hash::BuildHasher,
{
    #[inline]
    fn clear(&mut self) {
        collections::HashMap::clear(self)
    }
}

impl<T, S> Clear for collections::HashSet<T, S>
where
    T: hash::Hash + Eq,
    S: hash::BuildHasher,
{
    #[inline]
    fn clear(&mut self) {
        collections::HashSet::clear(self)
    }
}

impl Clear for String {
    #[inline]
    fn clear(&mut self) {
        String::clear(self)
    }
}

impl<T: Clear> Clear for sync::Mutex<T> {
    #[inline]
    fn clear(&mut self) {
        self.get_mut().unwrap().clear();
    }
}

impl<T: Clear> Clear for sync::RwLock<T> {
    #[inline]
    fn clear(&mut self) {
        self.write().unwrap().clear();
    }
}

#[cfg(all(loom, test))]
impl<T: Clear> Clear for crate::sync::alloc::Track<T> {
    fn clear(&mut self) {
        self.get_mut().clear()
    }
}
