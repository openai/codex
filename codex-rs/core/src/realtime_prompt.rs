const BACKEND_PROMPT: &str = include_str!("../templates/realtime/backend_prompt.md");
const DEFAULT_USER_FIRST_NAME: &str = "there";
const USER_FIRST_NAME_PLACEHOLDER: &str = "{{ user_first_name }}";

pub(crate) fn prepare_realtime_backend_prompt(
    prompt: Option<Option<String>>,
    config_prompt: Option<String>,
) -> String {
    if let Some(config_prompt) = config_prompt
        && !config_prompt.trim().is_empty()
    {
        return config_prompt;
    }

    match prompt {
        Some(Some(prompt)) => return prompt,
        Some(None) => return String::new(),
        None => {}
    }

    BACKEND_PROMPT
        .trim_end()
        .replace(USER_FIRST_NAME_PLACEHOLDER, &current_user_first_name())
}

fn current_user_first_name() -> String {
    [whoami::realname(), whoami::username()]
        .into_iter()
        .filter_map(|name| name.split_whitespace().next().map(str::to_string))
        .filter(|name| !name.is_empty())
        .next()
        .unwrap_or_else(|| DEFAULT_USER_FIRST_NAME.to_string())
}

#[cfg(test)]
mod tests {
    use super::prepare_realtime_backend_prompt;

    #[test]
    fn prepare_realtime_backend_prompt_prefers_config_override() {
        assert_eq!(
            prepare_realtime_backend_prompt(
                Some(Some("prompt from request".to_string())),
                Some("prompt from config".to_string()),
            ),
            "prompt from config"
        );
    }

    #[test]
    fn prepare_realtime_backend_prompt_uses_request_prompt() {
        assert_eq!(
            prepare_realtime_backend_prompt(Some(Some("prompt from request".to_string())), None),
            "prompt from request"
        );
    }

    #[test]
    fn prepare_realtime_backend_prompt_preserves_empty_request_prompt() {
        assert_eq!(
            prepare_realtime_backend_prompt(Some(Some(String::new())), None),
            ""
        );
        assert_eq!(prepare_realtime_backend_prompt(Some(None), None), "");
    }

    #[test]
    fn prepare_realtime_backend_prompt_renders_default() {
        let prompt = prepare_realtime_backend_prompt(None, None);

        assert!(prompt.starts_with("You are **Codex**"));
        assert!(prompt.contains("The user's name is "));
        assert!(!prompt.contains("{{ user_first_name }}"));
    }
}
