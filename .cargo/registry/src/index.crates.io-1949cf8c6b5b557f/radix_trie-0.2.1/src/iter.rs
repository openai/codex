//! Iterators over key-value pairs, keys, values and child subtries.

use std::iter::{FilterMap, FromIterator, Map};
use std::slice;

use crate::TrieNode;
use crate::{SubTrie, Trie, TrieKey};

use nibble_vec::Nibblet;

// MY EYES.
type Child<K, V> = Box<TrieNode<K, V>>;
type RawChildIter<'a, K, V> = slice::Iter<'a, Option<Child<K, V>>>;
type ChildMapFn<'a, K, V> = fn(&'a Option<Child<K, V>>) -> Option<&'a Child<K, V>>;
type ChildIter<'a, K, V> = FilterMap<RawChildIter<'a, K, V>, ChildMapFn<'a, K, V>>;

/// Iterator over the keys and values of a Trie.
pub struct Iter<'a, K: 'a, V: 'a> {
    root: &'a TrieNode<K, V>,
    root_visited: bool,
    stack: Vec<ChildIter<'a, K, V>>,
}

impl<'a, K, V> Iter<'a, K, V> {
    // TODO: make this private somehow (and same for the other iterators).
    pub fn new(root: &'a TrieNode<K, V>) -> Iter<'a, K, V> {
        Iter {
            root: root,
            root_visited: false,
            stack: vec![],
        }
    }
}

/// Iterator over the keys of a Trie.
pub struct Keys<'a, K: 'a, V: 'a> {
    inner: Map<Iter<'a, K, V>, KeyMapFn<'a, K, V>>,
}

type KeyMapFn<'a, K, V> = fn((&'a K, &'a V)) -> &'a K;

impl<'a, K, V> Keys<'a, K, V> {
    pub fn new(iter: Iter<'a, K, V>) -> Keys<'a, K, V> {
        fn first<'b, K, V>((k, _): (&'b K, &'b V)) -> &'b K {
            k
        }
        Keys {
            inner: iter.map(first),
        }
    }
}

impl<'a, K, V> Iterator for Keys<'a, K, V> {
    type Item = &'a K;

    fn next(&mut self) -> Option<&'a K> {
        self.inner.next()
    }
}

/// Iterator over the values of a Trie.
pub struct Values<'a, K: 'a, V: 'a> {
    inner: Map<Iter<'a, K, V>, ValueMapFn<'a, K, V>>,
}

type ValueMapFn<'a, K, V> = fn((&'a K, &'a V)) -> &'a V;

impl<'a, K, V> Values<'a, K, V> {
    pub fn new(iter: Iter<'a, K, V>) -> Values<'a, K, V> {
        fn second<'b, K, V>((_, v): (&'b K, &'b V)) -> &'b V {
            v
        }
        Values {
            inner: iter.map(second),
        }
    }
}

impl<'a, K, V> Iterator for Values<'a, K, V> {
    type Item = &'a V;

    fn next(&mut self) -> Option<&'a V> {
        self.inner.next()
    }
}

/// Iterator over the child subtries of a trie.
pub struct Children<'a, K: 'a, V: 'a> {
    prefix: Nibblet,
    inner: ChildIter<'a, K, V>,
}

impl<'a, K, V> Children<'a, K, V> {
    pub fn new(key: Nibblet, node: &'a TrieNode<K, V>) -> Self {
        Children {
            prefix: key,
            inner: node.child_iter(),
        }
    }
}

impl<'a, K, V> Iterator for Children<'a, K, V> {
    type Item = SubTrie<'a, K, V>;

    fn next(&mut self) -> Option<SubTrie<'a, K, V>> {
        self.inner.next().map(|node| SubTrie {
            prefix: self.prefix.clone().join(&node.key),
            node: node,
        })
    }
}

impl<K, V> TrieNode<K, V> {
    /// Helper function to get all the non-empty children of a node.
    fn child_iter(&self) -> ChildIter<K, V> {
        fn id<K, V>(x: &Option<Child<K, V>>) -> Option<&Child<K, V>> {
            x.as_ref()
        }

        self.children.iter().filter_map(id)
    }

    /// Get the key and value of a node as a pair.
    fn kv_as_pair(&self) -> Option<(&K, &V)> {
        self.key_value.as_ref().map(|kv| (&kv.key, &kv.value))
    }
}

enum IterAction<'a, K: 'a, V: 'a> {
    Push(&'a TrieNode<K, V>),
    Pop,
}

impl<'a, K, V> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        use self::IterAction::*;

        // Visit each node as it is reached from its parent (with special root handling).
        if !self.root_visited {
            self.root_visited = true;
            self.stack.push(self.root.child_iter());
            if let Some(kv) = self.root.kv_as_pair() {
                return Some(kv);
            }
        }

        loop {
            let action = match self.stack.last_mut() {
                Some(stack_top) => match stack_top.next() {
                    Some(child) => Push(child),
                    None => Pop,
                },
                None => return None,
            };

            match action {
                Push(trie) => {
                    self.stack.push(trie.child_iter());
                    if let Some(kv) = trie.kv_as_pair() {
                        return Some(kv);
                    }
                }
                Pop => {
                    self.stack.pop();
                }
            }
        }
    }
}

impl<K, V> FromIterator<(K, V)> for Trie<K, V>
where
    K: TrieKey,
{
    fn from_iter<T>(iter: T) -> Trie<K, V>
    where
        T: IntoIterator<Item = (K, V)>,
    {
        let mut trie = Trie::new();
        for (k, v) in iter {
            trie.insert(k, v);
        }
        trie
    }
}
