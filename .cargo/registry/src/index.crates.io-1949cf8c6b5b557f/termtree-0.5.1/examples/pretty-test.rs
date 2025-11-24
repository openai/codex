//! Make the output of `cargo test` prettier in tree style.
//! To see the complete demo coupling with rust-script, you can refer to
//! <https://users.rust-lang.org/t/cargo-test-output-with-indentation/100149/2>

use std::collections::{btree_map::Entry, BTreeMap};
use termtree::{GlyphPalette, Tree};

fn main() {
    let text = "
test a::b::c ... FAILED
test a::d ... ok
test works ... ok
";
    assert_eq!(
        pretty_test(text.trim().lines()).unwrap().to_string(),
        "\
test
├── a
│   ├── b
│   │   └─ ❌ c
│   └─ ✅ d
└─ ✅ works\n"
    );
}

#[derive(Debug)]
enum Node<'s> {
    Path(BTreeMap<&'s str, Node<'s>>),
    Status(&'s str),
}

fn pretty_test<'s>(lines: impl Iterator<Item = &'s str>) -> Option<Tree<&'s str>> {
    let mut path = BTreeMap::new();
    for line in lines {
        let mut iter = line.splitn(3, ' ');
        let mut split = iter.nth(1)?.split("::");
        let next = split.next();
        let status = iter.next()?;
        make_node(split, status, &mut path, next);
    }
    let mut tree = Tree::new("test");
    for (root, child) in path {
        make_tree(root, &child, &mut tree);
    }
    Some(tree)
}

// Add paths to Node
fn make_node<'s>(
    mut split: impl Iterator<Item = &'s str>,
    status: &'s str,
    path: &mut BTreeMap<&'s str, Node<'s>>,
    key: Option<&'s str>,
) {
    let Some(key) = key else { return };
    let next = split.next();
    match path.entry(key) {
        Entry::Vacant(empty) => {
            if next.is_some() {
                let mut btree = BTreeMap::new();
                make_node(split, status, &mut btree, next);
                empty.insert(Node::Path(btree));
            } else {
                empty.insert(Node::Status(status));
            }
        }
        Entry::Occupied(mut btree) => {
            if let Node::Path(btree) = btree.get_mut() {
                make_node(split, status, btree, next);
            }
        }
    }
}

// Add Node to Tree
fn make_tree<'s>(root: &'s str, node: &Node<'s>, parent: &mut Tree<&'s str>) {
    match node {
        Node::Path(btree) => {
            let mut t = Tree::new(root);
            for (path, child) in btree {
                make_tree(path, child, &mut t);
            }
            parent.push(t);
        }
        Node::Status(s) => {
            parent.push(Tree::new(root).with_glyphs(set_status(s)));
        }
    }
}

// Display with a status icon
fn set_status(status: &str) -> GlyphPalette {
    let mut glyph = GlyphPalette::new();
    glyph.item_indent = if status.ends_with("ok") {
        "─ ✅ "
    } else {
        "─ ❌ "
    };
    glyph
}
