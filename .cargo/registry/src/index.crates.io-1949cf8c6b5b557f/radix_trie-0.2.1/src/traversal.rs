//! This module contains the core algorithms.

use crate::keys::{match_keys, KeyMatch};
use crate::TrieKey;
use crate::TrieNode;
use std::borrow::Borrow;

use nibble_vec::Nibblet;

use self::DescendantResult::*;

impl<K, V> TrieNode<K, V>
where
    K: TrieKey,
{
    #[inline]
    pub fn get(&self, nv: &Nibblet) -> Option<&TrieNode<K, V>> {
        iterative_get(self, nv)
    }
    #[inline]
    pub fn get_mut(&mut self, nv: &Nibblet) -> Option<&mut TrieNode<K, V>> {
        iterative_get_mut(self, nv)
    }
    #[inline]
    pub fn insert(&mut self, key: K, value: V, nv: Nibblet) -> Option<V> {
        iterative_insert(self, key, value, nv)
    }
    #[inline]
    pub fn remove<Q: ?Sized>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: TrieKey,
    {
        recursive_remove(self, key)
    }
    #[inline]
    pub fn get_ancestor(&self, nv: &Nibblet) -> Option<(&TrieNode<K, V>, usize)> {
        get_ancestor(self, nv)
    }
    #[inline]
    pub fn get_raw_ancestor(&self, nv: &Nibblet) -> (&TrieNode<K, V>, usize) {
        get_raw_ancestor(self, nv)
    }
    #[inline]
    pub fn get_raw_descendant<'a>(&'a self, nv: &Nibblet) -> Option<DescendantResult<'a, K, V>> {
        get_raw_descendant(self, nv)
    }
}

macro_rules! get_func {
    (
        name: $name:ident,
        trie_type: $trie_type:ty,
        mutability: $($mut_:tt)*
    ) => {id!{
        #[inline]
        fn $name<'a, K, V>(trie: $trie_type, nv: &Nibblet) -> Option<$trie_type> {
            if nv.len() == 0 {
                return Some(trie);
            }

            let mut prev = trie;
            let mut depth = 0;

            loop {
                let bucket = nv.get(depth) as usize;
                let current = prev;
                if let Some(ref $($mut_)* child) = current.children[bucket] {
                    match match_keys(depth, nv, &child.key) {
                        KeyMatch::Full => {
                            return Some(child);
                        }
                        KeyMatch::SecondPrefix => {
                            depth += child.key.len();
                            prev = child;
                        }
                        _ => {
                            return None;
                        }
                    }
                } else {
                    return None;
                }
            }
        }
    }}
}

get_func!(name: iterative_get, trie_type: &'a TrieNode<K, V>, mutability: );
get_func!(name: iterative_get_mut, trie_type: &'a mut TrieNode<K, V>, mutability: mut);

#[inline]
fn iterative_insert<K, V>(trie: &mut TrieNode<K, V>, key: K, value: V, mut nv: Nibblet) -> Option<V>
where
    K: TrieKey,
{
    if nv.len() == 0 {
        return trie.replace_value(key, value);
    }

    let mut prev = trie;
    let mut depth = 0;

    loop {
        let bucket = nv.get(depth) as usize;
        let current = prev;
        if let Some(ref mut child) = current.children[bucket] {
            match match_keys(depth, &nv, &child.key) {
                KeyMatch::Full => {
                    return child.replace_value(key, value);
                }
                KeyMatch::Partial(idx) => {
                    // Split the existing child.
                    child.split(idx);

                    // Insert the new key below the prefix node.
                    let new_key = nv.split(depth + idx);
                    let new_key_bucket = new_key.get(0) as usize;

                    child.add_child(
                        new_key_bucket,
                        Box::new(TrieNode::with_key_value(new_key, key, value)),
                    );

                    return None;
                }
                KeyMatch::FirstPrefix => {
                    child.split(nv.len() - depth);
                    child.add_key_value(key, value);
                    return None;
                }
                KeyMatch::SecondPrefix => {
                    depth += child.key.len();
                    prev = child;
                }
            }
        } else {
            let node_key = nv.split(depth);
            current.add_child(
                bucket,
                Box::new(TrieNode::with_key_value(node_key, key, value)),
            );
            return None;
        }
    }
}

// TODO: clean this up and make it iterative.
#[inline]
fn recursive_remove<K, Q: ?Sized, V>(trie: &mut TrieNode<K, V>, key: &Q) -> Option<V>
where
    K: TrieKey,
    K: Borrow<Q>,
    Q: TrieKey,
{
    let nv = key.encode();

    if nv.len() == 0 {
        return trie.take_value(key);
    }

    let bucket = nv.get(0) as usize;

    let child = trie.take_child(bucket);

    match child {
        Some(mut child) => {
            match match_keys(0, &nv, &child.key) {
                KeyMatch::Full => {
                    let result = child.take_value(key);
                    if child.child_count != 0 {
                        // If removing this node's value has made it a value-less node with a
                        // single child, then merge its child.
                        let repl = if child.child_count == 1 {
                            get_merge_child(&mut child)
                        } else {
                            child
                        };
                        trie.add_child(bucket, repl);
                    }
                    result
                }
                KeyMatch::SecondPrefix => {
                    let depth = child.key.len();
                    rec_remove(trie, child, bucket, key, depth, &nv)
                }
                KeyMatch::FirstPrefix | KeyMatch::Partial(_) => {
                    trie.add_child(bucket, child);
                    None
                }
            }
        }
        None => None,
    }
}
#[inline]
fn get_merge_child<K, V>(trie: &mut TrieNode<K, V>) -> Box<TrieNode<K, V>>
where
    K: TrieKey,
{
    let mut child = trie.take_only_child();

    // Join the child's key onto the existing one.
    child.key = trie.key.clone().join(&child.key);

    child
}

// Tail-recursive remove function used by `recursive_remove`.
#[inline]
fn rec_remove<K, Q: ?Sized, V>(
    parent: &mut TrieNode<K, V>,
    mut middle: Box<TrieNode<K, V>>,
    prev_bucket: usize,
    key: &Q,
    depth: usize,
    nv: &Nibblet,
) -> Option<V>
where
    K: TrieKey,
    K: Borrow<Q>,
    Q: TrieKey,
{
    let bucket = nv.get(depth) as usize;

    let child = middle.take_child(bucket);
    parent.add_child(prev_bucket, middle);

    match child {
        Some(mut child) => {
            let middle = parent.children[prev_bucket].as_mut().unwrap();
            match match_keys(depth, nv, &child.key) {
                KeyMatch::Full => {
                    let result = child.take_value(key);

                    // If this node has children, keep it.
                    if child.child_count != 0 {
                        // If removing this node's value has made it a value-less node with a
                        // single child, then merge its child.
                        let repl = if child.child_count == 1 {
                            get_merge_child(&mut *child)
                        } else {
                            child
                        };
                        middle.add_child(bucket, repl);
                    }
                    // Otherwise, if the parent node now only has a single child, merge it.
                    else if middle.child_count == 1 && middle.key_value.is_none() {
                        let repl = get_merge_child(middle);
                        *middle = repl;
                    }

                    result
                }
                KeyMatch::SecondPrefix => {
                    let new_depth = depth + child.key.len();
                    rec_remove(middle, child, bucket, key, new_depth, nv)
                }
                KeyMatch::FirstPrefix | KeyMatch::Partial(_) => {
                    middle.add_child(bucket, child);
                    None
                }
            }
        }
        None => None,
    }
}
#[inline]
fn get_ancestor<'a, K, V>(
    trie: &'a TrieNode<K, V>,
    nv: &Nibblet,
) -> Option<(&'a TrieNode<K, V>, usize)>
where
    K: TrieKey,
{
    if nv.len() == 0 {
        return trie.as_value_node().map(|node| (node, 0));
    }

    let mut prev = trie;
    // The ancestor is such that all nodes upto and including `prev` have
    // already been considered.
    let mut ancestor = prev.as_value_node();
    let mut depth = 0;

    loop {
        let bucket = nv.get(depth) as usize;
        let current = prev;
        if let Some(ref child) = current.children[bucket] {
            match match_keys(depth, nv, &child.key) {
                KeyMatch::Full => {
                    return child
                        .as_value_node()
                        .map(|node| (node, depth + node.key.len()))
                        .or_else(|| ancestor.map(|anc| (anc, depth)));
                }
                KeyMatch::FirstPrefix | KeyMatch::Partial(_) => {
                    return ancestor.map(|anc| (anc, depth));
                }
                KeyMatch::SecondPrefix => {
                    depth += child.key.len();
                    ancestor = child.as_value_node().or(ancestor);
                    prev = child;
                }
            }
        } else {
            return ancestor.map(|anc| (anc, depth));
        }
    }
}
#[inline]
fn get_raw_ancestor<'a, K, V>(trie: &'a TrieNode<K, V>, nv: &Nibblet) -> (&'a TrieNode<K, V>, usize)
where
    K: TrieKey,
{
    if nv.len() == 0 {
        return (trie, 0);
    }

    let mut prev = trie;
    // The ancestor is such that all nodes upto and including `prev` have
    // already been considered.
    let mut ancestor = prev;
    let mut depth = 0;

    loop {
        let bucket = nv.get(depth) as usize;
        let current = prev;
        if let Some(ref child) = current.children[bucket] {
            match match_keys(depth, nv, &child.key) {
                KeyMatch::Full => {
                    return (child, depth + child.key.len());
                }
                KeyMatch::FirstPrefix | KeyMatch::Partial(_) => {
                    return (ancestor, depth);
                }
                KeyMatch::SecondPrefix => {
                    depth += child.key.len();
                    ancestor = child;
                    prev = child;
                }
            }
        } else {
            return (ancestor, depth);
        }
    }
}

// Type used to propogate subtrie construction instructions to the top-level `get_raw_descendant`
// method.
pub enum DescendantResult<'a, K: 'a, V: 'a> {
    NoModification(&'a TrieNode<K, V>),
    ExtendKey(&'a TrieNode<K, V>, usize, &'a Nibblet),
}
#[inline]
fn get_raw_descendant<'a, K, V>(
    trie: &'a TrieNode<K, V>,
    nv: &Nibblet,
) -> Option<DescendantResult<'a, K, V>> {
    if nv.len() == 0 {
        return Some(NoModification(trie));
    }

    let mut prev = trie;
    let mut depth = 0;

    loop {
        let bucket = nv.get(depth) as usize;
        let current = prev;
        if let Some(ref child) = current.children[bucket] {
            match match_keys(depth, nv, &child.key) {
                KeyMatch::Full => {
                    return Some(NoModification(child));
                }
                KeyMatch::FirstPrefix => {
                    return Some(ExtendKey(child, depth, &child.key));
                }
                KeyMatch::SecondPrefix => {
                    depth += child.key.len();
                    prev = child;
                }
                _ => {
                    return None;
                }
            }
        } else {
            return None;
        }
    }
}
