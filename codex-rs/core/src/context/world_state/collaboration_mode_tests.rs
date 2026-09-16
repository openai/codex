use super::super::PreviousSectionState;
use super::super::test_support::render_section_cases;
use super::*;
use crate::context::world_state::WorldState;
use codex_models_manager::model_info::model_info_from_slug;
use codex_prompts::ResolvedCollaborationModeMessages;
use codex_prompts::ResolvedMessage;
use codex_prompts::ResolvedModelMessages;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Settings;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::CollaborationModeMessages;
use codex_protocol::openai_models::ModelMessages;
use pretty_assertions::assert_eq;

#[test]
fn snapshots() {
    use PreviousSectionState::Absent;
    use PreviousSectionState::Known;
    use PreviousSectionState::Unknown;

    let default = collaboration_mode_state(ModeKind::Default, "pair with the user");
    let old_default = collaboration_mode_state(ModeKind::Default, "old instructions");
    let new_default = collaboration_mode_state(ModeKind::Default, "new instructions");
    let plan = collaboration_mode_state(ModeKind::Plan, "make a plan");

    insta::assert_snapshot!(render_section_cases(&[
        (Absent, Absent),
        (Absent, Known(&default)),
        (Known(&default), Known(&default)),
        (Known(&old_default), Known(&new_default)),
        (Known(&default), Known(&plan)),
        (Unknown, Known(&default)),
    ]));
}

#[test]
fn instruction_updates_are_applied_once_in_retained_history() {
    let mut history: Vec<ResponseItem> = vec![ContextualUserFragment::into(
        CollaborationModeInstructions {
            instructions: "old instructions".to_string(),
        },
    )];
    let mut previous = None;

    for instructions in [Some("old instructions"), Some("new instructions"), None] {
        let mut world_state = WorldState::default();
        world_state.add_section(CollaborationModeState::from_collaboration_mode(
            &collaboration_mode(ModeKind::Default, instructions),
            ResolvedModelMessages::bundled().collaboration_modes(),
            /*update_plan_enabled*/ true,
            /*custom_model_catalog*/ false,
        ));
        let expected: ResponseItem = ContextualUserFragment::into(CollaborationModeInstructions {
            instructions: instructions.unwrap_or_default().to_string(),
        });
        let updates = world_state
            .render_history_diff(previous.as_ref(), &history)
            .into_iter()
            .map(ContextualUserFragment::into_boxed_response_item)
            .collect::<Vec<_>>();
        assert_eq!(updates, vec![expected]);
        history.extend(updates);
        previous = Some(world_state.snapshot());

        assert!(
            world_state
                .render_history_diff(previous.as_ref(), &history)
                .is_empty()
        );
        assert_eq!(
            world_state
                .render_history_diff(previous.as_ref(), &[])
                .len(),
            usize::from(instructions.is_some()),
        );
    }
}

#[test]
fn catalog_collaboration_messages_select_mode_variant() {
    let bundled = ResolvedModelMessages::bundled().collaboration_modes();
    for (default, plan) in [
        ("catalog default instructions", "catalog plan instructions"),
        (bundled.default.text(), bundled.plan.text()),
    ] {
        let mut model = model_info_from_slug("test-model");
        model.model_messages = Some(ModelMessages {
            collaboration_modes: Some(CollaborationModeMessages {
                default: Some(default.to_string()),
                plan: Some(plan.to_string()),
            }),
            ..Default::default()
        });
        let messages = ResolvedModelMessages::from_model(&model).collaboration_modes();

        for (mode, expected) in [(ModeKind::Default, default), (ModeKind::Plan, plan)] {
            let state = CollaborationModeState::from_collaboration_mode(
                &collaboration_mode(mode, Some("legacy instructions")),
                messages,
                /*update_plan_enabled*/ true,
                /*custom_model_catalog*/ false,
            );

            assert_eq!(state.instructions.as_deref(), Some(expected));
        }
    }
}

#[test]
fn empty_catalog_collaboration_message_suppresses_legacy_instructions() {
    let messages = ResolvedCollaborationModeMessages {
        plan: ResolvedMessage::Catalog(""),
        ..ResolvedModelMessages::bundled().collaboration_modes()
    };
    let state = CollaborationModeState::from_collaboration_mode(
        &collaboration_mode(ModeKind::Plan, Some("legacy plan instructions")),
        messages,
        /*update_plan_enabled*/ true,
        /*custom_model_catalog*/ false,
    );

    assert_eq!(
        state
            .render_diff(PreviousSectionState::Absent)
            .expect("explicit empty collaboration message")
            .render(),
        format!("{COLLABORATION_MODE_OPEN_TAG}{COLLABORATION_MODE_CLOSE_TAG}")
    );
}

#[test]
fn missing_catalog_collaboration_message_uses_legacy_instructions() {
    let messages = ResolvedCollaborationModeMessages {
        default: ResolvedMessage::Catalog("catalog default instructions"),
        ..ResolvedModelMessages::bundled().collaboration_modes()
    };
    for instructions in [Some("legacy plan instructions"), Some(""), None] {
        let state = CollaborationModeState::from_collaboration_mode(
            &collaboration_mode(ModeKind::Plan, instructions),
            messages,
            /*update_plan_enabled*/ true,
            /*custom_model_catalog*/ false,
        );
        assert_eq!(
            state.instructions.as_deref(),
            instructions.filter(|text| !text.is_empty())
        );
    }
}

#[test]
fn legacy_collaboration_mode_snapshots_refresh_catalog_messages_once() {
    for serialized in ["\"default\"", r#"{"mode":"default","model":"test-model"}"#] {
        let previous = serde_json::from_str::<CollaborationModeSnapshot>(serialized)
            .expect("legacy collaboration mode snapshot");

        for instructions in ["catalog instructions", ""] {
            let messages = ResolvedCollaborationModeMessages {
                default: ResolvedMessage::Catalog(instructions),
                ..ResolvedModelMessages::bundled().collaboration_modes()
            };
            let state = CollaborationModeState::from_collaboration_mode(
                &collaboration_mode(ModeKind::Default, Some("stale legacy instructions")),
                messages,
                /*update_plan_enabled*/ true,
                /*custom_model_catalog*/ false,
            );

            assert_eq!(
                state
                    .render_diff(PreviousSectionState::Known(&previous))
                    .expect("legacy snapshot should refresh collaboration instructions")
                    .render(),
                format!(
                    "{COLLABORATION_MODE_OPEN_TAG}{instructions}{COLLABORATION_MODE_CLOSE_TAG}"
                )
            );
            assert!(
                state
                    .render_diff(PreviousSectionState::Known(&state.snapshot()))
                    .is_none()
            );
        }
    }
}

fn collaboration_mode(mode: ModeKind, instructions: Option<&str>) -> CollaborationMode {
    CollaborationMode {
        mode,
        settings: Settings {
            model: "test-model".to_string(),
            reasoning_effort: None,
            developer_instructions: instructions.map(str::to_string),
        },
    }
}

fn collaboration_mode_state(mode: ModeKind, instructions: &str) -> CollaborationModeState {
    CollaborationModeState::from_collaboration_mode(
        &collaboration_mode(mode, Some(instructions)),
        ResolvedModelMessages::bundled().collaboration_modes(),
        /*update_plan_enabled*/ true,
        /*custom_model_catalog*/ false,
    )
}
