//! Regression coverage for conflict diagnostics and validation pass ordering.

use super::RuntimeKeymap;
use codex_config::types::TuiKeymap;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;

#[test]
fn conflicting_contexts_report_the_first_conflict_in_validation_order() {
    let contexts = [
        ("editor", "insert_newline", "yank"),
        ("vim_normal", "enter_insert", "cancel_operator"),
        ("vim_operator", "delete_line", "cancel"),
        ("vim_text_object", "word", "cancel"),
        ("pager", "scroll_up", "close_transcript"),
        ("list", "move_up", "cancel"),
        ("agents", "search", "toggle_grouping"),
        ("approval", "open_fullscreen", "cancel"),
    ];
    let mut config = serde_json::Map::new();
    for (context, first, second) in contexts {
        config.insert(context.to_string(), json!({first: "f12", second: "f12"}));
    }

    for (context, first, second) in contexts {
        let keymap: TuiKeymap = serde_json::from_value(Value::Object(config.clone()))
            .expect("valid keymap configuration");
        assert_eq!(
            RuntimeKeymap::from_config(&keymap).expect_err("expected binding conflict"),
            format!(
                "Ambiguous `tui.keymap.{context}` bindings: `{first}` and `{second}` use the same key. \
Set unique keys in `~/.codex/config.toml` and retry. \
See the Codex keymap documentation for supported actions and examples."
            )
        );
        config.remove(context);
    }
}

#[test]
fn new_agents_defaults_preserve_existing_custom_bindings() {
    for context in ["agents", "list"] {
        let existing = if context == "agents" {
            "stop"
        } else {
            "move_down"
        };
        let keymap: TuiKeymap =
            serde_json::from_value(json!({context: {existing: "ctrl-w"}})).unwrap();
        let runtime =
            RuntimeKeymap::from_config(&keymap).expect("existing configuration remains valid");
        assert!(runtime.agents.hide.is_empty());
    }
}
