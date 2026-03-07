use crate::config::Config;
use crate::features::Feature;

const TERMINAL_HYPERLINKS_PROMPT_SUFFIX: &str = r#"

## Terminal Hyperlinks

- Prefer paths relative to the current working directory instead of absolute paths for files inside the workspace.
- Include the start line when it matters, for example `src/app.ts:42` or `src/app.ts#L42`.
- Avoid bare filenames when a cwd-relative path would be clearer.
- If a cwd-relative path is still noisy, shorten it by eliding middle directories, for example `codex-rs/.../terminal_hyperlinks.rs:42`, when the remaining reference stays unambiguous.
"#;

pub(crate) fn augment_model_instructions_for_terminal_hyperlinks(
    mut base_instructions: String,
    config: &Config,
) -> String {
    if !config.features.enabled(Feature::TerminalHyperlinks)
        || base_instructions.contains("## Terminal Hyperlinks")
    {
        return base_instructions;
    }

    base_instructions.push_str(TERMINAL_HYPERLINKS_PROMPT_SUFFIX);
    base_instructions
}

#[cfg(test)]
mod tests {
    use super::augment_model_instructions_for_terminal_hyperlinks;
    use crate::config::test_config;
    use crate::features::Feature;

    #[test]
    fn terminal_hyperlinks_suffix_is_only_appended_when_enabled() {
        let mut config = test_config();
        let original = "Base instructions".to_string();

        assert_eq!(
            augment_model_instructions_for_terminal_hyperlinks(original.clone(), &config),
            original
        );

        let _ = config.features.enable(Feature::TerminalHyperlinks);
        let augmented =
            augment_model_instructions_for_terminal_hyperlinks(original.clone(), &config);
        assert!(augmented.contains("## Terminal Hyperlinks"));
        assert!(augmented.starts_with("Base instructions"));
    }

    #[test]
    fn terminal_hyperlinks_suffix_is_not_duplicated() {
        let mut config = test_config();
        let _ = config.features.enable(Feature::TerminalHyperlinks);
        let base = "Base instructions\n\n## Terminal Hyperlinks\n".to_string();

        let augmented = augment_model_instructions_for_terminal_hyperlinks(base.clone(), &config);
        assert_eq!(augmented, base);
    }
}
