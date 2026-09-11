//! Cache-preserving effort updates and the request-effort baseline for a context window.
//!
//! Only trusted harness items establish overrides. Replay preserves startup prewarm's
//! baseline while it is retained.
//! Successful compaction retires the overrides and allows a fresh request baseline.

use super::session::Session;
use super::step_context::StepContext;
use super::step_settings::ResolvedStepSettings;
use crate::state::ReasoningEffortPin;
use codex_features::Feature;
use codex_history::CodexHarnessMetadata;
use codex_history::ResponseItemEnvelope;
use codex_protocol::models::ConfigurationReasoning;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ReasoningEffort;

/// Sampling can establish a pin; compaction must not change live state before it succeeds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RequestEffortUsage {
    Sampling,
    Compaction,
}

impl Session {
    /// Establishes the selected effort in surviving history, independent of replayed settings.
    pub(crate) async fn record_reasoning_effort_override(&self, step_context: &StepContext) {
        let settings = &step_context.settings;
        let Some(effort) = self.effort_for_configuration_update(settings).await else {
            return;
        };
        let should_skip = {
            let mut state = self.state.lock().await;
            if matches!(state.reasoning_effort_pin, ReasoningEffortPin::Compacted) {
                // Only successful compaction allows the request baseline to establish
                // the selection without another update.
                state
                    .reasoning_effort_pin
                    .pin(&settings.model_info.slug, effort.clone());
            }
            let latest_override = state
                .history
                .annotated_items()
                .iter()
                .rev()
                .enumerate()
                .find_map(|(index, envelope)| {
                    if !envelope
                        .metadata
                        .as_ref()
                        .is_some_and(|metadata| metadata.harness_authored_configuration)
                    {
                        return None;
                    }
                    match &envelope.item {
                        ResponseItem::ConfigurationUpdate { reasoning } => {
                            Some((index, &reasoning.effort))
                        }
                        _ => None,
                    }
                });
            // Recovery adds no user message. Reuse a matching trusted tail update even
            // before this runtime establishes its pin, rather than append adjacent updates.
            latest_override.is_some_and(|(index, established)| index == 0 && established == &effort)
                || state
                    .reasoning_effort_pin
                    .get(&settings.model_info.slug)
                    .is_some_and(|pinned| {
                        latest_override.map_or(&pinned, |(_, established)| established) == &effort
                    })
        };
        if should_skip {
            return;
        }

        self.record_annotated_conversation_items(
            step_context.turn.as_ref(),
            &step_context.settings.model_info,
            vec![ResponseItemEnvelope {
                item: ResponseItem::ConfigurationUpdate {
                    reasoning: ConfigurationReasoning { effort },
                },
                metadata: Some(CodexHarnessMetadata {
                    harness_authored_configuration: true,
                    ..Default::default()
                }),
            }],
        )
        .await;
    }

    /// Sampling and compaction share the original request effort for this context window.
    pub(crate) async fn reasoning_effort_for_request(
        &self,
        settings: &ResolvedStepSettings,
        usage: RequestEffortUsage,
    ) -> Option<ReasoningEffort> {
        let selected_effort = settings.reasoning_effort().cloned();
        if !self.enabled(Feature::ReasoningEffortOverride) {
            return selected_effort;
        }
        if usage == RequestEffortUsage::Compaction
            && let Some(pinned) = self
                .state
                .lock()
                .await
                .reasoning_effort_pin
                .get(&settings.model_info.slug)
        {
            return Some(pinned);
        }
        let effort = self.effort_for_configuration_update(settings).await;
        let mut state = self.state.lock().await;
        let Some(effort) = effort else {
            if usage == RequestEffortUsage::Sampling {
                state.reasoning_effort_pin = ReasoningEffortPin::Unset;
            }
            return selected_effort;
        };
        Some(match usage {
            RequestEffortUsage::Sampling => state
                .reasoning_effort_pin
                .pin(&settings.model_info.slug, effort),
            // Failed compaction and fallback-model lookups must not mutate the live pin.
            RequestEffortUsage::Compaction => effort,
        })
    }

    async fn effort_for_configuration_update(
        &self,
        settings: &ResolvedStepSettings,
    ) -> Option<ReasoningEffort> {
        if !self.enabled(Feature::ReasoningEffortOverride)
            || !settings.model_info.use_responses_lite
            || !self.provider().await.is_openai()
        {
            return None;
        }
        let effort = settings
            .model_info
            .resolve_reasoning_effort(settings.effective_reasoning_effort()?);
        // Persistent normalizes to "disabled". Keep unknown custom values out of
        // durable updates so injected items stay bounded to known backend modes.
        if matches!(&effort, ReasoningEffort::Custom(value) if value != "disabled") {
            return None;
        }
        Some(effort)
    }
}

#[cfg(test)]
#[path = "reasoning_effort_tests.rs"]
mod tests;
