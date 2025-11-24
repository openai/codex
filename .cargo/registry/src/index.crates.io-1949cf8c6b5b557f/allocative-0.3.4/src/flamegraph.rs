/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

use std::collections::hash_map;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt::Write as _;
use std::mem;
use std::ops::Index;
use std::ops::IndexMut;

use crate::global_root::roots;
use crate::key::Key;
use crate::visitor::NodeKind;
use crate::visitor::Visitor;
use crate::visitor::VisitorImpl;
use crate::Allocative;

/// Node in flamegraph tree.
///
/// Can be written to flamegraph format with [`write`](FlameGraph::write).
#[derive(Debug, Default, Clone)]
pub struct FlameGraph {
    children: HashMap<Key, FlameGraph>,
    /// Total size of all children, cached.
    children_size: usize,
    /// Node size excluding children.
    node_size: usize,
}

impl FlameGraph {
    pub fn total_size(&self) -> usize {
        self.node_size + self.children_size
    }

    /// Add another flamegraph to this one.
    pub fn add(&mut self, other: FlameGraph) {
        self.node_size += other.node_size;
        for (key, child) in other.children {
            self.add_child(key, child);
        }
    }

    /// Add a child node to the flamegraph, merging if it already exists.
    pub fn add_child(&mut self, key: Key, child: FlameGraph) {
        self.children_size += child.total_size();
        match self.children.entry(key) {
            hash_map::Entry::Occupied(mut entry) => {
                entry.get_mut().add(child);
            }
            hash_map::Entry::Vacant(entry) => {
                entry.insert(child);
            }
        }
    }

    /// Add size to this node.
    pub fn add_self(&mut self, size: usize) {
        self.node_size += size;
    }

    fn write_flame_graph_impl(&self, stack: &[&str], w: &mut String) {
        if self.node_size != 0 {
            if !stack.is_empty() {
                writeln!(w, "{} {}", stack.join(";"), self.node_size).unwrap();
            } else {
                // Don't care.
            }
        }
        let mut stack = stack.to_vec();
        let mut children = Vec::from_iter(self.children.iter());
        children.sort_by_key(|(key, _)| *key);
        for (key, child) in children {
            stack.push(key);
            child.write_flame_graph_impl(&stack, w);
            stack.pop().unwrap();
        }
    }

    /// Write flamegraph in format suitable for [`flamegraph.pl`] or [inferno].
    ///
    /// [flamegraph.pl]: https://github.com/brendangregg/FlameGraph
    /// [inferno]: https://github.com/jonhoo/inferno
    pub fn write(&self) -> String {
        let mut r = String::new();
        self.write_flame_graph_impl(&[], &mut r);
        r
    }
}

#[derive(Debug)]
pub struct FlameGraphOutput {
    flamegraph: FlameGraph,
    warnings: String,
}

impl FlameGraphOutput {
    /// Flamegraph source, can be fed to `flamegraph.pl` or `inferno`.
    pub fn flamegraph(&self) -> &FlameGraph {
        &self.flamegraph
    }

    /// Warnings. Text file in unspecified format.
    pub fn warnings(&self) -> String {
        self.warnings.clone()
    }
}

#[derive(Default, Eq, PartialEq, Clone, Debug)]
struct TreeData {
    /// Size of this node including children but excluding unique/shared children.
    /// For example for `String` this would be `size_of::<String>()`.
    size: usize,
    /// Size excluding children. This value is output to flamegraph for given stack.
    /// Can be negative if nodes provides sizes incorrectly.
    rem_size: isize,
    /// Whether this node is `Box` something.
    unique: bool,
    /// Child nodes.
    children: HashMap<Key, TreeId>,
}

impl TreeData {
    fn inline_children_size(&self) -> isize {
        self.size as isize - self.rem_size
    }
}

struct TreeRef<'a> {
    trees: &'a Trees,
    tree_id: TreeId,
}

impl<'a> TreeRef<'a> {
    fn write_flame_graph(&self, stack: &[&str], warnings: &mut String) -> FlameGraph {
        let mut flame_graph = FlameGraph::default();
        let tree = &self.trees[self.tree_id];
        if tree.rem_size > 0 {
            if stack.is_empty() {
                // don't care.
            } else {
                flame_graph.node_size = tree.rem_size as usize;
            }
        } else if tree.rem_size < 0 && !stack.is_empty() {
            writeln!(
                warnings,
                "Incorrect size declaration for node `{}`, size of self: {}, size of inline children: {}",
                stack.join(";"),
                tree.size,
                tree.inline_children_size()
            )
                .unwrap();
        }
        let mut children: Vec<(&Key, &TreeId)> = Vec::from_iter(&tree.children);
        let mut stack = stack.to_vec();
        children.sort_by_key(|(k, _)| *k);
        for (key, child) in children {
            stack.push(key);
            let child = TreeRef {
                trees: self.trees,
                tree_id: *child,
            };
            let child_framegraph = child.write_flame_graph(&stack, warnings);
            flame_graph.add_child(key.clone(), child_framegraph);
            stack.pop().unwrap();
        }
        flame_graph
    }

    fn to_flame_graph(&self) -> (FlameGraph, String) {
        let mut warnings = String::new();
        let flame_graph = self.write_flame_graph(&[], &mut warnings);
        (flame_graph, warnings)
    }
}

#[derive(Debug, Eq, PartialEq)]
struct Tree {
    trees: Trees,
    tree_id: TreeId,
}

impl Tree {
    fn as_ref(&self) -> TreeRef {
        TreeRef {
            trees: &self.trees,
            tree_id: self.tree_id,
        }
    }

    fn to_flame_graph(&self) -> (FlameGraph, String) {
        self.as_ref().to_flame_graph()
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
struct TreeId(usize);

#[derive(Default, Clone, Debug, Eq, PartialEq)]
struct Trees {
    trees: Vec<TreeData>,
}

impl Trees {
    fn new_tree(&mut self) -> TreeId {
        let id = TreeId(self.trees.len());
        self.trees.push(TreeData::default());
        id
    }
}

impl Index<TreeId> for Trees {
    type Output = TreeData;

    fn index(&self, index: TreeId) -> &Self::Output {
        &self.trees[index.0]
    }
}

impl IndexMut<TreeId> for Trees {
    fn index_mut(&mut self, index: TreeId) -> &mut Self::Output {
        &mut self.trees[index.0]
    }
}

#[derive(Clone, Debug)]
struct TreeStack {
    stack: Vec<TreeId>,
    tree: TreeId,
}

struct TreeStackRef<'t, 's> {
    trees: &'t mut Trees,
    stack: &'s mut TreeStack,
}

impl<'t, 's> TreeStackRef<'t, 's> {
    fn current_data(&'t mut self) -> &'t mut TreeData {
        &mut self.trees[self.stack.tree]
    }

    fn down(&mut self, key: Key) {
        self.stack.stack.push(self.stack.tree);
        let next_tree_id = TreeId(self.trees.trees.len());
        let child = match self.trees[self.stack.tree].children.entry(key) {
            hash_map::Entry::Occupied(e) => *e.get(),
            hash_map::Entry::Vacant(e) => {
                e.insert(next_tree_id);
                let child = self.trees.new_tree();
                assert_eq!(child, next_tree_id);
                child
            }
        };
        self.stack.tree = child;
    }

    #[must_use]
    fn up(&mut self) -> bool {
        if let Some(pop) = self.stack.stack.pop() {
            self.stack.tree = pop;
            true
        } else {
            false
        }
    }
}

#[derive(Eq, PartialEq, Hash, Clone, Copy, Debug)]
struct VisitedSharedPointer(*const ());
unsafe impl Send for VisitedSharedPointer {}

/// Build a flamegraph from given root objects.
///
/// # Example
///
/// ```
/// use allocative::Allocative;
/// use allocative::FlameGraphBuilder;
///
/// #[derive(Allocative)]
/// struct Foo {
///     data: String,
/// };
///
/// let foo1 = Foo {
///     data: "Hello, world!".to_owned(),
/// };
/// let foo2 = Foo {
///     data: "Another message!".to_owned(),
/// };
///
/// let mut flamegraph = FlameGraphBuilder::default();
/// flamegraph.visit_root(&foo1);
/// flamegraph.visit_root(&foo2);
/// let flamegraph_src = flamegraph.finish().flamegraph();
/// ```
///
/// And now `flamegraph_src` can be fed to either [flamegraph.pl] or [inferno].
///
/// [flamegraph.pl]: https://github.com/brendangregg/FlameGraph
/// [inferno]: https://github.com/jonhoo/inferno
#[derive(Debug)]
pub struct FlameGraphBuilder {
    /// Visited shared pointers.
    visited_shared: HashSet<VisitedSharedPointer>,
    /// Tree data storage.
    trees: Trees,
    /// Current node we are processing in `Visitor`.
    current: TreeStack,
    /// Previous stack when entering shared pointer.
    shared: Vec<TreeStack>,
    /// Data root.
    root: TreeId,
    /// Is root visitor created?
    entered_root_visitor: bool,
}

fn _assert_flame_graph_builder_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<FlameGraphBuilder>();
}

impl Default for FlameGraphBuilder {
    fn default() -> FlameGraphBuilder {
        let mut trees = Trees::default();
        let root = trees.new_tree();
        FlameGraphBuilder {
            trees,
            visited_shared: HashSet::new(),
            current: TreeStack {
                stack: Vec::new(),
                tree: root,
            },
            shared: Vec::new(),
            root,
            entered_root_visitor: false,
        }
    }
}

impl FlameGraphBuilder {
    pub fn root_visitor(&mut self) -> Visitor {
        assert!(!self.entered_root_visitor);
        self.entered_root_visitor = true;
        Visitor {
            visitor: self,
            node_kind: NodeKind::Root,
        }
    }

    /// Collect tree sizes starting from given root.
    pub fn visit_root(&mut self, root: &dyn Allocative) {
        let mut visitor = self.root_visitor();
        root.visit(&mut visitor);
        visitor.exit();
    }

    /// Collect data from global roots registered with
    /// [`register_root`](crate::register_root).
    pub fn visit_global_roots(&mut self) {
        for root in roots() {
            self.visit_root(root);
        }
    }

    fn finish_impl(mut self) -> Tree {
        assert!(self.shared.is_empty());
        assert!(self.current.stack.is_empty());
        assert!(!self.entered_root_visitor);
        Self::update_sizes(self.root, &mut self.trees);
        Tree {
            trees: self.trees,
            tree_id: self.root,
        }
    }

    /// Finish building the flamegraph.
    pub fn finish(self) -> FlameGraphOutput {
        let tree = self.finish_impl();
        let (flamegraph, warnings) = tree.to_flame_graph();
        FlameGraphOutput {
            flamegraph,
            warnings,
        }
    }

    /// Finish building the flamegraph and return the flamegraph output.
    pub fn finish_and_write_flame_graph(self) -> String {
        self.finish().flamegraph.write()
    }

    fn update_sizes(tree_id: TreeId, trees: &mut Trees) {
        let tree = &mut trees[tree_id];
        for child in tree.children.values().copied().collect::<Vec<_>>() {
            Self::update_sizes(child, trees);
        }
        let tree = &mut trees[tree_id];
        let children_size = if tree.unique {
            0
        } else {
            let tree = &trees[tree_id];
            tree.children
                .values()
                .map(|child| trees[*child].size)
                .sum::<usize>()
        };
        let tree = &mut trees[tree_id];
        let size = tree.size;
        tree.rem_size = (size as isize).saturating_sub(children_size as isize);
    }

    fn current(&mut self) -> TreeStackRef {
        TreeStackRef {
            trees: &mut self.trees,
            stack: &mut self.current,
        }
    }

    fn exit_impl(&mut self) {
        assert!(self.entered_root_visitor);

        let up = self.current().up();
        if !up {
            if let Some(shared) = self.shared.pop() {
                self.current = shared;
                assert!(self.current().up());
            } else {
                self.entered_root_visitor = false;
            }
        }
    }
}

impl VisitorImpl for FlameGraphBuilder {
    fn enter_inline_impl(&mut self, name: Key, size: usize, _parent: NodeKind) {
        self.current().down(name);
        self.current().current_data().size += size;
    }

    fn enter_unique_impl(&mut self, name: Key, size: usize, _parent: NodeKind) {
        self.current().down(name);
        self.current().current_data().size += size;
        // TODO: deal with potential issue when node is both unique and not.
        // TODO: record some malloc overhead.
        self.current().current_data().unique = true;
    }

    #[must_use]
    fn enter_shared_impl(
        &mut self,
        name: Key,
        size: usize,
        ptr: *const (),
        _parent: NodeKind,
    ) -> bool {
        self.current().down(name);
        self.current().current_data().size += size;

        if !self.visited_shared.insert(VisitedSharedPointer(ptr)) {
            self.exit_impl();
            return false;
        }

        self.shared.push(mem::replace(
            &mut self.current,
            TreeStack {
                stack: Vec::new(),
                tree: self.root,
            },
        ));
        true
    }

    fn exit_inline_impl(&mut self) {
        self.exit_impl();
    }

    fn exit_unique_impl(&mut self) {
        self.exit_impl();
    }

    fn exit_shared_impl(&mut self) {
        self.exit_impl();
    }

    fn exit_root_impl(&mut self) {
        self.exit_impl();
    }
}

#[cfg(test)]
mod tests {
    use crate::flamegraph::FlameGraphBuilder;
    use crate::flamegraph::Tree;
    use crate::flamegraph::Trees;
    use crate::key::Key;
    use crate::FlameGraph;

    #[test]
    fn test_empty() {
        let mut fg = FlameGraphBuilder::default();
        fg.root_visitor().exit();
        let tree = fg.finish_impl();

        let mut expected_trees = Trees::default();
        let expected_id = expected_trees.new_tree();
        let expected = Tree {
            trees: expected_trees,
            tree_id: expected_id,
        };

        assert_eq!(expected, tree);
        assert_eq!("", tree.to_flame_graph().0.write());
    }

    #[test]
    fn test_simple() {
        let mut fg = FlameGraphBuilder::default();
        fg.root_visitor().visit_simple(Key::new("a"), 10);
        let tree = fg.finish_impl();

        let mut expected = Trees::default();
        let expected_root = expected.new_tree();
        let expected_child = expected.new_tree();
        expected[expected_root].size = 0;
        expected[expected_root].rem_size = -10;
        expected[expected_root]
            .children
            .insert(Key::new("a"), expected_child);
        expected[expected_child].size = 10;
        expected[expected_child].rem_size = 10;
        let expected = Tree {
            trees: expected,
            tree_id: expected_root,
        };
        assert_eq!(expected, tree);
        assert_eq!("a 10\n", tree.to_flame_graph().0.write());
    }

    #[test]
    fn test_unique() {
        let mut fg = FlameGraphBuilder::default();
        let mut visitor = fg.root_visitor();
        let mut s = visitor.enter(Key::new("Struct"), 10);
        s.visit_simple(Key::new("a"), 3);
        let mut un = s.enter_unique(Key::new("p"), 6);
        un.visit_simple(Key::new("x"), 13);
        un.exit();
        s.exit();
        visitor.exit();

        let tree = fg.finish_impl();

        assert_eq!(
            "\
                Struct 1\n\
                Struct;a 3\n\
                Struct;p 6\n\
                Struct;p;x 13\n\
            ",
            tree.to_flame_graph().0.write(),
            "{:#?}",
            tree,
        );
    }

    #[test]
    fn test_shared() {
        let p = 10;

        let mut fg = FlameGraphBuilder::default();
        let mut visitor = fg.root_visitor();

        for _ in 0..2 {
            let mut s = visitor.enter(Key::new("Struct"), 10);
            s.visit_simple(Key::new("a"), 3);
            {
                let sh = s.enter_shared(Key::new("p"), 6, &p as *const i32 as *const ());
                if let Some(mut sh) = sh {
                    sh.visit_simple(Key::new("Shared"), 13);
                    sh.exit();
                }
            }
            s.exit();
        }

        visitor.exit();

        let tree = fg.finish_impl();

        assert_eq!(
            "\
            Shared 13\n\
            Struct 2\n\
            Struct;a 6\n\
            Struct;p 12\n\
        ",
            tree.to_flame_graph().0.write(),
            "{:#?}",
            tree,
        );
    }

    #[test]
    fn test_inline_children_too_large() {
        let mut fg = FlameGraphBuilder::default();
        let mut visitor = fg.root_visitor();
        let mut child_visitor = visitor.enter(Key::new("a"), 10);
        child_visitor.visit_simple(Key::new("b"), 13);
        child_visitor.exit();
        visitor.exit();
        let output = fg.finish();
        assert_eq!("a;b 13\n", output.flamegraph().write());
        assert_eq!(
            "Incorrect size declaration for node `a`, size of self: 10, size of inline children: 13\n",
            output.warnings()
        );
    }

    #[test]
    fn test_flamegraph_add() {
        let mut a = FlameGraph::default();

        let mut b_1 = FlameGraph::default();
        b_1.add_self(10);

        let mut b = FlameGraph::default();
        b.add_child(Key::new("x"), b_1);

        a.add(b);
        assert_eq!(10, a.total_size());
    }
}
