//! Parse structured annotations embedded in assistant-authored Markdown.
//!
//! For example, `::git-create-pr{cwd="/repo" isDraft=true}` becomes the name
//! `git-create-pr` and the attributes `cwd` = `/repo` and `isDraft` = `true`.
//! A review annotation such as `::code-comment{body="Keep \"x}\" literal."}`
//! can contain quotes and braces within its quoted value. Consumers choose
//! the quote-escaping rules and interpret the attributes; this module also
//! retains the exact directive source, excluding any trailing Markdown.
//! Scanners can share a byte-work budget across unsuccessful parse attempts.

use std::borrow::Cow;
use std::collections::BTreeMap;

/// An assistant annotation and its complete, unmodified source representation.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AssistantDirective<'source> {
    pub(crate) name: &'source str,
    pub(crate) label: Option<&'source str>,
    pub(crate) attributes: BTreeMap<&'source str, Cow<'source, str>>,
    pub(crate) raw: &'source str,
}

/// Git receipts preserve backslashes; review comments allow escaped quote delimiters.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum QuoteEscaping {
    Literal,
    Backslash,
}

/// Parse one inline, leaf, or container marker from the beginning of `source`.
pub(crate) fn parse_assistant_directive(
    source: &str,
    escaping: QuoteEscaping,
) -> Option<AssistantDirective<'_>> {
    let mut remaining = usize::MAX;
    parse_assistant_directive_with_budget(source, escaping, &mut remaining)
}

/// Parse with a shared byte-scanning budget for callers that retry at multiple offsets.
///
/// Charge inspected source, not the whole supplied suffix. A scan may finish its current token
/// before exhausting the budget; subsequent attempts return immediately. This bounds repeated
/// malformed candidates without imposing a fixed size or count limit on valid directives.
pub(crate) fn parse_assistant_directive_with_budget<'source>(
    source: &'source str,
    escaping: QuoteEscaping,
    remaining: &mut usize,
) -> Option<AssistantDirective<'source>> {
    spend_scan_budget(remaining, /*scanned*/ 1)?;
    // `::git-create-pr{...}` starts with a one-to-three-colon marker and a name;
    // Follow-ups additionally accept a `[label]` before their attributes.
    let rest = source.trim_start_matches(':');
    spend_scan_budget(remaining, source.len() - rest.len())?;
    if !(1..=3).contains(&(source.len() - rest.len())) {
        return None;
    }
    let name_len = rest.bytes().take_while(|byte| is_name_byte(*byte)).count();
    spend_scan_budget(remaining, name_len + 1)?;
    let (name, suffix) = rest.split_at(name_len);
    let (label, suffix) = if let Some(label) = suffix.strip_prefix('[') {
        if name != "codex-followup" {
            return None;
        }
        let mut characters = label.char_indices().peekable();
        let mut depth = 1;
        let mut code_ticks = 0;
        let mut code_start = None;
        let end = loop {
            let (index, character) = match characters.next() {
                Some((_, '\n' | '\r')) | None => {
                    // An unmatched backtick run is literal Markdown. Replay its contents
                    // with the same scan budget so brackets and escapes regain their meaning.
                    characters = code_start.take()?;
                    code_ticks = 0;
                    continue;
                }
                Some(character) => character,
            };
            spend_scan_budget(remaining, character.len_utf8())?;
            match character {
                '`' => {
                    let mut ticks = 1;
                    while characters.next_if(|(_, next)| *next == '`').is_some() {
                        spend_scan_budget(remaining, /*scanned*/ 1)?;
                        ticks += 1;
                    }
                    if code_ticks == 0 {
                        code_ticks = ticks;
                        code_start = Some(characters.clone());
                    } else if code_ticks == ticks {
                        code_ticks = 0;
                        code_start = None;
                    }
                }
                _ if code_ticks > 0 => {}
                '\\' => {
                    let (_, escaped) = characters.next()?;
                    spend_scan_budget(remaining, escaped.len_utf8())?;
                    if matches!(escaped, '\n' | '\r') {
                        return None;
                    }
                }
                '[' => depth += 1,
                ']' => {
                    depth -= 1;
                    if depth == 0 {
                        break index;
                    }
                }
                _ => {}
            }
        };
        (Some(&label[..end]), label[end..].strip_prefix(']')?)
    } else {
        (None, suffix)
    };
    let mut rest = suffix.strip_prefix('{')?;
    if !name.as_bytes().first().is_some_and(u8::is_ascii_alphabetic) {
        return None;
    }

    let mut attributes = BTreeMap::new();
    loop {
        rest = trim_attribute_space(rest, remaining)?;
        if let Some(suffix) = rest.strip_prefix('}') {
            // In `::git-push{cwd="/repo"} done`, retain the directive but not ` done`.
            return Some(AssistantDirective {
                name,
                label,
                attributes,
                raw: &source[..source.len() - suffix.len()],
            });
        }
        // For `cwd = "/repo" isDraft=true}`, split off `cwd` and leave the next
        // attribute for the next iteration after consuming this value.
        let key_len = rest.bytes().take_while(|byte| is_name_byte(*byte)).count();
        spend_scan_budget(remaining, key_len + 1)?;
        let (key, suffix) = rest.split_at(key_len);
        // Reject malformed keys and duplicates before scanning a potentially long value.
        if key.is_empty() || attributes.contains_key(key) {
            return None;
        }
        let value = trim_attribute_space(suffix, remaining)?.strip_prefix('=')?;
        rest = trim_attribute_space(value, remaining)?;
        let value = if let Some(delimiter @ (b'"' | b'\'')) = rest.as_bytes().first().copied() {
            // In `body="Keep \"x}\" literal."`, only the matching unescaped quote
            // ends the value, not the embedded `}`. Single-quoted values work too.
            let quoted = &rest[1..];
            let mut characters = quoted.char_indices().peekable();
            let end = loop {
                let (index, character) = characters.next()?;
                spend_scan_budget(remaining, character.len_utf8())?;
                match character {
                    c if c == char::from(delimiter) => break index,
                    '\n' | '\r' => return None,
                    '\\' if escaping == QuoteEscaping::Backslash
                        && characters
                            .peek()
                            .is_some_and(|(_, next)| *next == char::from(delimiter)) =>
                    {
                        // Backslash mode consumes `\"` as a literal quote. Literal
                        // mode instead lets the quote close `cwd="/repo\"`.
                        characters.next();
                        spend_scan_budget(remaining, /*scanned*/ 1)?;
                    }
                    _ => {}
                }
            };
            let value = &quoted[..end];
            rest = &quoted[end + 1..];
            let escaped = format!("\\{}", char::from(delimiter));
            if escaping == QuoteEscaping::Backslash && value.contains(&escaped) {
                Cow::Owned(value.replace(&escaped, &char::from(delimiter).to_string()))
            } else {
                Cow::Borrowed(value)
            }
        } else {
            // Unquoted `isDraft=true` ends at whitespace or `}`, not at a quote.
            let end = rest
                .find([' ', '\t', '}', '\n', '\r'])
                .unwrap_or(rest.len());
            spend_scan_budget(remaining, end + 1)?;
            if end == 0 {
                return None;
            }
            let value = &rest[..end];
            rest = &rest[end..];
            Cow::Borrowed(value)
        };
        attributes.insert(key, value);
    }
}

fn trim_attribute_space<'source>(
    source: &'source str,
    remaining: &mut usize,
) -> Option<&'source str> {
    let rest = source.trim_start_matches([' ', '\t']);
    spend_scan_budget(remaining, source.len() - rest.len() + 1)?;
    Some(rest)
}

fn spend_scan_budget(remaining: &mut usize, scanned: usize) -> Option<()> {
    let available = *remaining;
    *remaining = remaining.saturating_sub(scanned);
    (scanned <= available).then_some(())
}

fn is_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')
}

#[cfg(test)]
#[path = "assistant_directives_tests.rs"]
mod tests;
