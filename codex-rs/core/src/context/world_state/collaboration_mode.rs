use super::PreviousSectionState;
use super::SectionTransition;
use super::WorldStateHash;
use super::WorldStateSection;
use super::WorldStateUpdate;
use crate::context::ContextualUserFragment;
use codex_prompts::ResolvedCollaborationModeMessages;
use codex_prompts::ResolvedModelMessages;
use codex_prompts::without_update_plan_instructions;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::Settings;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::input_text;
use codex_protocol::protocol::COLLABORATION_MODE_CLOSE_TAG;
use codex_protocol::protocol::COLLABORATION_MODE_OPEN_TAG;
use serde::Deserialize;
use serde::Serialize;

/// Collaboration-mode instructions currently visible to the model.
#[derive(Clone, Debug)]
pub(crate) struct CollaborationModeState {
    snapshot: CollaborationModeSnapshot,
    instructions: Option<String>,
}

impl CollaborationModeState {
    pub(crate) fn from_collaboration_mode(
        mode: Option<ModeKind>,
        settings: &Settings,
        messages: ResolvedCollaborationModeMessages<'_>,
        update_plan_enabled: bool,
        custom_model_catalog: bool,
    ) -> Self {
        let bundled = ResolvedModelMessages::bundled().collaboration_modes();
        // Before Plan, preserve custom caller text only when it would win over the catalog.
        let supplied = settings.developer_instructions.as_deref().filter(|text| {
            !text.is_empty()
                && (mode.is_some()
                    || (messages.default.catalog_override().is_none()
                        && *text != bundled.default.text()))
        });
        let catalog_instructions = match mode {
            None if supplied.is_none() => messages.initial.filter(|text| !text.is_empty()),
            None => None,
            Some(ModeKind::Default) => messages.default.catalog_override(),
            Some(ModeKind::Plan) => messages.plan.catalog_override(),
        };
        let instructions = catalog_instructions.or(supplied).map(str::to_string);
        let instructions = instructions.map(|instructions| {
            if update_plan_enabled {
                return instructions;
            }
            let builtin = match catalog_instructions {
                Some(_) => !custom_model_catalog,
                None => {
                    // Without a catalog override, mode settings may contain a built-in
                    // preset or custom text. Compare whole strings against bundled presets
                    // only to recognize built-in text whose disabled update_plan guidance
                    // we can strip; custom instructions stay unchanged.
                    instructions == bundled.default.text() || instructions == bundled.plan.text()
                }
            };
            if builtin {
                without_update_plan_instructions(&instructions)
            } else {
                instructions
            }
        });
        // Keep an empty-state snapshot so removing instructions clears retained history only once.
        let fragment = CollaborationModeInstructions {
            instructions: instructions.clone().unwrap_or_default(),
        };
        let snapshot = CollaborationModeSnapshot::Current {
            mode: mode.unwrap_or(ModeKind::Default),
            model: settings.model.clone(),
            instructions: Some(WorldStateHash::from_fragment(&fragment)),
        };
        Self {
            snapshot,
            instructions,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum CollaborationModeSnapshot {
    Current {
        mode: ModeKind,
        model: String,
        // Older object snapshots do not have an instruction hash.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        instructions: Option<WorldStateHash>,
    },
    Legacy(ModeKind),
}

impl WorldStateSection for CollaborationModeState {
    const ID: &'static str = "collaboration_mode";
    type Snapshot = CollaborationModeSnapshot;

    fn matches_legacy_fragment(role: &str, text: &str) -> bool {
        role == "developer" && CollaborationModeInstructions::matches_text(text)
    }

    fn has_retained_fragment_matcher() -> bool {
        true
    }

    fn matches_retained_fragment(role: &str, text: &str) -> bool {
        Self::matches_legacy_fragment(role, text)
    }

    fn render_diff(
        &self,
        previous: PreviousSectionState<'_, Self::Snapshot>,
    ) -> SectionTransition<Self::Snapshot> {
        let current = self.snapshot.clone();
        let unchanged = match previous {
            PreviousSectionState::Absent => self.instructions.is_none(),
            PreviousSectionState::Unknown => false,
            PreviousSectionState::Known(previous) => {
                previous == &self.snapshot
                    || (self.instructions.as_deref().is_none_or(str::is_empty)
                        && matches!(
                            (previous, &self.snapshot),
                            (
                                CollaborationModeSnapshot::Current {
                                    instructions: Some(previous_hash),
                                    ..
                                },
                                CollaborationModeSnapshot::Current {
                                    instructions: Some(current_hash),
                                    ..
                                },
                            ) if previous_hash == current_hash
                        ))
            }
        };
        if unchanged {
            return (Some(current), Vec::new());
        }

        (
            Some(current),
            vec![WorldStateUpdate::fragment(CollaborationModeInstructions {
                instructions: self.instructions.clone().unwrap_or_default(),
            })],
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
struct CollaborationModeInstructions {
    instructions: String,
}

impl ContextualUserFragment for CollaborationModeInstructions {
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("collaboration_mode.instructions".to_string())
    }

    fn role(&self) -> &'static str {
        "developer"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        (COLLABORATION_MODE_OPEN_TAG, COLLABORATION_MODE_CLOSE_TAG)
    }

    fn body(&self) -> ContentItem {
        input_text(self.instructions.clone())
    }
}

#[cfg(test)]
#[path = "collaboration_mode_tests.rs"]
mod tests;
