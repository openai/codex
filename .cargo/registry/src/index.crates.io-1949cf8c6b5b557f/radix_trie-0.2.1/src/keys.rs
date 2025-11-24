use endian_type::{BigEndian, LittleEndian};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use nibble_vec::Nibblet;

/// Trait for types which can be used to key a Radix Trie.
///
/// Types that implement this trait should be convertible to a vector of half-bytes (nibbles)
/// such that no two instances of the type convert to the same vector.
/// To protect against faulty behaviour, the trie will **panic** if it finds two distinct keys
/// of type `K` which encode to the same `Nibblet`, so be careful!
///
/// If you would like to implement this trait for your own type, you need to implement
/// *either* `encode_bytes` or `encode`. You only need to implement one of the two.
/// If you don't implement one, your code will **panic** as soon you use the trie.
/// There is no performance penalty for implementing `encode_bytes` instead of `encode`,
/// so it is preferred except in the case where you require half-byte precision.
///
/// Many standard types implement this trait already. Integer types are encoded *big-endian*
/// by default but can be encoded little-endian using the `LittleEndian<T>` wrapper type.
pub trait TrieKey: PartialEq + Eq {
    /// Encode a value as a vector of bytes.
    fn encode_bytes(&self) -> Vec<u8> {
        panic!("implement this method or TrieKey::encode");
    }

    /// Encode a value as a NibbleVec.
    #[inline]
    fn encode(&self) -> Nibblet {
        Nibblet::from_byte_vec(self.encode_bytes())
    }
}

/// Key comparison result.
#[derive(Debug)]
pub enum KeyMatch {
    /// The keys match up to the given index.
    Partial(usize),
    /// The first key is a prefix of the second.
    FirstPrefix,
    /// The second key is a prefix of the first.
    SecondPrefix,
    /// The keys match exactly.
    Full,
}

/// Compare two Trie keys.
///
/// Compares `first[start_idx .. ]` to `second`, i.e. only looks at a slice of the first key.
#[inline]
pub fn match_keys(start_idx: usize, first: &Nibblet, second: &Nibblet) -> KeyMatch {
    let first_len = first.len() - start_idx;
    let min_length = ::std::cmp::min(first_len, second.len());

    for i in 0..min_length {
        if first.get(start_idx + i) != second.get(i) {
            return KeyMatch::Partial(i);
        }
    }

    match (first_len, second.len()) {
        (x, y) if x < y => KeyMatch::FirstPrefix,
        (x, y) if x == y => KeyMatch::Full,
        _ => KeyMatch::SecondPrefix,
    }
}

/// Check two keys for equality and panic if they differ.
#[inline]
pub fn check_keys<K: ?Sized>(key1: &K, key2: &K)
where
    K: TrieKey,
{
    if *key1 != *key2 {
        panic!("multiple-keys with the same bit representation.");
    }
}

// --- TrieKey Implementations for standard types --- ///

// This blanket implementation goes into play when specialization is stabilized
// impl<T> TrieKey for T where T: Into<Vec<u8>> + Clone + Eq + PartialEq {
// fn encode_bytes(&self) -> Vec<u8> {
// self.clone().into()
// }
// }

impl TrieKey for Vec<u8> {
    #[inline]
    fn encode_bytes(&self) -> Vec<u8> {
        self.clone()
    }
}

impl TrieKey for [u8] {
    #[inline]
    fn encode_bytes(&self) -> Vec<u8> {
        self.to_vec()
    }
}

impl TrieKey for String {
    #[inline]
    fn encode_bytes(&self) -> Vec<u8> {
        self.as_bytes().encode_bytes()
    }
}

impl TrieKey for str {
    #[inline]
    fn encode_bytes(&self) -> Vec<u8> {
        self.as_bytes().encode_bytes()
    }
}

impl<'a, T: ?Sized + TrieKey> TrieKey for &'a T {
    #[inline]
    fn encode_bytes(&self) -> Vec<u8> {
        (**self).encode_bytes()
    }
}

impl<'a, T: ?Sized + TrieKey> TrieKey for &'a mut T {
    #[inline]
    fn encode_bytes(&self) -> Vec<u8> {
        (**self).encode_bytes()
    }
}

impl TrieKey for i8 {
    #[inline]
    fn encode_bytes(&self) -> Vec<u8> {
        let mut v: Vec<u8> = Vec::with_capacity(1);
        v.push(*self as u8);
        v
    }
}

impl TrieKey for u8 {
    #[inline]
    fn encode_bytes(&self) -> Vec<u8> {
        let mut v: Vec<u8> = Vec::with_capacity(1);
        v.push(*self);
        v
    }
}

#[cfg(unix)]
impl TrieKey for PathBuf {
    fn encode_bytes(&self) -> Vec<u8> {
        use std::os::unix::ffi::OsStringExt;
        let str: OsString = self.clone().into();
        str.into_vec()
    }
}

#[cfg(unix)]
impl TrieKey for Path {
    fn encode_bytes(&self) -> Vec<u8> {
        use std::os::unix::ffi::OsStrExt;
        self.as_os_str().as_bytes().encode_bytes()
    }
}

impl<T> TrieKey for LittleEndian<T>
where
    T: Eq + Copy,
{
    fn encode_bytes(&self) -> Vec<u8> {
        self.as_bytes().encode_bytes()
    }
}

impl<T> TrieKey for BigEndian<T>
where
    T: Eq + Copy,
{
    fn encode_bytes(&self) -> Vec<u8> {
        self.as_bytes().to_vec()
    }
}

macro_rules! int_keys {
    ( $( $t:ty ),* ) => {
        $(
        impl TrieKey for $t {
            fn encode_bytes(&self) -> Vec<u8> {
                let be: BigEndian<$t> = From::from(*self);
                be.encode_bytes()
            }
        }
        )*
    };
}

int_keys!(u16, u32, u64, i16, i32, i64, usize, isize);

macro_rules! vec_int_keys {
  ( $( $t:ty ),* ) => {
      $(
      impl TrieKey for Vec<$t> {
          fn encode_bytes(&self) -> Vec<u8> {
              let mut v = Vec::<u8>::with_capacity(self.len() * std::mem::size_of::<$t>());
              for u in self {
                  v.extend_from_slice(&u.to_be_bytes());
              }
              v
          }
      }
      )*
   };
}

vec_int_keys!(u16, u32, u64, i16, i32, i64, usize, isize);

#[cfg(test)]
mod test {
    pub trait DefaultTrieKey {
        fn encode_bytes(&self) -> Vec<u8>;
    }

    impl<T: Into<Vec<u8>> + Clone + PartialEq + Eq> DefaultTrieKey for T {
        #[inline]
        fn encode_bytes(&self) -> Vec<u8> {
            self.clone().into()
        }
    }

    pub trait AsTrieKey {
        fn encode_bytes(&self) -> Vec<u8>;
    }

    impl<T: AsRef<[u8]> + Clone + PartialEq + Eq> AsTrieKey for &T {
        #[inline]
        fn encode_bytes(&self) -> Vec<u8> {
            self.as_ref().to_vec()
        }
    }

    macro_rules! encode_bytes {
        ($e:expr) => {
            (&$e).encode_bytes()
        };
    }

    #[test]
    fn test_autoref_specialization() {
        let _ = encode_bytes!([0_u8]);
        let _ = encode_bytes!("hello");
        let _ = encode_bytes!("hello".to_string());
    }
}
