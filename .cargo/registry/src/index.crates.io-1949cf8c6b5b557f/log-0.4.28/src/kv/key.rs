//! Structured keys.

use std::borrow::Borrow;
use std::fmt;

/// A type that can be converted into a [`Key`](struct.Key.html).
pub trait ToKey {
    /// Perform the conversion.
    fn to_key(&self) -> Key;
}

impl<'a, T> ToKey for &'a T
where
    T: ToKey + ?Sized,
{
    fn to_key(&self) -> Key {
        (**self).to_key()
    }
}

impl<'k> ToKey for Key<'k> {
    fn to_key(&self) -> Key {
        Key { key: self.key }
    }
}

impl ToKey for str {
    fn to_key(&self) -> Key {
        Key::from_str(self)
    }
}

/// A key in a key-value.
// These impls must only be based on the as_str() representation of the key
// If a new field (such as an optional index) is added to the key they must not affect comparison
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Key<'k> {
    // NOTE: This may become `Cow<'k, str>`
    key: &'k str,
}

impl<'k> Key<'k> {
    /// Get a key from a borrowed string.
    pub fn from_str(key: &'k str) -> Self {
        Key { key }
    }

    /// Get a borrowed string from this key.
    ///
    /// The lifetime of the returned string is bound to the borrow of `self` rather
    /// than to `'k`.
    pub fn as_str(&self) -> &str {
        self.key
    }

    /// Try get a borrowed string for the lifetime `'k` from this key.
    ///
    /// If the key is a borrow of a longer lived string, this method will return `Some`.
    /// If the key is internally buffered, this method will return `None`.
    pub fn to_borrowed_str(&self) -> Option<&'k str> {
        // NOTE: If the internals of `Key` support buffering this
        // won't be unconditionally `Some` anymore. We want to keep
        // this option open
        Some(self.key)
    }
}

impl<'k> fmt::Display for Key<'k> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.key.fmt(f)
    }
}

impl<'k> AsRef<str> for Key<'k> {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl<'k> Borrow<str> for Key<'k> {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl<'k> From<&'k str> for Key<'k> {
    fn from(s: &'k str) -> Self {
        Key::from_str(s)
    }
}

#[cfg(feature = "std")]
mod std_support {
    use super::*;

    use std::borrow::Cow;

    impl ToKey for String {
        fn to_key(&self) -> Key {
            Key::from_str(self)
        }
    }

    impl<'a> ToKey for Cow<'a, str> {
        fn to_key(&self) -> Key {
            Key::from_str(self)
        }
    }
}

#[cfg(feature = "kv_sval")]
mod sval_support {
    use super::*;

    use sval::Value;
    use sval_ref::ValueRef;

    impl<'a> Value for Key<'a> {
        fn stream<'sval, S: sval::Stream<'sval> + ?Sized>(
            &'sval self,
            stream: &mut S,
        ) -> sval::Result {
            self.key.stream(stream)
        }
    }

    impl<'a> ValueRef<'a> for Key<'a> {
        fn stream_ref<S: sval::Stream<'a> + ?Sized>(&self, stream: &mut S) -> sval::Result {
            self.key.stream(stream)
        }
    }
}

#[cfg(feature = "kv_serde")]
mod serde_support {
    use super::*;

    use serde::{Serialize, Serializer};

    impl<'a> Serialize for Key<'a> {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            self.key.serialize(serializer)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_from_string() {
        assert_eq!("a key", Key::from_str("a key").as_str());
    }

    #[test]
    fn key_to_borrowed() {
        assert_eq!("a key", Key::from_str("a key").to_borrowed_str().unwrap());
    }
}
