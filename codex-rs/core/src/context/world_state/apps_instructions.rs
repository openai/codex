//! Reconciles the newest retained Apps fragment with the selected model's wording.
//! The persisted availability bit lets older clients restore their bundled guidance.

use super::PreviousSectionState;
use super::SectionTransition;
use super::WorldStateHash;
use super::WorldStateSection;
use super::WorldStateUpdate;
use crate::context::AppsInstructions;
use crate::context::CatalogAppsInstructions;
use crate::context::ContextualUserFragment;
use codex_prompts::ResolvedAppsInstructions;
use serde::Deserialize;
use serde::Serialize;

const REPLACEMENT_NOTICE: &str =
    "\nThese Apps usage instructions replace all previously provided Apps usage instructions.\n\n";
const REMOVAL_NOTICE: &str = "\nThe previously provided Apps usage instructions no longer apply.\n";

#[derive(Clone, Debug)]
pub(crate) struct AppsInstructionsState {
    available: bool,
    resolved: ResolvedAppsInstructions,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum AppsInstructionsSnapshot {
    /// The pre-catalog reader appends its bundled instructions on false.
    BundledAvailable(bool),
    /// Read existing catalog checkpoints. We still use this form when catalog wording is
    /// byte-identical to bundled wording: an old reader can keep it, while a new reader
    /// must remember its catalog origin if Apps become unavailable.
    Catalog {
        instructions: Option<WorldStateHash>,
        catalog: bool,
    },
}

/// The instructions that still apply in the retained history.
enum RetainedGuidance<'a> {
    None,
    Bundled,
    Catalog(&'a str),
}

impl<'a> RetainedGuidance<'a> {
    fn recover(
        previous: PreviousSectionState<'_, AppsInstructionsSnapshot>,
        retained: Option<&'a str>,
        bundled: &'a str,
        catalog_selected: bool,
    ) -> Self {
        use AppsInstructionsSnapshot::BundledAvailable;
        use AppsInstructionsSnapshot::Catalog;

        // The World State matcher has already checked the developer role and both markers.
        let body = retained.map(str::trim).map(|text| {
            let (open, close) = if CatalogAppsInstructions::matches_text(text) {
                CatalogAppsInstructions::type_markers()
            } else {
                AppsInstructions::type_markers()
            };
            &text[open.len()..text.len() - close.len()]
        });
        let Some(body) = body else {
            return match previous {
                PreviousSectionState::Absent
                | PreviousSectionState::Known(BundledAvailable(false)) => Self::None,
                PreviousSectionState::Unknown
                | PreviousSectionState::Known(BundledAvailable(true)) => Self::Bundled,
                PreviousSectionState::Known(Catalog {
                    instructions: None, ..
                }) => Self::None,
                PreviousSectionState::Known(Catalog {
                    instructions: Some(_),
                    ..
                }) => Self::Bundled,
            };
        };
        // Prior catalog checkpoints carried a hash to distinguish literal notice-like content.
        let saved_literal = matches!(previous, PreviousSectionState::Known(Catalog { instructions: Some(hash), .. })
            if (body == REMOVAL_NOTICE || body.starts_with(REPLACEMENT_NOTICE)) && *hash == fingerprint(body));
        let wording = if saved_literal {
            body
        } else if body == REMOVAL_NOTICE {
            return Self::None;
        } else {
            body.strip_prefix(REPLACEMENT_NOTICE).unwrap_or(body)
        };
        if wording != bundled {
            return Self::Catalog(wording);
        }
        match previous {
            PreviousSectionState::Known(Catalog { catalog: true, .. }) => Self::Catalog(wording),
            // Older clients can disable Apps without appending a retirement notice. Re-enable
            // their bundled wording normally, but let an explicit catalog choice supersede it.
            PreviousSectionState::Known(
                BundledAvailable(false)
                | Catalog {
                    instructions: None,
                    catalog: false,
                },
            ) if !catalog_selected => Self::None,
            _ => Self::Bundled,
        }
    }
}

fn fingerprint(text: &str) -> WorldStateHash {
    WorldStateHash::from_json(&serde_json::Value::String(text.to_owned()))
}

impl AppsInstructionsState {
    pub(crate) fn new(available: bool, resolved: ResolvedAppsInstructions) -> Self {
        Self {
            available,
            resolved,
        }
    }
}

impl WorldStateSection for AppsInstructionsState {
    const ID: &'static str = "apps_instructions";
    type Snapshot = AppsInstructionsSnapshot;
    const NEEDS_LATEST_FRAGMENT: bool = true;

    fn matches_legacy_fragment(role: &str, text: &str) -> bool {
        role == "developer"
            && (AppsInstructions::matches_text(text) || CatalogAppsInstructions::matches_text(text))
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
        self.render_retained_diff(previous, None)
    }

    fn render_retained_diff(
        &self,
        previous: PreviousSectionState<'_, Self::Snapshot>,
        retained: Option<&str>,
    ) -> SectionTransition<Self::Snapshot> {
        let bundled = AppsInstructions::default().0;
        let catalog = self.resolved.catalog_override;
        let previous = RetainedGuidance::recover(previous, retained, &bundled, catalog);
        let instructions = self
            .available
            .then_some(self.resolved.usage_instructions.as_str())
            .filter(|text| !text.is_empty());
        let is_bundled = instructions == Some(bundled.as_str());
        let snapshot = if catalog && is_bundled {
            AppsInstructionsSnapshot::Catalog {
                instructions: Some(fingerprint(&bundled)),
                catalog: true,
            }
        } else {
            AppsInstructionsSnapshot::BundledAvailable(is_bundled)
        };
        let previous_wording = match previous {
            RetainedGuidance::None => None,
            RetainedGuidance::Bundled => Some(bundled.as_str()),
            RetainedGuidance::Catalog(text) => Some(text),
        };
        if previous_wording == instructions {
            return (Some(snapshot), Vec::new());
        }
        let body = match instructions {
            // A notice-like literal must have a replacement prefix even on its first appearance,
            // so recovery can strip exactly one layer and keep every byte of the supplied text.
            Some(text)
                if previous_wording.is_some()
                    || text.starts_with(REPLACEMENT_NOTICE)
                    || text == REMOVAL_NOTICE =>
            {
                format!("{REPLACEMENT_NOTICE}{text}")
            }
            Some(text) => text.to_owned(),
            None if catalog || matches!(previous, RetainedGuidance::Catalog(_)) => {
                REMOVAL_NOTICE.to_owned()
            }
            None => return (Some(snapshot), Vec::new()),
        };
        let update = if is_bundled {
            WorldStateUpdate::fragment(AppsInstructions(body))
        } else {
            WorldStateUpdate::fragment(CatalogAppsInstructions(body))
        };
        (Some(snapshot), vec![update])
    }
}

#[cfg(test)]
#[path = "apps_instructions_tests.rs"]
mod tests;
