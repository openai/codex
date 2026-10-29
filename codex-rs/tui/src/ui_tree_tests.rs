#![cfg(test)]

use super::*;
use pretty_assertions::assert_eq;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use serde_json::json;

fn tree(value: serde_json::Value) -> UiNode {
    serde_json::from_value::<UiTree>(value).unwrap().unwrap()
}

fn text(key: &str, value: &str) -> serde_json::Value {
    json!({"type":"text","key":key,"props":{"text":value}})
}

#[test]
fn normalized_tree_round_trips_and_enforces_leaf_shape() {
    let value = json!({"type":"row","key":7,"props":{"gap":2.0},"children":[{"type":"link","props":{"href":"https://example.com/pr/42"},"children":[text("usage", "12%")]}, null, {"type":"spacer"}]});
    assert_eq!(serde_json::to_value(tree(value.clone())).unwrap(), value);
    assert!(
        serde_json::from_value::<UiTree>(
            json!({"type":"text","props":{"text":"hello"},"children":[]})
        )
        .is_err()
    );
    assert_eq!(serde_json::from_value::<UiTree>(json!(null)).unwrap(), None);
    for href in [
        "relative/path",
        "file:///tmp/example",
        "https://example.com/\u{001b}",
    ] {
        assert!(
            serde_json::from_value::<UiTree>(
                json!({"type":"link","props":{"href":href},"children":[]})
            )
            .is_err()
        );
    }
}

#[test]
fn links_only_annotate_their_own_cells() {
    let tree = tree(
        json!({"type":"row","props":{"gap":2},"children":[text("prefix", "界"),{"type":"link","props":{"href":"https://example.com/pr/42"},"children":[{"type":"row","props":{},"children":[text("pr", "PR #42")]}]}]}),
    );
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 16, /*height*/ 1,
    );
    let mut buffer = Buffer::empty(area);
    tree.render_line(area.width)
        .render_fitted(area, &mut buffer);
    assert_eq!(buffer[(0, 0)].symbol(), "界");
    assert!(
        buffer[(3, 0)]
            .symbol()
            .contains("https://example.com/pr/42")
    );
    insta::assert_snapshot!(format!("{buffer:?}"));

    assert_eq!(tree.render_line(/*width*/ 1).line.to_string(), "…");
    let narrow = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 6, /*height*/ 2,
    );
    let mut buffer = Buffer::empty(narrow);
    tree.render_line(narrow.width)
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
fn extension_status_line_keeps_exactly_fitting_halfwidth_voiced_katakana() {
    let status = tree(text("name", "ｶﾞ"));
    let area = Rect::new(
        /*x*/ 0, /*y*/ 0, /*width*/ 2, /*height*/ 1,
    );
    let mut buffer = Buffer::empty(area);
    status
        .render_line(area.width)
        .render_fitted(area, &mut buffer);
    assert_eq!(buffer[(0, 0)].symbol(), "ｶﾞ");
}

#[test]
fn ellipsis_counts_toward_width_only_when_there_is_overflow() {
    let status =
        tree(json!({"type":"row","props":{"gap":2},"children":[text("label","abcde"), null]}));
    for (width, expected) in [(0, ""), (1, "…"), (4, "abc…"), (5, "abcde")] {
        let line = status.render_line(width);
        assert_eq!(line.line.to_string(), expected);
        assert!(line.width() <= usize::from(width));
    }
    let wide = tree(text("label", "a界界"));
    assert_eq!(wide.render_line(/*width*/ 4).line.to_string(), "a界…");
    let long = "x".repeat(513);
    assert_eq!(
        tree(text("label", &long))
            .render_line(/*width*/ 513)
            .line
            .to_string(),
        long
    );
}

#[test]
fn first_line_skips_hidden_text_before_rendering_its_sibling() {
    let multiline = format!("\u{1b}[2Jok\n{}", "x".repeat(16 * 1024));
    let tree = tree(json!({"type":"row","props":{},"children":[
        text("first", &multiline), text("sibling", "!")
    ]}));
    assert_eq!(tree.render_line(/*width*/ 4).line.to_string(), "ok!");
}
