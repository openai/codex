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
    for (gap, expected) in [(0, "AB"), (1, "A B"), (2, "A  B"), (MAX_GAP, "A  …")] {
        let row = tree(json!({"type":"row","props":{"gap":gap},"children":["A","B"]}));
        assert_eq!(painted_rows(&row, Size::new(4, 1)), [expected]);
    }
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
        assert_eq!(row(json!("\n")).measure(Size::new(5, 2)), Size::new(0, 1));
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
                "children":[first,{"type":"text","children":["\n"]}]}),
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
