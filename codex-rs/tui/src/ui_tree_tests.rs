#![cfg(test)]

use super::*;
use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use serde_json::Value;
use serde_json::json;
use syntect::highlighting::Theme;

fn parse(value: Value) -> serde_json::Result<UiTree> {
    value.to_string().parse()
}

fn tree(value: Value) -> UiTree {
    parse(value).unwrap()
}

#[test]
fn elements_round_trip_and_require_explicit_row_spacing() {
    let value = json!({"type":"row","props":{"gap":1},"children":[{"type":"link","props":{"href":"https://example.com/pr/42","title":"PR details"},"children":["12%"]}, null, " "]});
    assert_eq!(serde_json::to_value(tree(value.clone())).unwrap(), value);
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
fn first_line_skips_hidden_text_before_rendering_its_sibling() {
    let multiline = format!("\u{1b}[2Jok\n{}", "x".repeat(1024));
    let tree = tree(json!({"type":"row","props":{"gap":0},"children":[
        &multiline, "!"
    ]}));
    assert_eq!(
        tree.render_line(/*width*/ 4, &Theme::default())
            .line
            .to_string(),
        "ok!"
    );
}

#[test]
fn directly_constructed_text_keeps_a_bounded_utf8_prefix() {
    let prefix = "x".repeat(MAX_TEXT_BYTES - 1);
    let text = UiTree(Some(UiNode::String(format!("{prefix}界tail"))));
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
        assert_eq!(serde_json::to_value(tree(value.clone())).unwrap(), value);
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
        node.render_line(area.width, &theme)
            .render_fitted(area, &mut buffer);
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
