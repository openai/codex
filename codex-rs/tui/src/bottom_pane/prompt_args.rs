use codex_protocol::custom_prompts::CustomPrompt;
use once_cell::sync::Lazy;
use regex_lite::Regex;
use shlex::Shlex;
use std::collections::HashMap;
use std::collections::HashSet;

static PROMPT_ARG_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\$[A-Z][A-Z0-9_]*").unwrap_or_else(|_| std::process::abort()));

#[derive(Debug)]
pub enum PromptArgsError {
    MissingAssignment { token: String },
    MissingKey { token: String },
}

impl PromptArgsError {
    fn describe(&self, command: &str) -> String {
        match self {
            PromptArgsError::MissingAssignment { token } => format!(
                "Could not parse {command}: expected key=value but found '{token}'. Wrap values in double quotes if they contain spaces."
            ),
            PromptArgsError::MissingKey { token } => {
                format!("Could not parse {command}: expected a name before '=' in '{token}'.")
            }
        }
    }
}

#[derive(Debug)]
pub enum PromptExpansionError {
    Args {
        command: String,
        error: PromptArgsError,
    },
    MissingArgs {
        command: String,
        missing: Vec<String>,
    },
}

impl PromptExpansionError {
    pub fn user_message(&self) -> String {
        match self {
            PromptExpansionError::Args { command, error } => error.describe(command),
            PromptExpansionError::MissingArgs { command, missing } => {
                let list = missing.join(", ");
                format!(
                    "Missing required args for {command}: {list}. Provide as key=value (quote values with spaces)."
                )
            }
        }
    }
}

/// Extracts the unique placeholder variable names from a prompt template.
///
/// A placeholder is any token that matches the pattern `$[A-Z][A-Z0-9_]*`
/// (for example `$USER`). The function returns the variable names without
/// the leading `$`, de-duplicated and in the order of first appearance.
pub fn prompt_argument_names(content: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut names = Vec::new();
    for m in PROMPT_ARG_REGEX.find_iter(content) {
        let name = &content[m.start() + 1..m.end()];
        let name = name.to_string();
        if seen.insert(name.clone()) {
            names.push(name);
        }
    }
    names
}

/// Parses the `key=value` pairs that follow a custom prompt name.
///
/// The input is split using shlex rules, so quoted values are supported
/// (for example `USER="Alice Smith"`). The function returns a map of parsed
/// arguments, or an error if a token is missing `=` or if the key is empty.
pub fn parse_prompt_inputs(rest: &str) -> Result<HashMap<String, String>, PromptArgsError> {
    let mut map = HashMap::new();
    if rest.trim().is_empty() {
        return Ok(map);
    }

    for token in Shlex::new(rest) {
        let Some((key, value)) = token.split_once('=') else {
            return Err(PromptArgsError::MissingAssignment { token });
        };
        if key.is_empty() {
            return Err(PromptArgsError::MissingKey { token });
        }
        map.insert(key.to_string(), value.to_string());
    }
    Ok(map)
}

/// Expands a message of the form `/name key=value …` using a matching saved prompt.
///
/// If the text does not start with `/`, or if no prompt named `name` exists,
/// the function returns `Ok(None)`. On success it returns
/// `Ok(Some(expanded))`; otherwise it returns a descriptive error.
pub fn expand_custom_prompt(
    text: &str,
    custom_prompts: &[CustomPrompt],
) -> Result<Option<String>, PromptExpansionError> {
    let Some(stripped) = text.strip_prefix('/') else {
        return Ok(None);
    };
    let mut name_end = stripped.len();
    for (idx, ch) in stripped.char_indices() {
        if ch.is_whitespace() {
            name_end = idx;
            break;
        }
    }

    let name = &stripped[..name_end];
    if name.is_empty() {
        return Ok(None);
    }

    let prompt = match custom_prompts.iter().find(|p| p.name == name) {
        Some(prompt) => prompt,
        None => return Ok(None),
    };
    let rest = stripped[name_end..].trim();
    let inputs = parse_prompt_inputs(rest).map_err(|error| PromptExpansionError::Args {
        command: format!("/{name}"),
        error,
    })?;

    // Ensure that all required variables are provided.
    let required = prompt_argument_names(&prompt.content);
    let missing: Vec<String> = required
        .into_iter()
        .filter(|k| !inputs.contains_key(k))
        .collect();
    if !missing.is_empty() {
        return Err(PromptExpansionError::MissingArgs {
            command: format!("/{name}"),
            missing,
        });
    }

    let replaced =
        PROMPT_ARG_REGEX.replace_all(&prompt.content, |caps: &regex_lite::Captures<'_>| {
            let whole = caps.get(0).map(|m| m.as_str()).unwrap_or("");
            let key = &whole[1..];
            inputs
                .get(key)
                .cloned()
                .unwrap_or_else(|| whole.to_string())
        });

    Ok(Some(replaced.into_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_arguments_basic() {
        let prompts = vec![CustomPrompt {
            name: "my-prompt".to_string(),
            path: "/tmp/my-prompt.md".to_string().into(),
            content: "Review $USER changes on $BRANCH".to_string(),
        }];

        let out = expand_custom_prompt("/my-prompt USER=Alice BRANCH=main", &prompts).unwrap();
        assert_eq!(out, Some("Review Alice changes on main".to_string()));
    }

    #[test]
    fn quoted_values_ok() {
        let prompts = vec![CustomPrompt {
            name: "my-prompt".to_string(),
            path: "/tmp/my-prompt.md".to_string().into(),
            content: "Pair $USER with $BRANCH".to_string(),
        }];

        let out = expand_custom_prompt("/my-prompt USER=\"Alice Smith\" BRANCH=dev-main", &prompts)
            .unwrap();
        assert_eq!(out, Some("Pair Alice Smith with dev-main".to_string()));
    }

    #[test]
    fn invalid_arg_token_reports_error() {
        let prompts = vec![CustomPrompt {
            name: "my-prompt".to_string(),
            path: "/tmp/my-prompt.md".to_string().into(),
            content: "Review $USER changes".to_string(),
        }];
        let err = expand_custom_prompt("/my-prompt USER=Alice stray", &prompts)
            .unwrap_err()
            .user_message();
        assert!(err.contains("expected key=value"));
    }

    #[test]
    fn missing_required_args_reports_error() {
        let prompts = vec![CustomPrompt {
            name: "my-prompt".to_string(),
            path: "/tmp/my-prompt.md".to_string().into(),
            content: "Review $USER changes on $BRANCH".to_string(),
        }];
        let err = expand_custom_prompt("/my-prompt USER=Alice", &prompts)
            .unwrap_err()
            .user_message();
        assert!(err.to_lowercase().contains("missing required args"));
        assert!(err.contains("BRANCH"));
    }
}
