#[cfg(test)]
use codex_protocol::user_input::ByteRange;
#[cfg(test)]
use codex_protocol::user_input::TextElement;
#[cfg(test)]
use shlex::Shlex;

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
    let rest_offset = rest_start_in_stripped + 1;
    Some((name, rest, rest_offset))
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq)]
pub struct PromptArg {
    pub text: String,
    pub text_elements: Vec<TextElement>,
}

/// Parse positional arguments using shlex semantics (supports quoted tokens).
///
/// `text_elements` must be relative to `rest`.
#[cfg(test)]
pub fn parse_positional_args(rest: &str, text_elements: &[TextElement]) -> Vec<PromptArg> {
    parse_tokens_with_elements(rest, text_elements)
}

#[cfg(test)]
fn parse_tokens_with_elements(rest: &str, text_elements: &[TextElement]) -> Vec<PromptArg> {
    let mut elements = text_elements.to_vec();
    elements.sort_by_key(|elem| elem.byte_range.start);
    let (rest_for_shlex, replacements) = replace_text_elements_with_sentinels(rest, &elements);
    Shlex::new(&rest_for_shlex)
        .map(|token| apply_replacements_to_token(token, &replacements))
        .collect()
}

#[cfg(test)]
#[derive(Debug, Clone)]
struct ElementReplacement {
    sentinel: String,
    text: String,
    placeholder: Option<String>,
}

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

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
}
