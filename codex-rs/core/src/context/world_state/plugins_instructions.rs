//! Tracks literal generic plugin wording across catalog changes, legacy snapshots, and retained history.
//! Bundled-only availability changes retain their previous silent-removal behavior.

use super::PreviousSectionState;
use super::SectionTransition;
use super::WorldStateHash;
use super::WorldStateSection;
use super::WorldStateUpdate;
use crate::context::AvailablePluginsInstructions;
use crate::context::ContextualUserFragment;
use codex_prompts::ResolvedGenericPluginMessages;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

const REPLACEMENT_PREFIX: &str = "\nThese plugin usage instructions replace all previously provided plugin usage instructions.\n\n";
const REMOVAL_BODY: &str = "\nThe previously provided plugin usage instructions no longer apply.\n";

// Only a catalog retirement needs this extra checkpoint to distinguish it from the
// legacy behavior of silently disabling bundled guidance. Older clients read the primary bool.
pub(super) const RETIRED_CHECKPOINT_ID: &str = "plugins_instructions_catalog_retired";

pub(super) fn is_retired_checkpoint(snapshot: &Value) -> bool {
    snapshot.get("instructions").is_none() && snapshot.get("catalog") == Some(&Value::Bool(true))
}

#[derive(Clone, Debug)]
pub(crate) struct PluginsInstructionsState {
    available: bool,
    resolved: ResolvedGenericPluginMessages,
    retained_body: Option<String>,
}

/// Older clients read only availability. Keep writing false when guidance is disabled.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum PluginsInstructionsSnapshot {
    Current {
        instructions: Option<WorldStateHash>,
        catalog: bool,
    },
    Legacy(bool),
}

impl PluginsInstructionsState {
    pub(crate) fn new<'a>(
        available: bool,
        resolved: ResolvedGenericPluginMessages,
        history: impl DoubleEndedIterator<Item = &'a ResponseItem>,
    ) -> Self {
        let (open, close) = AvailablePluginsInstructions::type_markers();
        let retained_body = history.rev().find_map(|item| match item {
            ResponseItem::Message { role, content, .. } if role == "developer" => {
                content.iter().rev().find_map(|content| match content {
                    ContentItem::InputText { text }
                        if AvailablePluginsInstructions::matches_text(text) =>
                    {
                        let text = text.trim();
                        text.get(open.len()..text.len() - close.len())
                            .map(str::to_owned)
                    }
                    _ => None,
                })
            }
            _ => None,
        });
        Self {
            available,
            resolved,
            retained_body,
        }
    }
}

impl WorldStateSection for PluginsInstructionsState {
    const ID: &'static str = "plugins_instructions";
    type Snapshot = PluginsInstructionsSnapshot;

    fn matches_legacy_fragment(role: &str, text: &str) -> bool {
        role == "developer" && AvailablePluginsInstructions::matches_text(text)
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
        use PluginsInstructionsSnapshot::Current;
        use PluginsInstructionsSnapshot::Legacy;

        // JSON hashing preserves CRLF and all other literal bytes, unlike fragment hashing.
        let fingerprint =
            |text: &str| WorldStateHash::from_json(&serde_json::Value::String(text.to_string()));
        let rendered = self
            .available
            .then(|| AvailablePluginsInstructions::from(self.resolved.clone()).0);
        let instructions = rendered.as_deref();
        let current_hash = instructions.map(fingerprint);
        // Catalog selection can retire retained guidance even while availability is gated off.
        let catalog = self.resolved.catalog_override;
        let bundled = AvailablePluginsInstructions::default().0;
        let (mut previous_hash, mut previous_catalog) = match previous {
            PreviousSectionState::Absent | PreviousSectionState::Known(Legacy(false)) => {
                (None, false)
            }
            PreviousSectionState::Unknown | PreviousSectionState::Known(Legacy(true)) => {
                (Some(fingerprint(&bundled)), false)
            }
            PreviousSectionState::Known(Current {
                instructions,
                catalog,
            }) => (instructions.clone(), *catalog),
        };
        if !matches!(previous, PreviousSectionState::Absent)
            && let Some(body) = self.retained_body.as_deref()
        {
            // Catalog fields occur inside the "## Plugins" body, after any transition framing.
            let retained = (body != REMOVAL_BODY)
                .then(|| body.strip_prefix(REPLACEMENT_PREFIX).unwrap_or(body));
            let retained_catalog = retained != Some(bundled.as_str());
            // A legacy false snapshot can coexist with retained catalog guidance, while a
            // bundled-only disablement deliberately leaves the old bundled text in history.
            if previous_hash.is_some() || previous_catalog || catalog || retained_catalog {
                previous_hash = retained.map(fingerprint);
                previous_catalog |= retained_catalog;
            }
        }
        let current = if self.available {
            Current {
                instructions: current_hash.clone(),
                catalog,
            }
        } else if previous_catalog || (catalog && previous_hash.is_some()) {
            Current {
                instructions: None,
                catalog: true,
            }
        } else {
            Legacy(false)
        };
        if previous_hash == current_hash {
            return (Some(current), Vec::new());
        }
        let body = match instructions {
            Some(text) if previous_hash.is_some() => format!("{REPLACEMENT_PREFIX}{text}"),
            Some(text) => text.to_string(),
            None if previous_hash.is_some() && (previous_catalog || catalog) => {
                REMOVAL_BODY.to_string()
            }
            None => return (Some(current), Vec::new()),
        };
        // The 500/2,000-byte field allowances leave room for all framing below 4 KiB.
        (
            Some(current),
            vec![WorldStateUpdate::fragment(AvailablePluginsInstructions(
                body,
            ))],
        )
    }
}

#[cfg(test)]
#[path = "plugins_instructions_tests.rs"]
mod tests;
