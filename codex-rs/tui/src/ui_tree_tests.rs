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
        assert_eq!(
            row.render_line(/*width*/ 4, &Theme::default())
                .line
                .to_string(),
            expected
        );
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
    tree.render_line(area.width, &Theme::default())
        .render_fitted(area, &mut buffer);
    assert_eq!(buffer[(0, 0)].symbol(), "界");
    assert!(
        buffer[(3, 0)]
            .symbol()
            .contains("https://example.com/pr/42")
    );
    insta::assert_snapshot!(format!("{buffer:?}"));

    assert_eq!(
        tree.render_line(/*width*/ 1, &Theme::default())
            .line
            .to_string(),
        "…"
    );
    let narrow = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 6, /*height*/ 2,
    );
    let mut buffer = Buffer::empty(narrow);
    tree.render_line(narrow.width, &Theme::default())
        .render_fitted(narrow, &mut buffer);
    assert!(
        buffer[(4, 0)]
            .symbol()
            .contains("https://example.com/pr/42")
    );
    assert_eq!(buffer[(5, 0)].symbol(), "…");
    assert!((0..narrow.width).all(|x| buffer[(x, 1)].symbol() == " "));
}

#[test]
fn render_line_keeps_exactly_fitting_halfwidth_voiced_katakana() {
    let text = tree(json!("ｶﾞ"));
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 2, /*height*/ 1,
    );
    let mut buffer = Buffer::empty(area);
    text.render_line(area.width, &Theme::default())
        .render_fitted(area, &mut buffer);
    assert_eq!(buffer[(0, 0)].symbol(), "ｶﾞ");
}

#[test]
fn ellipsis_counts_toward_width_only_when_there_is_overflow() {
    let row = tree(json!({"type":"row","props":{"gap":1},"children":["abcde", null]}));
    for (width, expected) in [(0, ""), (1, "…"), (4, "abc…"), (5, "abcde")] {
        let line = row.render_line(width, &Theme::default());
        assert_eq!(line.line.to_string(), expected);
        assert!(line.width() <= usize::from(width));
    }
    let wide = tree(json!("a界界"));
    assert_eq!(
        wide.render_line(/*width*/ 4, &Theme::default())
            .line
            .to_string(),
        "a界…"
    );
    let long = "x".repeat(513);
    assert_eq!(
        tree(json!(&long))
            .render_line(/*width*/ 513, &Theme::default())
            .line
            .to_string(),
        long
    );
}

#[test]
fn zero_width_fragments_do_not_style_the_ellipsis_or_add_row_gaps() {
    let row = |text| json!({"type":"row","props":{"gap":0},"children":[text]});
    let mark = json!({"type":"bold","children":["\u{301}"]});
    for (mut root, width) in [
        (json!({"type":"text"}), 2),
        (json!({"type":"row","props":{"gap":1}}), 4),
    ] {
        root["children"] = json!([row("x"), mark, row("yz")]);
        let node = tree(root.clone());
        root["children"] = json!([row("x"), row("yz")]);
        assert_eq!(
            node.render_line(width, &Theme::default()),
            tree(root).render_line(width, &Theme::default())
        );
    }
    for (children, expected) in [
        (json!(["A", {"type":"bold","children":["\u{ad}BB"]}]), "A…"),
        (json!([{"type":"bold","children":["\u{ad}"]}, "AB"]), "…"),
    ] {
        let node = tree(json!({"type":"text","children":children}));
        let expected = Buffer::with_lines([expected]);
        let mut buf = Buffer::empty(expected.area);
        node.render(buf.area, &Theme::default(), &mut buf);
        assert_eq!(buf, expected);
    }
}

#[test]
fn single_line_view_marks_vertical_overflow() {
    let invisible = tree(json!({"type":"bold","children":["\u{ad}\na"]}));
    let clipped = invisible.render_line(/*width*/ 1, &Theme::default());
    assert_eq!(clipped.line.to_string(), "…");
    let multiline = format!("\u{1b}[2Jok\n{}", "x".repeat(1024));
    let tree = tree(json!({"type":"row","props":{"gap":0},"children":[
        &multiline, "!"
    ]}));
    assert_eq!(
        tree.render_line(/*width*/ 4, &Theme::default())
            .line
            .to_string(),
        "ok …"
    );
}

#[test]
fn directly_constructed_text_keeps_a_bounded_utf8_prefix() {
    let prefix = "x".repeat(MAX_TEXT_BYTES - 1);
    let text = UiTree(Some(UiNode::String(format!("{prefix}界tail")))).prepare();
    assert_eq!(
        text.render_line(u16::MAX, &Theme::default())
            .line
            .to_string(),
        prefix
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
    let clipped = node.render_line(/*width*/ 6, &Theme::default());
    assert_eq!(
        clipped.line.spans.last().unwrap(),
        &ratatui::text::Span::styled(
            "…",
            ratatui::style::Style::default().bg(ratatui::style::Color::Blue),
        )
    );
    let line = node.render_line(/*width*/ 12, &Theme::default());
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 12, /*height*/ 1,
    );
    let mut buffer = Buffer::empty(area);
    line.render_fitted(area, &mut buffer);
    insta::assert_snapshot!(format!("{buffer:?}"));
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
            let line = tree.render_line(/*width*/ 3, &Theme::default());
            let colors: Vec<_> = line
                .line
                .spans
                .iter()
                .map(|s| (s.style.fg, s.style.bg))
                .collect();
            let surface = Some(crate::terminal_palette::rgb_color((16, 32, 240)));
            let foreground = Some(crate::terminal_palette::rgb_color((171, 18, 240)));
            assert_eq!(
                colors,
                [(foreground, surface), (None, surface), (None, surface)]
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
        node.render(area, &theme, &mut buffer);
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
    let line = node.render_line(/*width*/ 40, &Theme::default());
    assert_eq!(
        line.hyperlinks,
        [crate::terminal_hyperlinks::TerminalHyperlink::web(
            10..15,
            url.to_owned()
        )]
    );
    insta::assert_snapshot!(line.line.to_string(), @"red label Ready");
}

#[test]
fn split_graphemes_use_the_first_style_and_require_unanimous_links() {
    use crate::terminal_hyperlinks::TerminalHyperlink;
    use ratatui::style::Stylize;
    use ratatui::text::Span;

    let url = "https://example.com/a";
    let link = |href, text| json!({"type":"link","props":{"href":href},"children":[text]});
    let node = tree(json!({"type":"text","children":[
        {"type":"bold","children":[{"type":"row","props":{"gap":0},"children":["R"]},"e"]}, {"type":"italic","children":[]}, "\u{301}",
        link(url, "👩"), link(url, "\u{200d}💻"),
        link(url, "👩"), link("https://example.com/b", "\u{200d}💻"),
        {"type":"bold","children":["\u{600}"]}, " x",
        link(url, "तक्क"), "\u{301}"
    ]}));
    let line = node.render_line(/*width*/ 20, &Theme::default());
    assert_eq!(
        line.line.spans,
        [
            Span::raw("R").bold(),
            Span::raw("e\u{301}").bold(),
            Span::raw("👩‍💻").underlined(),
            Span::raw("👩‍💻").underlined(),
            Span::raw("\u{600} ").bold(),
            Span::raw("x"),
            Span::raw("त").underlined(),
            Span::raw("क्क\u{301}").underlined()
        ]
    );
    assert_eq!(
        line.hyperlinks,
        [
            TerminalHyperlink::web(2..4, url.to_owned()),
            TerminalHyperlink::web(9..10, url.to_owned())
        ]
    );
}

#[test]
fn collapsed_space_keeps_the_first_whitespace_owners_link() {
    use crate::terminal_hyperlinks::TerminalHyperlink;

    let href = "https://example.com/build";
    let node = tree(json!({"type":"text","children":[
        "a", {"type":"link","props":{"href":href},"children":["\t"]},
        "\u{1b}[31m", "  b", {"type":"link","props":{"href":href},"children":[" "]},
        {"type":"row","props":{"gap":0},"children":[" c "]}, " d"
    ]}));
    let line = node.render_line(/*width*/ 10, &Theme::default());
    assert_eq!(line.line.to_string(), "a bcd");
    assert_eq!(
        line.hyperlinks,
        [TerminalHyperlink::web(1..2, href.to_owned())]
    );
}

fn painted_rows(node: &PreparedUiTree, bounds: Size) -> Vec<String> {
    let area = Rect::from((ratatui::layout::Position::ORIGIN, bounds));
    let mut buf = Buffer::empty(area);
    node.render(area, &Theme::default(), &mut buf);
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
fn measures_natural_and_viewport_sizes_in_cells() {
    let node =
        tree(json!({"type":"row","props":{"gap":1,"wrap":"wrap"},"children":["ab\ncd","界界"]}));
    assert_eq!(node.measure_natural(), Size::new(7, 2));
    assert_eq!(node.measure(Size::new(4, 8)), Size::new(4, 3));
    let wide = tree(json!("a界\nnext"));
    assert_eq!(wide.measure(Size::new(3, 1)), Size::new(2, 1));
    assert_eq!(painted_rows(&wide, Size::new(3, 1)), ["a…"]);
    let huge =
        tree(json!({"type":"row","props":{"gap":MAX_GAP,"wrap":"wrap"},"children":["x","y"]}));
    assert_eq!(huge.measure_natural(), Size::new(u16::MAX, 1));
    assert_eq!(tree(Value::Null).measure_natural(), Size::default());
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
fn reads_row_wrap_without_moving_existing_positional_props() {
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
    let node = tree(json!({"type":"row","props":{"gap":1},"children":["aa-bb", "!"]}));
    assert_eq!(painted_rows(&node, Size::new(6, 2)), ["aa-  !", "bb"]);
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
    node.render(area, &Theme::default(), &mut buffer);
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
    assert_eq!(
        node.render_line(/*width*/ 20, &Theme::default())
            .line
            .to_string(),
        "ready now…"
    );
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
