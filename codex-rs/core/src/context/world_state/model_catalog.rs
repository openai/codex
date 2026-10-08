//! Keeps spawn model choices in append-only context instead of the tool schema.
//! Snapshots compare the bounded text actually shown to the model; catalog refreshes
//! never rewrite earlier messages, and missing retained context gets a fresh catalog.

use super::PreviousSectionState;
use super::SectionTransition;
use super::WorldStateHash;
use super::WorldStateSection;
use super::WorldStateUpdate;
use crate::agent::child_config::MAX_SPAWN_AGENT_MODEL_OVERRIDES;
use crate::agent::child_config::model_supports_multi_agent_backend;
use crate::context::ContextualUserFragment;
use crate::context::environment_context::push_xml_escaped_text;
use codex_protocol::models::ContentItemKind;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::protocol::MultiAgentVersion;

// Includes XML escaping and markers; at most 1,000 tokens even with byte fallback.
const MAX_RENDERED_BYTES: usize = 1_000;
const MAX_DESCRIPTION_CHARS: usize = 250;
const OMITTED_NOTICE: &str = "Additional model choices omitted.\n";

/// The current model choices, or an empty catalog to invalidate a retained listing.
#[derive(Clone, Default)]
pub(crate) struct ModelCatalogState {
    catalog: String,
}

impl ModelCatalogState {
    pub(crate) fn new(models: &[ModelPreset], multi_agent_version: MultiAgentVersion) -> Self {
        let mut catalog = String::new();
        let mut models = models
            .iter()
            .filter(|model| model.show_in_picker)
            .filter(|model| model_supports_multi_agent_backend(model, multi_agent_version))
            .take(MAX_SPAWN_AGENT_MODEL_OVERRIDES)
            .collect::<Vec<_>>();
        // Stabilize the selected choices independently of their picker ordering.
        models.sort_by(|left, right| left.model.cmp(&right.model));
        if models.is_empty() {
            catalog.push_str("No picker-visible model overrides are currently loaded.\n");
        }
        let (open, close) = Self::type_markers();
        let body_budget = MAX_RENDERED_BYTES - open.len() - close.len() - 2;
        for model in models {
            let mut line = format!("- `{}`: ", model.model);
            line.extend(model.description.chars().take(MAX_DESCRIPTION_CHARS));
            if model.description.chars().count() > MAX_DESCRIPTION_CHARS {
                line.push('…');
            }
            if !model.supported_reasoning_efforts.is_empty() {
                line.push_str(" Reasoning efforts: ");
                for (index, preset) in model.supported_reasoning_efforts.iter().enumerate() {
                    if index > 0 {
                        line.push_str(", ");
                    }
                    line.push_str(preset.effort.as_str());
                    if preset.effort == model.default_reasoning_effort {
                        line.push_str(" (default)");
                    }
                }
                line.push('.');
            }
            if !model.service_tiers.is_empty() {
                line.push_str(" Service tiers: ");
                for (index, tier) in model.service_tiers.iter().enumerate() {
                    if index > 0 {
                        line.push_str(", ");
                    }
                    line.push_str(tier.id.as_str());
                }
                line.push('.');
            }
            let mut escaped = String::new();
            push_xml_escaped_text(&mut escaped, &line);
            if catalog.len() + escaped.len() + 1 + OMITTED_NOTICE.len() > body_budget {
                // Keep identifiers and capability lists intact instead of advertising
                // a truncated model ID, reasoning effort, or service tier.
                catalog.push_str(OMITTED_NOTICE);
                break;
            }
            catalog.push_str(&escaped);
            catalog.push('\n');
        }
        Self { catalog }
    }
}

impl ContextualUserFragment for ModelCatalogState {
    fn content_kind(&self) -> ContentItemKind {
        ContentItemKind("multi_agent.model_catalog".to_string())
    }

    fn role(&self) -> &'static str {
        "developer"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn type_markers() -> (&'static str, &'static str) {
        ("<model_catalog>", "</model_catalog>")
    }

    fn body(&self) -> String {
        if self.catalog.is_empty() {
            "\nThe previous spawn_agent model catalog no longer applies.\n".to_string()
        } else {
            format!("\n{}", self.catalog)
        }
    }
}

impl WorldStateSection for ModelCatalogState {
    const ID: &'static str = "model_catalog";
    type Snapshot = WorldStateHash;

    fn matches_legacy_fragment(role: &str, text: &str) -> bool {
        role == "developer" && Self::matches_text(text)
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
        let current = WorldStateHash::from_fragment(self);
        if matches!(previous, PreviousSectionState::Known(previous) if previous == &current)
            || self.catalog.is_empty() && matches!(previous, PreviousSectionState::Absent)
        {
            return (Some(current), Vec::new());
        }
        (
            Some(current),
            vec![WorldStateUpdate::fragment(self.clone())],
        )
    }
}
