use std::collections::HashMap;
use std::collections::VecDeque;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LinkedMention {
    pub(crate) mention: String,
    pub(crate) path: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DecodedHistoryText {
    pub(crate) text: String,
    pub(crate) mentions: Vec<LinkedMention>,
}

pub(crate) fn encode_history_mentions(text: &str, mentions: &[LinkedMention]) -> String {
    if mentions.is_empty() || text.is_empty() {
        return text.to_string();
    }

    let mut mentions_by_name: HashMap<&str, VecDeque<&str>> = HashMap::new();
    for mention in mentions {
        mentions_by_name
            .entry(mention.mention.as_str())
            .or_default()
            .push_back(mention.path.as_str());
    }

    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut index = 0usize;

    while index < bytes.len() {
        if bytes[index] == b'$' {
            let name_start = index + 1;
            if let Some(first) = bytes.get(name_start)
                && is_mention_name_char(*first)
            {
                let mut name_end = name_start + 1;
                while let Some(next) = bytes.get(name_end)
                    && is_mention_name_char(*next)
                {
                    name_end += 1;
                }

                let name = &text[name_start..name_end];
                if let Some(path) = mentions_by_name.get_mut(name).and_then(VecDeque::pop_front) {
                    out.push('[');
                    out.push('$');
                    out.push_str(name);
                    out.push_str("](");
                    out.push_str(path);
                    out.push(')');
                    index = name_end;
                    continue;
                }
            }
        }

        let Some(ch) = text[index..].chars().next() else {
            break;
        };
        out.push(ch);
        index += ch.len_utf8();
    }

    out
}

pub(crate) fn decode_history_mentions(text: &str) -> DecodedHistoryText {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut mentions = Vec::new();
    let mut index = 0usize;

    while index < bytes.len() {
        if bytes[index] == b'['
            && let Some((name, path, end_index)) = parse_linked_tool_mention(text, bytes, index)
            && !is_common_env_var(name)
            && is_tool_path(path)
        {
            out.push('$');
            out.push_str(name);
            mentions.push(LinkedMention {
                mention: name.to_string(),
                path: path.to_string(),
            });
            index = end_index;
            continue;
        }

        let Some(ch) = text[index..].chars().next() else {
            break;
        };
        out.push(ch);
        index += ch.len_utf8();
    }

    DecodedHistoryText {
        text: out,
        mentions,
    }
}

fn parse_linked_tool_mention<'a>(
    text: &'a str,
    text_bytes: &[u8],
    start: usize,
) -> Option<(&'a str, &'a str, usize)> {
    let dollar_index = start + 1;
    if text_bytes.get(dollar_index) != Some(&b'$') {
        return None;
    }

    let name_start = dollar_index + 1;
    let first_name_byte = text_bytes.get(name_start)?;
    if !is_mention_name_char(*first_name_byte) {
        return None;
    }

    let mut name_end = name_start + 1;
    while let Some(next_byte) = text_bytes.get(name_end)
        && is_mention_name_char(*next_byte)
    {
        name_end += 1;
    }

    if text_bytes.get(name_end) != Some(&b']') {
        return None;
    }

    let mut path_start = name_end + 1;
    while let Some(next_byte) = text_bytes.get(path_start)
        && next_byte.is_ascii_whitespace()
    {
        path_start += 1;
    }
    if text_bytes.get(path_start) != Some(&b'(') {
        return None;
    }

    let mut path_end = path_start + 1;
    while let Some(next_byte) = text_bytes.get(path_end)
        && *next_byte != b')'
    {
        path_end += 1;
    }
    if text_bytes.get(path_end) != Some(&b')') {
        return None;
    }

    let path = text[path_start + 1..path_end].trim();
    if path.is_empty() {
        return None;
    }

    let name = &text[name_start..name_end];
    Some((name, path, path_end + 1))
}

fn is_mention_name_char(byte: u8) -> bool {
    matches!(byte, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'-')
}

fn is_common_env_var(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    matches!(
        upper.as_str(),
        "PATH"
            | "HOME"
            | "USER"
            | "SHELL"
            | "PWD"
            | "TMPDIR"
            | "TEMP"
            | "TMP"
            | "LANG"
            | "TERM"
            | "XDG_CONFIG_HOME"
    )
}

fn is_tool_path(path: &str) -> bool {
    path.starts_with("app://")
        || path.starts_with("mcp://")
        || path.starts_with("skill://")
        || path
            .rsplit(['/', '\\'])
            .next()
            .is_some_and(|name| name.eq_ignore_ascii_case("SKILL.md"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn decode_history_mentions_restores_visible_tokens() {
        let decoded = decode_history_mentions(
            "Use [$figma](app://figma-1) and [$figma](/tmp/figma/SKILL.md).",
        );
        assert_eq!(decoded.text, "Use $figma and $figma.");
        assert_eq!(
            decoded.mentions,
            vec![
                LinkedMention {
                    mention: "figma".to_string(),
                    path: "app://figma-1".to_string(),
                },
                LinkedMention {
                    mention: "figma".to_string(),
                    path: "/tmp/figma/SKILL.md".to_string(),
                },
            ]
        );
    }

    #[test]
    fn encode_history_mentions_links_bound_mentions_in_order() {
        let text = "$figma then $figma then $other";
        let encoded = encode_history_mentions(
            text,
            &[
                LinkedMention {
                    mention: "figma".to_string(),
                    path: "app://figma-app".to_string(),
                },
                LinkedMention {
                    mention: "figma".to_string(),
                    path: "/tmp/figma/SKILL.md".to_string(),
                },
            ],
        );
        assert_eq!(
            encoded,
            "[$figma](app://figma-app) then [$figma](/tmp/figma/SKILL.md) then $other"
        );
    }
}
