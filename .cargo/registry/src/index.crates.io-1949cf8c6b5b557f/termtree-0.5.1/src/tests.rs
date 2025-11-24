use snapbox::assert_data_eq;
use snapbox::str;

use super::*;

#[test]
fn render_tree_root() {
    let tree = Tree::new("foo");
    assert_data_eq!(
        format!("{}", tree),
        str![[r#"
foo

"#]]
    );
}

#[test]
fn render_tree_with_leaves() {
    let tree = Tree::new("foo").with_leaves([Tree::new("bar").with_leaves(["baz"])]);
    assert_data_eq!(
        format!("{}", tree),
        str![[r#"
foo
└── bar
    └── baz

"#]]
    );
}

#[test]
fn render_tree_with_multiple_leaves() {
    let tree = Tree::new("foo").with_leaves(["bar", "baz"]);
    assert_data_eq!(
        format!("{}", tree),
        str![[r#"
foo
├── bar
└── baz

"#]]
    );
}

#[test]
fn render_tree_with_multiline_leaf() {
    let tree = Tree::new("foo").with_leaves([
        Tree::new("hello\nworld").with_multiline(true),
        Tree::new("goodbye\nworld").with_multiline(true),
    ]);
    assert_data_eq!(
        format!("{}", tree),
        str![[r#"
foo
├── hello
│   world
└── goodbye
    world

"#]]
    );
}

#[test]
fn render_custom_glyphs() {
    let root = GlyphPalette {
        middle_item: "[mid  ]",
        last_item: "[last ]",
        item_indent: "[indent ]",

        middle_skip: "[mskip]",
        last_skip: "[lskip]",
        skip_indent: "[iskip  ]",
    };
    let middle = GlyphPalette {
        middle_item: "(mid  )",
        last_item: "(last )",
        item_indent: "(indent )",

        middle_skip: "(mskip)",
        last_skip: "(lskip)",
        skip_indent: "(iskip  )",
    };

    let tree = Tree::new("node 1").with_glyphs(root).with_leaves([
        Tree::new("node 1.1"),
        Tree::new("node 1.2"),
        Tree::new("node 1.3").with_leaves([
            Tree::new("node 1.3.1").with_glyphs(middle),
            Tree::new("node 1.3.2").with_glyphs(middle),
            Tree::new("node 1.3.3")
                .with_glyphs(middle)
                .with_leaves(["node 1.3.3.1", "node 1.3.3.2"]),
        ]),
        Tree::new("node 1.4").with_leaves([
            Tree::new("node 1.4.1"),
            Tree::new("node 1.4.2"),
            Tree::new("node 1.4.3").with_leaves(["node 1.4.3.1", "node 1.4.3.2"]),
        ]),
    ]);
    assert_data_eq!(
        format!("{}", tree),
        str![[r#"
node 1
[mid  ][indent ]node 1.1
[mid  ][indent ]node 1.2
[mid  ][indent ]node 1.3
[mskip][iskip  ](mid  )(indent )node 1.3.1
[mskip][iskip  ](mid  )(indent )node 1.3.2
[mskip][iskip  ](last )(indent )node 1.3.3
[mskip][iskip  ](lskip)(iskip  )(mid  )(indent )node 1.3.3.1
[mskip][iskip  ](lskip)(iskip  )(last )(indent )node 1.3.3.2
[last ][indent ]node 1.4
[lskip][iskip  ][mid  ][indent ]node 1.4.1
[lskip][iskip  ][mid  ][indent ]node 1.4.2
[lskip][iskip  ][last ][indent ]node 1.4.3
[lskip][iskip  ][lskip][iskip  ][mid  ][indent ]node 1.4.3.1
[lskip][iskip  ][lskip][iskip  ][last ][indent ]node 1.4.3.2

"#]]
    );
}
