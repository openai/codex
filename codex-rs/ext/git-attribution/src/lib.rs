use std::sync::Arc;

use codex_core::config::Config;
use codex_extension_api::ContextContributor;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::PromptFragment;
use codex_extension_api::ThreadStartContributor;
use codex_features::Feature;

const DEFAULT_ATTRIBUTION_VALUE: &str = "Codex <noreply@openai.com>";

/// Contributes the configured git-attribution instruction.
#[derive(Clone, Copy, Debug, Default)]
pub struct GitAttributionExtension;

impl ContextContributor for GitAttributionExtension {
    fn contribute(
        &self,
        _session_store: &ExtensionData,
        thread_store: &ExtensionData,
    ) -> Vec<PromptFragment> {
        let Some(config_store) = thread_store.get::<GitAttributionConfig>() else {
            return Vec::new();
        };
        if !config_store.enabled {
            return Vec::new();
        }
        build_instruction(config_store.prompt.as_deref())
            .map(PromptFragment::developer_policy)
            .into_iter()
            .collect()
    }
}

#[derive(Clone, Debug, Default)]
struct GitAttributionConfig {
    enabled: bool,
    prompt: Option<String>,
}

impl ThreadStartContributor<Config> for GitAttributionExtension {
    fn contribute(
        &self,
        config: &Config,
        _session_store: &ExtensionData,
        thread_store: &ExtensionData,
    ) {
        thread_store.insert(GitAttributionConfig {
            enabled: config.features.enabled(Feature::CodexGitCommit),
            prompt: config.commit_attribution.clone(),
        });
    }
}

/// Installs the git-attribution contributors into the extension registry.
pub fn install(registry: &mut ExtensionRegistryBuilder<Config>) {
    let extension = Arc::new(GitAttributionExtension);
    registry.thread_start_contributor(extension.clone());
    registry.prompt_contributor(extension);
}

fn build_commit_message_trailer(config_attribution: Option<&str>) -> Option<String> {
    let value = resolve_attribution_value(config_attribution)?;
    Some(format!("Co-authored-by: {value}"))
}

fn build_instruction(config_attribution: Option<&str>) -> Option<String> {
    let trailer = build_commit_message_trailer(config_attribution)?;
    Some(format!(
        "When you write or edit a git commit message, ensure the message ends with this trailer exactly once:\n{trailer}\n\nRules:\n- Keep existing trailers and append this trailer at the end if missing.\n- Do not duplicate this trailer if it already exists.\n- Keep one blank line between the commit body and trailer block."
    ))
}

fn resolve_attribution_value(config_attribution: Option<&str>) -> Option<String> {
    match config_attribution {
        Some(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        None => Some(DEFAULT_ATTRIBUTION_VALUE.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::build_commit_message_trailer;
    use super::build_instruction;
    use super::resolve_attribution_value;

    #[test]
    fn blank_attribution_disables_trailer_prompt() {
        assert_eq!(build_commit_message_trailer(Some("")), None);
        assert_eq!(build_instruction(Some("   ")), None);
    }

    #[test]
    fn default_attribution_uses_codex_trailer() {
        assert_eq!(
            build_commit_message_trailer(/*config_attribution*/ None).as_deref(),
            Some("Co-authored-by: Codex <noreply@openai.com>")
        );
    }

    #[test]
    fn resolve_value_handles_default_custom_and_blank() {
        assert_eq!(
            resolve_attribution_value(/*config_attribution*/ None),
            Some("Codex <noreply@openai.com>".to_string())
        );
        assert_eq!(
            resolve_attribution_value(Some("MyAgent <me@example.com>")),
            Some("MyAgent <me@example.com>".to_string())
        );
        assert_eq!(
            resolve_attribution_value(Some("MyAgent")),
            Some("MyAgent".to_string())
        );
        assert_eq!(resolve_attribution_value(Some("   ")), None);
    }

    #[test]
    fn instruction_mentions_trailer_and_omits_generated_with() {
        let instruction =
            build_instruction(Some("AgentX <agent@example.com>")).expect("instruction expected");
        assert!(instruction.contains("Co-authored-by: AgentX <agent@example.com>"));
        assert!(instruction.contains("exactly once"));
        assert!(!instruction.contains("Generated-with"));
    }
}
