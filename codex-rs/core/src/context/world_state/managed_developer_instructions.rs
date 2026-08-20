use super::PreviousSectionState;
use super::WorldStateHash;
use super::WorldStateSection;
use crate::context::ContextualUserFragment;
use codex_config::Sourced;
use codex_utils_string::approx_bytes_for_tokens;
use codex_utils_string::approx_tokens_from_byte_count;
use serde::Deserialize;
use serde::Serialize;
use std::io;

const MAX_MANAGED_DEVELOPER_INSTRUCTIONS_TOKENS: usize = 10_000;
const REPLACEMENT_NOTICE: &str = "These managed developer instructions replace all previously provided managed developer instructions.";
const REMOVAL_NOTICE: &str =
    "The previously provided managed developer instructions no longer apply.";

/// A requirements-owned developer message, kept separate from client instructions.
#[derive(Clone, Debug)]
pub(crate) struct ManagedDeveloperInstructions {
    instructions: String,
}

impl ContextualUserFragment for ManagedDeveloperInstructions {
    fn role(&self) -> &'static str {
        "developer"
    }

    fn requires_separate_message(&self) -> bool {
        true
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        (
            "<managed_developer_instructions>",
            "</managed_developer_instructions>",
        )
    }

    fn body(&self) -> String {
        format!("\n{}\n", self.instructions)
    }
}

/// Reject oversized policy rather than silently dropping part of its instructions.
pub(crate) fn validate_managed_developer_instructions(
    requirement: Option<&Sourced<String>>,
) -> io::Result<()> {
    let Some(requirement) = requirement else {
        return Ok(());
    };
    // Reserve the notice used when an existing conversation receives a new policy.
    let overhead = ManagedDeveloperInstructions {
        instructions: format!("{REPLACEMENT_NOTICE}\n\n"),
    }
    .render()
    .len();
    let rendered_bytes = requirement.value.len().saturating_add(overhead);
    if rendered_bytes > approx_bytes_for_tokens(MAX_MANAGED_DEVELOPER_INSTRUCTIONS_TOKENS) {
        let estimated_tokens = approx_tokens_from_byte_count(rendered_bytes);
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "`additional_developer_instructions` from {} exceeds the model-context limit of {MAX_MANAGED_DEVELOPER_INSTRUCTIONS_TOKENS} estimated tokens ({estimated_tokens} including context markers)",
                requirement.source,
            ),
        ));
    }
    Ok(())
}

/// The managed policy currently visible in retained model history.
#[derive(Clone, Debug)]
pub(crate) struct ManagedDeveloperInstructionsState {
    instructions: Option<ManagedDeveloperInstructions>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ManagedDeveloperInstructionsSnapshot {
    instructions: Option<WorldStateHash>,
}

impl ManagedDeveloperInstructionsState {
    pub(crate) fn new(requirement: Option<&Sourced<String>>) -> Self {
        Self {
            instructions: requirement
                .filter(|requirement| !requirement.value.is_empty())
                .map(|requirement| ManagedDeveloperInstructions {
                    instructions: requirement.value.clone(),
                }),
        }
    }
}

impl WorldStateSection for ManagedDeveloperInstructionsState {
    const ID: &'static str = "managed_developer_instructions";
    type Snapshot = ManagedDeveloperInstructionsSnapshot;

    fn snapshot(&self) -> Self::Snapshot {
        ManagedDeveloperInstructionsSnapshot {
            instructions: self
                .instructions
                .as_ref()
                .map(WorldStateHash::from_fragment),
        }
    }

    fn matches_legacy_fragment(role: &str, text: &str) -> bool {
        role == "developer" && ManagedDeveloperInstructions::matches_text(text)
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
        if matches!(previous, PreviousSectionState::Known(previous) if previous == &self.snapshot())
        {
            return None;
        }
        let previous_had_instructions = match previous {
            PreviousSectionState::Absent => false,
            PreviousSectionState::Unknown => true,
            PreviousSectionState::Known(previous) => previous.instructions.is_some(),
        };
        let fragment = match (&self.instructions, previous_had_instructions) {
            (Some(instructions), false) => instructions.clone(),
            (Some(instructions), true) => ManagedDeveloperInstructions {
                instructions: format!("{REPLACEMENT_NOTICE}\n\n{}", instructions.instructions),
            },
            (None, true) => ManagedDeveloperInstructions {
                instructions: REMOVAL_NOTICE.to_string(),
            },
            (None, false) => return None,
        };
        Some(Box::new(fragment))
    }
}

#[cfg(test)]
#[path = "managed_developer_instructions_tests.rs"]
mod tests;
