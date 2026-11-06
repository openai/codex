//! Selects the reviewer and its metadata from one eligible catalog snapshot.

use codex_protocol::openai_models::CatalogModel;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ReasoningEffort;

/// Effective model choice, including the attribution carried by existing review events.
pub struct ReviewModel {
    pub model: String,
    pub reasoning_effort: Option<ReasoningEffort>,
    pub default_review_model_id: String,
    pub catalog_contains_auto_review: bool,
    pub model_overridden: bool,
    pub model_override: Option<String>,
}

/// Prefers the review model and low effort when advertised; preserves the parent fallback.
/// Returns the matching catalog entry so its metadata comes from the same selection.
pub fn select_review_model(
    parent_model: &ModelInfo,
    parent_reasoning_effort: Option<&ReasoningEffort>,
    default_review_model_id: &str,
    available_models: Vec<CatalogModel>,
) -> (ReviewModel, Option<CatalogModel>) {
    let preferred_reasoning_effort = |supports_low: bool, fallback| {
        if supports_low {
            Some(codex_protocol::openai_models::ReasoningEffort::Low)
        } else {
            fallback
        }
    };
    let model_override = parent_model.auto_review_model_override.as_deref();
    let review_model_id = model_override.unwrap_or(default_review_model_id);
    let guardian_catalog_contains_auto_review = available_models
        .iter()
        .any(|entry| entry.slug == default_review_model_id);
    let reviewer = available_models
        .into_iter()
        .filter(|entry| entry.slug == review_model_id)
        .min_by_key(|entry| entry.priority);
    let guardian_review_model_overridden = model_override.is_some();
    let guardian_review_model_override = model_override.map(str::to_string);
    let (guardian_model, guardian_reasoning_effort) = if let Some(model) = &reviewer {
        let reasoning_effort = preferred_reasoning_effort(
            model
                .supported_reasoning_levels
                .iter()
                .any(|effort| effort.effort == codex_protocol::openai_models::ReasoningEffort::Low),
            Some(
                model
                    .default_reasoning_level
                    .clone()
                    .unwrap_or(ReasoningEffort::None),
            ),
        );
        (review_model_id.to_string(), reasoning_effort)
    } else {
        let reasoning_effort = preferred_reasoning_effort(
            parent_model
                .supported_reasoning_levels
                .iter()
                .any(|preset| preset.effort == codex_protocol::openai_models::ReasoningEffort::Low),
            parent_reasoning_effort
                .or(parent_model.default_reasoning_level.as_ref())
                .cloned(),
        );
        (
            model_override
                .unwrap_or(parent_model.slug.as_str())
                .to_string(),
            reasoning_effort,
        )
    };

    let review_model = ReviewModel {
        model: guardian_model,
        reasoning_effort: guardian_reasoning_effort,
        default_review_model_id: default_review_model_id.to_string(),
        catalog_contains_auto_review: guardian_catalog_contains_auto_review,
        model_overridden: guardian_review_model_overridden,
        model_override: guardian_review_model_override,
    };
    (review_model, reviewer)
}
