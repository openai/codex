use super::WorldStateSection;
use crate::agents_md::LoadedAgentsMd;
use crate::context::ContextualUserFragment;
use crate::context::UserInstructions;
use serde::Deserialize;
use serde::Serialize;

const REPLACEMENT_NOTICE: &str =
    "These AGENTS.md instructions replace all previously provided AGENTS.md instructions.";
const REMOVAL_NOTICE: &str = "The previously provided AGENTS.md instructions no longer apply.";

/// The AGENTS.md instructions currently visible to the model.
#[derive(Clone, Debug, Default)]
pub(crate) struct AgentsMdState {
    instructions: Option<UserInstructions>,
}

/// Persisted model-visible AGENTS.md state, without filesystem provenance.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub(crate) struct AgentsMdSnapshot {
    directory: Option<String>,
    text: Option<String>,
}

impl AgentsMdState {
    pub(crate) fn new(loaded: Option<&LoadedAgentsMd>) -> Self {
        Self {
            instructions: loaded.map(LoadedAgentsMd::contextual_user_fragment),
        }
    }
}

impl WorldStateSection for AgentsMdState {
    const ID: &'static str = "agents_md";
    type Snapshot = AgentsMdSnapshot;

    fn snapshot(&self) -> Self::Snapshot {
        match &self.instructions {
            Some(instructions) => AgentsMdSnapshot {
                directory: instructions.directory.clone(),
                text: Some(instructions.text.clone()),
            },
            None => AgentsMdSnapshot::default(),
        }
    }

    fn render_diff(
        &self,
        previous: Option<&Self::Snapshot>,
    ) -> Option<Box<dyn ContextualUserFragment>> {
        let current = self.snapshot();
        if previous == Some(&current) {
            return None;
        }

        let previous_instructions = previous.and_then(|state| state.text.as_ref());
        let instructions = match (&self.instructions, previous_instructions) {
            (Some(instructions), Some(_)) => UserInstructions {
                directory: instructions.directory.clone(),
                text: format!("{REPLACEMENT_NOTICE}\n\n{}", instructions.text),
            },
            (Some(instructions), None) => instructions.clone(),
            (None, Some(_)) => UserInstructions {
                directory: None,
                text: REMOVAL_NOTICE.to_string(),
            },
            (None, None) => return None,
        };
        Some(Box::new(instructions))
    }
}

#[cfg(test)]
#[path = "agents_md_tests.rs"]
mod tests;
