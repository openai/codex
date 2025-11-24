use crate::keys::*;
use crate::TrieNode;
use crate::{SubTrie, SubTrieMut, SubTrieResult};
use std::borrow::Borrow;

use nibble_vec::Nibblet;

impl<'a, K, V> SubTrie<'a, K, V>
where
    K: TrieKey,
{
    /// Look up the value for the given key, which should be an extension of this subtrie's key.
    ///
    /// The key may be any borrowed form of the trie's key type, but TrieKey on the borrowed
    /// form *must* match those for the key type
    pub fn get<Q: ?Sized>(&self, key: &Q) -> SubTrieResult<&V>
    where
        K: Borrow<Q>,
        Q: TrieKey,
    {
        subtrie_get(&self.prefix, self.node, key)
    }
}

fn subtrie_get<'a, K, Q: ?Sized, V>(
    prefix: &Nibblet,
    node: &'a TrieNode<K, V>,
    key: &Q,
) -> SubTrieResult<&'a V>
where
    K: TrieKey,
    K: Borrow<Q>,
    Q: TrieKey,
{
    let key_enc = key.encode();
    match match_keys(0, prefix, &key_enc) {
        KeyMatch::Full => Ok(node.value()),
        KeyMatch::FirstPrefix => Ok(node
            .get(&stripped(key_enc, prefix))
            .and_then(TrieNode::value)),
        _ => Err(()),
    }
}

impl<'a, K, V> SubTrieMut<'a, K, V>
where
    K: TrieKey,
{
    /// Mutable reference to the node's value.
    pub fn value_mut(&mut self) -> Option<&mut V> {
        self.node.value_mut()
    }

    /// Look up the value for the given key, which should be an extension of this subtrie's key.
    ///
    /// The key may be any borrowed form of the trie's key type, but TrieKey on the borrowed
    /// form *must* match those for the key type
    pub fn get<Q: ?Sized>(&self, key: &Q) -> SubTrieResult<&V>
    where
        K: Borrow<Q>,
        Q: TrieKey,
    {
        subtrie_get(&self.prefix, &*self.node, key)
    }

    /// Insert a value in this subtrie. The key should be an extension of this subtrie's key.
    pub fn insert(&mut self, key: K, value: V) -> SubTrieResult<V> {
        let key_enc = key.encode();
        let previous = match match_keys(0, &self.prefix, &key_enc) {
            KeyMatch::Full => self.node.replace_value(key, value),
            KeyMatch::FirstPrefix => self
                .node
                .insert(key, value, stripped(key_enc, &self.prefix)),
            _ => {
                return Err(());
            }
        };

        if previous.is_none() {
            *self.length += 1;
        }

        Ok(previous)
    }

    /// Remove a value from this subtrie. The key should be an extension of this subtrie's key.
    ///
    /// The key may be any borrowed form of the trie's key type, but TrieKey on the borrowed
    /// form *must* match those for the key type
    pub fn remove<Q: ?Sized>(&mut self, key: &Q) -> SubTrieResult<V>
    where
        K: Borrow<Q>,
        Q: TrieKey,
    {
        let key_enc = key.encode();
        let removed = match match_keys(0, &self.prefix, &key_enc) {
            KeyMatch::Full => self.node.take_value(key),
            KeyMatch::FirstPrefix => self.node.remove(key),
            _ => {
                return Err(());
            }
        };

        if removed.is_some() {
            *self.length -= 1;
        }

        Ok(removed)
    }
}

fn stripped(mut key: Nibblet, prefix: &Nibblet) -> Nibblet {
    key.split(prefix.len())
}
