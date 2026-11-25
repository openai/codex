#![cfg(test)]

use super::*;
use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::buffer::CellWidth;
use ratatui::layout::Rect;
use ratatui::layout::Size;
use serde_json::Value;
use serde_json::json;
use syntect::highlighting::Theme;

fn parse(value: Value) -> serde_json::Result<UiTree> {
    value.to_string().parse()
}

fn tree(value: Value) -> PreparedUiTree {
    parse(value).unwrap().prepare()
}

#[test]
fn elements_round_trip_and_require_explicit_row_spacing() {
    let value = json!({"type":"row","props":{"gap":1},"children":[{"type":"link","props":{"href":"https://example.com/pr/42","title":"PR details"},"children":["12%"]}, null, " "]});
    assert_eq!(
        serde_json::to_value(parse(value.clone()).unwrap()).unwrap(),
        value
    );
    for href in [
        "relative/path",
        "file:///tmp/example",
        "https://example.com/\u{001b}",
    ] {
        assert!(parse(json!({"type":"link","props":{"href":href},"children":[]})).is_err());
    }
    for (gap, expected) in [
        (0, "AB"),
        (1, "A B"),
        (2, "A  B"),
        (MAX_SAFE_INTEGER, "A  …"),
    ] {
        let row = tree(json!({"type":"row","props":{"gap":gap},"children":["A","B"]}));
        assert_eq!(painted_rows(&row, Size::new(4, 1)), [expected]);
    }
}

#[test]
fn code_accepts_inline_children_and_counts_them_toward_tree_limits() {
    let code = |children: Value| json!({"type":"code","children":children});
    let literal = code(json!(["cargo test"]));
    for children in [
        json!([]),
        json!(["cargo ", null, {"type":"bold","children":[" test"]}]),
    ] {
        let element = code(children);
        assert_eq!(
            serde_json::to_value(parse(element.clone()).unwrap()).unwrap(),
            element
        );
    }
    assert!(parse(code(json!("cargo test"))).is_err());
    assert!(parse(json!({"type":"code","props":{},"children":["x"]})).is_err());
    assert!(parse(code(json!(["x".repeat(MAX_TEXT_BYTES)]))).is_ok());
    assert!(parse(code(json!(["x".repeat(MAX_TEXT_BYTES + 1)]))).is_err());

    let mut children = vec![literal.clone(); 127];
    children.push(json!(""));
    let group = |items: &[Value]| json!({"type":"text","children":items});
    assert!(parse(group(&children)).is_ok());
    children.push(json!(""));
    assert!(parse(group(&children)).is_err());
    let nested = (0..15).fold(literal, |child, _| group(&[child]));
    assert!(parse(nested.clone()).is_ok());
    assert!(parse(group(&[nested])).is_err());
}

#[test]
fn inline_code_preserves_literal_spacing_and_wraps_in_the_text_flow() {
    let node = tree(json!({"type":"text","children":[
        "run ", {"type":"code","children":["printf  '%s'\t`ok`"]}, " now"
    ]}));
    insta::assert_snapshot!(painted_rows(&node, Size::new(40, 1)).join("\n"), @"run printf  '%s'        `ok` now");
    let narrow = tree(json!({"type":"code","children":["foo  bar"]}));
    assert_eq!(narrow.measure(Size::new(5, 2)), Size::new(5, 2));
    assert_eq!(painted_rows(&narrow, Size::new(5, 2)), ["foo", "bar"]);
}

#[test]
fn prose_before_code_can_wrap_before_a_preserved_space() {
    let node = tree(json!({"type":"text","children":[
        "last run ", {"type":"code","children":[{"type":"bold","children":[" pending"]}]}
    ]}));
    assert_eq!(
        painted_rows(&node, Size::new(8, 2)),
        ["last run", " pending"]
    );
    assert_eq!(painted_rows(&node, Size::new(17, 1)), ["last run  pending"]);
}

#[test]
fn punctuation_can_wrap_at_the_start_of_a_code_span() {
    let node = tree(json!({"type":"text","children":[
        {"type":"code","children":["ci-#12"]}, "-", {"type":"code","children":["#34567"]}
    ]}));
    assert_eq!(painted_rows(&node, Size::new(7, 2)), ["ci-#12-", "#34567"]);
}

#[test]
fn literal_space_between_sibling_code_spans_can_start_a_line() {
    let node = tree(json!({"type":"text","children":[
        {"type":"code","children":["fix-#123"]},
        {"type":"code","children":[{"type":"bold","children":[" pending"]}]}
    ]}));
    assert_eq!(
        painted_rows(&node, Size::new(8, 2)),
        ["fix-#123", " pending"]
    );
    let node = tree(json!({"type":"code","children":[
        {"type":"bold","children":["fix-#123"]},
        {"type":"bold","children":[" pending"]}
    ]}));
    assert_eq!(
        painted_rows(&node, Size::new(8, 3)),
        ["fix-", "#123", "pending"]
    );
}

#[test]
fn prose_after_code_keeps_one_separator_until_the_line_wraps() {
    for suffix in [" b", "  \t b"] {
        let node = tree(json!({"type":"text","children":[
            {"type":"code","children":["a "]}, suffix
        ]}));
        assert_eq!(painted_rows(&node, Size::new(4, 2)), ["a  b"]);
        let bounds = Size::new(3, 2);
        assert_eq!(painted_rows(&node, bounds), ["a", "b"]);
        assert_eq!(node.measure(bounds), Size::new(2, 2));
    }
}

#[test]
fn inline_code_keeps_spaces_and_wraps_across_styled_arguments() {
    use ratatui::style::Modifier;

    let code = |children: Value| tree(json!({"type":"code","children":children}));
    let joined = code(json!(["cargo  test"]));
    let split = code(json!(["cargo ", " test"]));
    let styled = code(json!(["cargo ", {"type":"bold","children":[" test"]}]));
    for width in 1..=11 {
        let bounds = Size::new(width, 11);
        let expected = painted_rows(&joined, bounds);
        for node in [&split, &styled] {
            assert_eq!(node.measure(bounds), joined.measure(bounds));
            assert_eq!(painted_rows(node, bounds), expected, "width {width}");
        }
    }
    let area = Rect::new(0, 0, 11, 1);
    insta::assert_snapshot!(painted_rows(&styled, area.as_size()).join("\n"), @"cargo  test");
    let mut buf = Buffer::empty(area);
    styled.render(area, &Theme::default(), &mut buf, /*plain_style*/ None);
    for x in 0..area.width {
        assert_eq!(buf[(x, 0)].modifier.contains(Modifier::BOLD), x >= 6);
    }
}

#[test]
fn inline_code_uses_the_subdued_fallback_for_translucent_highlights() {
    let node = tree(json!({"type":"code","children":["x"]}));
    let area = Rect::new(0, 0, 1, 1);
    let background = |highlight: &str| {
        let mut theme = Theme::default();
        theme.settings.line_highlight = Some(highlight.parse().unwrap());
        let mut buf = Buffer::empty(area);
        node.render(area, &theme, &mut buf, /*plain_style*/ None);
        buf[(0, 0)].bg
    };
    let palette = crate::terminal_probe::DefaultColors {
        fg: (255, 255, 255),
        bg: (0, 0, 0),
    };
    crate::terminal_palette::with_test_default_colors(palette, || {
        assert_eq!(
            background("#ffffff24"),
            crate::style::user_message_style().bg.unwrap()
        );
        assert_eq!(
            background("#ffffff"),
            crate::terminal_palette::rgb_color((255, 255, 255))
        );
    });
}

#[test]
fn preserved_text_keeps_spacing_across_inline_styles_and_wraps() {
    let preserved = |children: Value| {
        tree(json!({
            "type":"text","props":{"preserveWhitespace":true},"children":children
        }))
    };
    let mixed = tree(json!({"type":"text","children":[
        "last run ", {"type":"text","props":{"preserveWhitespace":true},"children":[" pending"]}
    ]}));
    assert_eq!(
        painted_rows(&mixed, Size::new(8, 2)),
        ["last run", " pending"]
    );
    let joined = preserved(json!(["  build  ready\t!"]));
    let styled = preserved(json!(["  build ", {"type":"bold","children":[
        {"type":"text","props":{"preserveWhitespace":false},"children":[" ready"]}
    ]}, "\t!"]));
    for width in 1..=18 {
        let bounds = Size::new(width, 20);
        assert_eq!(painted_rows(&styled, bounds), painted_rows(&joined, bounds));
    }
    let area = Rect::new(0, 0, 18, 1);
    insta::assert_snapshot!(painted_rows(&styled, area.as_size()).join("\n"), @"  build  ready  !");
    let mut buf = Buffer::empty(area);
    styled.render(area, &Theme::default(), &mut buf, /*plain_style*/ None);
    assert_eq!(buf[(2, 0)].style(), Buffer::empty(area)[(2, 0)].style());
    assert!(
        buf[(9, 0)]
            .modifier
            .contains(ratatui::style::Modifier::BOLD)
    );
}

#[test]
fn clamping_text_reserves_space_for_following_fields() {
    let node = tree(json!({"type":"col","props":{"gap":0},"children":[
        {"type":"text","props":{"maxLines":2,"preserveWhitespace":true},"children":["aa  bb cc dd ee"]},
        "ok"
    ]}));
    insta::assert_snapshot!(painted_rows(&node, Size::new(5, 3)).join("\n"), @"aa bb\ncc d…\nok");
    assert_eq!(
        painted_rows(&node, Size::new(8, 3)),
        ["aa bb cc", "dd ee", "ok"]
    );
    assert_eq!(painted_rows(&node, Size::new(5, 1)), ["aa b…"]);
    assert_eq!(painted_rows(&node, Size::new(3, 2)), ["aa", "bb…"]);
    let word = tree(json!({"type":"text","props":{"maxLines":2},"children":["123456789"]}));
    assert_eq!(painted_rows(&word, Size::new(4, 3)), ["1234", "567…"]);
    let job = tree(json!({"type":"text","props":{"maxLines":2},"children":["job-123"]}));
    assert_eq!(painted_rows(&job, Size::new(3, 2)), ["job", "-1…"]);
    for invalid in [
        json!(0),
        json!(-1),
        json!(1.5),
        json!(null),
        json!(9007199254740992u64),
    ] {
        assert!(parse(json!({"type":"text","props":{"maxLines":invalid},"children":[]})).is_err());
    }
}

#[test]
fn viewport_overflow_cannot_reclaim_padding_already_used_by_a_clamp() {
    let node = tree(json!({"type":"col","props":{"gap":0},"children":[
        {"type":"row","props":{"gap":0},"children":[
            {"type":"text","props":{"maxLines":1},"children":[
                {"type":"row","props":{"gap":1,"justify":"end","wrap":"wrap"},"children":[
                    {"type":"text","children":["X"]}, {"type":"text","children":["Y"]}
                ]}
            ]},
            "B"
        ]},
        "C"
    ]}));
    assert_eq!(painted_rows(&node, Size::new(3, 1)), ["X……"]);
    assert_eq!(painted_rows(&node, Size::new(3, 2)), ["X…B", "C"]);
    assert_eq!(painted_rows(&node, Size::new(4, 2)), ["X YB", "C"]);
}

#[test]
fn clamped_link_keeps_whole_graphemes_and_an_unlinked_marker() {
    let href = "https://example.com/";
    let node = tree(json!({"type":"link","props":{"href":href},"children":[
        {"type":"text","props":{"maxLines":1},"children":["a界\nb"]}
    ]}));
    let bounds = Size::new(3, 3);
    assert_eq!(node.measure(bounds), Size::new(2, 1));
    assert_eq!(
        painted_rows(&node, bounds),
        [format!(
            "{}…",
            crate::terminal_hyperlinks::osc8_hyperlink(href, "a")
        )]
    );
}

#[test]
fn empty_horizontal_box_keeps_its_flex_slot_and_separates_text_blocks() {
    let empty = json!({"type":"box","props":{"direction":"row","gap":0},"children":[]});
    let children = json!(["A", empty, "B"]);
    for props in [json!({"gap":1}), json!({"gap":1,"justify":"start"})] {
        let row = tree(json!({"type":"row","props":props,"children":children}));
        assert_eq!(painted_rows(&row, Size::new(9, 1)), ["A  B"]);
    }
    let block = tree(json!({"type":"text","children":children}));
    assert_eq!(painted_rows(&block, Size::new(9, 3)), ["A", "B"]);
}

#[test]
fn box_defaults_to_a_column_and_uses_row_options_in_row_direction() {
    let make = |props: Value| json!({"type":"box","props":props,"children":["a","b",null,{"type":"text","children":["c"]}]});
    let bounds = Size::new(5, 4);
    for props in [json!({"gap":1}), json!({"direction":"col","gap":1})] {
        let value = make(props);
        assert_eq!(
            serde_json::to_value(parse(value.clone()).unwrap()).unwrap(),
            value
        );
        assert_eq!(painted_rows(&tree(value), bounds), ["ab", "", "c"]);
    }
    let row = make(json!({"direction":"row","gap":1,"wrap":"wrap","justify":"end"}));
    assert_eq!(
        painted_rows(&tree(row.clone()), Size::new(2, 3)),
        ["ab", " c"]
    );
    let nested = tree(json!({"type":"text","children":["x",row,"y"]}));
    insta::assert_snapshot!(painted_rows(&nested, bounds).join("\n"), @"x\n ab c\ny");
    for props in [
        json!({}),
        json!({"direction":"vertical","gap":0}),
        json!({"direction":"col","gap":0,"justify":"end"}),
        json!({"gap":-1}),
    ] {
        assert!(parse(make(props)).is_err());
    }
}

#[test]
fn wrapped_code_keeps_a_leading_hyphen_with_the_hash() {
    let bounds = Size::new(8, 3);
    for children in [
        json!(["detached-123456789abcdef"]),
        json!(["detached-", "123456789abcdef"]),
        json!(["detached-", {"type":"bold","children":["123456789abcdef"]}]),
    ] {
        let node = tree(json!({"type":"code","children":children}));
        assert_eq!(
            painted_rows(&node, bounds),
            ["detached", "-1234567", "89abcdef"]
        );
    }
}

#[test]
fn inline_code_wraps_at_spaces_then_graphemes_and_expands_tabs_on_each_line() {
    for (literal, width, expected) in [
        ("a\tb", 10, vec!["a       b"]),
        ("a\tb", 4, vec!["a", "", "b"]),
        ("ab cd\tQ", 5, vec!["ab", "cd", "", "Q"]),
        ("abcdefghij", 5, vec!["abcde", "fghij"]),
        ("git --help", 5, vec!["git -", "-help"]),
        ("job-123", 5, vec!["job-", "123"]),
        ("fix issue-#1234", 10, vec!["fix issue-", "#1234"]),
        ("sum = -(value)", 7, vec!["sum = -", "(value)"]),
        ("tmp-*[0-9]", 5, vec!["tmp-*", "[0-9]"]),
        ("foo?bar", 5, vec!["foo?", "bar"]),
        ("ls|sha256sum", 6, vec!["ls|sha", "256sum"]),
        (r"\\server\share", 8, vec![r"\\server", r"\share"]),
        ("foo/bar", 5, vec!["foo/b", "ar"]),
        ("/usr/lib/bin", 6, vec!["/usr/l", "ib/bin"]),
        ("--help", 4, vec!["--", "help"]),
        ("foo-bar", 5, vec!["foo-", "bar"]),
        ("e\u{301}  x", 3, vec!["e\u{301}", "x"]),
    ] {
        let node = tree(json!({"type":"code","children":[literal]}));
        assert_eq!(
            painted_rows(&node, Size::new(width, 4)),
            expected,
            "{literal:?}"
        );
    }
    let clipped = tree(json!({"type":"code","children":["abcdefghij"]}));
    assert_eq!(painted_rows(&clipped, Size::new(5, 1)), ["abcd…"]);
}

#[test]
fn inline_code_uses_its_own_foreground_background_and_weight_inside_styled_links() {
    use ratatui::style::Color;
    use ratatui::style::Modifier;
    use syntect::highlighting::StyleModifier;
    use syntect::highlighting::ThemeItem;

    let href = "https://example.com/check";
    let node = tree(
        json!({"type":"row","props":{"gap":1,"background":"blue"},"children":[
            {"type":"bold","children":[{"type":"link","props":{"href":href},"children":[
                {"type":"code","children":["\u{1b}[31ma  b\u{1b}[0m"]}
            ]}]},
            {"type":"text","props":{"color":"red","italic":true},"children":[{"type":"code","children":["X"]}]}
        ]}),
    );
    let area = Rect::new(0, 0, 6, 1);
    assert_eq!(
        painted_rows(&node, area.as_size()),
        [linked_columns("a  b X", 0..4, href)]
    );
    let mut theme = Theme::default();
    theme.settings.line_highlight = Some(syntect::highlighting::Color {
        r: 3,
        g: 0,
        b: 0,
        a: 0,
    });
    theme.scopes.push(ThemeItem {
        scope: "markup.raw.inline.markdown".parse().unwrap(),
        style: StyleModifier {
            foreground: Some(syntect::highlighting::Color {
                r: 6,
                g: 0,
                b: 0,
                a: 0,
            }),
            ..StyleModifier::default()
        },
    });
    let mut buf = Buffer::empty(area);
    node.render(area, &theme, &mut buf, /*plain_style*/ None);
    for x in 0..4 {
        let cell = &buf[(x, 0)];
        assert_eq!((cell.fg, cell.bg), (Color::Cyan, Color::Yellow));
        assert_eq!(cell.modifier, Modifier::UNDERLINED);
    }
    assert_eq!(buf[(4, 0)].bg, Color::Blue);
    let last = &buf[(5, 0)];
    assert_eq!((last.fg, last.bg), (Color::Cyan, Color::Yellow));
    assert_eq!(last.modifier, Modifier::ITALIC);
}

#[test]
fn rejects_invalid_gaps_and_elements() {
    for gap in ["-1", "1.5", "9007199254740992", r#""1""#, "[1]"] {
        let row = format!(r#"{{"children":[],"props":{{"gap":{gap}}},"type":"row"}}"#);
        assert!(row.parse::<UiTree>().is_err(), "{row}");
    }
    for input in [
        "[]",
        "null true",
        r#"{"type":{"text":null},"children":[]}"#,
        r#"{"type":"text","children":[["Ready"]]}"#,
        r#"{"type":"text","props":null,"children":[]}"#,
        r#"{"type":"row","props":{},"children":[]}"#,
        r#"{"type":"col","props":{},"children":[]}"#,
        r#"{"type":"row","props":{"gap":0,"extra":true},"children":[]}"#,
        r#"{"props":{"gap":0,"gap":1},"children":[],"type":"row"}"#,
    ] {
        assert!(input.parse::<UiTree>().is_err(), "{input}");
    }
}

#[test]
fn deserialization_checks_limits_across_the_whole_tree() {
    let parent = |children: Vec<Value>| json!({"type":"text","children":children});
    assert!(parse(parent(vec![Value::Null; 257])).is_ok());
    let nested = ["text", "bold", "italic", "underline"]
        .into_iter()
        .cycle()
        .take(16)
        .fold(
            json!("X"),
            |child, kind| json!({"type":kind,"children":[child]}),
        );
    assert!(parse(nested.clone()).is_ok());
    assert!(parse(parent(vec![nested])).is_err());
    let mut leaves = vec![json!(""); 255];
    assert!(parse(parent(leaves.clone())).is_ok());
    leaves.push(json!(""));
    assert!(parse(parent(leaves)).is_err());
    let text = "界".repeat(5461); // 16,383 UTF-8 bytes.
    for (suffix, ok) in [("x", true), ("xx", false)] {
        assert_eq!(parse(parent(vec![json!(text), json!(suffix)])).is_ok(), ok);
    }
    let at_limit = format!("null{}", " ".repeat(64 * 1024 - 4));
    assert_eq!(at_limit.parse::<UiTree>().unwrap(), UiTree(None));
    assert!(format!("{at_limit} ").parse::<UiTree>().is_err());
    let prefix = "https://example.com/";
    let href = format!("{prefix}{}", "x".repeat(8192 - prefix.len()));
    let link = |url: &str| json!({"type":"link","props":{"href":url},"children":["ok"]});
    assert!(parse(link(&href)).is_ok());
    assert!(parse(link(&format!("{href}x"))).is_err());
}

#[test]
fn links_only_annotate_their_own_cells() {
    let tree = tree(
        json!({"type":"row","props":{"gap":1},"children":["界",{"type":"link","props":{"href":"https://example.com/pr/42"},"children":[{"type":"text","children":["PR #42"]}]}]}),
    );
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 16, /*height*/ 1,
    );
    let mut buffer = Buffer::empty(area);
    tree.render(
        area,
        &Theme::default(),
        &mut buffer,
        /*plain_style*/ None,
    );
    assert_eq!(buffer[(0, 0)].symbol(), "界");
    assert!(
        buffer[(3, 0)]
            .symbol()
            .contains("https://example.com/pr/42")
    );
    insta::assert_snapshot!(format!("{buffer:?}"));
}

#[test]
fn exactly_fitting_halfwidth_voiced_katakana_is_visible() {
    assert_eq!(painted_rows(&tree(json!("ｶﾞ")), Size::new(2, 1)), ["ｶﾞ"]);
}

#[test]
fn ellipsis_counts_toward_width_only_when_there_is_overflow() {
    let row = tree(json!({"type":"row","props":{"gap":1},"children":["abcde", null]}));
    for (width, expected) in [(1, "…"), (4, "abc…"), (5, "abcde")] {
        assert_eq!(painted_rows(&row, Size::new(width, 1)), [expected]);
    }
    let wide = tree(json!("a界界"));
    assert_eq!(painted_rows(&wide, Size::new(4, 1)), ["a界…"]);
}

#[test]
fn zero_width_fragments_do_not_style_the_ellipsis_or_add_row_gaps() {
    let row = |text| json!({"type":"row","props":{"gap":0},"children":[text]});
    let children = json!([row("x"), {"type":"bold","children":["\u{301}"]}, row("yz")]);
    for (root, expected) in [
        (json!({"type":"text","children":children}), "x…"),
        (
            json!({"type":"row","props":{"gap":1},"children":children}),
            "x yz",
        ),
        (
            json!({"type":"text","children":["A", {"type":"bold","children":["\u{ad}BB"]}]}),
            "A…",
        ),
        (
            json!({"type":"text","children":[{"type":"bold","children":["\u{ad}"]}, "AB"]}),
            "…",
        ),
    ] {
        let node = tree(root);
        let expected = Buffer::with_lines([expected]);
        let mut buf = Buffer::empty(expected.area);
        node.render(
            buf.area,
            &Theme::default(),
            &mut buf,
            /*plain_style*/ None,
        );
        assert_eq!(buf, expected);
    }
}

#[test]
fn single_line_view_marks_vertical_overflow() {
    let invisible = tree(json!({"type":"bold","children":["\u{ad}\na"]}));
    assert_eq!(painted_rows(&invisible, Size::new(1, 1)), ["…"]);
    let multiline = format!("\u{1b}[2Jok\n{}", "x".repeat(1024));
    let tree = tree(json!({"type":"row","props":{"gap":0},"children":[
        &multiline, "!"
    ]}));
    assert_eq!(painted_rows(&tree, Size::new(4, 1)), ["ok …"]);
}

#[test]
fn directly_constructed_text_keeps_a_bounded_utf8_prefix() {
    let prefix = "x".repeat(MAX_TEXT_BYTES - 1);
    let text = UiTree(Some(UiNode::String(format!("{prefix}界tail")))).prepare();
    assert_eq!(
        painted_rows(&text, Size::new(MAX_TEXT_BYTES as u16, 1)),
        [prefix]
    );
}

#[test]
fn text_and_row_share_the_color_contract() {
    for color in [
        "red",
        "white",
        "#ab12f0",
        "primary",
        "accent",
        "surface",
        "status-branch",
    ] {
        let value = json!({"type":"row","props":{"gap":0,"background":color},"children":[
            {"type":"text","props":{"color":color,"weight":"bold","italic":true,"underline":false},"children":["ok"]}
        ]});
        assert_eq!(
            serde_json::to_value(parse(value.clone()).unwrap()).unwrap(),
            value
        );
    }
    for props in [
        json!({"weight":"semibold"}),
        json!({"weight":{"bold":null}}),
        json!({"color":{"red":null}}),
        json!({"color":"#abcdef00"}),
        json!({"color":"#+12345"}),
        json!({"color":"#123"}),
        json!({"color":"teal"}),
        json!({"underline":"dotted"}),
    ] {
        assert!(
            serde_json::from_value::<UiNode>(json!({"type":"text","props":props,"children":[]}))
                .is_err(),
            "{props}"
        );
    }
    assert!(
        serde_json::from_value::<UiNode>(json!({"type":"italic","props":{},"children":[]}))
            .is_err()
    );
}

#[test]
fn inline_modifiers_and_row_surfaces_follow_the_tree() {
    let node = tree(json!({
        "type":"row", "props":{"gap":1,"background":"white"}, "children":[
            {"type":"bold","children":[{"type":"italic","children":[
                {"type":"underline","children":["a"]},
                {"type":"text","props":{"weight":"normal","italic":false,"color":"cyan"},"children":["b"]},
                "c"
            ]}]},
            {"type":"row","props":{"gap":1,"background":"blue"},"children":["d","e"]},
            {"type":"link","props":{"href":"https://example.com"},"children":["f"]}
        ]
    }));
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 12, /*height*/ 1,
    );
    let mut buffer = Buffer::empty(area);
    node.render(
        area,
        &Theme::default(),
        &mut buffer,
        /*plain_style*/ None,
    );
    insta::assert_snapshot!(format!("{buffer:?}"));

    let clipped = Rect { width: 6, ..area };
    let mut buffer = Buffer::empty(clipped);
    node.render(
        clipped,
        &Theme::default(),
        &mut buffer,
        /*plain_style*/ None,
    );
    assert_eq!(buffer[(5, 0)].symbol(), "…");
    assert_eq!(buffer[(5, 0)].bg, ratatui::style::Color::Blue);
}

#[test]
fn hex_colors_use_the_terminal_palette_for_both_channels() {
    let tree = tree(json!({
        "type":"row", "props":{"gap":1,"background":"#1020F0"}, "children":[
            {"type":"text","props":{"color":"#AB12f0"},"children":["a"]}, "b"
        ]
    }));
    crate::terminal_palette::with_test_default_colors(
        crate::terminal_probe::DefaultColors {
            fg: (255, 255, 255),
            bg: (0, 0, 0),
        },
        || {
            let area = Rect::new(
                /*x*/ 0, /*y*/ 0, /*width*/ 3, /*height*/ 1,
            );
            let mut buffer = Buffer::empty(area);
            tree.render(
                area,
                &Theme::default(),
                &mut buffer,
                /*plain_style*/ None,
            );
            let colors: Vec<_> = buffer
                .content
                .iter()
                .map(|cell| (cell.fg, cell.bg))
                .collect();
            let surface = crate::terminal_palette::rgb_color((16, 32, 240));
            let foreground = crate::terminal_palette::rgb_color((171, 18, 240));
            let reset = ratatui::style::Color::Reset;
            assert_eq!(
                colors,
                [(foreground, surface), (reset, surface), (reset, surface)]
            );
        },
    );
}

#[test]
fn semantic_colors_follow_the_theme_and_terminal() {
    use crate::terminal_palette::with_test_default_colors;
    use crate::terminal_probe::DefaultColors;

    const CHILD: &str = "CODEX_STATUSLINE_SEMANTIC_PALETTE_TEST";
    if std::env::var_os(CHILD).is_some() {
        insta::assert_snapshot!("semantic_statusline_ansi16", paint_semantic_samples());
        return;
    }
    let limited = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "ui_tree::tests::semantic_colors_follow_the_theme_and_terminal",
        ])
        .env(CHILD, "1")
        .env("FORCE_COLOR", "1")
        .output()
        .unwrap();
    with_test_default_colors(
        DefaultColors {
            fg: (255, 255, 255),
            bg: (0, 0, 0),
        },
        || {
            insta::assert_snapshot!("semantic_statusline_truecolor", paint_semantic_samples());
        },
    );
    assert!(
        limited.status.success(),
        "{}",
        String::from_utf8_lossy(&limited.stdout)
    );
}

fn paint_semantic_samples() -> String {
    use ratatui::style::Color as TerminalColor;
    use ratatui::style::Style;
    use syntect::highlighting::Color;
    use syntect::highlighting::StyleModifier;
    use syntect::highlighting::ThemeItem;

    let node = tree(
        json!({"type":"row","props":{"gap":0,"background":"surface"},"children":[
            {"type":"text","props":{"color":"primary"},"children":["p"]},
            {"type":"text","props":{"color":"accent"},"children":["a"]},
            {"type":"text","props":{"color":"status-branch"},"children":["b",
                {"type":"row","props":{"gap":0,"background":"status-branch"},"children":["i",
                    {"type":"text","props":{"color":"status-branch"},"children":["d"]}]}]}
        ]}),
    );
    let rgb = |r, g, b| Color { r, g, b, a: 255 };
    let scope = |name: &str, color| ThemeItem {
        scope: name.parse().unwrap(),
        style: StyleModifier {
            foreground: Some(color),
            ..Default::default()
        },
    };
    let mut dark = Theme::default();
    dark.settings.background = Some(rgb(0, 0, 0));
    dark.settings.foreground = Some(rgb(220, 221, 222));
    dark.settings.accent = Some(rgb(0, 200, 80));
    dark.scopes
        .push(scope("entity.name.function", rgb(230, 230, 230)));
    let mut light = Theme::default();
    light.settings.background = Some(rgb(255, 255, 255));
    light.settings.foreground = Some(rgb(1, 2, 3));
    light.settings.accent = dark.settings.accent;
    light.scopes.push(scope(
        "codex.accent",
        Color {
            a: 1,
            ..Color::BLACK
        },
    ));
    light
        .scopes
        .push(scope("entity.name.function", rgb(80, 0, 120)));
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 5, /*height*/ 1,
    );
    let mut samples = Vec::new();
    for theme in [dark, light, Theme::default()] {
        let mut buffer = Buffer::empty(area);
        buffer.set_style(
            area,
            Style::default()
                .fg(TerminalColor::Yellow)
                .bg(TerminalColor::Blue),
        );
        node.render(area, &theme, &mut buffer, /*plain_style*/ None);
        samples.push(format!("{buffer:?}"));
    }
    samples.join("\n")
}

#[test]
fn terminal_commands_are_sanitized_within_each_string() {
    let url = "https://example.com/ready";
    let node = tree(json!({"type":"text","children":[
        "\u{1b}[31mre\rd\u{1b}[0m \u{1b}]8;;https://example.com\u{7}label\u{1b}]8;;\u{1b}\\\u{7}",
        " \u{1b}[31", {"type":"link","props":{"href":url},"children":["Ready"]}
    ]}));
    assert_eq!(
        painted_rows(&node, Size::new(40, 1)),
        [linked_columns("red label Ready", 10..15, url)]
    );
}

#[test]
fn split_graphemes_use_the_first_style_and_require_unanimous_links() {
    use crate::terminal_hyperlinks::osc8_hyperlink;
    use ratatui::style::Modifier;

    let url = "https://example.com/a";
    let link = |href, text| json!({"type":"link","props":{"href":href},"children":[text]});
    let node = tree(json!({"type":"text","children":[
        {"type":"bold","children":[{"type":"row","props":{"gap":0},"children":["R"]},"e"]}, {"type":"italic","children":[]}, "\u{301}",
        link(url, "👩"), link(url, "\u{200d}💻"),
        link(url, "👩"), link("https://example.com/b", "\u{200d}💻"),
        {"type":"bold","children":["\u{600}"]}, " x",
        link(url, "तक्क"), "\u{301}"
    ]}));
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 20, /*height*/ 1,
    );
    let mut buffer = Buffer::empty(area);
    node.render(
        area,
        &Theme::default(),
        &mut buffer,
        /*plain_style*/ None,
    );
    for (x, symbol, modifier) in [
        (0, "R".to_owned(), Modifier::BOLD),
        (1, "e\u{301}".to_owned(), Modifier::BOLD),
        (2, osc8_hyperlink(url, "👩‍💻"), Modifier::UNDERLINED),
        (4, "👩‍💻".to_owned(), Modifier::UNDERLINED),
        (6, "\u{600} ".to_owned(), Modifier::BOLD),
        (8, "x".to_owned(), Modifier::empty()),
        (9, osc8_hyperlink(url, "त"), Modifier::UNDERLINED),
        (10, "क्क\u{301}".to_owned(), Modifier::UNDERLINED),
    ] {
        let cell = &buffer[(x, 0)];
        assert_eq!((cell.symbol(), cell.modifier), (symbol.as_str(), modifier));
    }
}

#[test]
fn collapsed_space_keeps_the_first_whitespace_owners_link() {
    let href = "https://example.com/build";
    let node = tree(json!({"type":"text","children":[
        "a", {"type":"link","props":{"href":href},"children":["\t"]},
        "\u{1b}[31m", "  b", {"type":"link","props":{"href":href},"children":[" "]},
        {"type":"row","props":{"gap":0},"children":[" c "]}, " d"
    ]}));
    assert_eq!(
        painted_rows(&node, Size::new(10, 1)),
        [linked_columns("a bcd", 1..2, href)]
    );
}

fn painted_rows(node: &PreparedUiTree, bounds: Size) -> Vec<String> {
    let area = Rect::from((ratatui::layout::Position::ORIGIN, bounds));
    let mut buf = Buffer::empty(area);
    node.render(area, &Theme::default(), &mut buf, /*plain_style*/ None);
    (0..node.measure(bounds).height)
        .map(|y| {
            let mut text = String::new();
            let mut x = 0;
            while x < bounds.width {
                let cell = &buf[(x, y)];
                text.push_str(cell.symbol());
                x += cell.cell_width().max(1);
            }
            text.trim_end_matches(' ').to_owned()
        })
        .collect()
}

fn linked_columns(text: &str, columns: std::ops::Range<usize>, href: &str) -> String {
    text.chars()
        .enumerate()
        .map(|(column, ch)| {
            let symbol = ch.to_string();
            if columns.contains(&column) {
                crate::terminal_hyperlinks::osc8_hyperlink(href, &symbol)
            } else {
                symbol
            }
        })
        .collect()
}

#[test]
fn measures_viewport_sizes_in_cells() {
    let node =
        tree(json!({"type":"row","props":{"gap":1,"wrap":"wrap"},"children":["ab\ncd","界界"]}));
    assert_eq!(node.measure(Size::new(4, 8)), Size::new(4, 3));
    let wide = tree(json!("a界\nnext"));
    assert_eq!(wide.measure(Size::new(3, 1)), Size::new(2, 1));
    assert_eq!(painted_rows(&wide, Size::new(3, 1)), ["a…"]);
}

#[test]
fn col_stacks_rows_after_wrapping_and_skips_absent_children() {
    let node = tree(json!({"type":"col","props":{"gap":1},"children":[
        null, {"type":"row","props":{"gap":1,"wrap":"wrap"},"children":[{"type":"text","children":["model"]},{"type":"text","children":["main"]}]},
        " ",
        {"type":"row","props":{"gap":1},"children":[{"type":"text","children":["75%"]},{"type":"text","children":["1k"]}]}, null
    ]}));
    let wide = Size::new(20, 4);
    assert_eq!(node.measure(wide), Size::new(10, 3));
    insta::assert_snapshot!(painted_rows(&node, wide).join("\n"), @r"
    model main

    75% 1k
    ");
    assert_eq!(
        painted_rows(&node, Size::new(7, 4)),
        ["model", "main", "", "75% 1k"]
    );
    let clipped = tree(
        json!({"type":"col","props":{"gap":MAX_SAFE_INTEGER},"children":[{"type":"text","children":["a"]},{"type":"text","children":["b"]}]}),
    );
    assert_eq!(painted_rows(&clipped, Size::new(5, 3)), ["a", "", "…"]);
}

#[test]
fn col_groups_bare_strings_and_counts_empty_elements_in_its_gaps() {
    let node = tree(json!({"type":"col","props":{"gap":1},"children":[
        {"type":"text","children":[]}, "L", null, "R",
        {"type":"col","props":{"gap":0},"children":[]}, {"type":"text","children":["end"]}
    ]}));
    let rows = painted_rows(&node, Size::new(10, 5));
    let framed = rows
        .iter()
        .map(|row| format!("|{row}|"))
        .collect::<Vec<_>>();
    insta::assert_snapshot!(framed.join("\n"), @r"
    ||
    |LR|
    ||
    ||
    |end|
    ");
    let nulls = tree(json!({"type":"col","props":{"gap":1},"children":[null," ",null]}));
    assert_eq!(nulls.measure(Size::new(10, 4)), Size::default());
}

#[test]
fn col_skips_whitespace_only_anonymous_items_and_keeps_authored_breaks() {
    let text = |s: &str| json!({"type":"text","children":[s]});
    let bounds = Size::new(8, 6);
    let col = |middle| {
        tree(json!({"type":"col","props":{"gap":1},"children":[
            text("a"), middle, text("b")
        ]}))
    };
    assert_eq!(painted_rows(&col(json!(" \t\n")), bounds), ["a", "", "b"]);
    assert_eq!(painted_rows(&col(text("")), bounds), ["a", "", "", "b"]);
    assert_eq!(
        painted_rows(&col(text("\n")), bounds),
        ["a", "", "", "", "b"]
    );
    let stack = |children| {
        let node = tree(json!({"type":"col","props":{"gap":0},"children":children}));
        painted_rows(&node, bounds)
    };
    assert_eq!(stack(json!([text("a"), "\n", null, "b"])), ["a", "b"]);
    assert_eq!(stack(json!([text("a"), "\nb"])), ["a", "", "b"]);
    assert_eq!(stack(json!(["a", "\n", null, "b"])), ["a", "b"]);
}

#[test]
fn col_interrupts_inline_flow_even_when_empty() {
    use ratatui::style::Modifier;

    let surround = |children| {
        tree(json!({"type":"text","children":[
            "x ", {"type":"bold","children":[
                {"type":"col","props":{"gap":0},"children":children}, "y"
            ]}, "z"
        ]}))
    };
    let area = Rect::new(0, 0, 8, 4);
    let bounds = area.as_size();
    let full = surround(json!([{"type":"text","children":["a"]},{"type":"text","children":["b"]}]));
    assert_eq!(full.measure(bounds), Size::new(2, 4));
    assert_eq!(painted_rows(&full, bounds), ["x", "a", "b", "yz"]);
    let mut buf = Buffer::empty(area);
    full.render(area, &Theme::default(), &mut buf, /*plain_style*/ None);
    for y in 1..4 {
        assert!(buf[(0, y)].modifier.contains(Modifier::BOLD));
    }
    assert!(buf[(1, 3)].modifier.is_empty());
    let empty = surround(json!([null, ""]));
    assert_eq!(painted_rows(&empty, bounds), ["x", "yz"]);
}

#[test]
fn wrapping_rows_reserve_a_gap_on_each_side_of_an_empty_column() {
    for children in [
        json!([]),
        json!([null]),
        json!([{"type":"text","children":[""]}]),
    ] {
        let node = tree(
            json!({"type":"row","props":{"gap":1,"wrap":"wrap"},"children":[
                "a", {"type":"col","props":{"gap":0},"children":children}, "b"
            ]}),
        );
        assert_eq!(painted_rows(&node, Size::new(3, 2)), ["a", "b"]);
        assert_eq!(painted_rows(&node, Size::new(8, 2)), ["a  b"]);
    }
}

#[test]
fn col_inside_a_row_wraps_within_its_assigned_width() {
    let node = tree(json!({"type":"row","props":{"gap":1},"children":[
        ">", {"type":"col","props":{"gap":0},"children":[{"type":"text","children":["ab c"]},{"type":"text","children":["xyz"]}]}, "!"
    ]}));
    assert_eq!(
        painted_rows(&node, Size::new(7, 3)),
        ["> ab  !", "  c", "  xyz"]
    );

    let clipped = tree(json!({"type":"row","props":{"gap":1},"children":[
        "abc", {"type":"col","props":{"gap":1},"children":[{"type":"text","children":["a"]},{"type":"text","children":["b"]}]}
    ]}));
    assert_eq!(painted_rows(&clipped, Size::new(3, 3)), ["ab…", "", "  …"]);
}

#[test]
fn whitespace_collapses_across_styled_leaves() {
    let node = tree(json!({"type":"text","children":[
        "  build\t", {"type":"bold","children":["  ready  "]}, "\nnext"
    ]}));
    let text = painted_rows(&node, Size::new(20, 2));
    insta::assert_snapshot!(text.join("\n"), @r"
    build ready
    next
    ");
}

#[test]
fn reads_row_layout_without_moving_existing_positional_props() {
    let row = |props| json!({"type":"row","props":props,"children":["ok"]});
    assert_eq!(
        parse(row(json!([1, "red"]))).unwrap(),
        parse(row(json!({"gap":1,"background":"red"}))).unwrap()
    );
    assert_eq!(
        parse(row(json!({"gap":1,"wrap":"nowrap"}))).unwrap(),
        parse(row(json!({"gap":1}))).unwrap()
    );
    for invalid in [json!("wrap-reverse"), json!(null), json!({"wrap":null})] {
        assert!(parse(row(json!({"gap":0,"wrap":invalid}))).is_err());
    }
    assert_eq!(
        parse(row(json!([1, "red", "wrap"]))).unwrap(),
        parse(row(json!({"gap":1,"background":"red","wrap":"wrap"}))).unwrap()
    );
    let explicit = row(json!({"gap":1,"background":"red","wrap":"wrap","justify":"start"}));
    let positional = parse(row(json!([1, "red", "wrap", "start"]))).unwrap();
    assert_eq!(parse(explicit.clone()).unwrap(), positional);
    assert_eq!(serde_json::to_value(positional).unwrap(), explicit);
    for invalid in [json!("right"), json!("space-between"), json!(null)] {
        assert!(parse(row(json!({"gap":0,"justify":invalid}))).is_err());
    }
}

#[test]
fn between_keeps_statusline_groups_at_opposite_edges_when_resized() {
    let group = |fields: [&str; 2]| {
        let children = fields.map(|field| json!({"type": "text", "children": [field]}));
        json!({"type": "row", "props": {"gap": 1}, "children": children})
    };
    let node = tree(json!({
        "type": "row", "props": {"gap": 1, "justify": "between", "wrap": "wrap"},
        "children": [group(["界", "git"]), group(["75%", "2k"])]
    }));
    let wide = Size::new(20, 2);
    assert_eq!(node.measure(wide), Size::new(20, 1));
    insta::assert_snapshot!(painted_rows(&node, wide).join("\n"), @"界 git        75% 2k");
    assert_eq!(painted_rows(&node, Size::new(13, 2)), ["界 git 75% 2k"]);
    assert_eq!(painted_rows(&node, Size::new(12, 2)), ["界 git", "75% 2k"]);
}

#[test]
fn center_and_end_align_each_wrapped_line_with_its_requested_gap() {
    let layout = |justify| {
        let children = ["界", "bb", "xyz"].map(|s| json!({"type":"text","children":[s]}));
        let node = tree(
            json!({"type":"row","props":{"gap":1,"wrap":"wrap","justify":justify},
            "children":children}),
        );
        painted_rows(&node, Size::new(8, 2))
            .iter()
            .map(|line| format!("|{line}|"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    insta::assert_snapshot!(layout("center"), @r"
    | 界 bb|
    |  xyz|
    ");
    insta::assert_snapshot!(layout("end"), @r"
    |   界 bb|
    |     xyz|
    ");
}

#[test]
fn end_alignment_uses_spare_padding_for_vertical_overflow() {
    let node = tree(json!({
        "type": "row", "props": {"gap": 1, "justify": "end", "wrap": "wrap"},
        "children": ["界", {"type": "text", "children": ["12345678"]}]
    }));
    let bounds = Size::new(8, 1);
    assert_eq!(node.measure(bounds), bounds);
    insta::assert_snapshot!(format!("|{}|", painted_rows(&node, bounds).join("\n")), @"|     界…|");
    assert_eq!(
        painted_rows(&node, Size::new(8, 2)),
        ["      界", "12345678"]
    );
}

#[test]
fn center_and_end_place_consecutive_strings_as_one_item() {
    let layout = |justify| {
        let row =
            json!({"type":"row","props":{"gap":1,"justify":justify},"children":["L",null,"R"]});
        let outer = json!({"type":"row","props":{"gap":0},"children":[row]});
        [row, outer].map(|row| painted_rows(&tree(row), Size::new(10, 1)).join("\n"))
    };
    assert_eq!(layout("center"), ["    LR", "LR"]);
    assert_eq!(layout("end"), ["        LR", "LR"]);
}

#[test]
fn aligned_rows_pad_visible_content_and_keep_link_targets_on_their_text() {
    let href = "https://example.com/pr";
    for (justify, expected) in [("center", "  PR"), ("end", "     PR")] {
        let row = |child: Value| {
            tree(json!({"type":"row","props":{"gap":0,"justify":justify},"children":[child]}))
        };
        let size = Size::new(5, 2);
        assert_eq!(row(json!("\n")).measure(size), Size::default());
        assert_eq!(
            row(json!({"type":"text","children":["\n"]})).measure(size),
            Size::new(0, 1)
        );
        assert_eq!(
            painted_rows(&row(json!("abcdef")), Size::new(3, 1)),
            ["ab…"]
        );
        let link = row(json!({"type":"link","props":{"href":href},"children":["PR"]}));
        assert_eq!(
            painted_rows(&link, Size::new(7, 1)),
            [linked_columns(
                expected,
                expected.len() - 2..expected.len(),
                href
            )]
        );
    }
}

#[test]
fn offscreen_blank_lines_count_toward_the_height_of_a_nested_row() {
    for justify in ["center", "end"] {
        let node = tree(
            json!({"type":"row","props":{"gap":0,"wrap":"wrap"},"children":[
                {"type":"row","props":{"gap":6,"justify":justify},"children":[
                    {"type":"text","children":[""]}, {"type":"text","children":["\n\nB"]}
                ]}, "ok"
            ]}),
        );
        assert_eq!(painted_rows(&node, Size::new(5, 1)), ["…"]);
        assert_eq!(painted_rows(&node, Size::new(5, 2)), ["", "…"]);
    }
}

#[test]
fn nested_aligned_rows_leave_authored_blank_lines_unpainted() {
    let area = Rect::new(0, 0, 8, 2);
    for justify in ["center", "end"] {
        let row = |children: Value| {
            tree(
                json!({"type":"row","props":{"gap":0,"justify":justify,"background":"red"},
                "children":[{"type":"row","props":{"gap":1},"children":children}]}),
            )
        };
        assert_eq!(
            row(json!(["\n", "\n"])).measure(area.as_size()),
            Size::new(0, 1)
        );
        let mut buf = Buffer::empty(area);
        row(json!(["A\n\n", "B\n\n"])).render(
            area,
            &Theme::default(),
            &mut buf,
            /*plain_style*/ None,
        );
        assert_eq!(buf[(0, 0)].bg, ratatui::style::Color::Red);
        for x in 0..area.width {
            assert_eq!(buf[(x, 1)], ratatui::buffer::Cell::default());
        }
    }
}

#[test]
fn start_and_between_keep_their_gaps_before_blank_children() {
    let area = Rect::new(0, 0, 8, 1);
    for (justify, blank, visible) in [("start", 1, 2), ("between", 8, 8)] {
        for (first, expected) in [("\n", blank), ("A", visible)] {
            let node = tree(
                json!({"type":"row","props":{"gap":1,"justify":justify,"background":"red"},
                "children":[{"type":"text","children":[first]},{"type":"text","children":["\n"]}]}),
            );
            assert_eq!(node.measure(area.as_size()), Size::new(expected, 1));
            let mut buf = Buffer::empty(area);
            node.render(area, &Theme::default(), &mut buf, /*plain_style*/ None);
            for x in 0..expected {
                assert_eq!(buf[(x, 0)].bg, ratatui::style::Color::Red);
            }
        }
    }
}

#[test]
fn a_link_around_an_aligned_row_keeps_its_padding_unlinked() {
    let href = "https://example.com/pr";
    let area = Rect::new(0, 0, 8, 1);
    for (justify, leading) in [("center", 2), ("end", 5)] {
        let node = tree(json!({"type":"link","props":{"href":href},"children":[
            {"type":"row","props":{"gap":1,"justify":justify,"background":"red"},
             "children":["P",{"type":"text","children":["R"]}]}
        ]}));
        let mut buf = Buffer::empty(area);
        node.render(area, &Theme::default(), &mut buf, /*plain_style*/ None);
        for x in 0..leading {
            let cell = &buf[(x, 0)];
            assert_eq!(cell.symbol(), " ");
            assert_eq!(cell.bg, ratatui::style::Color::Red);
            assert!(cell.modifier.is_empty());
        }
        assert_eq!(
            painted_rows(&node, area.as_size()),
            [format!(
                "{}{}",
                " ".repeat(usize::from(leading)),
                linked_columns("P R", 0..3, href)
            )]
        );
    }
}

#[test]
fn between_distributes_indivisible_cells_on_each_wrapped_line() {
    let children = ["a", "b", "c", "longest"].map(|s| json!({"type": "text", "children": [s]}));
    let node = tree(json!({
        "type": "row", "props": {"gap": 1, "justify": "between", "wrap": "wrap"},
        "children": children
    }));
    assert_eq!(
        painted_rows(&node, Size::new(8, 2)),
        ["a   b  c", "longest"]
    );
}

#[test]
fn explicit_start_groups_bare_strings_and_omitted_justify_keeps_authored_gaps() {
    let children = json!(["L", null, "R", {"type":"text","children":["end"]}]);
    let row = |props| tree(json!({"type":"row","props":props,"children":children}));
    let size = Size::new(12, 1);
    assert_eq!(painted_rows(&row(json!({"gap":1})), size), ["L R end"]);
    assert_eq!(
        painted_rows(&row(json!({"gap":1,"justify":"start"})), size),
        ["LR end"]
    );
}

#[test]
fn between_matches_web_for_root_and_nested_rows() {
    let bounds = Size::new(10, 1);
    let text = |s: &str| json!({"type": "text", "children": [s]});
    let mut rows = vec!["children  |root      |nested    |".to_string()];
    for (label, children) in [
        ("strings", json!(["L", "R"])),
        ("elements", json!([text("L"), text("R")])),
        ("mixed", json!(["L", null, "M", text("R")])),
        ("empty", json!([text(""), text("R")])),
    ] {
        let inner =
            json!({"type": "row", "props": {"gap": 0, "justify": "between"}, "children": children});
        let outer = json!({"type": "row", "props": {"gap": 0}, "children": [inner]});
        let [root, nested] = [inner, outer].map(|row| painted_rows(&tree(row), bounds).join("\n"));
        rows.push(format!("{label:<10}|{root:<10}|{nested:<10}|"));
    }
    insta::assert_snapshot!(rows.join("\n"), @r"
    children  |root      |nested    |
    strings   |LR        |LR        |
    elements  |L        R|LR        |
    mixed     |LM       R|LMR       |
    empty     |         R|R         |
    ");
}

#[test]
fn between_rounds_each_fractional_position_to_the_nearest_cell() {
    let children = ["A", "B", "C", "D", "E", "F"].map(|s| json!({"type":"text","children":[s]}));
    let node =
        tree(json!({"type":"row","props":{"gap":0,"justify":"between"},"children":children}));
    insta::assert_snapshot!(painted_rows(&node, Size::new(14, 1)).join("\n"), @"A  B C  D E  F");
}

#[test]
fn wrapped_link_offsets_follow_each_row_and_stop_before_clipping() {
    let node = tree(json!({"type":"row","props":{"gap":0},"children":[
        ">",
        {"type":"link","props":{"href":"https://example.com"},"children":[
            {"type":"row","props":{"gap":0,"wrap":"wrap"},"children":["12345","67"]}
        ]}
    ]}));
    for (size, expected) in [
        (Size::new(6, 2), vec![(">12345", 1..6), (" 67", 1..3)]),
        (Size::new(4, 2), vec![(">12…", 1..3), (" 67", 1..3)]),
        (Size::new(6, 1), vec![(">1234…", 1..5)]),
    ] {
        let expected: Vec<_> = expected
            .into_iter()
            .map(|(text, columns)| linked_columns(text, columns, "https://example.com"))
            .collect();
        assert_eq!(painted_rows(&node, size), expected);
    }
}

#[test]
fn styled_text_wraps_in_the_space_its_nowrap_row_assigns() {
    let href = "https://example.com/build";
    let (plain, bold) = "build ".split_at(3);
    let node = tree(json!({"type":"row","props":{"gap":1},"children":[
        ">", {"type":"text","children":[
            plain, {"type":"bold","children":[bold]},
            {"type":"link","props":{"href":href},"children":["was green"]}
        ]}, "!"
    ]}));
    assert_eq!(
        painted_rows(&node, Size::new(10, 4)),
        [
            "> build  !".to_string(),
            linked_columns("  was", 2..5, href),
            linked_columns("  green", 2..7, href)
        ]
    );
}

#[test]
fn shrinking_a_hyphenated_word_reserves_the_siblings_column() {
    for props in [json!({"gap":1}), json!({"gap":1,"justify":"between"})] {
        let node = tree(json!({"type":"row","props":props,"children":[
            "aa-bb", {"type":"text","children":["!"]}
        ]}));
        assert_eq!(painted_rows(&node, Size::new(6, 2)), ["aa-  !", "bb"]);
    }
}

#[test]
fn wrapping_row_moves_a_hash_and_nonbreaking_pr_label_together() {
    let href = "https://example.com/pull/123";
    let node = tree(
        json!({"type":"row","props":{"gap":1,"wrap":"wrap"},"children":[
            "gpt-5", {"type":"row","props":{"gap":1},"children":[
                "a1b2c3d", {"type":"link","props":{"href":href},"children":["PR\u{a0}#123"]}
            ]}
        ]}),
    );
    for (width, expected, linked) in [
        (21, vec!["gpt-5 a1b2c3d PR\u{a0}#123"], 14..21),
        (20, vec!["gpt-5", "a1b2c3d PR\u{a0}#123"], 8..15),
        (12, vec!["gpt-5", "a1b2c3d PR\u{a0}…"], 8..11),
    ] {
        let lines = painted_rows(&node, Size::new(width, 4));
        let text: Vec<_> = lines
            .iter()
            .map(|line| crate::terminal_hyperlinks::strip_osc8(line))
            .collect();
        assert_eq!(text, expected);
        assert_eq!(
            lines.last().unwrap(),
            &linked_columns(expected.last().unwrap(), linked, href)
        );
    }
}

#[test]
fn authored_line_breaks_and_collapsed_whitespace_preserve_styles_and_links() {
    let href = "https://example.com/pr/42";
    let node = tree(json!({"type":"text","children":[
        "  ready  \t", {"type":"bold","children":["  now"]}, "\r", "\n\n",
        {"type":"link","props":{"href":href},"children":["PR \t", "#42\u{7}"]},
        "\nmore"
    ]}));
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 8, /*height*/ 4,
    );
    let mut buffer = Buffer::empty(area);
    node.render(
        area,
        &Theme::default(),
        &mut buffer,
        /*plain_style*/ None,
    );
    for (x, g) in "PR #42".chars().enumerate() {
        assert_eq!(
            buffer[(x as u16, 3)].symbol(),
            crate::terminal_hyperlinks::osc8_hyperlink(href, &g.to_string())
        );
    }
    insta::assert_snapshot!(crate::terminal_hyperlinks::strip_osc8(&format!(
        "{buffer:?}"
    )));
    assert_eq!(buffer[(6, 3)].symbol(), "…");
}

#[test]
fn earlier_sibling_grows_after_reserving_the_later_siblings_minimum() {
    let node = tree(json!({"type":"row","props":{"gap":0},"children":["aaa a bb", "aaa a bb"]}));
    assert_eq!(
        painted_rows(&node, Size::new(10, 3)),
        ["aaa a  aaa", "bb     a", "       bb"]
    );
}

#[test]
fn width_fitting_respects_word_hyphens_across_styles() {
    for (prefix, suffix, expected) in [
        ("1-", "2", vec!["1-", "2"]),
        ("--", "help", vec!["-…"]),
        ("a\u{ad}", "bc", vec!["a…"]),
        ("中", "文", vec!["中", "文"]),
    ] {
        let node =
            tree(json!({"type":"text","children":[{"type":"bold","children":[prefix]},suffix]}));
        assert_eq!(painted_rows(&node, Size::new(2, 3)), expected);
    }
}

#[test]
fn blank_nested_row_keeps_its_height_without_displacing_a_wide_sibling() {
    for (blank, gap, width, expected) in [
        ("\n", 0, 2, vec!["界"]),
        ("\n\n", 0, 2, vec!["界", ""]),
        ("\n\n", 1, 3, vec![" 界", ""]),
    ] {
        let node = tree(json!({
            "type": "row", "props": {"gap": gap, "justify": "end"}, "children": [
                {"type": "row", "props": {"gap": 0, "justify": "end"}, "children": [
                    {"type": "text", "children": [blank]}
                ]},
                "界"
            ]
        }));
        assert_eq!(painted_rows(&node, Size::new(width, 2)), expected);
    }
}

#[test]
fn clamped_word_shrinks_to_keep_its_sibling_visible() {
    let node = tree(json!({"type":"row","props":{"gap":1},"children":[
        {"type":"text","props":{"maxLines":1},"children":["abcdefghij"]}, "!"
    ]}));
    assert_eq!(painted_rows(&node, Size::new(8, 1)), ["abcde… !"]);
    assert_eq!(painted_rows(&node, Size::new(3, 1)), ["… !"]);
    assert_eq!(painted_rows(&node, Size::new(1, 1)), ["…"]);
}

#[test]
fn blank_clamp_keeps_its_sibling_end_aligned_without_a_marker() {
    let node = tree(
        json!({"type":"row","props":{"gap":0,"justify":"end"},"children":[
            {"type":"text","props":{"maxLines":1},"children":["\n\n"]}, "ok"
        ]}),
    );
    assert_eq!(painted_rows(&node, Size::new(5, 1)), ["   ok"]);
    assert_eq!(painted_rows(&node, Size::new(2, 1)), ["ok"]);
}

#[test]
fn clamped_blank_text_keeps_its_height_in_a_zero_width_slot() {
    let node = tree(json!({
        "type": "row", "props": {"gap": 0}, "children": [
            {"type": "text", "props": {"maxLines": 2}, "children": ["\n\n\n"]},
            "A"
        ]
    }));
    assert_eq!(painted_rows(&node, Size::new(1, 3)), ["A", ""]);
}

#[test]
fn spacer_only_wrapping_row_stays_in_its_zero_width_slot() {
    let spacer = json!({"type": "spacer"});
    for children in [json!([spacer]), json!([spacer, spacer])] {
        let node = tree(json!({
            "type": "row", "props": {"gap": 0}, "children": [
                {"type": "row", "props": {"gap": 1, "wrap": "wrap"}, "children": children},
                "A"
            ]
        }));
        assert_eq!(painted_rows(&node, Size::new(1, 1)), ["A"]);
    }
}

#[test]
fn trailing_gap_keeps_the_aligned_glyph_at_the_viewport_edge() {
    let node = tree(json!({
        "type":"row","props":{"gap":1},"children":[
            {"type":"row","props":{"gap":0,"wrap":"wrap","justify":"end"},"children":[
                "界",{"type":"text","children":["1234"]}
            ]},
            {"type":"text","children":["\n"]}
        ]
    }));
    assert_eq!(painted_rows(&node, Size::new(4, 1)), [" 界…"]);
    assert_eq!(painted_rows(&node, Size::new(4, 2)), ["  界", "1234"]);
}

#[test]
fn nested_row_background_fills_blank_lines_independently_of_siblings() {
    let area = Rect::new(0, 0, 8, 4);
    for sibling in ["S\nT\n\nU", "S\n\n\nU"] {
        for inner in [
            json!({"type":"row","props":{"gap":1,"background":"red"},"children":["a\n\n\nc","b\n\n\nd"]}),
            json!({"type":"row","props":{"gap":1,"justify":"end","background":"red"},"children":[
                {"type":"text","children":["a\n\n\nc"]},
                {"type":"text","children":["b\n\n\nd"]}
            ]}),
        ] {
            let node = tree(json!({
                "type":"row","props":{"gap":1,"justify":"end"},"children":[
                    inner,
                    {"type":"text","children":[sibling]}
                ]
            }));
            let mut buf = Buffer::empty(area);
            node.render(area, &Theme::default(), &mut buf, /*plain_style*/ None);
            for y in 0..area.height {
                for x in 0..area.width {
                    let expected = if (3..6).contains(&x) {
                        ratatui::style::Color::Red
                    } else {
                        ratatui::style::Color::Reset
                    };
                    assert_eq!(buf[(x, y)].bg, expected, "cell ({x}, {y})");
                }
            }
            assert_eq!(
                buf[(7, 1)].symbol(),
                if sibling.contains('T') { "T" } else { " " }
            );
        }
    }
}

#[test]
fn inline_code_checks_theme_contrast_on_its_fill() {
    use ratatui::style::Color;
    use syntect::highlighting::StyleModifier;
    use syntect::highlighting::ThemeItem;

    let node = tree(json!({"type":"code","children":["X"]}));
    let area = Rect::new(0, 0, 1, 1);
    let colors = |color: &str| {
        let color = Some(color.parse().unwrap());
        let mut theme = Theme::default();
        theme.settings.line_highlight = color;
        theme.scopes.push(ThemeItem {
            scope: "markup.raw.inline.markdown".parse().unwrap(),
            style: StyleModifier {
                foreground: color,
                ..Default::default()
            },
        });
        let mut buf = Buffer::empty(area);
        node.render(area, &theme, &mut buf, /*plain_style*/ None);
        (buf[(0, 0)].fg, buf[(0, 0)].bg)
    };
    let palette = crate::terminal_probe::DefaultColors {
        fg: (255, 255, 255),
        bg: (0, 0, 0),
    };
    crate::terminal_palette::with_test_default_colors(palette, || {
        let white = crate::terminal_palette::rgb_color((255, 255, 255));
        assert_eq!(
            colors("#ffffff"),
            (crate::style::readable_color_on(white, Some(white)), white)
        );
        assert_eq!(colors("#03000000"), (Color::Reset, Color::Reset));
    });
}

#[test]
fn clipped_code_tab_styles_the_marker_only_when_it_paints_cells() {
    let node = tree(json!({"type":"row","props":{"gap":0},"children":[
        {"type":"bold","children":["abc"]},{"type":"code","children":["\t"]}
    ]}));
    for width in [4, 5] {
        let area = Rect::new(0, 0, width, 1);
        let mut buf = Buffer::empty(area);
        node.render(area, &Theme::default(), &mut buf, /*plain_style*/ None);
        let marker = &buf[(width - 1, 0)];
        assert_eq!(marker.symbol(), "…");
        assert_eq!(marker.style(), buf[(width - 2, 0)].style());
    }
}

#[test]
fn spacer_is_an_empty_leaf() {
    for input in [
        json!({"type":"spacer"}),
        json!({"type":"spacer","children":[]}),
    ] {
        let node = parse(input).unwrap();
        assert_eq!(
            serde_json::to_value(&node).unwrap(),
            json!({"type":"spacer"})
        );
        assert_eq!(node.prepare().measure(Size::new(8, 3)), Size::default());
    }
    for input in [
        json!({"type":"spacer","children":[null]}),
        json!({"type":"spacer","children":["text"]}),
        json!({"type":"spacer","props":{"minSize":1}}),
    ] {
        assert!(parse(input).is_err());
    }
}

#[test]
fn spacers_share_each_bands_spare_width_before_justification() {
    let spacer = json!({"type":"spacer"});
    let node = tree(
        json!({"type":"box","props":{"direction":"row","gap":1,"wrap":"wrap","justify":"end"},
        "children":["A",spacer,"B",spacer,"C"]}),
    );
    insta::assert_snapshot!(painted_rows(&node, Size::new(10, 3)).join("\n"), @"A    B   C");
    assert_eq!(painted_rows(&node, Size::new(7, 3)), ["A  B  C"]);
    assert_eq!(painted_rows(&node, Size::new(4, 3)), ["A  B", "   C"]);
    let row = tree(json!({"type":"row","props":{"gap":0},"children":["a", "b", spacer, "界\nx"]}));
    assert_eq!(painted_rows(&row, Size::new(8, 1)), ["ab   界…"]);
}

#[test]
fn spacers_share_height_only_in_stretched_columns() {
    let spacer = json!({"type":"spacer"});
    let col = json!({"type":"col","props":{"gap":0},"children":["A",spacer,"B",spacer,"C"]});
    assert_eq!(
        painted_rows(&tree(col.clone()), Size::new(8, 8)),
        ["A", "B", "C"]
    );
    let row = tree(json!({"type":"row","props":{"gap":0},"children":[col,"1\n2\n3\n4\n5\n6"]}));
    assert_eq!(
        painted_rows(&row, Size::new(8, 6)),
        ["A1", " 2", " 3", "B4", " 5", "C6"]
    );
    assert_eq!(painted_rows(&row, Size::new(8, 3)), ["A1", " 2", " 3…"]);
    let huge = tree(json!({"type":"row","props":{"gap":0},"children":[col,
        {"type":"col","props":{"gap":MAX_SAFE_INTEGER},"children":["1",spacer,"2"]}]}));
    assert_eq!(painted_rows(&huge, Size::new(8, 3)), ["A1", "", "…"]);
}

#[test]
fn spacer_paints_the_row_fill_through_its_allocated_width() {
    let node = tree(
        json!({"type":"row","props":{"gap":1,"background":"red"},"children":["A",{"type":"spacer"}]}),
    );
    let area = Rect::new(0, 0, 8, 1);
    let mut buf = Buffer::empty(area);
    node.render(area, &Theme::default(), &mut buf, /*plain_style*/ None);
    assert!(
        buf.content()
            .iter()
            .all(|c| c.bg == ratatui::style::Color::Red)
    );
}

#[test]
fn zero_width_spacer_keeps_vertical_overflow_safe() {
    let s = json!({"type":"spacer"});
    let node = tree(json!({"type":"row","props":{"gap":0},"children":[s,"界\nx",s]}));
    assert_eq!(painted_rows(&node, Size::new(2, 1)), ["…"]);
    assert_eq!(painted_rows(&node, Size::new(3, 1)), ["界…"]);
}

#[test]
fn stretch_reaches_columns_through_nested_row_bands() {
    let col = json!({"type":"col","props":{"gap":0},"children":["A",{"type":"spacer"},"B"]});
    let nested = json!({"type":"row","props":{"gap":0,"wrap":"wrap"},"children":[col,col]});
    let node = tree(json!({"type":"row","props":{"gap":0},"children":[nested,"1\n2\n3\n4\n5\n6"]}));
    assert_eq!(
        painted_rows(&node, Size::new(2, 6)),
        ["A1", " 2", "B3", "A4", " 5", "B6"]
    );
}

#[test]
fn wrapped_spacer_bands_share_a_taller_siblings_height() {
    let spacer = json!({"type":"spacer"});
    let inner = json!({"type":"row","props":{"gap":0,"wrap":"wrap"},"children":["A",spacer,"B",spacer,"C"]});
    let node = tree(json!({"type":"row","props":{"gap":0},"children":[inner,"1\n2\n3\n4\n5\n6"]}));
    assert_eq!(
        painted_rows(&node, Size::new(2, 6)),
        ["A1", " 2", "B3", " 4", "C5", " 6"]
    );
}

#[test]
fn spacer_keeps_ordinary_gaps_on_authored_blank_lines() {
    let node = tree(
        json!({"type":"row","props":{"gap":1,"background":"red"},"children":["A\n\nB",{"type":"spacer"},"C\n\nD"]}),
    );
    let area = Rect::new(0, 0, 8, 3);
    let mut buf = Buffer::empty(area);
    node.render(area, &Theme::default(), &mut buf, /*plain_style*/ None);
    assert_eq!(buf[(4, 1)].bg, ratatui::style::Color::Red);
}

#[test]
fn badge_is_one_sanitized_inline_box() {
    let badge = json!({"type":"badge","children":[" \u{1b}[31mok\u{1b}[0m \t \n"]});
    let node = tree(json!({"type":"text","children":["A",badge,"B"]}));
    insta::assert_snapshot!(painted_rows(&node,Size::new(6,3)).join("\n"), @"A ok B");
    assert_eq!(painted_rows(&node, Size::new(4, 3)), ["A", " ok", "B"]);
    let node = tree(
        json!({"type":"code","children":["a",{"type":"badge","children":["  a\t \nb  "]},"c"]}),
    );
    assert_eq!(painted_rows(&node, Size::new(7, 1)), ["a a b c"]);
    let node = tree(
        json!({"type":"row","props":{"gap":0},"children":[{"type":"badge","children":[""]},"X"]}),
    );
    assert_eq!(painted_rows(&node, Size::new(3, 1)), ["  X"]);
}

#[test]
fn badge_labels_count_toward_tree_limits() {
    let badge = |children: Value| json!({"type":"badge","children":children});
    for children in [
        json!([]),
        json!(["a", "b"]),
        json!([null]),
        json!(["a".repeat(MAX_TEXT_BYTES + 1)]),
    ] {
        assert!(parse(badge(children)).is_err());
    }
    let mut node = badge(json!(["ok"]));
    for _ in 0..15 {
        node = json!({"type":"bold","children":[node]});
    }
    assert!(parse(node.clone()).is_ok());
    assert!(parse(json!({"type":"bold","children":[node]})).is_err());
    let badges = vec![badge(json!(["ok"])); 128];
    assert!(parse(json!({"type":"text","children":badges})).is_err());
}

#[test]
fn semantic_badge_paints_its_gutters_and_keeps_the_surrounding_style() {
    let palette = crate::terminal_probe::DefaultColors {
        fg: (255, 255, 255),
        bg: (0, 0, 0),
    };
    crate::terminal_palette::with_test_default_colors(palette, || {
        let mut styles = Vec::new();
        for color in ["secondary", "info", "success", "warning", "danger"] {
            let node = tree(
                json!({"type":"bold","children":["A",{"type":"badge","props":{"color":color},"children":["ok"]},"B"]}),
            );
            let area = Rect::new(0, 0, 6, 1);
            let mut buf = Buffer::empty(area);
            node.render(area, &Theme::default(), &mut buf, /*plain_style*/ None);
            let style = buf[(2, 0)].style();
            for x in 1..5 {
                assert_eq!(buf[(x, 0)].style(), style);
            }
            assert_eq!(buf[(0, 0)].style(), buf[(5, 0)].style());
            assert_ne!(style.fg, style.bg);
            assert!(!styles.contains(&style));
            styles.push(style);
        }
    });
}

#[test]
fn linked_and_plain_badges_share_colors_but_keep_distinct_underlines() {
    use ratatui::style::Modifier;

    let badge = json!({"type":"badge","children":["ok"]});
    let link = json!({"type":"link","props":{"href":"https://example.com"},"children":[badge]});
    for (children, linked, plain) in [(json!([link, badge]), 1, 5), (json!([badge, link]), 5, 1)] {
        let node = tree(json!({"type":"text","children":children}));
        let area = Rect::new(0, 0, 8, 1);
        let mut buf = Buffer::empty(area);
        node.render(area, &Theme::default(), &mut buf, /*plain_style*/ None);
        let linked = &buf[(linked, 0)];
        let plain = &buf[(plain, 0)];
        assert_eq!((linked.fg, linked.bg), (plain.fg, plain.bg));
        assert!(linked.modifier.contains(Modifier::UNDERLINED));
        assert!(!plain.modifier.contains(Modifier::UNDERLINED));
    }
}

#[test]
fn badge_uses_a_measurable_surface_for_its_fill() {
    use ratatui::style::Color as TermColor;

    let mut theme = Theme::default();
    theme.settings.foreground = Some("#cdd6f4".parse().unwrap());
    theme.settings.background = Some("#1e1e2e".parse().unwrap());
    let colors = |theme: &Theme, background: Option<&str>, role: &str| {
        let badge = json!({"type":"badge","props":{"color":role},"children":["X"]});
        let node = match background {
            Some(bg) => {
                tree(json!({"type":"row","props":{"gap":0,"background":bg},"children":[badge]}))
            }
            None => tree(badge),
        };
        let area = Rect::new(0, 0, 3, 1);
        let mut buf = Buffer::empty(area);
        node.render(area, theme, &mut buf, /*plain_style*/ None);
        let cell = &buf[(1, 0)];
        (cell.fg, cell.bg)
    };
    let palette = crate::terminal_probe::DefaultColors {
        fg: (255, 255, 255),
        bg: (16, 16, 16),
    };
    for (palette, surface) in [(Some(palette), "#101010"), (None, "#1e1e2e")] {
        crate::terminal_palette::with_test_default_colors(palette, || {
            let secondary = |bg| colors(&theme, bg, "secondary");
            assert_eq!(secondary(Some("white")), secondary(Some(surface)));
            assert_eq!(secondary(None), secondary(Some(surface)));
            assert_ne!(secondary(Some("white")), secondary(Some("#c0c0c0")));
            assert_eq!(
                colors(&theme, Some("#008000"), "success"),
                (TermColor::Green, TermColor::Reset)
            );
        });
    }
    theme.settings.background = None;
    crate::terminal_palette::with_test_default_colors(/*colors*/ None, || {
        assert_eq!(
            colors(&theme, Some("white"), "secondary"),
            (TermColor::Reset, TermColor::Reset)
        );
    });
}

#[test]
fn badge_text_and_fill_follow_theme_changes_on_the_painted_surface() {
    use crate::terminal_palette::rgb_color;
    use syntect::highlighting::StyleModifier;
    use syntect::highlighting::ThemeItem;

    let palette = crate::terminal_probe::DefaultColors {
        fg: (0, 0, 0),
        bg: (255, 255, 255),
    };
    crate::terminal_palette::with_test_default_colors(palette, || {
        for (role, scope) in [
            ("secondary", None),
            ("info", Some("codex.accent")),
            ("success", Some("markup.inserted.diff")),
            ("warning", Some("markup.changed.diff")),
            ("danger", Some("markup.deleted")),
        ] {
            let node = tree(
                json!({"type":"row","props":{"gap":0,"background":"#101010"},"children":[
                    {"type":"badge","props":{"color":role},"children":["X"]}
                ]}),
            );
            let mut theme = Theme::default();
            let name = format!("codex.{role}");
            // Paint with a conventional scope, then override it on the same prepared tree.
            for (scope, value) in [(scope, "#ffcc88"), (Some(name.as_str()), "#88ccff")] {
                let color: syntect::highlighting::Color = value.parse().unwrap();
                if let Some(scope) = scope {
                    theme.scopes.push(ThemeItem {
                        scope: scope.parse().unwrap(),
                        style: StyleModifier {
                            foreground: Some(color),
                            ..Default::default()
                        },
                    });
                } else {
                    theme.settings.foreground = Some(color);
                }
                let area = Rect::new(0, 0, 3, 1);
                let mut buf = Buffer::empty(area);
                node.render(area, &theme, &mut buf, /*plain_style*/ None);
                let rgb = (color.r, color.g, color.b);
                let fill = crate::color::blend(rgb, (16, 16, 16), /*alpha*/ 0.15);
                let cell = &buf[(1, 0)];
                assert_eq!(
                    (cell.fg, cell.bg),
                    (rgb_color(rgb), rgb_color(fill)),
                    "{role}"
                );
            }
        }
    });
}

#[test]
fn clamped_text_reserves_room_for_a_whole_trailing_badge() {
    let node = tree(
        json!({"type":"text","props":{"maxLines":1},"children":["A",{"type":"badge","children":["ok"]},"B"]}),
    );
    assert_eq!(painted_rows(&node, Size::new(5, 1)), ["A…"]);
    assert_eq!(painted_rows(&node, Size::new(6, 1)), ["A ok B"]);
    let node = tree(json!({"type":"badge","children":["A\rB"]}));
    assert_eq!(painted_rows(&node, Size::new(8, 1)), [" A B"]);
}

#[test]
fn clipped_badge_accounts_for_previous_row_children() {
    let badge = json!({"type":"badge","children":["ok"]});
    let row = json!({"type":"row","props":{"gap":0},"children":["A",badge]});
    let centered = json!({"type":"row","props":{"gap":0,"justify":"center"},"children":[badge]});
    for (line, width, expected) in [(row, 5, "A…"), (centered, 6, "  ok …")] {
        let col = json!({"type":"col","props":{"gap":0},"children":[line,"B"]});
        let footer = tree(col.clone());
        let clamped = tree(json!({"type":"text","props":{"maxLines":1},"children":[col]}));
        assert_eq!(painted_rows(&footer, Size::new(width, 1)), [expected]);
        assert_eq!(painted_rows(&clamped, Size::new(width, 2)), [expected]);
    }
}

#[test]
fn clipping_only_the_badge_gutter_keeps_the_whole_label() {
    let badge = json!({"type":"badge","children":["界"]});
    assert_eq!(painted_rows(&tree(badge.clone()), Size::new(3, 1)), [" 界"]);
    let row = tree(json!({"type":"row","props":{"gap":0},"children":["A",badge]}));
    assert_eq!(painted_rows(&row, Size::new(4, 1)), ["A 界"]);
    assert_eq!(painted_rows(&row, Size::new(3, 1)), ["A…"]);
}
