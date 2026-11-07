#![cfg(test)]

use super::PreparedNode;
use crate::ui_tree::UiNode;
use crate::ui_tree::theme::RenderTheme;
use pretty_assertions::assert_eq;
use ratatui::style::Style;
use serde_json::json;
use syntect::highlighting::Theme;

#[test]
fn a_row_measures_whole_children_and_intervening_gaps() {
    let node: UiNode = serde_json::from_value(json!({
        "type": "row", "props": { "gap": 2 }, "children": [
            {"type":"text","children":[{"type":"bold","children":["bu"]},"ild ready"]}, // codespell:ignore bu
            null, "", "\u{301}\n", {"type":"row","props":{"gap":1},"children":["中","PR\u{a0}2"]}
        ]
    }))
    .unwrap();
    let theme = Theme::default();
    let node = PreparedNode::from_node(
        &node,
        Style::default(),
        /*href*/ None,
        &RenderTheme::new(&theme),
    )
    .unwrap();
    assert_eq!((node.widths.minimum, node.widths.preferred), (16, 22));
}
