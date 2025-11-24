use std::cmp::min;
use std::collections::BTreeMap;
use std::ops::{Index, IndexMut};

use crate::graph::{Graph, Node, NodeId};

#[derive(Debug)]
pub struct Meta {
    map: BTreeMap<NodeId, MetaItem>,
}

#[derive(Debug, Default)]
pub struct MetaItem {
    /// Number of references to this node
    pub refcount: usize,
    /// Minimum number of bytes that ought to be read for this
    /// node to find a match
    pub min_read: usize,
    /// Marks whether or not this node leads to a loop entry node.
    pub is_loop_init: bool,
    /// Ids of other nodes that point to this node while this
    /// node is on a stack (creating a loop)
    pub loop_entry_from: Vec<NodeId>,
}

impl Index<NodeId> for Meta {
    type Output = MetaItem;

    fn index(&self, id: NodeId) -> &MetaItem {
        &self.map[&id]
    }
}

impl IndexMut<NodeId> for Meta {
    fn index_mut(&mut self, id: NodeId) -> &mut MetaItem {
        self.map.entry(id).or_default()
    }
}

impl MetaItem {
    fn loop_entry(&mut self, id: NodeId) {
        if let Err(idx) = self.loop_entry_from.binary_search(&id) {
            self.loop_entry_from.insert(idx, id);
        }
    }
}

impl Meta {
    pub fn analyze<T>(root: NodeId, graph: &Graph<T>) -> Self {
        let mut meta = Meta {
            map: Default::default(),
        };

        meta.first_pass(root, root, graph, &mut Vec::new());

        meta
    }

    pub fn first_pass<T>(
        &mut self,
        this: NodeId,
        parent: NodeId,
        graph: &Graph<T>,
        stack: &mut Vec<NodeId>,
    ) -> &MetaItem {
        let meta = &mut self[this];
        let is_done = meta.refcount > 0;

        meta.refcount += 1;

        if stack.contains(&this) {
            meta.loop_entry(parent);
            self[parent].is_loop_init = true;
        }
        if is_done {
            return &self[this];
        }

        stack.push(this);

        let mut min_read;

        match &graph[this] {
            Node::Fork(fork) => {
                min_read = usize::max_value();
                for (_, id) in fork.branches() {
                    let meta = self.first_pass(id, this, graph, stack);

                    if meta.is_loop_init {
                        min_read = 1;
                    } else {
                        min_read = min(min_read, meta.min_read + 1);
                    }
                }
                if let Some(id) = fork.miss {
                    let meta = self.first_pass(id, this, graph, stack);

                    if meta.is_loop_init {
                        min_read = 0;
                    } else {
                        min_read = min(min_read, meta.min_read);
                    }
                }
                if min_read == usize::max_value() {
                    min_read = 0;
                }
            }
            Node::Rope(rope) => {
                min_read = rope.pattern.len();
                let meta = self.first_pass(rope.then, this, graph, stack);

                if !meta.is_loop_init {
                    min_read += meta.min_read;
                }

                if let Some(id) = rope.miss.first() {
                    let meta = self.first_pass(id, this, graph, stack);

                    if meta.is_loop_init {
                        min_read = 0;
                    } else {
                        min_read = min(min_read, meta.min_read);
                    }
                }
            }
            Node::Leaf(_) => min_read = 0,
        }

        stack.pop();

        let meta = &mut self[this];
        meta.min_read = min_read;
        let second_pass = meta.loop_entry_from.clone();

        for id in second_pass {
            self.meta_second_pass(id, graph);
        }

        &self[this]
    }

    fn meta_second_pass<T>(&mut self, id: NodeId, graph: &Graph<T>) {
        let mut min_read;

        match &graph[id] {
            Node::Fork(fork) => {
                min_read = usize::max_value();
                for (_, id) in fork.branches() {
                    let meta = &self[id];

                    if meta.is_loop_init {
                        min_read = 1;
                    } else {
                        min_read = min(min_read, meta.min_read + 1);
                    }
                }
                if min_read == usize::max_value() {
                    min_read = 0;
                }
            }
            Node::Rope(rope) => {
                min_read = rope.pattern.len();
                let meta = &self[rope.then];

                if !meta.is_loop_init {
                    min_read += meta.min_read;
                }
            }
            Node::Leaf(_) => unreachable!(),
        }

        self[id].min_read = min_read;
    }
}
