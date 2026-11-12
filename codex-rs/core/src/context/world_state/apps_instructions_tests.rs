#![cfg(test)]

use super::*;
use crate::context::world_state::WorldState;
use crate::context::world_state::WorldStateSnapshot;
use crate::context::world_state::test_support::render_section_cases;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_prompts::ResolvedModelMessages;
use codex_protocol::models::ResponseItem;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn snapshots() {
    use PreviousSectionState::Absent;
    use PreviousSectionState::Known;
    use PreviousSectionState::Unknown;

    let bundled =
        ResolvedModelMessages::bundled().apps_usage_instructions(CODEX_APPS_MCP_SERVER_NAME);
    let unavailable = AppsInstructionsState::new(/*available*/ false, bundled.clone());
    let available = AppsInstructionsState::new(/*available*/ true, bundled);

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
fn legacy_guidance_is_reused_or_retired_by_catalog_intent() {
    let bundled = AppsInstructions::default().0;
    let replacement = format!("{REPLACEMENT_NOTICE}{bundled}");
    let custom = format!("{REPLACEMENT_NOTICE}B");
    let removal = REMOVAL_NOTICE.to_owned();
    for (saved_available, retained, catalog, available, expected) in [
        (None, bundled.as_str(), None, true, None),
        (Some(true), bundled.as_str(), None, true, None),
        (
            Some(false),
            bundled.as_str(),
            Some("B"),
            false,
            Some(removal.as_str()),
        ),
        (
            Some(false),
            bundled.as_str(),
            Some(""),
            false,
            Some(removal.as_str()),
        ),
        // Older clients can save false while retaining custom instructions from a newer client.
        (Some(false), "B", None, true, Some(replacement.as_str())),
        (Some(false), "B", None, false, Some(removal.as_str())),
        (
            Some(false),
            custom.as_str(),
            None,
            true,
            Some(replacement.as_str()),
        ),
        (
            Some(false),
            custom.as_str(),
            None,
            false,
            Some(removal.as_str()),
        ),
        // Bundled-only false snapshots keep their historical availability-cycle behavior.
        (
            Some(false),
            bundled.as_str(),
            None,
            true,
            Some(bundled.as_str()),
        ),
        (Some(false), bundled.as_str(), None, false, None),
        (
            Some(false),
            replacement.as_str(),
            None,
            true,
            Some(bundled.as_str()),
        ),
        (Some(false), replacement.as_str(), None, false, None),
    ] {
        let legacy: ResponseItem =
            ContextualUserFragment::into(AppsInstructions(retained.to_string()));
        let mut model = codex_models_manager::model_info::model_info_from_slug("gpt-5.5");
        model.model_messages =
            Some(serde_json::from_value(json!({"apps": {"usage_instructions": catalog}})).unwrap());
        let mut world_state = WorldState::default();
        world_state.add_section(AppsInstructionsState::new(
            available,
            ResolvedModelMessages::from_model(&model)
                .apps_usage_instructions(CODEX_APPS_MCP_SERVER_NAME),
        ));
        let snapshot: Option<WorldStateSnapshot> = saved_available.map(|available| {
            serde_json::from_value(json!({"apps_instructions": available})).unwrap()
        });
        assert_eq!(
            world_state
                .render_history_fragment_diff(snapshot.as_ref(), std::slice::from_ref(&legacy))
                .1
                .into_iter()
                .map(ContextualUserFragment::into_boxed_response_item)
                .collect::<Vec<_>>(),
            expected
                .map(|text| {
                    if text == bundled || text == format!("{REPLACEMENT_NOTICE}{bundled}") {
                        ContextualUserFragment::into(AppsInstructions(text.to_string()))
                    } else {
                        ContextualUserFragment::into(CatalogAppsInstructions(text.to_string()))
                    }
                })
                .into_iter()
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn resume_after_an_older_catalog_client_silently_disabled_bundled_guidance() {
    let retained = ContextualUserFragment::into(AppsInstructions::default());
    let previous: WorldStateSnapshot =
        serde_json::from_value(json!({"apps_instructions": {"catalog": false}})).unwrap();
    let bundled =
        ResolvedModelMessages::bundled().apps_usage_instructions(CODEX_APPS_MCP_SERVER_NAME);
    for (resolved, expected) in [
        (bundled, retained.clone()),
        (
            ResolvedAppsInstructions {
                usage_instructions: String::new(),
                catalog_override: true,
            },
            ContextualUserFragment::into(CatalogAppsInstructions(REMOVAL_NOTICE.to_string())),
        ),
    ] {
        let mut world_state = WorldState::default();
        world_state.add_section(AppsInstructionsState::new(
            /*available*/ true, resolved,
        ));
        let (_, updates) = world_state
            .render_history_fragment_diff(Some(&previous), std::slice::from_ref(&retained));
        assert_eq!(
            updates
                .into_iter()
                .map(ContextualUserFragment::into_boxed_response_item)
                .collect::<Vec<_>>(),
            [expected]
        );
    }
}

#[test]
fn persisted_guidance_is_restored_only_when_missing_from_history() {
    let retained: ResponseItem = ContextualUserFragment::into(AppsInstructions::default());
    let bundled =
        ResolvedModelMessages::bundled().apps_usage_instructions(CODEX_APPS_MCP_SERVER_NAME);
    for history in [vec![], vec![retained]] {
        let mut world_state = WorldState::default();
        world_state.add_section(AppsInstructionsState::new(
            /*available*/ true,
            bundled.clone(),
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
fn catalog_guidance_updates_clears_and_returns_to_bundled() {
    let bundled = AppsInstructions::default().0;
    let replacement = format!("{REPLACEMENT_NOTICE}B");
    let fallback = format!("{REPLACEMENT_NOTICE}{bundled}");
    let removal = REMOVAL_NOTICE.to_owned();
    let mut history = Vec::new();
    let mut previous = None;
    for (available, catalog, expected) in [
        (true, Some("A"), Some("A")),
        (true, Some("A"), None),
        (true, Some("B"), Some(replacement.as_str())),
        (true, Some(""), Some(removal.as_str())),
        (true, Some(""), None),
        (true, Some("A"), Some("A")),
        (true, None, Some(fallback.as_str())),
        (false, None, None),
        (true, None, Some(bundled.as_str())),
        (true, Some("B"), Some(replacement.as_str())),
        (false, Some("B"), Some(removal.as_str())),
        (false, Some("B"), None),
        // Selecting catalog intent while unavailable must retire retained bundled guidance.
        (true, None, Some(bundled.as_str())),
        (false, Some("B"), Some(removal.as_str())),
        (false, Some("B"), None),
        (true, Some("B"), Some("B")),
        (true, None, Some(fallback.as_str())),
        (false, Some(""), Some(removal.as_str())),
        (true, Some(""), None),
        // A prior bundled-only unavailable turn must not hide later catalog intent.
        (true, None, Some(bundled.as_str())),
        (false, None, None),
        (false, Some("B"), Some(removal.as_str())),
        (true, Some("B"), Some("B")),
        (true, None, Some(fallback.as_str())),
        (false, None, None),
        (false, Some(""), Some(removal.as_str())),
        (true, Some(""), None),
    ] {
        let mut model = codex_models_manager::model_info::model_info_from_slug("gpt-5.5");
        model.model_messages =
            Some(serde_json::from_value(json!({"apps": {"usage_instructions": catalog}})).unwrap());
        let resolved = ResolvedModelMessages::from_model(&model)
            .apps_usage_instructions(CODEX_APPS_MCP_SERVER_NAME);
        let mut world_state = WorldState::default();
        world_state.add_section(AppsInstructionsState::new(available, resolved));
        let (snapshot, fragments) =
            world_state.render_history_fragment_diff(previous.as_ref(), &history);
        let updates = fragments
            .into_iter()
            .map(ContextualUserFragment::into_boxed_response_item)
            .collect::<Vec<_>>();
        assert_eq!(
            updates,
            expected
                .map(|text| {
                    if text == bundled || text == format!("{REPLACEMENT_NOTICE}{bundled}") {
                        ContextualUserFragment::into(AppsInstructions(text.to_string()))
                    } else {
                        ContextualUserFragment::into(CatalogAppsInstructions(text.to_string()))
                    }
                })
                .into_iter()
                .collect::<Vec<_>>()
        );
        history.extend(updates);
        previous = Some(snapshot);
    }
}

// These are the matching and rendering rules of the released availability-only client.
struct PreCatalogApps;

impl WorldStateSection for PreCatalogApps {
    const ID: &'static str = "apps_instructions";
    type Snapshot = bool;

    fn has_retained_fragment_matcher() -> bool {
        true
    }

    fn matches_retained_fragment(role: &str, text: &str) -> bool {
        role == "developer" && AppsInstructions::matches_text(text)
    }

    fn matches_legacy_fragment(role: &str, text: &str) -> bool {
        Self::matches_retained_fragment(role, text)
    }

    fn render_diff(&self, previous: PreviousSectionState<'_, bool>) -> SectionTransition<bool> {
        let updates = match previous {
            PreviousSectionState::Unknown | PreviousSectionState::Known(true) => Vec::new(),
            PreviousSectionState::Absent | PreviousSectionState::Known(false) => {
                vec![WorldStateUpdate::fragment(AppsInstructions::default())]
            }
        };
        (Some(true), updates)
    }
}

#[test]
fn downgrade_restores_bundled_guidance_after_catalog_selection_and_retirement() {
    let bundled: ResponseItem = ContextualUserFragment::into(AppsInstructions::default());
    let mut previous = None;
    let mut catalog_history = Vec::new();
    let mut older_client = WorldState::default();
    older_client.add_section(PreCatalogApps);
    for text in ["custom", ""] {
        let mut world_state = WorldState::default();
        world_state.add_section(AppsInstructionsState::new(
            /*available*/ true,
            ResolvedAppsInstructions {
                usage_instructions: text.to_string(),
                catalog_override: true,
            },
        ));
        let (snapshot, updates) =
            world_state.render_history_fragment_diff(previous.as_ref(), &catalog_history);
        assert_eq!(updates.len(), 1);
        catalog_history.extend(
            updates
                .into_iter()
                .map(ContextualUserFragment::into_boxed_response_item),
        );
        let saved = serde_json::to_string(&snapshot).unwrap();
        let restored: WorldStateSnapshot = serde_json::from_str(&saved).unwrap();
        let mixed_history = std::iter::once(&bundled).chain(&catalog_history);
        for (saved, history) in [
            (Some(&restored), mixed_history.collect::<Vec<_>>()),
            (None, catalog_history.iter().collect()),
        ] {
            assert_eq!(
                older_client
                    .render_history_fragment_diff(saved, history)
                    .1
                    .into_iter()
                    .map(ContextualUserFragment::into_boxed_response_item)
                    .collect::<Vec<_>>(),
                std::slice::from_ref(&bundled)
            );
        }
        previous = Some(snapshot);
    }
}

#[test]
fn literal_catalog_notices_remain_active_guidance_after_snapshot_loss() {
    for text in [
        REMOVAL_NOTICE.to_owned(),
        format!("{REPLACEMENT_NOTICE}literal"),
    ] {
        let mut world_state = WorldState::default();
        world_state.add_section(AppsInstructionsState::new(
            /*available*/ true,
            ResolvedAppsInstructions {
                usage_instructions: text.clone(),
                catalog_override: true,
            },
        ));
        let (_, first) = world_state.render_full_fragments();
        let history: Vec<_> = first
            .into_iter()
            .map(ContextualUserFragment::into_boxed_response_item)
            .collect();
        assert_eq!(
            history,
            [ContextualUserFragment::into(CatalogAppsInstructions(
                format!("{REPLACEMENT_NOTICE}{text}")
            ))]
        );
        assert!(
            world_state
                .render_history_fragment_diff(/*previous*/ None, &history)
                .1
                .is_empty()
        );
    }
}

#[test]
fn retained_catalog_guidance_recovers_legacy_snapshots_and_repairs_lost_updates() {
    let mut history = Vec::new();
    let mut previous = None;
    let catalog = |text| ContextualUserFragment::into(CatalogAppsInstructions(text));
    let replacement = |text: &str| format!("{REPLACEMENT_NOTICE}{text}");
    let removal = catalog(REMOVAL_NOTICE.to_owned());
    let two_byte_limit = "é".repeat(400);
    let four_byte_limit = "\u{10ffff}".repeat(200);
    let oversized = "é".repeat(500);
    for (available, text, expected) in [
        (true, "A", catalog("A".to_string())),
        (true, "B", catalog(replacement("B"))),
        (false, "B", removal.clone()),
        (true, "B\r\n", catalog("B\r\n".to_string())),
        (true, "B\n", catalog(replacement("B\n"))),
        (
            true,
            two_byte_limit.as_str(),
            catalog(replacement(&two_byte_limit)),
        ),
        (
            true,
            four_byte_limit.as_str(),
            catalog(replacement(&four_byte_limit)),
        ),
        (
            true,
            oversized.as_str(),
            ContextualUserFragment::into(AppsInstructions(replacement(
                &AppsInstructions::default().0,
            ))),
        ),
        (true, "", removal),
    ] {
        let mut model = codex_models_manager::model_info::model_info_from_slug("gpt-5.5");
        model.model_messages =
            Some(serde_json::from_value(json!({"apps": {"usage_instructions": text}})).unwrap());
        let resolved = ResolvedModelMessages::from_model(&model)
            .apps_usage_instructions(CODEX_APPS_MCP_SERVER_NAME);
        let mut world_state = WorldState::default();
        world_state.add_section(AppsInstructionsState::new(available, resolved));
        let (snapshot, fragments) =
            world_state.render_history_fragment_diff(previous.as_ref(), &history);
        for fragment in &fragments {
            assert!(fragment.render().into_text().unwrap().len() <= 1_000);
        }
        let updates = fragments
            .into_iter()
            .map(ContextualUserFragment::into_boxed_response_item)
            .collect::<Vec<_>>();
        assert_eq!(updates, [expected]);
        for saved in [
            None,
            Some(json!({"apps_instructions": true})),
            Some(serde_json::to_value(&snapshot).unwrap()),
        ] {
            let saved: Option<WorldStateSnapshot> =
                saved.map(|value| serde_json::from_value(value).unwrap());
            assert!(
                world_state
                    .render_history_fragment_diff(saved.as_ref(), &updates)
                    .1
                    .is_empty()
            );
        }
        if !history.is_empty() {
            // The newest update was compacted away; an older marker must not suppress repair.
            assert_eq!(
                world_state
                    .render_history_fragment_diff(Some(&snapshot), &history)
                    .1
                    .into_iter()
                    .map(ContextualUserFragment::into_boxed_response_item)
                    .collect::<Vec<_>>(),
                updates
            );
        }
        history.extend(updates);
        previous = Some(snapshot);
    }
}
