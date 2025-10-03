use codex_protocol::custom_prompts::CustomPrompt;
use codex_protocol::custom_prompts::PROMPTS_CMD_PREFIX;
use lazy_static::lazy_static;
use regex_lite::Regex;
use shlex::Shlex;
use std::collections::HashMap;
use std::collections::HashSet;

lazy_static! {
    static ref PROMPT_ARG_REGEX: Regex =
        Regex::new(r"\$[A-Z][A-Z0-9_]*").unwrap_or_else(|_| std::process::abort());
}

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

/// Parse a first-line slash command of the form `/name <rest>`.
/// Returns `(name, rest_after_name)` if the line begins with `/` and contains
/// a non-empty name; otherwise returns `None`.
pub fn parse_slash_name(line: &str) -> Option<(&str, &str)> {
    let stripped = line.strip_prefix('/')?;
    let mut name_end = stripped.len();
    for (idx, ch) in stripped.char_indices() {
        if ch.is_whitespace() {
            name_end = idx;
            break;
        }
    }
    let name = &stripped[..name_end];
    if name.is_empty() {
        return None;
    }
    let rest = stripped[name_end..].trim_start();
    Some((name, rest))
}

/// Parse positional arguments using shlex semantics (supports quoted tokens).
pub fn parse_positional_args(rest: &str) -> Vec<String> {
    Shlex::new(rest).collect()
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
        if m.start() > 0 && content.as_bytes()[m.start() - 1] == b'$' {
            continue;
        }
        let name = &content[m.start() + 1..m.end()];
        // Exclude special positional aggregate token from named args.
        if name == "ARGUMENTS" {
            continue;
        }
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

/// Expands a message of the form `/prompts:name [value] [value] …` using a matching saved prompt.
///
/// If the text does not start with `/prompts:`, or if no prompt named `name` exists,
/// the function returns `Ok(None)`. On success it returns
/// `Ok(Some(expanded))`; otherwise it returns a descriptive error.
pub fn expand_custom_prompt(
    text: &str,
    custom_prompts: &[CustomPrompt],
) -> Result<Option<String>, PromptExpansionError> {
    let Some((name, rest)) = parse_slash_name(text) else {
        return Ok(None);
    };

    // Only handle custom prompts when using the explicit prompts prefix with a colon.
    let Some(prompt_name) = name.strip_prefix(&format!("{PROMPTS_CMD_PREFIX}:")) else {
        return Ok(None);
    };

    let prompt = match custom_prompts.iter().find(|p| p.name == prompt_name) {
        Some(prompt) => prompt,
        None => return Ok(None),
    };
    // If there are named placeholders, expect key=value inputs.
    let required = prompt_argument_names(&prompt.content);
    if !required.is_empty() {
        let inputs = parse_prompt_inputs(rest).map_err(|error| PromptExpansionError::Args {
            command: format!("/{name}"),
            error,
        })?;
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
        let content = &prompt.content;
        let replaced = PROMPT_ARG_REGEX.replace_all(content, |caps: &regex_lite::Captures<'_>| {
            if let Some(matched) = caps.get(0)
                && matched.start() > 0
                && content.as_bytes()[matched.start() - 1] == b'$'
            {
                return matched.as_str().to_string();
            }
            let whole = &caps[0];
            let key = &whole[1..];
            inputs
                .get(key)
                .cloned()
                .unwrap_or_else(|| whole.to_string())
        });
        return Ok(Some(replaced.into_owned()));
    }

    // Otherwise, treat it as numeric/positional placeholder prompt (or none).
    let pos_args: Vec<String> = Shlex::new(rest).collect();
    let expanded = expand_numeric_placeholders(&prompt.content, &pos_args);
    Ok(Some(expanded))
}

/// Detect whether `content` contains numeric placeholders ($1..$9) or `$ARGUMENTS`.
pub fn prompt_has_numeric_placeholders(content: &str) -> bool {
    if content.contains("$ARGUMENTS") {
        return true;
    }
    let bytes = content.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'$' {
            let b1 = bytes[i + 1];
            if (b'1'..=b'9').contains(&b1) {
                return true;
            }
        }
        i += 1;
    }
    false
}

/// Extract positional arguments from a composer first line like "/name a b" for a given prompt name.
/// Returns empty when the command name does not match or when there are no args.
pub fn extract_positional_args_for_prompt_line(line: &str, prompt_name: &str) -> Vec<String> {
    let trimmed = line.trim_start();
    let Some(rest) = trimmed.strip_prefix('/') else {
        return Vec::new();
    };
    // Require the explicit prompts prefix for custom prompt invocations.
    let Some(after_prefix) = rest.strip_prefix(&format!("{PROMPTS_CMD_PREFIX}:")) else {
        return Vec::new();
    };
    let mut parts = after_prefix.splitn(2, char::is_whitespace);
    let cmd = parts.next().unwrap_or("");
    if cmd != prompt_name {
        return Vec::new();
    }
    let args_str = parts.next().unwrap_or("").trim();
    if args_str.is_empty() {
        return Vec::new();
    }
    parse_positional_args(args_str)
}

/// If the prompt only uses numeric placeholders and the first line contains
/// positional args for it, expand and return Some(expanded); otherwise None.
pub fn expand_if_numeric_with_positional_args(
    prompt: &CustomPrompt,
    first_line: &str,
) -> Option<String> {
    if !prompt_argument_names(&prompt.content).is_empty() {
        return None;
    }
    if !prompt_has_numeric_placeholders(&prompt.content) {
        return None;
    }
    let args = extract_positional_args_for_prompt_line(first_line, &prompt.name);
    if args.is_empty() {
        return None;
    }
    Some(expand_numeric_placeholders(&prompt.content, &args))
}

/// Expand `$1..$9` and `$ARGUMENTS` in `content` with values from `args`.
pub fn expand_numeric_placeholders(content: &str, args: &[String]) -> String {
    let mut out = String::with_capacity(content.len());
    let mut i = 0;
    let mut cached_joined_args: Option<String> = None;
    while let Some(off) = content[i..].find('$') {
        let j = i + off;
        out.push_str(&content[i..j]);
        let rest = &content[j..];
        let bytes = rest.as_bytes();
        if bytes.len() >= 2 {
            match bytes[1] {
                b'$' => {
                    out.push_str("$$");
                    i = j + 2;
                    continue;
                }
                b'1'..=b'9' => {
                    let idx = (bytes[1] - b'1') as usize;
                    if let Some(val) = args.get(idx) {
                        out.push_str(val);
                    }
                    i = j + 2;
                    continue;
                }
                _ => {}
            }
        }
        if rest.len() > "ARGUMENTS".len() && rest[1..].starts_with("ARGUMENTS") {
            if !args.is_empty() {
                let joined = cached_joined_args.get_or_insert_with(|| args.join(" "));
                out.push_str(joined);
            }
            i = j + 1 + "ARGUMENTS".len();
            continue;
        }
        out.push('$');
        i = j + 1;
    }
    out.push_str(&content[i..]);
    out
}

/// Constructs a command text for a custom prompt with arguments.
/// Returns the text and the cursor position (inside the first double quote).
pub fn prompt_command_with_arg_placeholders(name: &str, args: &[String]) -> (String, usize) {
    let mut text = format!("/{PROMPTS_CMD_PREFIX}:{name}");
    let mut cursor: usize = text.len();
    for (i, arg) in args.iter().enumerate() {
        text.push_str(format!(" {arg}=\"\"").as_str());
        if i == 0 {
            cursor = text.len() - 1; // inside first ""
        }
    }
    (text, cursor)
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
            description: None,
            argument_hint: None,
        }];

        let out =
            expand_custom_prompt("/prompts:my-prompt USER=Alice BRANCH=main", &prompts).unwrap();
        assert_eq!(out, Some("Review Alice changes on main".to_string()));
    }

    #[test]
    fn quoted_values_ok() {
        let prompts = vec![CustomPrompt {
            name: "my-prompt".to_string(),
            path: "/tmp/my-prompt.md".to_string().into(),
            content: "Pair $USER with $BRANCH".to_string(),
            description: None,
            argument_hint: None,
        }];

        let out = expand_custom_prompt(
            "/prompts:my-prompt USER=\"Alice Smith\" BRANCH=dev-main",
            &prompts,
        )
        .unwrap();
        assert_eq!(out, Some("Pair Alice Smith with dev-main".to_string()));
    }

    #[test]
    fn invalid_arg_token_reports_error() {
        let prompts = vec![CustomPrompt {
            name: "my-prompt".to_string(),
            path: "/tmp/my-prompt.md".to_string().into(),
            content: "Review $USER changes".to_string(),
            description: None,
            argument_hint: None,
        }];
        let err = expand_custom_prompt("/prompts:my-prompt USER=Alice stray", &prompts)
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
            description: None,
            argument_hint: None,
        }];
        let err = expand_custom_prompt("/prompts:my-prompt USER=Alice", &prompts)
            .unwrap_err()
            .user_message();
        assert!(err.to_lowercase().contains("missing required args"));
        assert!(err.contains("BRANCH"));
    }

    #[test]
    fn escaped_placeholder_is_ignored() {
        assert_eq!(
            prompt_argument_names("literal $$USER"),
            Vec::<String>::new()
        );
        assert_eq!(
            prompt_argument_names("literal $$USER and $REAL"),
            vec!["REAL".to_string()]
        );
    }

    #[test]
    fn escaped_placeholder_remains_literal() {
        let prompts = vec![CustomPrompt {
            name: "my-prompt".to_string(),
            path: "/tmp/my-prompt.md".to_string().into(),
            content: "literal $$USER".to_string(),
            description: None,
            argument_hint: None,
        }];

        let out = expand_custom_prompt("/prompts:my-prompt", &prompts).unwrap();
        assert_eq!(out, Some("literal $$USER".to_string()));
    }
}
