use super::PreviousSectionState;
use super::WorldStateHash;
use super::WorldStateSection;
use crate::context::ContextualUserFragment;
use codex_prompts::ResolvedCollaborationModeMessages;
use codex_prompts::ResolvedModelMessages;
use codex_prompts::without_update_plan_instructions;
use codex_protocol::config_types::CollaborationMode;
use codex_protocol::config_types::ModeKind;
use codex_protocol::models::ContentItemKind;
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
        collaboration_mode: &CollaborationMode,
        messages: ResolvedCollaborationModeMessages<'_>,
        update_plan_enabled: bool,
        custom_model_catalog: bool,
    ) -> Self {
        let catalog_instructions = match collaboration_mode.mode {
            ModeKind::Default => messages.default,
            ModeKind::Plan => messages.plan,
        }
        .catalog_override();

        let instructions = catalog_instructions.map(str::to_string).or_else(|| {
            collaboration_mode
                .settings
                .developer_instructions
                .clone()
                .filter(|instructions| !instructions.is_empty())
        });
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
                    let bundled = ResolvedModelMessages::bundled().collaboration_modes();
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
            mode: collaboration_mode.mode,
            model: collaboration_mode.settings.model.clone(),
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

    fn snapshot(&self) -> Self::Snapshot {
        self.snapshot.clone()
    }

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
    ) -> Option<Box<dyn ContextualUserFragment>> {
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
            return None;
        }

        Some(Box::new(CollaborationModeInstructions {
            instructions: self.instructions.clone().unwrap_or_default(),
        }))
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

    fn body(&self) -> String {
        self.instructions.clone()
    }
}

#[cfg(test)]
#[path = "collaboration_mode_tests.rs"]
mod tests;
