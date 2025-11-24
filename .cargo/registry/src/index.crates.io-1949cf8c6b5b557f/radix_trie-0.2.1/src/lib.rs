//! A wonderful, fast, safe, generic radix trie implementation.
//!
//! To get started, see the docs for `Trie` below.

// #![warn(missing_docs)]

extern crate endian_type;
extern crate nibble_vec;
#[cfg(test)]
extern crate quickcheck;
#[cfg(test)]
extern crate rand;

pub use keys::TrieKey;
pub use nibble_vec::NibbleVec;
pub use trie_common::TrieCommon;
use trie_node::TrieNode;

use nibble_vec::Nibblet;

#[macro_use]
mod macros;
pub mod iter;
mod keys;
#[cfg(feature = "serde")]
mod serde;
mod subtrie;
mod traversal;
mod trie;
mod trie_common;
mod trie_node;

#[cfg(test)]
mod qc_test;
#[cfg(test)]
mod test;

const BRANCH_FACTOR: usize = 16;

/// Data-structure for storing and querying string-like keys and associated values.
///
/// Any keys which share a common *prefix* are stored below a single copy of that prefix.
/// This saves space, and also allows the longest prefix of any given key to be found.
///
/// You can read more about Radix Tries on [Wikipedia][radix-wiki].
///
/// Lots of the methods on `Trie` return optional values - they can be composed
/// nicely using `Option::and_then`.
///
/// [radix-wiki]: http://en.wikipedia.org/wiki/Radix_tree
#[derive(Debug, Clone)]
pub struct Trie<K, V> {
    /// The number of values stored in this sub-trie (this node and all descendants).
    length: usize,
    /// The main content of this trie.
    node: TrieNode<K, V>,
}

/// Immutable view of a sub-tree a larger trie.
#[derive(Debug)]
pub struct SubTrie<'a, K: 'a, V: 'a> {
    prefix: Nibblet,
    node: &'a TrieNode<K, V>,
}

/// Mutable view of a sub-tree of a larger trie.
#[derive(Debug)]
pub struct SubTrieMut<'a, K: 'a, V: 'a> {
    prefix: Nibblet,
    length: &'a mut usize,
    node: &'a mut TrieNode<K, V>,
}

/// Wrapper for subtrie lookup results.
///
/// When fetching from a subtrie, if the prefix is wrong you'll get an `Err(())`.
/// Otherwise you'll get an `Ok(_)`, where the contained option value is what would ordinarily
/// be returned by get/insert/whatever.
pub type SubTrieResult<T> = Result<Option<T>, ()>;
