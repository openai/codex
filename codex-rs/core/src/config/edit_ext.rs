//! Extension trait for ConfigEditsBuilder to support model_provider setting.

use super::edit::ConfigEdit;
use super::edit::ConfigEditsBuilder;
use toml_edit::value;

/// Extension trait for ConfigEditsBuilder to support model_provider setting.
pub trait ConfigEditsBuilderExt {
    /// Set the model_provider field in config.toml.
    /// Pass None to clear the field.
    fn set_model_provider(self, provider: Option<&str>) -> Self;
}

impl ConfigEditsBuilderExt for ConfigEditsBuilder {
    fn set_model_provider(self, provider: Option<&str>) -> Self {
        match provider {
            Some(p) => self.with_edits(vec![ConfigEdit::SetPath {
                segments: vec!["model_provider".to_string()],
                value: value(p.to_string()),
            }]),
            None => self.with_edits(vec![ConfigEdit::ClearPath {
                segments: vec!["model_provider".to_string()],
            }]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CONFIG_TOML_FILE;
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;

    #[test]
    fn test_set_model_provider() {
        let tmp = tempdir().expect("tmpdir");
        let codex_home = tmp.path();

        ConfigEditsBuilder::new(codex_home)
            .set_model_provider(Some("deepseek"))
            .apply_blocking()
            .expect("persist");

        let contents =
            std::fs::read_to_string(codex_home.join(CONFIG_TOML_FILE)).expect("read config");
        assert_eq!(contents, "model_provider = \"deepseek\"\n");
    }

    #[test]
    fn test_clear_model_provider() {
        let tmp = tempdir().expect("tmpdir");
        let codex_home = tmp.path();

        // First set a provider
        ConfigEditsBuilder::new(codex_home)
            .set_model_provider(Some("deepseek"))
            .apply_blocking()
            .expect("persist");

        // Then clear it
        ConfigEditsBuilder::new(codex_home)
            .set_model_provider(None)
            .apply_blocking()
            .expect("persist");

        let contents =
            std::fs::read_to_string(codex_home.join(CONFIG_TOML_FILE)).expect("read config");
        // model_provider should be removed
        assert!(!contents.contains("model_provider"));
    }

    #[test]
    fn test_set_model_with_provider() {
        use codex_protocol::openai_models::ReasoningEffort;

        let tmp = tempdir().expect("tmpdir");
        let codex_home = tmp.path();

        ConfigEditsBuilder::new(codex_home)
            .set_model(Some("deepseek-r1"), Some(ReasoningEffort::High))
            .set_model_provider(Some("deepseek"))
            .apply_blocking()
            .expect("persist");

        let contents =
            std::fs::read_to_string(codex_home.join(CONFIG_TOML_FILE)).expect("read config");
        assert!(contents.contains("model = \"deepseek-r1\""));
        assert!(contents.contains("model_reasoning_effort = \"high\""));
        assert!(contents.contains("model_provider = \"deepseek\""));
    }
}
