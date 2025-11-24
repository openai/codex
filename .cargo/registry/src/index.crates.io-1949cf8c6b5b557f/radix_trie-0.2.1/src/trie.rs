use crate::traversal::DescendantResult::*;
use crate::TrieNode;
use crate::{SubTrie, SubTrieMut, Trie, TrieCommon, TrieKey};
use std::borrow::Borrow;

use nibble_vec::Nibblet;

impl<K, V> Trie<K, V>
where
    K: TrieKey,
{
    /// Create an empty Trie.
    #[inline]
    pub fn new() -> Trie<K, V> {
        Trie {
            length: 0,
            node: TrieNode::new(),
        }
    }

    /// Fetch a reference to the given key's corresponding value, if any.
    ///
    /// The key may be any borrowed form of the trie's key type, but TrieKey on the borrowed
    /// form *must* match those for the key type.
    #[inline]
    pub fn get<Q: ?Sized>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: TrieKey,
    {
        let key_fragments = key.encode();
        self.node
            .get(&key_fragments)
            .and_then(|t| t.value_checked(key))
    }

    /// Fetch a mutable reference to the given key's corresponding value, if any.
    ///
    /// The key may be any borrowed form of the trie's key type, but TrieKey on the borrowed
    /// form *must* match those for the key type.
    #[inline]
    pub fn get_mut<Q: ?Sized>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: TrieKey,
    {
        let key_fragments = key.encode();
        self.node
            .get_mut(&key_fragments)
            .and_then(|t| t.value_checked_mut(key))
    }

    /// Insert the given key-value pair, returning any previous value associated with the key.
    #[inline]
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        let key_fragments = key.encode();
        let result = self.node.insert(key, value, key_fragments);
        if result.is_none() {
            self.length += 1;
        }
        result
    }

    /// Remove the value associated with the given key.
    ///
    /// The key may be any borrowed form of the trie's key type, but TrieKey on the borrowed
    /// form *must* match those for the key type.
    #[inline]
    pub fn remove<Q: ?Sized>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: TrieKey,
    {
        let removed = self.node.remove(key);
        if removed.is_some() {
            self.length -= 1;
        }
        removed
    }

    /// Get a mutable reference to the value stored at this node, if any.
    pub fn value_mut(&mut self) -> Option<&mut V> {
        self.node.value_mut()
    }

    /// Fetch a reference to the subtrie for a given key.
    ///
    /// The key may be any borrowed form of the trie's key type, but TrieKey on the borrowed
    /// form *must* match those for the key type.
    #[inline]
    pub fn subtrie<'a, Q: ?Sized>(&'a self, key: &Q) -> Option<SubTrie<'a, K, V>>
    where
        K: Borrow<Q>,
        Q: TrieKey,
    {
        let key_fragments = key.encode();
        self.node
            .get(&key_fragments)
            .map(|node| node.as_subtrie(key_fragments))
    }

    /// Fetch a mutable reference to the subtrie for a given key.
    ///
    /// The key may be any borrowed form of the trie's key type, but TrieKey on the borrowed
    /// form *must* match those for the key type.
    #[inline]
    pub fn subtrie_mut<'a, Q: ?Sized>(&'a mut self, key: &Q) -> Option<SubTrieMut<'a, K, V>>
    where
        K: Borrow<Q>,
        Q: TrieKey,
    {
        let key_fragments = key.encode();
        let length_ref = &mut self.length;
        self.node
            .get_mut(&key_fragments)
            .map(move |node| node.as_subtrie_mut(key_fragments, length_ref))
    }

    /// Fetch a reference to the closest ancestor node of the given key.
    ///
    /// If `key` is encoded as byte-vector `b`, return the node `n` in the tree
    /// such that `n`'s key's byte-vector is the longest possible prefix of `b`, and `n`
    /// has a value.
    ///
    /// Invariant: `result.is_some() => result.key_value.is_some()`.
    ///
    /// The key may be any borrowed form of the trie's key type, but TrieKey on the borrowed
    /// form *must* match those for the key type.
    #[inline]
    pub fn get_ancestor<'a, Q: ?Sized>(&'a self, key: &Q) -> Option<SubTrie<'a, K, V>>
    where
        K: Borrow<Q>,
        Q: TrieKey,
    {
        let mut key_fragments = key.encode();
        self.node
            .get_ancestor(&key_fragments)
            .map(|(node, node_key_len)| {
                key_fragments.split(node_key_len);
                node.as_subtrie(key_fragments)
            })
    }

    /// Fetch the closest ancestor *value* for a given key.
    ///
    /// See `get_ancestor` for precise semantics, this is just a shortcut.
    ///
    /// The key may be any borrowed form of the trie's key type, but TrieKey on the borrowed
    /// form *must* match those for the key type.
    #[inline]
    pub fn get_ancestor_value<Q: ?Sized>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: TrieKey,
    {
        self.get_ancestor(key).and_then(|t| t.node.value())
    }

    /// The key may be any borrowed form of the trie's key type, but TrieKey on the borrowed
    /// form *must* match those for the key type
    #[inline]
    pub fn get_raw_ancestor<'a, Q: ?Sized>(&'a self, key: &Q) -> SubTrie<'a, K, V>
    where
        K: Borrow<Q>,
        Q: TrieKey,
    {
        let mut nv = key.encode();
        let (ancestor_node, depth) = self.node.get_raw_ancestor(&nv);
        nv.split(depth);
        ancestor_node.as_subtrie(nv)
    }

    /// Fetch the closest descendant for a given key.
    ///
    /// If the key is in the trie, this is the same as `subtrie`.
    ///
    /// The key may be any borrowed form of the trie's key type, but TrieKey on the borrowed
    /// form *must* match those for the key type
    #[inline]
    pub fn get_raw_descendant<'a, Q: ?Sized>(&'a self, key: &Q) -> Option<SubTrie<'a, K, V>>
    where
        K: Borrow<Q>,
        Q: TrieKey,
    {
        let mut nv = key.encode();
        self.node.get_raw_descendant(&nv).map(|desc| {
            let (node, prefix) = match desc {
                NoModification(node) => (node, nv),
                ExtendKey(node, depth, extension) => {
                    nv.split(depth);
                    (node, nv.join(extension))
                }
            };
            node.as_subtrie(prefix)
        })
    }

    /// Take a function `f` and apply it to the value stored at `key`.
    ///
    /// If no value is stored at `key`, store `default`.
    #[inline]
    pub fn map_with_default<F>(&mut self, key: K, f: F, default: V)
    where
        F: Fn(&mut V),
    {
        {
            if let Some(v) = self.get_mut(&key) {
                f(v);
                return;
            }
        }
        self.insert(key, default);
    }

    /// Check that the Trie invariants are satisfied - you shouldn't ever have to call this!
    /// Quite slow!
    #[doc(hidden)]
    pub fn check_integrity(&self) -> bool {
        let (ok, length) = self.node.check_integrity_recursive(&Nibblet::new());
        ok && length == self.length
    }
}

impl<K, V> PartialEq for Trie<K, V>
where
    K: TrieKey,
    V: PartialEq,
{
    #[inline]
    fn eq(&self, other: &Trie<K, V>) -> bool {
        if self.len() != other.len() {
            return false;
        }

        self.iter()
            .all(|(key, value)| other.get(key).map_or(false, |v| *value == *v))
    }
}

impl<K: TrieKey, V> Default for Trie<K, V> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}
