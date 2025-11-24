//! Sources for key-values.
//!
//! This module defines the [`Source`] type and supporting APIs for
//! working with collections of key-values.

use crate::kv::{Error, Key, ToKey, ToValue, Value};
use std::fmt;

/// A source of key-values.
///
/// The source may be a single pair, a set of pairs, or a filter over a set of pairs.
/// Use the [`VisitSource`](trait.VisitSource.html) trait to inspect the structured data
/// in a source.
///
/// A source is like an iterator over its key-values, except with a push-based API
/// instead of a pull-based one.
///
/// # Examples
///
/// Enumerating the key-values in a source:
///
/// ```
/// # fn main() -> Result<(), log::kv::Error> {
/// use log::kv::{self, Source, Key, Value, VisitSource};
///
/// // A `VisitSource` that prints all key-values
/// // VisitSources are fed the key-value pairs of each key-values
/// struct Printer;
///
/// impl<'kvs> VisitSource<'kvs> for Printer {
///     fn visit_pair(&mut self, key: Key<'kvs>, value: Value<'kvs>) -> Result<(), kv::Error> {
///         println!("{key}: {value}");
///
///         Ok(())
///     }
/// }
///
/// // A source with 3 key-values
/// // Common collection types implement the `Source` trait
/// let source = &[
///     ("a", 1),
///     ("b", 2),
///     ("c", 3),
/// ];
///
/// // Pass an instance of the `VisitSource` to a `Source` to visit it
/// source.visit(&mut Printer)?;
/// # Ok(())
/// # }
/// ```
pub trait Source {
    /// Visit key-values.
    ///
    /// A source doesn't have to guarantee any ordering or uniqueness of key-values.
    /// If the given visitor returns an error then the source may early-return with it,
    /// even if there are more key-values.
    ///
    /// # Implementation notes
    ///
    /// A source should yield the same key-values to a subsequent visitor unless
    /// that visitor itself fails.
    fn visit<'kvs>(&'kvs self, visitor: &mut dyn VisitSource<'kvs>) -> Result<(), Error>;

    /// Get the value for a given key.
    ///
    /// If the key appears multiple times in the source then which key is returned
    /// is implementation specific.
    ///
    /// # Implementation notes
    ///
    /// A source that can provide a more efficient implementation of this method
    /// should override it.
    fn get(&self, key: Key) -> Option<Value<'_>> {
        get_default(self, key)
    }

    /// Count the number of key-values that can be visited.
    ///
    /// # Implementation notes
    ///
    /// A source that knows the number of key-values upfront may provide a more
    /// efficient implementation.
    ///
    /// A subsequent call to `visit` should yield the same number of key-values.
    fn count(&self) -> usize {
        count_default(self)
    }
}

/// The default implementation of `Source::get`
fn get_default<'v>(source: &'v (impl Source + ?Sized), key: Key) -> Option<Value<'v>> {
    struct Get<'k, 'v> {
        key: Key<'k>,
        found: Option<Value<'v>>,
    }

    impl<'k, 'kvs> VisitSource<'kvs> for Get<'k, 'kvs> {
        fn visit_pair(&mut self, key: Key<'kvs>, value: Value<'kvs>) -> Result<(), Error> {
            if self.key == key {
                self.found = Some(value);
            }

            Ok(())
        }
    }

    let mut get = Get { key, found: None };

    let _ = source.visit(&mut get);
    get.found
}

/// The default implementation of `Source::count`.
fn count_default(source: impl Source) -> usize {
    struct Count(usize);

    impl<'kvs> VisitSource<'kvs> for Count {
        fn visit_pair(&mut self, _: Key<'kvs>, _: Value<'kvs>) -> Result<(), Error> {
            self.0 += 1;

            Ok(())
        }
    }

    let mut count = Count(0);
    let _ = source.visit(&mut count);
    count.0
}

impl<'a, T> Source for &'a T
where
    T: Source + ?Sized,
{
    fn visit<'kvs>(&'kvs self, visitor: &mut dyn VisitSource<'kvs>) -> Result<(), Error> {
        Source::visit(&**self, visitor)
    }

    fn get(&self, key: Key) -> Option<Value<'_>> {
        Source::get(&**self, key)
    }

    fn count(&self) -> usize {
        Source::count(&**self)
    }
}

impl<K, V> Source for (K, V)
where
    K: ToKey,
    V: ToValue,
{
    fn visit<'kvs>(&'kvs self, visitor: &mut dyn VisitSource<'kvs>) -> Result<(), Error> {
        visitor.visit_pair(self.0.to_key(), self.1.to_value())
    }

    fn get(&self, key: Key) -> Option<Value<'_>> {
        if self.0.to_key() == key {
            Some(self.1.to_value())
        } else {
            None
        }
    }

    fn count(&self) -> usize {
        1
    }
}

impl<S> Source for [S]
where
    S: Source,
{
    fn visit<'kvs>(&'kvs self, visitor: &mut dyn VisitSource<'kvs>) -> Result<(), Error> {
        for source in self {
            source.visit(visitor)?;
        }

        Ok(())
    }

    fn get(&self, key: Key) -> Option<Value<'_>> {
        for source in self {
            if let Some(found) = source.get(key.clone()) {
                return Some(found);
            }
        }

        None
    }

    fn count(&self) -> usize {
        self.iter().map(Source::count).sum()
    }
}

impl<const N: usize, S> Source for [S; N]
where
    S: Source,
{
    fn visit<'kvs>(&'kvs self, visitor: &mut dyn VisitSource<'kvs>) -> Result<(), Error> {
        Source::visit(self as &[_], visitor)
    }

    fn get(&self, key: Key) -> Option<Value<'_>> {
        Source::get(self as &[_], key)
    }

    fn count(&self) -> usize {
        Source::count(self as &[_])
    }
}

impl<S> Source for Option<S>
where
    S: Source,
{
    fn visit<'kvs>(&'kvs self, visitor: &mut dyn VisitSource<'kvs>) -> Result<(), Error> {
        if let Some(source) = self {
            source.visit(visitor)?;
        }

        Ok(())
    }

    fn get(&self, key: Key) -> Option<Value<'_>> {
        self.as_ref().and_then(|s| s.get(key))
    }

    fn count(&self) -> usize {
        self.as_ref().map_or(0, Source::count)
    }
}

/// A visitor for the key-value pairs in a [`Source`](trait.Source.html).
pub trait VisitSource<'kvs> {
    /// Visit a key-value pair.
    fn visit_pair(&mut self, key: Key<'kvs>, value: Value<'kvs>) -> Result<(), Error>;
}

impl<'a, 'kvs, T> VisitSource<'kvs> for &'a mut T
where
    T: VisitSource<'kvs> + ?Sized,
{
    fn visit_pair(&mut self, key: Key<'kvs>, value: Value<'kvs>) -> Result<(), Error> {
        (**self).visit_pair(key, value)
    }
}

impl<'a, 'b: 'a, 'kvs> VisitSource<'kvs> for fmt::DebugMap<'a, 'b> {
    fn visit_pair(&mut self, key: Key<'kvs>, value: Value<'kvs>) -> Result<(), Error> {
        self.entry(&key, &value);
        Ok(())
    }
}

impl<'a, 'b: 'a, 'kvs> VisitSource<'kvs> for fmt::DebugList<'a, 'b> {
    fn visit_pair(&mut self, key: Key<'kvs>, value: Value<'kvs>) -> Result<(), Error> {
        self.entry(&(key, value));
        Ok(())
    }
}

impl<'a, 'b: 'a, 'kvs> VisitSource<'kvs> for fmt::DebugSet<'a, 'b> {
    fn visit_pair(&mut self, key: Key<'kvs>, value: Value<'kvs>) -> Result<(), Error> {
        self.entry(&(key, value));
        Ok(())
    }
}

impl<'a, 'b: 'a, 'kvs> VisitSource<'kvs> for fmt::DebugTuple<'a, 'b> {
    fn visit_pair(&mut self, key: Key<'kvs>, value: Value<'kvs>) -> Result<(), Error> {
        self.field(&key);
        self.field(&value);
        Ok(())
    }
}

#[cfg(feature = "std")]
mod std_support {
    use super::*;
    use std::borrow::Borrow;
    use std::collections::{BTreeMap, HashMap};
    use std::hash::{BuildHasher, Hash};
    use std::rc::Rc;
    use std::sync::Arc;

    impl<S> Source for Box<S>
    where
        S: Source + ?Sized,
    {
        fn visit<'kvs>(&'kvs self, visitor: &mut dyn VisitSource<'kvs>) -> Result<(), Error> {
            Source::visit(&**self, visitor)
        }

        fn get(&self, key: Key) -> Option<Value<'_>> {
            Source::get(&**self, key)
        }

        fn count(&self) -> usize {
            Source::count(&**self)
        }
    }

    impl<S> Source for Arc<S>
    where
        S: Source + ?Sized,
    {
        fn visit<'kvs>(&'kvs self, visitor: &mut dyn VisitSource<'kvs>) -> Result<(), Error> {
            Source::visit(&**self, visitor)
        }

        fn get(&self, key: Key) -> Option<Value<'_>> {
            Source::get(&**self, key)
        }

        fn count(&self) -> usize {
            Source::count(&**self)
        }
    }

    impl<S> Source for Rc<S>
    where
        S: Source + ?Sized,
    {
        fn visit<'kvs>(&'kvs self, visitor: &mut dyn VisitSource<'kvs>) -> Result<(), Error> {
            Source::visit(&**self, visitor)
        }

        fn get(&self, key: Key) -> Option<Value<'_>> {
            Source::get(&**self, key)
        }

        fn count(&self) -> usize {
            Source::count(&**self)
        }
    }

    impl<S> Source for Vec<S>
    where
        S: Source,
    {
        fn visit<'kvs>(&'kvs self, visitor: &mut dyn VisitSource<'kvs>) -> Result<(), Error> {
            Source::visit(&**self, visitor)
        }

        fn get(&self, key: Key) -> Option<Value<'_>> {
            Source::get(&**self, key)
        }

        fn count(&self) -> usize {
            Source::count(&**self)
        }
    }

    impl<'kvs, V> VisitSource<'kvs> for Box<V>
    where
        V: VisitSource<'kvs> + ?Sized,
    {
        fn visit_pair(&mut self, key: Key<'kvs>, value: Value<'kvs>) -> Result<(), Error> {
            (**self).visit_pair(key, value)
        }
    }

    impl<K, V, S> Source for HashMap<K, V, S>
    where
        K: ToKey + Borrow<str> + Eq + Hash,
        V: ToValue,
        S: BuildHasher,
    {
        fn visit<'kvs>(&'kvs self, visitor: &mut dyn VisitSource<'kvs>) -> Result<(), Error> {
            for (key, value) in self {
                visitor.visit_pair(key.to_key(), value.to_value())?;
            }
            Ok(())
        }

        fn get(&self, key: Key) -> Option<Value<'_>> {
            HashMap::get(self, key.as_str()).map(|v| v.to_value())
        }

        fn count(&self) -> usize {
            self.len()
        }
    }

    impl<K, V> Source for BTreeMap<K, V>
    where
        K: ToKey + Borrow<str> + Ord,
        V: ToValue,
    {
        fn visit<'kvs>(&'kvs self, visitor: &mut dyn VisitSource<'kvs>) -> Result<(), Error> {
            for (key, value) in self {
                visitor.visit_pair(key.to_key(), value.to_value())?;
            }
            Ok(())
        }

        fn get(&self, key: Key) -> Option<Value<'_>> {
            BTreeMap::get(self, key.as_str()).map(|v| v.to_value())
        }

        fn count(&self) -> usize {
            self.len()
        }
    }

    #[cfg(test)]
    mod tests {
        use crate::kv::value;

        use super::*;

        #[test]
        fn count() {
            assert_eq!(1, Source::count(&Box::new(("a", 1))));
            assert_eq!(2, Source::count(&vec![("a", 1), ("b", 2)]));
        }

        #[test]
        fn get() {
            let source = vec![("a", 1), ("b", 2), ("a", 1)];
            assert_eq!(
                value::inner::Token::I64(1),
                Source::get(&source, Key::from_str("a")).unwrap().to_token()
            );

            let source = Box::new(None::<(&str, i32)>);
            assert!(Source::get(&source, Key::from_str("a")).is_none());
        }

        #[test]
        fn hash_map() {
            let mut map = HashMap::new();
            map.insert("a", 1);
            map.insert("b", 2);

            assert_eq!(2, Source::count(&map));
            assert_eq!(
                value::inner::Token::I64(1),
                Source::get(&map, Key::from_str("a")).unwrap().to_token()
            );
        }

        #[test]
        fn btree_map() {
            let mut map = BTreeMap::new();
            map.insert("a", 1);
            map.insert("b", 2);

            assert_eq!(2, Source::count(&map));
            assert_eq!(
                value::inner::Token::I64(1),
                Source::get(&map, Key::from_str("a")).unwrap().to_token()
            );
        }
    }
}

// NOTE: Deprecated; but aliases can't carry this attribute
#[cfg(feature = "kv_unstable")]
pub use VisitSource as Visitor;

#[cfg(test)]
mod tests {
    use crate::kv::value;

    use super::*;

    #[test]
    fn source_is_object_safe() {
        fn _check(_: &dyn Source) {}
    }

    #[test]
    fn visitor_is_object_safe() {
        fn _check(_: &dyn VisitSource) {}
    }

    #[test]
    fn count() {
        struct OnePair {
            key: &'static str,
            value: i32,
        }

        impl Source for OnePair {
            fn visit<'kvs>(&'kvs self, visitor: &mut dyn VisitSource<'kvs>) -> Result<(), Error> {
                visitor.visit_pair(self.key.to_key(), self.value.to_value())
            }
        }

        assert_eq!(1, Source::count(&("a", 1)));
        assert_eq!(2, Source::count(&[("a", 1), ("b", 2)] as &[_]));
        assert_eq!(0, Source::count(&None::<(&str, i32)>));
        assert_eq!(1, Source::count(&OnePair { key: "a", value: 1 }));
    }

    #[test]
    fn get() {
        let source = &[("a", 1), ("b", 2), ("a", 1)] as &[_];
        assert_eq!(
            value::inner::Token::I64(1),
            Source::get(source, Key::from_str("a")).unwrap().to_token()
        );
        assert_eq!(
            value::inner::Token::I64(2),
            Source::get(source, Key::from_str("b")).unwrap().to_token()
        );
        assert!(Source::get(&source, Key::from_str("c")).is_none());

        let source = None::<(&str, i32)>;
        assert!(Source::get(&source, Key::from_str("a")).is_none());
    }
}
