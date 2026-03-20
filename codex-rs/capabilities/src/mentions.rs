use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolMentionKind {
    App,
    Mcp,
    Plugin,
    Skill,
    Other,
}

const APP_PATH_PREFIX: &str = "app://";
const MCP_PATH_PREFIX: &str = "mcp://";
const PLUGIN_PATH_PREFIX: &str = "plugin://";
const SKILL_PATH_PREFIX: &str = "skill://";
const SKILL_FILENAME: &str = "SKILL.md";

pub fn tool_kind_for_path(path: &str) -> ToolMentionKind {
    if path.starts_with(APP_PATH_PREFIX) {
        ToolMentionKind::App
    } else if path.starts_with(MCP_PATH_PREFIX) {
        ToolMentionKind::Mcp
    } else if path.starts_with(PLUGIN_PATH_PREFIX) {
        ToolMentionKind::Plugin
    } else if path.starts_with(SKILL_PATH_PREFIX) || is_skill_filename(path) {
        ToolMentionKind::Skill
    } else {
        ToolMentionKind::Other
    }
}

pub fn app_id_from_path(path: &str) -> Option<&str> {
    path.strip_prefix(APP_PATH_PREFIX)
        .filter(|value| !value.is_empty())
}

pub fn plugin_config_name_from_path(path: &str) -> Option<&str> {
    path.strip_prefix(PLUGIN_PATH_PREFIX)
        .filter(|value| !value.is_empty())
}

pub fn normalize_skill_path(path: &str) -> &str {
    path.strip_prefix(SKILL_PATH_PREFIX).unwrap_or(path)
}

pub fn extract_tool_mentions_with_sigil(text: &str, sigil: char) -> ToolMentions<'_> {
    let text_bytes = text.as_bytes();
    let mut mentioned_names: HashSet<&str> = HashSet::new();
    let mut mentioned_paths: HashSet<&str> = HashSet::new();
    let mut plain_names: HashSet<&str> = HashSet::new();

    let mut index = 0;
    while index < text_bytes.len() {
        let byte = text_bytes[index];
        if byte == b'['
            && let Some((name, path, end_index)) =
                parse_linked_tool_mention(text, text_bytes, index, sigil)
        {
            if !is_common_env_var(name) {
                if !matches!(
                    tool_kind_for_path(path),
                    ToolMentionKind::App | ToolMentionKind::Mcp | ToolMentionKind::Plugin
                ) {
                    mentioned_names.insert(name);
                }
                mentioned_paths.insert(path);
            }
            index = end_index;
            continue;
        }

        if byte != sigil as u8 {
            index += 1;
            continue;
        }

        let name_start = index + 1;
        let Some(first_name_byte) = text_bytes.get(name_start) else {
            index += 1;
            continue;
        };
        if !is_mention_name_char(*first_name_byte) {
            index += 1;
            continue;
        }

        let mut name_end = name_start + 1;
        while let Some(byte) = text_bytes.get(name_end) {
            if !is_mention_name_char(*byte) {
                break;
            }
            name_end += 1;
        }
        let name = &text[name_start..name_end];
        if !is_common_env_var(name) {
            mentioned_names.insert(name);
            plain_names.insert(name);
        }
        index = name_end;
    }

    ToolMentions {
        mentioned_names,
        mentioned_paths,
        plain_names,
    }
}

pub struct ToolMentions<'a> {
    mentioned_names: HashSet<&'a str>,
    mentioned_paths: HashSet<&'a str>,
    plain_names: HashSet<&'a str>,
}

impl<'a> ToolMentions<'a> {
    pub fn names(&self) -> impl Iterator<Item = &'a str> {
        self.mentioned_names.iter().copied()
    }

    pub fn paths(&self) -> impl Iterator<Item = &'a str> {
        self.mentioned_paths.iter().copied()
    }

    pub fn plain_names(&self) -> impl Iterator<Item = &'a str> {
        self.plain_names.iter().copied()
    }
}

fn is_skill_filename(path: &str) -> bool {
    let file_name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    file_name.eq_ignore_ascii_case(SKILL_FILENAME)
}

fn parse_linked_tool_mention<'a>(
    text: &'a str,
    text_bytes: &[u8],
    index: usize,
    sigil: char,
) -> Option<(&'a str, &'a str, usize)> {
    let name_start = index.checked_add(2)?;
    if text_bytes.get(index + 1).copied()? != sigil as u8 {
        return None;
    }
    let mut name_end = name_start;
    while let Some(byte) = text_bytes.get(name_end) {
        if *byte == b']' {
            break;
        }
        if !is_mention_name_char(*byte) {
            return None;
        }
        name_end += 1;
    }
    if text_bytes.get(name_end).copied()? != b']' {
        return None;
    }
    if text_bytes.get(name_end + 1).copied()? != b'(' {
        return None;
    }
    let path_start = name_end + 2;
    let path_end = text[path_start..].find(')')? + path_start;
    let name = &text[name_start..name_end];
    let path = &text[path_start..path_end];
    Some((name, path, path_end + 1))
}

fn is_common_env_var(name: &str) -> bool {
    matches!(
        name,
        "PATH" | "HOME" | "PWD" | "SHELL" | "USER" | "TMPDIR" | "TMP" | "TEMP"
    )
}

fn is_mention_name_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b'/')
}
