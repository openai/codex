const DEFAULT_ATTRIBUTION_VALUE: &str = "Codex <noreply@openai.com>";

fn build_commit_message_trailer(config_attribution: Option<&str>) -> Option<String> {
    let value = resolve_attribution_value(config_attribution)?;
    Some(format!("Co-authored-by: {value}"))
}

pub(crate) fn commit_message_trailer_instruction(
    config_attribution: Option<&str>,
) -> Option<String> {
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
    use super::build_commit_message_trailer;
    use super::commit_message_trailer_instruction;
    use super::resolve_attribution_value;

    #[test]
    fn blank_attribution_disables_trailer_prompt() {
        assert_eq!(build_commit_message_trailer(Some("")), None);
        assert_eq!(commit_message_trailer_instruction(Some("   ")), None);
    }

    #[test]
    fn default_attribution_uses_codex_trailer() {
        assert_eq!(
            build_commit_message_trailer(None).as_deref(),
            Some("Co-authored-by: Codex <noreply@openai.com>")
        );
    }

    #[test]
    fn resolve_value_handles_default_custom_and_blank() {
        assert_eq!(
            resolve_attribution_value(None),
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
        let instruction = commit_message_trailer_instruction(Some("AgentX <agent@example.com>"))
            .expect("instruction expected");
        assert!(instruction.contains("Co-authored-by: AgentX <agent@example.com>"));
        assert!(instruction.contains("exactly once"));
        assert!(!instruction.contains("Generated-with"));
    }
}
