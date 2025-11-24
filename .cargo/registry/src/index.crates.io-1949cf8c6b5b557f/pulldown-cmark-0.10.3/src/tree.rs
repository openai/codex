// Copyright 2018 Google LLC
//
// Use of this source code is governed by an MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT.

//! A Vec-based container for a tree structure.

use std::num::NonZeroUsize;
use std::ops::{Add, Sub};

use crate::parse::{Item, ItemBody};

#[derive(Debug, Eq, PartialEq, Copy, Clone, PartialOrd)]
pub(crate) struct TreeIndex(NonZeroUsize);

impl TreeIndex {
    fn new(i: usize) -> Self {
        TreeIndex(NonZeroUsize::new(i).unwrap())
    }

    pub fn get(self) -> usize {
        self.0.get()
    }
}

impl Add<usize> for TreeIndex {
    type Output = TreeIndex;

    fn add(self, rhs: usize) -> Self {
        let inner = self.0.get() + rhs;
        TreeIndex::new(inner)
    }
}

impl Sub<usize> for TreeIndex {
    type Output = TreeIndex;

    fn sub(self, rhs: usize) -> Self {
        let inner = self.0.get().checked_sub(rhs).unwrap();
        TreeIndex::new(inner)
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Node<T> {
    pub child: Option<TreeIndex>,
    pub next: Option<TreeIndex>,
    pub item: T,
}

/// A tree abstraction, intended for fast building as a preorder traversal.
#[derive(Clone)]
pub(crate) struct Tree<T> {
    nodes: Vec<Node<T>>,
    spine: Vec<TreeIndex>, // indices of nodes on path to current node
    cur: Option<TreeIndex>,
}

impl<T: Default> Tree<T> {
    // Indices start at one, so we place a dummy value at index zero.
    // The alternative would be subtracting one from every TreeIndex
    // every time we convert it to usize to index our nodes.
    pub(crate) fn with_capacity(cap: usize) -> Tree<T> {
        let mut nodes = Vec::with_capacity(cap);
        nodes.push(Node {
            child: None,
            next: None,
            item: <T as Default>::default(),
        });
        Tree {
            nodes,
            spine: Vec::new(),
            cur: None,
        }
    }

    /// Returns the index of the element currently in focus.
    pub(crate) fn cur(&self) -> Option<TreeIndex> {
        self.cur
    }

    /// Append one item to the current position in the tree.
    pub(crate) fn append(&mut self, item: T) -> TreeIndex {
        let ix = self.create_node(item);
        let this = Some(ix);

        if let Some(ix) = self.cur {
            self[ix].next = this;
        } else if let Some(&parent) = self.spine.last() {
            self[parent].child = this;
        }
        self.cur = this;
        ix
    }

    /// Create an isolated node.
    pub(crate) fn create_node(&mut self, item: T) -> TreeIndex {
        let this = self.nodes.len();
        self.nodes.push(Node {
            child: None,
            next: None,
            item,
        });
        TreeIndex::new(this)
    }

    /// Push down one level, so that new items become children of the current node.
    /// The new focus index is returned.
    pub(crate) fn push(&mut self) -> TreeIndex {
        let cur_ix = self.cur.unwrap();
        self.spine.push(cur_ix);
        self.cur = self[cur_ix].child;
        cur_ix
    }

    /// Pop back up a level.
    pub(crate) fn pop(&mut self) -> Option<TreeIndex> {
        let ix = Some(self.spine.pop()?);
        self.cur = ix;
        ix
    }

    /// Remove the last node, as `pop` but removing it.
    pub(crate) fn remove_node(&mut self) -> Option<TreeIndex> {
        let ix = self.spine.pop()?;
        self.cur = Some(ix);
        self.nodes.pop()?;
        self[ix].child = None;
        Some(ix)
    }

    /// Look at the parent node.
    pub(crate) fn peek_up(&self) -> Option<TreeIndex> {
        self.spine.last().copied()
    }

    /// Look at grandparent node.
    pub(crate) fn peek_grandparent(&self) -> Option<TreeIndex> {
        if self.spine.len() >= 2 {
            Some(self.spine[self.spine.len() - 2])
        } else {
            None
        }
    }

    /// Returns true when there are no nodes other than the root node
    /// in the tree, false otherwise.
    pub(crate) fn is_empty(&self) -> bool {
        self.nodes.len() <= 1
    }

    /// Returns the length of the spine.
    pub(crate) fn spine_len(&self) -> usize {
        self.spine.len()
    }

    /// Resets the focus to the first node added to the tree, if it exists.
    pub(crate) fn reset(&mut self) {
        self.cur = if self.is_empty() {
            None
        } else {
            Some(TreeIndex::new(1))
        };
        self.spine.clear();
    }

    /// Walks the spine from a root node up to, but not including, the current node.
    pub(crate) fn walk_spine(&self) -> impl std::iter::DoubleEndedIterator<Item = &TreeIndex> {
        self.spine.iter()
    }

    /// Moves focus to the next sibling of the given node.
    pub(crate) fn next_sibling(&mut self, cur_ix: TreeIndex) -> Option<TreeIndex> {
        self.cur = self[cur_ix].next;
        self.cur
    }
}

impl Tree<Item> {
    /// Truncates the preceding siblings to the given end position,
    /// and returns the new current node.
    pub(crate) fn truncate_siblings(&mut self, end_byte_ix: usize) {
        let parent_ix = self.peek_up().unwrap();
        let mut next_child_ix = self[parent_ix].child;
        let mut prev_child_ix = None;

        // drop or truncate children based on its range
        while let Some(child_ix) = next_child_ix {
            let child_end = self[child_ix].item.end;
            if child_end < end_byte_ix {
                // preserve this node, and go to the next
                prev_child_ix = Some(child_ix);
                next_child_ix = self[child_ix].next;
                continue;
            } else if child_end == end_byte_ix {
                // this will be the last node
                self[child_ix].next = None;
                // focus to the new last child (this node)
                self.cur = Some(child_ix);
            } else if self[child_ix].item.start == end_byte_ix {
                // check whether the previous character is a backslash
                let is_previous_char_backslash_escape = match self[child_ix].item.body {
                    ItemBody::Text { backslash_escaped } => backslash_escaped,
                    _ => false,
                };
                if is_previous_char_backslash_escape {
                    // rescue the backslash as a plain text content
                    let last_byte_ix = end_byte_ix - 1;
                    self[child_ix].item.start = last_byte_ix;
                    self[child_ix].item.end = end_byte_ix;
                    self.cur = Some(child_ix);
                } else if let Some(prev_child_ix) = prev_child_ix {
                    // the node will become empty. drop the node
                    // a preceding sibling exists
                    self[prev_child_ix].next = None;
                    self.cur = Some(prev_child_ix);
                } else {
                    // no preceding siblings. remove the node from the parent
                    self[parent_ix].child = None;
                    self.cur = None;
                }
            } else {
                debug_assert!(self[child_ix].item.start < end_byte_ix);
                debug_assert!(end_byte_ix < child_end);
                // truncate the node
                self[child_ix].item.end = end_byte_ix;
                self[child_ix].next = None;
                // focus to the new last child
                self.cur = Some(child_ix);
            }
            break;
        }
    }
}

impl<T> std::fmt::Debug for Tree<T>
where
    T: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fn debug_tree<T>(
            tree: &Tree<T>,
            cur: TreeIndex,
            indent: usize,
            f: &mut std::fmt::Formatter<'_>,
        ) -> std::fmt::Result
        where
            T: std::fmt::Debug,
        {
            for _ in 0..indent {
                write!(f, "  ")?;
            }
            writeln!(f, "{:?}", &tree[cur].item)?;
            if let Some(child_ix) = tree[cur].child {
                debug_tree(tree, child_ix, indent + 1, f)?;
            }
            if let Some(next_ix) = tree[cur].next {
                debug_tree(tree, next_ix, indent, f)?;
            }
            Ok(())
        }

        if self.nodes.len() > 1 {
            let cur = TreeIndex(NonZeroUsize::new(1).unwrap());
            debug_tree(self, cur, 0, f)
        } else {
            write!(f, "Empty tree")
        }
    }
}

impl<T> std::ops::Index<TreeIndex> for Tree<T> {
    type Output = Node<T>;

    fn index(&self, ix: TreeIndex) -> &Self::Output {
        self.nodes.index(ix.get())
    }
}

impl<T> std::ops::IndexMut<TreeIndex> for Tree<T> {
    fn index_mut(&mut self, ix: TreeIndex) -> &mut Node<T> {
        self.nodes.index_mut(ix.get())
    }
}
