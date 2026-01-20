use codex_protocol::custom_prompts::CustomPrompt;
use codex_protocol::custom_prompts::PROMPTS_CMD_PREFIX;
use codex_protocol::user_input::ByteRange;
use codex_protocol::user_input::TextElement;
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
/// Returns `(name, rest_after_name, rest_offset)` if the line begins with `/`
/// and contains a non-empty name; otherwise returns `None`.
///
/// `rest_offset` is the byte index into the original line where `rest_after_name`
/// starts after trimming leading whitespace (so `line[rest_offset..] == rest_after_name`).
pub fn parse_slash_name(line: &str) -> Option<(&str, &str, usize)> {
    let stripped = line.strip_prefix('/')?;
    let mut name_end_in_stripped = stripped.len();
    for (idx, ch) in stripped.char_indices() {
        if ch.is_whitespace() {
            name_end_in_stripped = idx;
            break;
        }
    }
    let name = &stripped[..name_end_in_stripped];
    if name.is_empty() {
        return None;
    }
    let rest_untrimmed = &stripped[name_end_in_stripped..];
    let rest = rest_untrimmed.trim_start();
    let rest_start_in_stripped = name_end_in_stripped + (rest_untrimmed.len() - rest.len());
    // `stripped` is `line` without the leading '/', so add 1 to get the original offset.
    let rest_offset = rest_start_in_stripped + 1;
    Some((name, rest, rest_offset))
}

#[derive(Debug, Clone, PartialEq)]
pub struct PromptArg {
    pub text: String,
    pub text_elements: Vec<TextElement>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PromptExpansion {
    pub text: String,
    pub text_elements: Vec<TextElement>,
}

/// Parse positional arguments using shlex semantics (supports quoted tokens).
///
/// `text_elements` must be relative to `rest`.
pub fn parse_positional_args(rest: &str, text_elements: &[TextElement]) -> Vec<PromptArg> {
    parse_tokens_with_elements(rest, text_elements)
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

/// Shift a text element's byte range left by `offset`, returning `None` if empty.
///
/// `offset` is the byte length of the prefix removed from the original text.
fn shift_text_element_left(elem: &TextElement, offset: usize) -> Option<TextElement> {
    if elem.byte_range.end <= offset {
        return None;
    }
    let start = elem.byte_range.start.saturating_sub(offset);
    let end = elem.byte_range.end.saturating_sub(offset);
    (start < end).then_some(elem.map_range(|_| ByteRange { start, end }))
}

/// Parses the `key=value` pairs that follow a custom prompt name.
///
/// The input is split using shlex rules, so quoted values are supported
/// (for example `USER="Alice Smith"`). The function returns a map of parsed
/// arguments, or an error if a token is missing `=` or if the key is empty.
pub fn parse_prompt_inputs(
    rest: &str,
    text_elements: &[TextElement],
) -> Result<HashMap<String, PromptArg>, PromptArgsError> {
    let mut map = HashMap::new();
    if rest.trim().is_empty() {
        return Ok(map);
    }

    // Tokenize the rest of the command using shlex rules, but keep text element
    // ranges relative to each emitted token.
    for token in parse_tokens_with_elements(rest, text_elements) {
        let Some((key, value)) = token.text.split_once('=') else {
            return Err(PromptArgsError::MissingAssignment { token: token.text });
        };
        if key.is_empty() {
            return Err(PromptArgsError::MissingKey { token: token.text });
        }
        // The token is `key=value`; translate element ranges into the value-only
        // coordinate space by subtracting the `key=` prefix length.
        let value_start = key.len() + 1;
        let value_elements = token
            .text_elements
            .iter()
            .filter_map(|elem| shift_text_element_left(elem, value_start))
            .collect();
        map.insert(
            key.to_string(),
            PromptArg {
                text: value.to_string(),
                text_elements: value_elements,
            },
        );
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
    text_elements: &[TextElement],
    custom_prompts: &[CustomPrompt],
) -> Result<Option<PromptExpansion>, PromptExpansionError> {
    let Some((name, rest, rest_offset)) = parse_slash_name(text) else {
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
    let local_elements: Vec<TextElement> = text_elements
        .iter()
        .filter_map(|elem| {
            let mut shifted = shift_text_element_left(elem, rest_offset)?;
            if shifted.byte_range.start >= rest.len() {
                return None;
            }
            let end = shifted.byte_range.end.min(rest.len());
            shifted.byte_range.end = end;
            (shifted.byte_range.start < shifted.byte_range.end).then_some(shifted)
        })
        .collect();
    if !required.is_empty() {
        let inputs = parse_prompt_inputs(rest, &local_elements).map_err(|error| {
            PromptExpansionError::Args {
                command: format!("/{name}"),
                error,
            }
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
        let (text, elements) = expand_named_placeholders_with_elements(&prompt.content, &inputs);
        return Ok(Some(PromptExpansion {
            text,
            text_elements: elements,
        }));
    }

    // Otherwise, treat it as numeric/positional placeholder prompt (or none).
    let pos_args = parse_positional_args(rest, &local_elements);
    Ok(Some(expand_numeric_placeholders(
        &prompt.content,
        &pos_args,
    )))
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
pub fn extract_positional_args_for_prompt_line(
    line: &str,
    prompt_name: &str,
    text_elements: &[TextElement],
) -> Vec<PromptArg> {
    let trimmed = line.trim_start();
    let trim_offset = line.len() - trimmed.len();
    let Some((name, rest, rest_offset)) = parse_slash_name(trimmed) else {
        return Vec::new();
    };
    // Require the explicit prompts prefix for custom prompt invocations.
    let Some(after_prefix) = name.strip_prefix(&format!("{PROMPTS_CMD_PREFIX}:")) else {
        return Vec::new();
    };
    if after_prefix != prompt_name {
        return Vec::new();
    }
    let rest_trimmed_start = rest.trim_start();
    let args_str = rest_trimmed_start.trim_end();
    if args_str.is_empty() {
        return Vec::new();
    }
    let args_offset = trim_offset + rest_offset + (rest.len() - rest_trimmed_start.len());
    let local_elements: Vec<TextElement> = text_elements
        .iter()
        .filter_map(|elem| {
            let mut shifted = shift_text_element_left(elem, args_offset)?;
            if shifted.byte_range.start >= args_str.len() {
                return None;
            }
            let end = shifted.byte_range.end.min(args_str.len());
            shifted.byte_range.end = end;
            (shifted.byte_range.start < shifted.byte_range.end).then_some(shifted)
        })
        .collect();
    parse_positional_args(args_str, &local_elements)
}

/// If the prompt only uses numeric placeholders and the first line contains
/// positional args for it, expand and return Some(expanded); otherwise None.
pub fn expand_if_numeric_with_positional_args(
    prompt: &CustomPrompt,
    first_line: &str,
    text_elements: &[TextElement],
) -> Option<PromptExpansion> {
    if !prompt_argument_names(&prompt.content).is_empty() {
        return None;
    }
    if !prompt_has_numeric_placeholders(&prompt.content) {
        return None;
    }
    let args = extract_positional_args_for_prompt_line(first_line, &prompt.name, text_elements);
    if args.is_empty() {
        return None;
    }
    Some(expand_numeric_placeholders(&prompt.content, &args))
}

/// Expand `$1..$9` and `$ARGUMENTS` in `content` with values from `args`.
pub fn expand_numeric_placeholders(content: &str, args: &[PromptArg]) -> PromptExpansion {
    let mut out = String::with_capacity(content.len());
    let mut out_elements = Vec::new();
    let mut i = 0;
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
                    if let Some(arg) = args.get(idx) {
                        append_arg_with_elements(&mut out, &mut out_elements, arg);
                    }
                    i = j + 2;
                    continue;
                }
                _ => {}
            }
        }
        if rest.len() > "ARGUMENTS".len() && rest[1..].starts_with("ARGUMENTS") {
            if !args.is_empty() {
                append_joined_args_with_elements(&mut out, &mut out_elements, args);
            }
            i = j + 1 + "ARGUMENTS".len();
            continue;
        }
        out.push('$');
        i = j + 1;
    }
    out.push_str(&content[i..]);
    PromptExpansion {
        text: out,
        text_elements: out_elements,
    }
}

fn parse_tokens_with_elements(rest: &str, text_elements: &[TextElement]) -> Vec<PromptArg> {
    let mut elements = text_elements.to_vec();
    elements.sort_by_key(|elem| elem.byte_range.start);
    // Keep element placeholders intact across shlex splitting by replacing
    // each element range with a unique sentinel token first.
    let (rest_for_shlex, replacements) = replace_text_elements_with_sentinels(rest, &elements);
    Shlex::new(&rest_for_shlex)
        .map(|token| apply_replacements_to_token(token, &replacements))
        .collect()
}

#[derive(Debug, Clone)]
struct ElementReplacement {
    sentinel: String,
    text: String,
    placeholder: Option<String>,
}

/// Replace each text element range with a unique sentinel token.
///
/// The sentinel is chosen so it will survive shlex tokenization as a single word.
fn replace_text_elements_with_sentinels(
    rest: &str,
    elements: &[TextElement],
) -> (String, Vec<ElementReplacement>) {
    let mut out = String::with_capacity(rest.len());
    let mut replacements = Vec::new();
    let mut cursor = 0;

    for (idx, elem) in elements.iter().enumerate() {
        let start = elem.byte_range.start;
        let end = elem.byte_range.end;
        out.push_str(&rest[cursor..start]);
        let mut sentinel = format!("__CODEX_ELEM_{idx}__");
        // Ensure we never collide with user content so a sentinel can't be mistaken for text.
        while rest.contains(&sentinel) {
            sentinel.push('_');
        }
        out.push_str(&sentinel);
        replacements.push(ElementReplacement {
            sentinel,
            text: rest[start..end].to_string(),
            placeholder: elem.placeholder(rest).map(str::to_string),
        });
        cursor = end;
    }

    out.push_str(&rest[cursor..]);
    (out, replacements)
}

/// Rehydrate a shlex token by swapping sentinels back to the original text
/// and rebuilding text element ranges relative to the resulting token.
fn apply_replacements_to_token(token: String, replacements: &[ElementReplacement]) -> PromptArg {
    if replacements.is_empty() {
        return PromptArg {
            text: token,
            text_elements: Vec::new(),
        };
    }

    let mut out = String::with_capacity(token.len());
    let mut out_elements = Vec::new();
    let mut cursor = 0;

    while cursor < token.len() {
        let Some((offset, replacement)) = next_replacement(&token, cursor, replacements) else {
            out.push_str(&token[cursor..]);
            break;
        };
        let start_in_token = cursor + offset;
        out.push_str(&token[cursor..start_in_token]);
        let start = out.len();
        out.push_str(&replacement.text);
        let end = out.len();
        if start < end {
            out_elements.push(TextElement::new(
                ByteRange { start, end },
                replacement.placeholder.clone(),
            ));
        }
        cursor = start_in_token + replacement.sentinel.len();
    }

    PromptArg {
        text: out,
        text_elements: out_elements,
    }
}

/// Find the earliest sentinel occurrence at or after `cursor`.
fn next_replacement<'a>(
    token: &str,
    cursor: usize,
    replacements: &'a [ElementReplacement],
) -> Option<(usize, &'a ElementReplacement)> {
    let slice = &token[cursor..];
    let mut best: Option<(usize, &'a ElementReplacement)> = None;
    for replacement in replacements {
        if let Some(pos) = slice.find(&replacement.sentinel) {
            match best {
                Some((best_pos, _)) if best_pos <= pos => {}
                _ => best = Some((pos, replacement)),
            }
        }
    }
    best
}

fn expand_named_placeholders_with_elements(
    content: &str,
    args: &HashMap<String, PromptArg>,
) -> (String, Vec<TextElement>) {
    let mut out = String::with_capacity(content.len());
    let mut out_elements = Vec::new();
    let mut cursor = 0;
    for m in PROMPT_ARG_REGEX.find_iter(content) {
        let start = m.start();
        let end = m.end();
        if start > 0 && content.as_bytes()[start - 1] == b'$' {
            out.push_str(&content[cursor..end]);
            cursor = end;
            continue;
        }
        out.push_str(&content[cursor..start]);
        cursor = end;
        let key = &content[start + 1..end];
        if let Some(arg) = args.get(key) {
            append_arg_with_elements(&mut out, &mut out_elements, arg);
        } else {
            out.push_str(&content[start..end]);
        }
    }
    out.push_str(&content[cursor..]);
    (out, out_elements)
}

fn append_arg_with_elements(
    out: &mut String,
    out_elements: &mut Vec<TextElement>,
    arg: &PromptArg,
) {
    let start = out.len();
    out.push_str(&arg.text);
    if arg.text_elements.is_empty() {
        return;
    }
    out_elements.extend(arg.text_elements.iter().map(|elem| {
        elem.map_range(|range| ByteRange {
            start: start + range.start,
            end: start + range.end,
        })
    }));
}

fn append_joined_args_with_elements(
    out: &mut String,
    out_elements: &mut Vec<TextElement>,
    args: &[PromptArg],
) {
    // `$ARGUMENTS` joins args with single spaces while preserving element ranges.
    for (idx, arg) in args.iter().enumerate() {
        if idx > 0 {
            out.push(' ');
        }
        append_arg_with_elements(out, out_elements, arg);
    }
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
    use pretty_assertions::assert_eq;

    #[test]
    fn expand_arguments_basic() {
        let prompts = vec![CustomPrompt {
            name: "my-prompt".to_string(),
            path: "/tmp/my-prompt.md".to_string().into(),
            content: "Review $USER changes on $BRANCH".to_string(),
            description: None,
            argument_hint: None,
        }];

        let out = expand_custom_prompt("/prompts:my-prompt USER=Alice BRANCH=main", &[], &prompts)
            .unwrap();
        assert_eq!(
            out,
            Some(PromptExpansion {
                text: "Review Alice changes on main".to_string(),
                text_elements: Vec::new(),
            })
        );
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
            &[],
            &prompts,
        )
        .unwrap();
        assert_eq!(
            out,
            Some(PromptExpansion {
                text: "Pair Alice Smith with dev-main".to_string(),
                text_elements: Vec::new(),
            })
        );
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
        let err = expand_custom_prompt("/prompts:my-prompt USER=Alice stray", &[], &prompts)
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
        let err = expand_custom_prompt("/prompts:my-prompt USER=Alice", &[], &prompts)
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

        let out = expand_custom_prompt("/prompts:my-prompt", &[], &prompts).unwrap();
        assert_eq!(
            out,
            Some(PromptExpansion {
                text: "literal $$USER".to_string(),
                text_elements: Vec::new(),
            })
        );
    }

    #[test]
    fn positional_args_treat_placeholder_with_spaces_as_single_token() {
        let placeholder = "[Image #1]";
        let rest = format!("alpha {placeholder} beta");
        let start = rest.find(placeholder).expect("placeholder");
        let end = start + placeholder.len();
        let text_elements = vec![TextElement::new(
            ByteRange { start, end },
            Some(placeholder.to_string()),
        )];

        let args = parse_positional_args(&rest, &text_elements);
        assert_eq!(
            args,
            vec![
                PromptArg {
                    text: "alpha".to_string(),
                    text_elements: Vec::new(),
                },
                PromptArg {
                    text: placeholder.to_string(),
                    text_elements: vec![TextElement::new(
                        ByteRange {
                            start: 0,
                            end: placeholder.len(),
                        },
                        Some(placeholder.to_string()),
                    )],
                },
                PromptArg {
                    text: "beta".to_string(),
                    text_elements: Vec::new(),
                }
            ]
        );
    }

    #[test]
    fn extract_positional_args_shifts_element_offsets_into_args_str() {
        let placeholder = "[Image #1]";
        let line = format!("  /{PROMPTS_CMD_PREFIX}:my-prompt  alpha {placeholder} beta   ");
        let start = line.find(placeholder).expect("placeholder");
        let end = start + placeholder.len();
        let text_elements = vec![TextElement::new(
            ByteRange { start, end },
            Some(placeholder.to_string()),
        )];

        let args = extract_positional_args_for_prompt_line(&line, "my-prompt", &text_elements);
        assert_eq!(
            args,
            vec![
                PromptArg {
                    text: "alpha".to_string(),
                    text_elements: Vec::new(),
                },
                PromptArg {
                    text: placeholder.to_string(),
                    text_elements: vec![TextElement::new(
                        ByteRange {
                            start: 0,
                            end: placeholder.len(),
                        },
                        Some(placeholder.to_string()),
                    )],
                },
                PromptArg {
                    text: "beta".to_string(),
                    text_elements: Vec::new(),
                }
            ]
        );
    }

    #[test]
    fn key_value_args_treat_placeholder_with_spaces_as_single_token() {
        let placeholder = "[Image #1]";
        let rest = format!("IMG={placeholder} NOTE=hello");
        let start = rest.find(placeholder).expect("placeholder");
        let end = start + placeholder.len();
        let text_elements = vec![TextElement::new(
            ByteRange { start, end },
            Some(placeholder.to_string()),
        )];

        let args = parse_prompt_inputs(&rest, &text_elements).expect("inputs");
        assert_eq!(
            args.get("IMG"),
            Some(&PromptArg {
                text: placeholder.to_string(),
                text_elements: vec![TextElement::new(
                    ByteRange {
                        start: 0,
                        end: placeholder.len(),
                    },
                    Some(placeholder.to_string()),
                )],
            })
        );
        assert_eq!(
            args.get("NOTE"),
            Some(&PromptArg {
                text: "hello".to_string(),
                text_elements: Vec::new(),
            })
        );
    }

    #[test]
    fn positional_args_allow_placeholder_inside_quotes() {
        let placeholder = "[Image #1]";
        let rest = format!("alpha \"see {placeholder} here\" beta");
        let start = rest.find(placeholder).expect("placeholder");
        let end = start + placeholder.len();
        let text_elements = vec![TextElement::new(
            ByteRange { start, end },
            Some(placeholder.to_string()),
        )];

        let args = parse_positional_args(&rest, &text_elements);
        assert_eq!(
            args,
            vec![
                PromptArg {
                    text: "alpha".to_string(),
                    text_elements: Vec::new(),
                },
                PromptArg {
                    text: format!("see {placeholder} here"),
                    text_elements: vec![TextElement::new(
                        ByteRange {
                            start: "see ".len(),
                            end: "see ".len() + placeholder.len(),
                        },
                        Some(placeholder.to_string()),
                    )],
                },
                PromptArg {
                    text: "beta".to_string(),
                    text_elements: Vec::new(),
                }
            ]
        );
    }

    #[test]
    fn key_value_args_allow_placeholder_inside_quotes() {
        let placeholder = "[Image #1]";
        let rest = format!("IMG=\"see {placeholder} here\" NOTE=ok");
        let start = rest.find(placeholder).expect("placeholder");
        let end = start + placeholder.len();
        let text_elements = vec![TextElement::new(
            ByteRange { start, end },
            Some(placeholder.to_string()),
        )];

        let args = parse_prompt_inputs(&rest, &text_elements).expect("inputs");
        assert_eq!(
            args.get("IMG"),
            Some(&PromptArg {
                text: format!("see {placeholder} here"),
                text_elements: vec![TextElement::new(
                    ByteRange {
                        start: "see ".len(),
                        end: "see ".len() + placeholder.len(),
                    },
                    Some(placeholder.to_string()),
                )],
            })
        );
        assert_eq!(
            args.get("NOTE"),
            Some(&PromptArg {
                text: "ok".to_string(),
                text_elements: Vec::new(),
            })
        );
    }
}
