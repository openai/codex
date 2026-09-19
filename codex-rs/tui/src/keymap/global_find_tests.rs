//! Global Find joins the existing chord inventory without replacing paging or user bindings.

use super::*;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn global_find_resolves_remaps_unbinding_and_dispatch_in_main_and_activity_contexts() {
    for (configured, label) in [
        (json!("f12"), Some("f12")),
        (json!([]), None),
        (json!(["ctrl-x f", "f12"]), Some("ctrl+x f")),
    ] {
        let config =
            serde_json::from_value(json!({"global":{"find_transcript":configured}})).unwrap();
        let runtime = RuntimeKeymap::from_config(&config).unwrap();
        assert_eq!(
            runtime
                .primary_hint(KeymapContext::Global, "find_transcript")
                .map(crate::key_hint::ShortcutHint::display_label),
            label.map(str::to_owned)
        );
        assert!(!runtime.app.find_transcript.is_pressed(KeyCode::F(3).into()));
        assert!(runtime.pager.find.is_pressed(KeyCode::F(3).into()));
        assert!(runtime.pager.find.is_pressed(KeyCode::Char('/').into()));
        assert!(
            runtime
                .pager
                .page_down
                .is_pressed(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL))
        );
    }
    let config = serde_json::from_value(json!({"global":{"find_transcript":"ctrl-x f"}})).unwrap();
    let runtime = RuntimeKeymap::from_config(&config).unwrap();
    for context in [
        KeymapContextSet::new(KeymapContext::Global),
        KeymapContextSet::activity(),
    ] {
        let mut matcher = KeyChordMatcher::default();
        assert!(matches!(
            matcher.advance(
                KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
                &runtime.chords,
                context
            ),
            KeyChordMatch::Pending(_)
        ));
        let KeyChordMatch::Completed(key) =
            matcher.advance(KeyCode::Char('f').into(), &runtime.chords, context)
        else {
            panic!("Find chord")
        };
        assert!(runtime.app.find_transcript.is_pressed(key));
    }
}

#[test]
fn new_find_defaults_yield_to_existing_keys_and_prefixes_but_explicit_conflicts_fail() {
    for (context, action) in [
        ("global", "open_transcript"),
        ("editor", "move_line_start"),
        ("list", "move_up"),
        ("approval", "approve"),
    ] {
        for binding in ["f3", "f3 x"] {
            let config = serde_json::from_value(json!({context:{action:binding}})).unwrap();
            let runtime = RuntimeKeymap::from_config(&config)
                .expect("existing binding wins over new default");
            assert!(!runtime.app.find_transcript.is_pressed(KeyCode::F(3).into()));
        }
        let config = serde_json::from_value(
            json!({"global":{"find_transcript":"f12"},context:{action:"f12"}}),
        )
        .unwrap();
        // Distinct JSON global keys would overwrite each other, so compose that case explicitly.
        let config = if context == "global" {
            serde_json::from_value(
                json!({"global":{"find_transcript":"f12","open_transcript":"f12"}}),
            )
            .unwrap()
        } else {
            config
        };
        let error = RuntimeKeymap::from_config(&config)
            .expect_err("explicit simultaneous bindings conflict");
        assert!(
            error.contains("find_transcript") && error.contains(action),
            "{error}"
        );
    }
}
