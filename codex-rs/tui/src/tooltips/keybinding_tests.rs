//! Tooltip placeholders follow current keybindings and chords, render safely as Markdown,
//! and suppress tips when any referenced shortcut is invalid or unbound.

use super::*;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn configured_shortcut_tips_render_at_narrow_width() {
    let config = serde_json::from_value(json!({
        "global": {
            "open_transcript": ["ctrl-x t", "f12"],
            "find_transcript": "ctrl-x f",
            "open_external_editor": "ctrl-x e",
            "copy": "ctrl-x `"
        },
        "composer": {
            "history_search_previous": "ctrl-x r",
            "queue": "ctrl-x q"
        },
        "chat": {
            "increase_reasoning_effort": "ctrl-x +",
            "decrease_reasoning_effort": "ctrl-x minus"
        }
    }))
    .unwrap();
    let keymap = RuntimeKeymap::from_config(&config).unwrap();
    let mut lines = Vec::new();
    for (label, keymap) in [("Default", RuntimeKeymap::defaults()), ("Remapped", keymap)] {
        lines.push(label.into());
        for template in TOOLTIPS.iter().filter(|tip| tip.contains("{key:")) {
            let tip = render_tooltip(template, Some(&keymap)).expect("valid shortcut tip");
            crate::markdown::append_markdown(
                &format!("**Tip:** {tip}"),
                Some(40),
                /*cwd*/ None,
                &mut lines,
            );
            lines.push("".into());
        }
    }
    insta::assert_snapshot!(
        lines
            .iter()
            .map(|line| format!("{line:?}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn skips_unbound_or_invalid_placeholders() {
    let config = serde_json::from_value(json!({ "global": { "copy": [] } })).unwrap();
    let keymap = RuntimeKeymap::from_config(&config).unwrap();
    for template in [
        "Press {key:global.copy} to copy.",
        "Press {key:global.open_transcript} or {key:global.copy}.",
        "Press {key:global.missing}.",
        "Press {key:missing.copy}.",
        "Press {key:copy}.",
        "Press {key:global.copy",
    ] {
        assert_eq!(render_tooltip(template, Some(&keymap)), None, "{template}");
    }
    assert_eq!(
        render_tooltip("Press {key:global.copy}.", /*keymap*/ None),
        None
    );
    assert_eq!(
        render_tooltip("Use /copy to copy a response.", /*keymap*/ None),
        Some("Use /copy to copy a response.".to_string())
    );
}

#[test]
fn reasoning_tip_requires_both_bindings() {
    let template = TOOLTIPS
        .iter()
        .find(|tip| tip.contains("{key:chat.increase_reasoning_effort}"))
        .unwrap();
    for action in ["increase_reasoning_effort", "decrease_reasoning_effort"] {
        let config = serde_json::from_value(json!({ "chat": { action: [] } })).unwrap();
        let keymap = RuntimeKeymap::from_config(&config).unwrap();
        assert_eq!(render_tooltip(template, Some(&keymap)), None);
    }
}

#[test]
fn subsequent_tips_reflect_remapped_bindings() {
    for (configured, label) in [
        (json!("f12"), Some("f12")),
        (json!(["ctrl-x t", "f12"]), Some("ctrl+x t")),
        (json!([]), None),
    ] {
        let config =
            serde_json::from_value(json!({ "global": { "open_transcript": configured } })).unwrap();
        let keymap = RuntimeKeymap::from_config(&config).unwrap();
        assert_eq!(
            resolved_tooltips(Some(&keymap))
                .find(|tip| tip.ends_with(" to open the full transcript.")),
            label.map(|label| format!("Press `` {label} `` to open the full transcript."))
        );
    }
}
