#![cfg(test)]
//! Skill completion and its rendering in both mention pickers.

use super::tests::new_test_composer;
use super::tests::snapshot_composer_state;
use super::tests::test_skill_binding;
use super::tests::test_skill_metadata;
use super::*;
use pretty_assertions::assert_eq;

#[test]
fn skill_completion_attaches_typed_punctuation_snapshot() {
    snapshot_composer_state(
        "skill_completion_attaches_typed_punctuation",
        /*enhanced_keys_supported*/ false,
        |composer| {
            composer.set_skill_mentions(Some(vec![test_skill_metadata("rustdoc")]));
            composer.insert_str("请用 $ru");
            composer.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
            composer.handle_key_event(KeyEvent::new(KeyCode::Char('》'), KeyModifiers::NONE));
            let recovered = composer.recovery_snapshot();
            let expected = "请用 $rustdoc》";
            assert_eq!(
                (recovered.text.as_str(), recovered.cursor),
                (expected, expected.len())
            );
            composer.handle_paste_burst_flush(
                tokio::time::Instant::now().into_std() + PasteBurst::recommended_flush_delay(),
            );
            assert_eq!(
                composer.current_text_elements(),
                vec![TextElement::new((7..15).into(), Some("$rustdoc".into()))]
            );
            assert_eq!(
                composer.mention_bindings(),
                vec![test_skill_binding("rustdoc")]
            );
        },
    );
}

#[test]
fn unified_skill_completion_keeps_draft_spacing_after_punctuation() {
    for (suffix, ch, modifiers, expected) in [
        ("", ',', KeyModifiers::NONE, "$rustdoc,"),
        ("", '!', KeyModifiers::SHIFT, "$rustdoc!"),
        (
            " then continue",
            '!',
            KeyModifiers::SHIFT,
            "$rustdoc! then continue",
        ),
        (" ", '!', KeyModifiers::SHIFT, "$rustdoc !"),
        ("", '。', KeyModifiers::NONE, "$rustdoc。"),
        ("", '，', KeyModifiers::NONE, "$rustdoc，"),
        ("", '、', KeyModifiers::NONE, "$rustdoc、"),
        ("", '！', KeyModifiers::NONE, "$rustdoc！"),
        ("", '？', KeyModifiers::NONE, "$rustdoc？"),
        ("", '》', KeyModifiers::NONE, "$rustdoc》"),
        ("", '/', KeyModifiers::NONE, "$rustdoc /"),
        ("", '\\', KeyModifiers::NONE, "$rustdoc \\"),
    ] {
        let (mut composer, _rx) = new_test_composer();
        composer.set_mentions_v2_enabled(/*enabled*/ true);
        composer.set_skill_mentions(Some(vec![test_skill_metadata("rustdoc")]));
        composer.set_text_content(format!("@ru{suffix}"), Vec::new(), Vec::new());
        composer.set_current_cursor("@ru".len());
        composer.sync_popups();
        composer.handle_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        composer.handle_key_event(KeyEvent::new(KeyCode::Char(ch), modifiers));
        composer.handle_paste_burst_flush(
            tokio::time::Instant::now().into_std() + PasteBurst::recommended_flush_delay(),
        );

        assert_eq!(composer.current_text(), expected);
        assert_eq!(
            composer.mention_bindings(),
            vec![test_skill_binding("rustdoc")]
        );
    }
}

#[test]
fn skill_completion_keeps_space_after_cursor_navigation() {
    let (mut composer, _rx) = new_test_composer();
    composer.set_disable_paste_burst(/*disabled*/ true);
    composer.set_skill_mentions(Some(vec![test_skill_metadata("rustdoc")]));
    composer.insert_str("$ru");
    for key in [
        KeyCode::Tab,
        KeyCode::Left,
        KeyCode::Right,
        KeyCode::Char(','),
    ] {
        composer.handle_key_event(KeyEvent::new(key, KeyModifiers::NONE));
    }

    assert_eq!(composer.current_text(), "$rustdoc ,");
}

#[tokio::test(start_paused = true)]
async fn skill_completion_keeps_space_before_a_punctuation_paste_burst() {
    for (ch, expected) in [(',', "$rustdoc ,x\n"), ('。', "$rustdoc 。x\n")] {
        let (mut composer, _rx) = new_test_composer();
        composer.set_skill_mentions(Some(vec![test_skill_metadata("rustdoc")]));
        composer.insert_str("$ru");
        for key in [
            KeyCode::Tab,
            KeyCode::Char(ch),
            KeyCode::Char('x'),
            KeyCode::Enter,
        ] {
            let (result, _) = composer.handle_key_event(KeyEvent::new(key, KeyModifiers::NONE));
            assert!(matches!(result, InputResult::None));
        }
        tokio::time::advance(PasteBurst::recommended_active_flush_delay()).await;
        composer.flush_paste_burst_if_due();

        assert_eq!(composer.current_text(), expected);
    }
}

#[test]
fn default_unified_mention_popup_snapshot() {
    snapshot_composer_state(
        "default_unified_mention_popup",
        /*enhanced_keys_supported*/ false,
        |composer| {
            let features = codex_features::Features::with_defaults();
            composer.set_mentions_v2_enabled(features.enabled(codex_features::Feature::MentionsV2));
            composer.set_skill_mentions(Some(vec![
                SkillMetadata {
                    name: "sample-tools:search".to_string(),
                    plugin_id: Some("sample@test".to_string()),
                    ..test_skill_metadata("sample-search")
                },
                SkillMetadata {
                    name: "sample:summarize".to_string(),
                    plugin_id: Some("sample@test".to_string()),
                    ..test_skill_metadata("sample-summarize")
                },
                SkillMetadata {
                    name: "sample:notes".to_string(),
                    ..test_skill_metadata("sample-notes")
                },
                SkillMetadata {
                    name: "sample:other".to_string(),
                    plugin_id: Some("sample@other-marketplace".to_string()),
                    ..test_skill_metadata("sample-other")
                },
            ]));
            composer.set_text_content("@sa".to_string(), Vec::new(), Vec::new());
            composer.set_plugin_mentions(Some(vec![PluginCapabilitySummary {
                config_name: "sample@test".to_string(),
                display_name: "Sample Plugin".to_string(),
                plugin_namespace: Some("sample".to_string()),
                description: Some("Plugin with skills and an MCP server".to_string()),
                has_skills: true,
                mcp_server_names: vec!["sample".to_string()],
                app_connector_ids: Vec::new(),
            }]));
        },
    );
}
