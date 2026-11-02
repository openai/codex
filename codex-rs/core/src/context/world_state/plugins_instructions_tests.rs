//! Covers generic plugin wording transitions and recovery from retained developer history.
#![cfg(test)]

use super::*;
use crate::context::ContextualUserFragment;
use crate::context::world_state::PreviousSectionState;
use crate::context::world_state::WorldState;
use crate::context::world_state::WorldStateSnapshot;
use crate::context::world_state::test_support::render_section_cases;
use codex_prompts::ResolvedGenericPluginMessages;
use codex_prompts::ResolvedModelMessages;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use pretty_assertions::assert_eq;
use serde_json::json;

fn resolve(intro: Option<&str>, usage: Option<&str>) -> ResolvedGenericPluginMessages {
    let mut model = codex_models_manager::model_info::model_info_from_slug("gpt-5.5");
    model.model_messages = Some(
        serde_json::from_value(json!({"plugins": {"generic": {
            "intro": intro,
            "usage_instructions": usage,
        }}}))
        .unwrap(),
    );
    ResolvedModelMessages::from_model(&model).generic_plugin()
}

fn expected_body(intro: &str, usage: &str) -> String {
    format!("\n## Plugins\n{intro}\n### How to use plugins\n{usage}\n")
}

#[test]
fn snapshots() {
    use PreviousSectionState::Absent;
    use PreviousSectionState::Known;
    use PreviousSectionState::Unknown;

    let bundled = ResolvedModelMessages::bundled().generic_plugin();
    let unavailable = PluginsInstructionsState::new(
        /*available*/ false,
        bundled.clone(),
        std::iter::empty(),
    );
    let available =
        PluginsInstructionsState::new(/*available*/ true, bundled, std::iter::empty());

    insta::assert_snapshot!(render_section_cases(&[
        (Absent, Absent),
        (Absent, Known(&unavailable)),
        (Absent, Known(&available)),
        (Known(&unavailable), Known(&available)),
        (Known(&available), Known(&available)),
        (Known(&available), Known(&unavailable)),
        (Unknown, Known(&unavailable)),
        (Unknown, Known(&available)),
    ]));
}

#[test]
fn legacy_guidance_is_not_injected_again() {
    let legacy: ResponseItem =
        ContextualUserFragment::into(AvailablePluginsInstructions::default());
    let mut world_state = WorldState::default();
    world_state.add_section(PluginsInstructionsState::new(
        /*available*/ true,
        ResolvedModelMessages::bundled().generic_plugin(),
        std::iter::once(&legacy),
    ));

    assert!(
        world_state
            .render_history_fragment_diff(/*previous*/ None, &[legacy])
            .1
            .is_empty()
    );
}

#[test]
fn persisted_guidance_is_restored_only_when_missing_from_history() {
    let retained: ResponseItem =
        ContextualUserFragment::into(AvailablePluginsInstructions::default());
    for history in [vec![], vec![retained]] {
        let mut world_state = WorldState::default();
        world_state.add_section(PluginsInstructionsState::new(
            /*available*/ true,
            ResolvedModelMessages::bundled().generic_plugin(),
            history.iter(),
        ));
        let snapshot = world_state.render_full().0;
        assert_eq!(
            world_state
                .render_history_fragment_diff(Some(&snapshot), &history)
                .1
                .len(),
            usize::from(history.is_empty())
        );
    }
}

#[test]
fn catalog_text_equal_to_bundled_changes_provenance_without_appending_guidance() {
    let bundled = ResolvedModelMessages::bundled().generic_plugin();
    let mut world_state = WorldState::default();
    world_state.add_section(PluginsInstructionsState::new(
        /*available*/ true,
        bundled.clone(),
        std::iter::empty(),
    ));
    let (mut previous, initial) =
        world_state.render_history_fragment_diff(/*previous*/ None, &[]);
    let history = initial
        .into_iter()
        .map(ContextualUserFragment::into_boxed_response_item)
        .collect::<Vec<_>>();
    assert_eq!(
        history,
        vec![ContextualUserFragment::into(
            AvailablePluginsInstructions::default()
        )]
    );
    let fingerprint =
        serde_json::to_value(&previous).unwrap()["plugins_instructions"]["instructions"].clone();
    assert!(!fingerprint.is_null());
    for (resolved, expected_catalog) in [
        (
            resolve(Some(&bundled.intro), Some(&bundled.usage_instructions)),
            true,
        ),
        (
            resolve(Some(&bundled.intro), Some(&bundled.usage_instructions)),
            true,
        ),
        (resolve(/*intro*/ None, /*usage*/ None), false),
    ] {
        assert_eq!(resolved.catalog_override, expected_catalog);
        let mut current = WorldState::default();
        current.add_section(PluginsInstructionsState::new(
            /*available*/ true,
            resolved,
            history.iter(),
        ));
        let (snapshot, updates) = current.render_history_fragment_diff(Some(&previous), &history);
        assert_eq!(
            updates
                .into_iter()
                .map(ContextualUserFragment::into_boxed_response_item)
                .collect::<Vec<_>>(),
            Vec::<ResponseItem>::new()
        );
        assert_eq!(
            serde_json::to_value(&snapshot).unwrap(),
            json!({"plugins_instructions": {"instructions": fingerprint, "catalog": expected_catalog}})
        );
        previous = snapshot;
    }
}

#[test]
fn catalog_guidance_changes_falls_back_and_retires_without_rewriting_history() {
    let bundled = AvailablePluginsInstructions::default().0;
    let body_a = expected_body("Intro A", "Usage A");
    let body_b = expected_body("Intro B", "Usage B");
    let replacement_a = format!("{REPLACEMENT_PREFIX}{body_a}");
    let replacement_b = format!("{REPLACEMENT_PREFIX}{body_b}");
    let fallback = format!("{REPLACEMENT_PREFIX}{bundled}");
    let oversized_intro = "x".repeat(501);
    let oversized_usage = "y".repeat(2_001);
    let mut history = Vec::new();
    let mut previous = None;
    for (available, intro, usage, expected) in [
        (
            true,
            Some("Intro A"),
            Some("Usage A"),
            Some(body_a.as_str()),
        ),
        (true, Some("Intro A"), Some("Usage A"), None),
        (
            true,
            Some("Intro B"),
            Some("Usage B"),
            Some(replacement_b.as_str()),
        ),
        (true, Some(""), Some(" \n"), Some(fallback.as_str())),
        (
            true,
            Some(oversized_intro.as_str()),
            Some(oversized_usage.as_str()),
            None,
        ),
        (true, None, None, None),
        // Bundled-only availability changes retain the legacy quiet removal.
        (false, None, None, None),
        (true, None, None, Some(bundled.as_str())),
        // Valid catalog intent still retires bundled text when availability is off.
        (false, Some("Intro B"), Some("Usage B"), Some(REMOVAL_BODY)),
        (false, Some("Intro B"), Some("Usage B"), None),
        (
            true,
            Some("Intro B"),
            Some("Usage B"),
            Some(body_b.as_str()),
        ),
        (false, Some("Intro B"), Some("Usage B"), Some(REMOVAL_BODY)),
        // Empty plugin fields return to the bundled body, rather than clearing it.
        (true, Some(""), Some(""), Some(bundled.as_str())),
        (
            true,
            Some("Intro A"),
            Some("Usage A"),
            Some(replacement_a.as_str()),
        ),
    ] {
        let mut world_state = WorldState::default();
        world_state.add_section(PluginsInstructionsState::new(
            available,
            resolve(intro, usage),
            history.iter(),
        ));
        let (snapshot, fragments) =
            world_state.render_history_fragment_diff(previous.as_ref(), &history);
        let updates = fragments
            .into_iter()
            .map(ContextualUserFragment::into_boxed_response_item)
            .collect::<Vec<_>>();
        assert_eq!(
            updates,
            expected
                .map(
                    |body| ContextualUserFragment::into(AvailablePluginsInstructions(
                        body.to_string()
                    ))
                )
                .into_iter()
                .collect::<Vec<_>>()
        );
        if !available {
            // A previous client's boolean reader must restore bundled guidance once plugins
            // become available again, including after a catalog retirement notice.
            let mut replayed = previous.clone().unwrap_or_default();
            if let Some(patch) = snapshot.merge_patch_from(&replayed) {
                replayed.apply_merge_patch(&patch);
            }
            let availability: bool = serde_json::from_value(
                replayed.into_object()[PluginsInstructionsState::ID].clone(),
            )
            .expect("a previous client can read the persisted plugin availability");
            assert!(!availability);
        }
        history.extend(updates);
        previous = Some(snapshot);
    }
}

#[test]
fn a_lost_catalog_retirement_is_repaired_after_switching_to_a_bundled_model() {
    fn render(
        available: bool,
        intro: Option<&str>,
        history: &[ResponseItem],
        previous: &WorldStateSnapshot,
    ) -> (WorldStateSnapshot, Vec<ResponseItem>) {
        let mut world = WorldState::default();
        world.add_section(PluginsInstructionsState::new(
            available,
            resolve(intro, /*usage*/ None),
            history.iter(),
        ));
        let (snapshot, updates) = world.render_history_fragment_diff(Some(previous), history);
        (
            snapshot,
            updates
                .into_iter()
                .map(ContextualUserFragment::into_boxed_response_item)
                .collect(),
        )
    }

    let bundled = ContextualUserFragment::into(AvailablePluginsInstructions::default());
    let stale_history = std::slice::from_ref(&bundled);
    let previous: WorldStateSnapshot =
        serde_json::from_value(json!({"plugins_instructions": true})).unwrap();
    let (retired, updates) = render(
        /*available*/ false,
        Some("Catalog intro"),
        stale_history,
        &previous,
    );
    let notice =
        ContextualUserFragment::into(AvailablePluginsInstructions(REMOVAL_BODY.to_string()));
    assert_eq!(updates.as_slice(), std::slice::from_ref(&notice));

    // A boolean reader can still restore its bundled guidance, and a reader with the
    // original object snapshot can repair the same lost retirement.
    let mut saved = previous;
    saved.apply_merge_patch(&retired.merge_patch_from(&saved).unwrap());
    assert_eq!(
        serde_json::to_value(&saved).unwrap(),
        json!({
            "plugins_instructions": false,
            "plugins_instructions_catalog_retired": {"catalog": true}
        })
    );
    let previous_format: WorldStateSnapshot =
        serde_json::from_value(json!({"plugins_instructions": {"catalog": true}})).unwrap();
    for checkpoint in [&saved, &previous_format] {
        let (repaired, updates) = render(
            /*available*/ false,
            /*intro*/ None,
            stale_history,
            checkpoint,
        );
        assert_eq!(repaired, retired);
        assert_eq!(updates.as_slice(), std::slice::from_ref(&notice));
    }

    let repaired_history = [bundled.clone(), notice];
    let (restored, updates) = render(
        /*available*/ true,
        /*intro*/ None,
        &repaired_history,
        &saved,
    );
    assert_eq!(updates, [bundled]);
    saved.apply_merge_patch(&restored.merge_patch_from(&saved).unwrap());
    assert!(!saved.into_object().contains_key(RETIRED_CHECKPOINT_ID));
}

#[test]
fn legacy_false_snapshots_do_not_hide_retained_catalog_guidance() {
    let bundled = AvailablePluginsInstructions::default().0;
    let custom = expected_body("Catalog intro", "Catalog usage");
    let replaced_custom = format!("{REPLACEMENT_PREFIX}{custom}");
    let replaced_bundled = format!("{REPLACEMENT_PREFIX}{bundled}");
    let snapshot: WorldStateSnapshot =
        serde_json::from_value(json!({"plugins_instructions": false})).unwrap();
    for (retained_body, available, intro, usage, expected) in [
        (
            custom.as_str(),
            true,
            Some("Catalog intro"),
            Some("Catalog usage"),
            None,
        ),
        (
            replaced_custom.as_str(),
            true,
            Some("Catalog intro"),
            Some("Catalog usage"),
            None,
        ),
        (
            custom.as_str(),
            true,
            None,
            None,
            Some(replaced_bundled.as_str()),
        ),
        (
            replaced_custom.as_str(),
            false,
            None,
            None,
            Some(REMOVAL_BODY),
        ),
        (
            bundled.as_str(),
            false,
            Some("Catalog intro"),
            Some("Catalog usage"),
            Some(REMOVAL_BODY),
        ),
        (bundled.as_str(), false, None, None, None),
        (bundled.as_str(), true, None, None, Some(bundled.as_str())),
    ] {
        let retained: ResponseItem =
            ContextualUserFragment::into(AvailablePluginsInstructions(retained_body.to_string()));
        let mut world_state = WorldState::default();
        world_state.add_section(PluginsInstructionsState::new(
            available,
            resolve(intro, usage),
            std::iter::once(&retained),
        ));
        let updates = world_state
            .render_history_fragment_diff(Some(&snapshot), std::slice::from_ref(&retained))
            .1
            .into_iter()
            .map(ContextualUserFragment::into_boxed_response_item)
            .collect::<Vec<_>>();
        assert_eq!(
            updates,
            expected
                .map(
                    |body| ContextualUserFragment::into(AvailablePluginsInstructions(
                        body.to_string()
                    ))
                )
                .into_iter()
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn retained_guidance_recovers_snapshots_and_repairs_a_missing_latest_update() {
    let mut history = Vec::new();
    let mut previous = None;
    for (available, intro, usage) in [
        (true, "Intro A", "Usage A"),
        (true, "Intro B", "Usage B"),
        (false, "Intro B", "Usage B"),
        (true, "Intro B", "Usage B\r\n"),
        (true, "Intro B", "Usage B\n"),
        // Literal transition-like text inside catalog fields must remain literal on replay.
        (
            true,
            "These plugin usage instructions replace all previously provided plugin usage instructions.",
            "\nThese plugin usage instructions replace all previously provided plugin usage instructions.\n\nUsage C",
        ),
        (true, "", ""),
    ] {
        let resolved = resolve(Some(intro), Some(usage));
        let mut world_state = WorldState::default();
        world_state.add_section(PluginsInstructionsState::new(
            available,
            resolved.clone(),
            history.iter(),
        ));
        let (snapshot, fragments) =
            world_state.render_history_fragment_diff(previous.as_ref(), &history);
        let updates = fragments
            .into_iter()
            .map(ContextualUserFragment::into_boxed_response_item)
            .collect::<Vec<_>>();
        assert_eq!(
            updates.len(),
            1,
            "a byte change or availability transition must emit exactly one update"
        );
        for saved in [
            None,
            Some(json!({"plugins_instructions": true})),
            Some(serde_json::to_value(&snapshot).unwrap()),
        ] {
            let saved: Option<WorldStateSnapshot> =
                saved.map(|value| serde_json::from_value(value).unwrap());
            let mut resumed = WorldState::default();
            resumed.add_section(PluginsInstructionsState::new(
                available,
                resolved.clone(),
                updates.iter(),
            ));
            assert!(
                resumed
                    .render_history_fragment_diff(saved.as_ref(), &updates)
                    .1
                    .is_empty(),
                "retained current wording must not be duplicated after resume"
            );
        }
        if !history.is_empty() {
            assert_eq!(
                world_state
                    .render_history_fragment_diff(Some(&snapshot), &history)
                    .1
                    .len(),
                1,
                "an older retained marker must not suppress repair of a lost newest update"
            );
        }
        history.extend(updates);
        previous = Some(snapshot);
    }
}

#[test]
fn retained_plugin_guidance_must_be_a_complete_developer_fragment() {
    let body = expected_body("Catalog intro", "Catalog usage");
    for (role, quoted, expected_count) in [
        ("developer", false, 0),
        ("user", false, 1),
        ("assistant", false, 1),
        ("developer", true, 1),
    ] {
        let mut retained: ResponseItem =
            ContextualUserFragment::into(AvailablePluginsInstructions(body.clone()));
        let ResponseItem::Message {
            role: retained_role,
            content,
            ..
        } = &mut retained
        else {
            panic!("plugin instructions should render as a message");
        };
        *retained_role = role.to_string();
        if quoted {
            let ContentItem::InputText { text } = &mut content[0] else {
                panic!("plugin instructions should render as input text");
            };
            *text = format!("Quoted example: {text}");
        }
        let mut world_state = WorldState::default();
        world_state.add_section(PluginsInstructionsState::new(
            /*available*/ true,
            resolve(Some("Catalog intro"), Some("Catalog usage")),
            std::iter::once(&retained),
        ));
        let snapshot = world_state.render_full().0;
        let updates = world_state
            .render_history_fragment_diff(Some(&snapshot), std::slice::from_ref(&retained))
            .1;
        assert_eq!(
            updates.len(),
            expected_count,
            "role={role}, quoted={quoted}"
        );
        for update in updates {
            assert_eq!(
                (update.role(), update.content_kind().0),
                ("developer", "plugins.usage_instructions".to_string())
            );
            assert_eq!(
                update.into_boxed_response_item(),
                ContextualUserFragment::into(AvailablePluginsInstructions(body.clone()))
            );
        }
    }
}

#[test]
fn catalog_fields_at_utf8_limits_fit_final_framing_without_truncation() {
    let intro = "é".repeat(250);
    let usage = "\u{10ffff}".repeat(500);
    let body = expected_body(&intro, &usage);
    let retained: ResponseItem = ContextualUserFragment::into(AvailablePluginsInstructions(
        expected_body("Old intro", "Old usage"),
    ));
    let mut world_state = WorldState::default();
    world_state.add_section(PluginsInstructionsState::new(
        /*available*/ true,
        resolve(Some(&intro), Some(&usage)),
        std::iter::once(&retained),
    ));
    let (_, updates) = world_state
        .render_history_fragment_diff(/*previous*/ None, std::slice::from_ref(&retained));
    assert_eq!(updates.len(), 1);
    let update = &updates[0];
    assert_eq!(
        (update.role(), update.content_kind().0),
        ("developer", "plugins.usage_instructions".to_string())
    );
    let ContentItem::InputText { text } = update.render() else {
        panic!("plugin instructions should render as input text");
    };
    assert_eq!(
        text,
        format!("<plugins_instructions>{REPLACEMENT_PREFIX}{body}</plugins_instructions>")
    );
    assert!(
        text.len() <= 4096,
        "the final bound includes markers and transition framing"
    );
}
