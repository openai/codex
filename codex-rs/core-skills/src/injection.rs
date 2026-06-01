use crate::SkillMetadata;
use crate::build_skill_name_counts;
use codex_analytics::AnalyticsEventsClient;
use codex_analytics::InvocationType;
use codex_analytics::SkillInvocation;
use codex_analytics::TrackEventsContext;
use codex_exec_server::EnvironmentPathRef;
use codex_otel::SessionTelemetry;
use codex_protocol::user_input::UserInput;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_plugins::mention_syntax::TOOL_MENTION_SIGIL;
use std::collections::HashMap;
use std::collections::HashSet;

#[derive(Debug, Default)]
pub struct SkillInjections {
    pub items: Vec<SkillInjection>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillInjection {
    pub name: String,
    pub path: String,
    pub contents: String,
}

pub async fn build_skill_injections(
    mentioned_skills: &[SkillMetadata],
    otel: Option<&SessionTelemetry>,
    analytics_client: &AnalyticsEventsClient,
    tracking: TrackEventsContext,
) -> SkillInjections {
    if mentioned_skills.is_empty() {
        return SkillInjections::default();
    }

    let mut result = SkillInjections {
        items: Vec::with_capacity(mentioned_skills.len()),
        warnings: Vec::new(),
    };
    let mut invocations = Vec::new();

    for skill in mentioned_skills {
        match skill.source_path.read_to_string(/*sandbox*/ None).await {
            Ok(contents) => {
                emit_skill_injected_metric(otel, skill, "ok");
                invocations.push(SkillInvocation {
                    skill_name: skill.name.clone(),
                    skill_scope: skill.scope,
                    skill_path: skill.path_to_skills_md.to_path_buf(),
                    plugin_id: skill.plugin_id.clone(),
                    invocation_type: InvocationType::Explicit,
                });
                result.items.push(SkillInjection {
                    name: skill.name.clone(),
                    path: skill.path_to_skills_md.to_string_lossy().into_owned(),
                    contents,
                });
            }
            Err(err) => {
                emit_skill_injected_metric(otel, skill, "error");
                let message = format!(
                    "Failed to load skill {name} at {path}: {err:#}",
                    name = skill.name,
                    path = skill.path_to_skills_md.display()
                );
                result.warnings.push(message);
            }
        }
    }

    analytics_client.track_skill_invocations(tracking, invocations);

    result
}

fn emit_skill_injected_metric(
    otel: Option<&SessionTelemetry>,
    skill: &SkillMetadata,
    status: &str,
) {
    let Some(otel) = otel else {
        return;
    };

    otel.counter(
        "codex.skill.injected",
        /*inc*/ 1,
        &[("status", status), ("skill", skill.name.as_str())],
    );
}

/// Collect explicitly mentioned skills from structured and text mentions.
///
/// Structured `UserInput::Skill` selections are resolved first by path against
/// enabled skills. Text inputs are then scanned to extract `$skill-name` tokens, and we
/// iterate `skills` in their existing order to preserve prior ordering semantics.
/// Explicit links are resolved by path, using the linked name only when duplicate
/// filesystem-bound copies share the same raw path. Plain names are only used when the match
/// is unambiguous.
///
/// Complexity: `O(T + (N_s + N_t) * S)` time, `O(S + M)` space, where:
/// `S` = number of skills, `T` = total text length, `N_s` = number of structured skill inputs,
/// `N_t` = number of text inputs, `M` = max mentions parsed from a single text input.
pub fn collect_explicit_skill_mentions(
    inputs: &[UserInput],
    skills: &[SkillMetadata],
    disabled_paths: &HashSet<EnvironmentPathRef>,
    connector_slug_counts: &HashMap<String, usize>,
) -> Vec<SkillMetadata> {
    let skill_name_counts = build_skill_name_counts(skills, disabled_paths).0;

    let selection_context = SkillSelectionContext {
        skills,
        disabled_paths,
        skill_name_counts: &skill_name_counts,
        connector_slug_counts,
    };
    let mut selected: Vec<SkillMetadata> = Vec::new();
    let mut seen_names: HashSet<String> = HashSet::new();
    let mut seen_paths: HashSet<EnvironmentPathRef> = HashSet::new();
    let mut blocked_plain_names: HashSet<String> = HashSet::new();

    for input in inputs {
        if let UserInput::Skill { name, path, .. } = input {
            blocked_plain_names.insert(name.clone());
            let Ok(path) = AbsolutePathBuf::relative_to_current_dir(path) else {
                continue;
            };

            let matching_skills = selection_context
                .skills
                .iter()
                .filter(|skill| {
                    skill.name == *name
                        && skill.path_to_skills_md == path
                        && !selection_context
                            .disabled_paths
                            .contains(&skill.source_path)
                })
                .collect::<Vec<_>>();
            if let [skill] = matching_skills.as_slice()
                && !seen_paths.contains(&skill.source_path)
            {
                seen_paths.insert(skill.source_path.clone());
                seen_names.insert(skill.name.clone());
                selected.push((*skill).clone());
            }
        }
    }

    for input in inputs {
        if let UserInput::Text { text, .. } = input {
            let mentioned_names = extract_tool_mentions(text);
            select_skills_from_mentions(
                &selection_context,
                &blocked_plain_names,
                &mentioned_names,
                &mut seen_names,
                &mut seen_paths,
                &mut selected,
            );
        }
    }

    selected
}

struct SkillSelectionContext<'a> {
    skills: &'a [SkillMetadata],
    disabled_paths: &'a HashSet<EnvironmentPathRef>,
    skill_name_counts: &'a HashMap<String, usize>,
    connector_slug_counts: &'a HashMap<String, usize>,
}

pub struct ToolMentions<'a> {
    names: HashSet<&'a str>,
    paths: HashSet<&'a str>,
    plain_names: HashSet<&'a str>,
    linked_mentions: HashSet<LinkedToolMention<'a>>,
}

impl<'a> ToolMentions<'a> {
    fn is_empty(&self) -> bool {
        self.names.is_empty() && self.paths.is_empty()
    }

    pub fn plain_names(&self) -> impl Iterator<Item = &'a str> + '_ {
        self.plain_names.iter().copied()
    }

    pub fn paths(&self) -> impl Iterator<Item = &'a str> + '_ {
        self.paths.iter().copied()
    }

    fn linked_mentions(&self) -> impl Iterator<Item = LinkedToolMention<'a>> + '_ {
        self.linked_mentions.iter().copied()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct LinkedToolMention<'a> {
    name: &'a str,
    path: &'a str,
}

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

fn is_skill_filename(path: &str) -> bool {
    let file_name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    file_name.eq_ignore_ascii_case(SKILL_FILENAME)
}

pub fn app_id_from_path(path: &str) -> Option<&str> {
    path.strip_prefix(APP_PATH_PREFIX)
        .filter(|value| !value.is_empty())
}

pub fn plugin_config_name_from_path(path: &str) -> Option<&str> {
    path.strip_prefix(PLUGIN_PATH_PREFIX)
        .filter(|value| !value.is_empty())
}

pub(crate) fn normalize_skill_path(path: &str) -> &str {
    path.strip_prefix(SKILL_PATH_PREFIX).unwrap_or(path)
}

/// Extract `$tool-name` mentions from a single text input.
///
/// Supports explicit resource links in the form `[$tool-name](resource path)`. When a
/// resource path is present, it is captured for exact path matching while also tracking
/// the name for fallback matching.
pub fn extract_tool_mentions(text: &str) -> ToolMentions<'_> {
    extract_tool_mentions_with_sigil(text, TOOL_MENTION_SIGIL)
}

pub fn extract_tool_mentions_with_sigil(text: &str, sigil: char) -> ToolMentions<'_> {
    let text_bytes = text.as_bytes();
    let mut mentioned_names: HashSet<&str> = HashSet::new();
    let mut mentioned_paths: HashSet<&str> = HashSet::new();
    let mut plain_names: HashSet<&str> = HashSet::new();
    let mut linked_mentions: HashSet<LinkedToolMention<'_>> = HashSet::new();

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
                linked_mentions.insert(LinkedToolMention { name, path });
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
        while let Some(next_byte) = text_bytes.get(name_end)
            && is_mention_name_char(*next_byte)
        {
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
        names: mentioned_names,
        paths: mentioned_paths,
        plain_names,
        linked_mentions,
    }
}

/// Select mentioned skills while preserving the order of `skills`.
fn select_skills_from_mentions(
    selection_context: &SkillSelectionContext<'_>,
    blocked_plain_names: &HashSet<String>,
    mentions: &ToolMentions<'_>,
    seen_names: &mut HashSet<String>,
    seen_paths: &mut HashSet<EnvironmentPathRef>,
    selected: &mut Vec<SkillMetadata>,
) {
    if mentions.is_empty() {
        return;
    }

    let mention_skill_paths: HashSet<&str> = mentions
        .paths()
        .filter(|path| {
            !matches!(
                tool_kind_for_path(path),
                ToolMentionKind::App | ToolMentionKind::Mcp | ToolMentionKind::Plugin
            )
        })
        .map(normalize_skill_path)
        .collect();
    let mention_skill_links: HashSet<LinkedToolMention<'_>> = mentions
        .linked_mentions()
        .filter(|mention| {
            !matches!(
                tool_kind_for_path(mention.path),
                ToolMentionKind::App | ToolMentionKind::Mcp | ToolMentionKind::Plugin
            )
        })
        .map(|mention| LinkedToolMention {
            name: mention.name,
            path: normalize_skill_path(mention.path),
        })
        .collect();

    let mention_skill_path_counts = selection_context
        .skills
        .iter()
        .filter(|skill| {
            !selection_context
                .disabled_paths
                .contains(&skill.source_path)
        })
        .filter_map(|skill| {
            let path = skill.path_to_skills_md.to_string_lossy();
            mention_skill_paths
                .contains(path.as_ref())
                .then_some(path.into_owned())
        })
        .fold(HashMap::new(), |mut counts, path| {
            *counts.entry(path).or_insert(0usize) += 1;
            counts
        });
    let mention_skill_link_counts = selection_context
        .skills
        .iter()
        .filter(|skill| {
            !selection_context
                .disabled_paths
                .contains(&skill.source_path)
        })
        .filter_map(|skill| {
            let path = skill.path_to_skills_md.to_string_lossy();
            mention_skill_links
                .contains(&LinkedToolMention {
                    name: skill.name.as_str(),
                    path: path.as_ref(),
                })
                .then_some((skill.name.clone(), path.into_owned()))
        })
        .fold(HashMap::new(), |mut counts, link| {
            *counts.entry(link).or_insert(0usize) += 1;
            counts
        });

    for skill in selection_context.skills {
        if selection_context
            .disabled_paths
            .contains(&skill.source_path)
            || seen_paths.contains(&skill.source_path)
        {
            continue;
        }

        let path_str = skill.path_to_skills_md.to_string_lossy();
        let path = path_str.as_ref();
        let linked_mention = LinkedToolMention {
            name: skill.name.as_str(),
            path,
        };
        let linked_mention_key = (skill.name.clone(), path.to_string());
        if mention_skill_paths.contains(path)
            && (mention_skill_path_counts.get(path) == Some(&1)
                || (mention_skill_links.contains(&linked_mention)
                    && mention_skill_link_counts.get(&linked_mention_key) == Some(&1)))
        {
            seen_paths.insert(skill.source_path.clone());
            seen_names.insert(skill.name.clone());
            selected.push(skill.clone());
        }
    }

    for skill in selection_context.skills {
        if selection_context
            .disabled_paths
            .contains(&skill.source_path)
            || seen_paths.contains(&skill.source_path)
        {
            continue;
        }

        if blocked_plain_names.contains(skill.name.as_str()) {
            continue;
        }
        if !mentions.plain_names.contains(skill.name.as_str()) {
            continue;
        }

        let skill_count = selection_context
            .skill_name_counts
            .get(skill.name.as_str())
            .copied()
            .unwrap_or(0);
        let connector_count = selection_context
            .connector_slug_counts
            .get(&skill.name.to_ascii_lowercase())
            .copied()
            .unwrap_or(0);
        if skill_count != 1 || connector_count != 0 {
            continue;
        }

        if seen_names.insert(skill.name.clone()) {
            seen_paths.insert(skill.source_path.clone());
            selected.push(skill.clone());
        }
    }
}

fn parse_linked_tool_mention<'a>(
    text: &'a str,
    text_bytes: &[u8],
    start: usize,
    sigil: char,
) -> Option<(&'a str, &'a str, usize)> {
    let sigil_index = start + 1;
    if text_bytes.get(sigil_index) != Some(&(sigil as u8)) {
        return None;
    }

    let name_start = sigil_index + 1;
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

#[cfg(test)]
fn text_mentions_skill(text: &str, skill_name: &str) -> bool {
    if skill_name.is_empty() {
        return false;
    }

    let text_bytes = text.as_bytes();
    let skill_bytes = skill_name.as_bytes();

    for (index, byte) in text_bytes.iter().copied().enumerate() {
        if byte != b'$' {
            continue;
        }

        let name_start = index + 1;
        let Some(rest) = text_bytes.get(name_start..) else {
            continue;
        };
        if !rest.starts_with(skill_bytes) {
            continue;
        }

        let after_index = name_start + skill_bytes.len();
        let after = text_bytes.get(after_index).copied();
        if after.is_none_or(|b| !is_mention_name_char(b)) {
            return true;
        }
    }

    false
}

fn is_mention_name_char(byte: u8) -> bool {
    matches!(byte, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'-' | b':')
}

#[cfg(test)]
#[path = "injection_tests.rs"]
mod tests;
