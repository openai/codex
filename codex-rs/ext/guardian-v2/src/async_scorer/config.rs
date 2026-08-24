use codex_core::config::Config;
use codex_features::FeatureToml;
use codex_features::GuardianV2ConfigToml;
use codex_features::GuardianV2TranscriptConfigToml;
use codex_protocol::openai_models::GuardianV2ModelConfig;
use codex_protocol::openai_models::ReasoningEffort;

use super::transcript::MAX_MESSAGE_ENTRY_TOKENS;
use super::transcript::MAX_MESSAGE_TRANSCRIPT_TOKENS;
use super::transcript::MAX_RECENT_NON_USER_ENTRIES;
use super::transcript::MAX_TOOL_ENTRY_TOKENS;
use super::transcript::MAX_TOOL_TRANSCRIPT_TOKENS;
use super::transcript::TranscriptConfig;
use super::transcript::TranscriptSource;
use super::transcript::truncate_entry;

pub(crate) const DEFAULT_MODEL_CONTEXT_ITEM_TOKENS: usize = 10_000;
pub(crate) const DEFAULT_PARENT_COMPACTION_TOKENS: usize = 25_000;
const MIN_MODEL_CONTEXT_ITEM_TOKENS: usize = 100;
const MAX_MODEL_CONTEXT_ITEM_TOKENS: usize = 100_000;
const DEFAULT_REVIEW_THRESHOLD: f64 = 0.5;
const LEGACY_REVIEW_THRESHOLD: f64 = 0.8;
const DEFAULT_MAX_TOOL_CALL_LAG: usize = 3;
pub(crate) const DEFAULT_CLASSIFIER_INSTRUCTIONS: &str = include_str!("classifier_instructions.md");
pub(crate) const CLASSIFICATION_OUTPUT_INSTRUCTIONS: &str = "Your first output token is the entire classification: `high` for high risk or `low` for low risk. Output that token immediately and nothing else.";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GuardianV2ReviewScope {
    Standard { sandboxed_exec_commands: bool },
    ComputerUseOnly,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct GuardianV2Config {
    local_overrides: GuardianV2ConfigToml,
    pub(crate) classifier_instructions: String,
    pub(crate) review_threshold: f64,
    pub(crate) max_tool_call_lag: usize,
    pub(crate) reasoning_effort: ReasoningEffort,
    pub(crate) max_action_tokens: usize,
    /// No truncation limit is applied unless local or model configuration supplies one.
    pub(crate) max_classifier_instruction_tokens: Option<usize>,
    pub(crate) reuse_parent_compaction: bool,
    pub(crate) max_parent_compaction_tokens: usize,
    pub(crate) review_scope: GuardianV2ReviewScope,
    pub(crate) transcript: TranscriptConfig,
}

impl GuardianV2Config {
    pub(crate) fn resolve(config: &Config) -> Result<Self, String> {
        let effective_config = config.config_layer_stack.effective_config();
        let configured = match effective_config
            .get("features")
            .and_then(|features| features.get("guardianv2"))
            .cloned()
        {
            Some(value) => {
                let feature: FeatureToml<GuardianV2ConfigToml> = value
                    .try_into()
                    .map_err(|error| format!("invalid Guardian v2 configuration: {error}"))?;
                match feature {
                    FeatureToml::Enabled(_) => GuardianV2ConfigToml::default(),
                    FeatureToml::Config(configured) => configured,
                }
            }
            None => GuardianV2ConfigToml::default(),
        };

        Self::from_overrides(configured)
    }

    pub(crate) fn with_model_defaults(
        &self,
        model_defaults: Option<&GuardianV2ModelConfig>,
    ) -> Result<Self, String> {
        let mut configured = self.local_overrides.clone();
        if let Some(model_defaults) = model_defaults {
            configured.classifier_instructions = configured
                .classifier_instructions
                .or_else(|| model_defaults.classifier_instructions.clone());
            configured.review_threshold = configured.review_threshold.or_else(|| {
                model_defaults
                    .review_threshold_basis_points
                    .map(|basis_points| f64::from(basis_points) / 10_000.0)
            });
            configured.max_tool_call_lag = configured
                .max_tool_call_lag
                .or(model_defaults.max_tool_call_lag);
            configured.reasoning_effort = configured
                .reasoning_effort
                .or_else(|| model_defaults.reasoning_effort.clone());
            configured.max_action_tokens = configured
                .max_action_tokens
                .or(model_defaults.max_action_tokens);
            configured.max_classifier_instruction_tokens = configured
                .max_classifier_instruction_tokens
                .or(model_defaults.max_classifier_instruction_tokens);
            configured.reuse_parent_compaction = configured
                .reuse_parent_compaction
                .or(model_defaults.reuse_parent_compaction);
            configured.max_parent_compaction_tokens = configured
                .max_parent_compaction_tokens
                .or(model_defaults.max_parent_compaction_tokens);

            if let Some(model_transcript) = model_defaults.transcript.as_ref() {
                let transcript = configured
                    .transcript
                    .get_or_insert_with(GuardianV2TranscriptConfigToml::default);
                if transcript.sources.is_none()
                    && let Some(sources) = model_transcript.sources.as_ref()
                {
                    transcript.sources = Some(
                        sources
                            .iter()
                            .map(|source| match source.as_str() {
                                "tool_calls" => Ok(TranscriptSource::ToolCalls),
                                "tool_outputs" => Ok(TranscriptSource::ToolOutputs),
                                "reasoning" => Ok(TranscriptSource::Reasoning),
                                _ => {
                                    Err(format!("invalid Guardian v2 transcript source: {source}"))
                                }
                            })
                            .collect::<Result<Vec<_>, _>>()?,
                    );
                }
                transcript.include_images = transcript
                    .include_images
                    .or(model_transcript.include_images);
                transcript.max_message_entry_tokens = transcript
                    .max_message_entry_tokens
                    .or(model_transcript.max_message_entry_tokens);
                transcript.max_tool_entry_tokens = transcript
                    .max_tool_entry_tokens
                    .or(model_transcript.max_tool_entry_tokens);
                transcript.max_message_transcript_tokens = transcript
                    .max_message_transcript_tokens
                    .or(model_transcript.max_message_transcript_tokens);
                transcript.max_tool_transcript_tokens = transcript
                    .max_tool_transcript_tokens
                    .or(model_transcript.max_tool_transcript_tokens);
                transcript.max_recent_non_user_entries = transcript
                    .max_recent_non_user_entries
                    .or(model_transcript.max_recent_non_user_entries);
            }
        }

        let mut resolved = Self::from_overrides(configured)?;
        resolved.local_overrides = self.local_overrides.clone();
        Ok(resolved)
    }

    fn from_overrides(configured: GuardianV2ConfigToml) -> Result<Self, String> {
        // Existing custom and model-owned prompts retain their original calibration
        // unless their owner explicitly supplies a matching threshold.
        let review_threshold = configured.review_threshold.unwrap_or_else(|| {
            if configured.classifier_instructions.is_some() {
                LEGACY_REVIEW_THRESHOLD
            } else {
                DEFAULT_REVIEW_THRESHOLD
            }
        });
        if !review_threshold.is_finite() || !(0.0..=1.0).contains(&review_threshold) {
            return Err("Guardian v2 review_threshold must be between 0.0 and 1.0".to_owned());
        }

        let max_action_tokens = bounded_tokens(
            configured.max_action_tokens,
            DEFAULT_MODEL_CONTEXT_ITEM_TOKENS,
            "max_action_tokens",
        )?;
        let max_classifier_instruction_tokens = configured
            .max_classifier_instruction_tokens
            .map(|tokens| bounded_tokens(Some(tokens), tokens, "max_classifier_instruction_tokens"))
            .transpose()?;
        let max_parent_compaction_tokens = bounded_tokens(
            configured.max_parent_compaction_tokens,
            DEFAULT_PARENT_COMPACTION_TOKENS,
            "max_parent_compaction_tokens",
        )?;
        let transcript_config = configured.transcript.as_ref();
        let max_message_entry_tokens = bounded_tokens(
            transcript_config.and_then(|transcript| transcript.max_message_entry_tokens),
            MAX_MESSAGE_ENTRY_TOKENS,
            "max_message_entry_tokens",
        )?;
        let max_tool_entry_tokens = bounded_tokens(
            transcript_config.and_then(|transcript| transcript.max_tool_entry_tokens),
            MAX_TOOL_ENTRY_TOKENS,
            "max_tool_entry_tokens",
        )?;
        let max_message_transcript_tokens = bounded_tokens(
            transcript_config.and_then(|transcript| transcript.max_message_transcript_tokens),
            MAX_MESSAGE_TRANSCRIPT_TOKENS,
            "max_message_transcript_tokens",
        )?;
        let max_tool_transcript_tokens = bounded_tokens(
            transcript_config.and_then(|transcript| transcript.max_tool_transcript_tokens),
            MAX_TOOL_TRANSCRIPT_TOKENS,
            "max_tool_transcript_tokens",
        )?;
        for (entry_tokens, transcript_tokens, entry_setting, transcript_setting) in [
            (
                max_message_entry_tokens,
                max_message_transcript_tokens,
                "max_message_entry_tokens",
                "max_message_transcript_tokens",
            ),
            (
                max_tool_entry_tokens,
                max_tool_transcript_tokens,
                "max_tool_entry_tokens",
                "max_tool_transcript_tokens",
            ),
        ] {
            if entry_tokens > transcript_tokens {
                return Err(format!(
                    "Guardian v2 transcript {entry_setting} must not exceed {transcript_setting}"
                ));
            }
        }
        let max_recent_non_user_entries = transcript_config
            .and_then(|transcript| transcript.max_recent_non_user_entries)
            .unwrap_or(MAX_RECENT_NON_USER_ENTRIES);
        if max_recent_non_user_entries == 0 {
            return Err(
                "Guardian v2 transcript max_recent_non_user_entries must be positive".to_owned(),
            );
        }

        Ok(Self {
            local_overrides: configured.clone(),
            classifier_instructions: configured
                .classifier_instructions
                .unwrap_or_else(|| DEFAULT_CLASSIFIER_INSTRUCTIONS.to_owned()),
            review_threshold,
            max_tool_call_lag: configured
                .max_tool_call_lag
                .unwrap_or(DEFAULT_MAX_TOOL_CALL_LAG),
            reasoning_effort: configured.reasoning_effort.unwrap_or(ReasoningEffort::Low),
            max_action_tokens,
            max_classifier_instruction_tokens,
            reuse_parent_compaction: configured.reuse_parent_compaction.unwrap_or(true),
            max_parent_compaction_tokens,
            review_scope: if configured
                .review_scope
                .as_ref()
                .and_then(|review_scope| review_scope.computer_use_only)
                .unwrap_or(false)
            {
                GuardianV2ReviewScope::ComputerUseOnly
            } else {
                GuardianV2ReviewScope::Standard {
                    sandboxed_exec_commands: configured
                        .review_scope
                        .as_ref()
                        .and_then(|review_scope| review_scope.sandboxed_exec_commands)
                        .unwrap_or(false),
                }
            },
            transcript: TranscriptConfig {
                sources: transcript_config
                    .and_then(|transcript| transcript.sources.clone())
                    .unwrap_or_else(|| {
                        vec![TranscriptSource::ToolCalls, TranscriptSource::ToolOutputs]
                    }),
                include_images: transcript_config
                    .and_then(|transcript| transcript.include_images)
                    .unwrap_or(false),
                max_message_entry_tokens,
                max_tool_entry_tokens,
                max_message_transcript_tokens,
                max_tool_transcript_tokens,
                max_recent_non_user_entries,
            },
        })
    }

    pub(crate) fn render_classifier_instructions(&self, policy: &str) -> String {
        let instructions = if self
            .classifier_instructions
            .contains("{{ tenant_policy_config }}")
        {
            self.classifier_instructions
                .replace("{{ tenant_policy_config }}", policy)
        } else {
            format!(
                "{}\n\n# Security Policy\n{policy}",
                self.classifier_instructions
            )
        };
        let instructions = if instructions
            .trim_end()
            .ends_with(CLASSIFICATION_OUTPUT_INSTRUCTIONS)
        {
            instructions
        } else {
            format!("{instructions}\n\n{CLASSIFICATION_OUTPUT_INSTRUCTIONS}")
        };
        match self.max_classifier_instruction_tokens {
            Some(max_tokens) => truncate_entry(&instructions, max_tokens),
            None => instructions,
        }
    }
}

fn bounded_tokens(value: Option<usize>, default: usize, setting: &str) -> Result<usize, String> {
    let tokens = value.unwrap_or(default);
    if !(MIN_MODEL_CONTEXT_ITEM_TOKENS..=MAX_MODEL_CONTEXT_ITEM_TOKENS).contains(&tokens) {
        return Err(format!(
            "Guardian v2 {setting} must be between {MIN_MODEL_CONTEXT_ITEM_TOKENS} and {MAX_MODEL_CONTEXT_ITEM_TOKENS} tokens"
        ));
    }
    Ok(tokens)
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
