use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;

use crate::analytics_client::AnalyticsEventsClient;
use crate::analytics_client::SkillInvocation;
use crate::analytics_client::TrackEventsContext;
use crate::instructions::SkillInstructions;
use crate::mentions::build_skill_name_counts;
use crate::skills::SkillMetadata;
use codex_otel::OtelManager;
use codex_protocol::models::ResponseItem;
use codex_protocol::user_input::UserInput;
use tokio::fs;

#[derive(Debug, Default)]
pub(crate) struct SkillInjections {
    pub(crate) items: Vec<ResponseItem>,
    pub(crate) warnings: Vec<String>,
}

pub(crate) async fn build_skill_injections(
    mentioned_skills: &[SkillMetadata],
    otel: Option<&OtelManager>,
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
        match fs::read_to_string(&skill.path).await {
            Ok(contents) => {
                emit_skill_injected_metric(otel, skill, "ok");
                invocations.push(SkillInvocation {
                    skill_name: skill.name.clone(),
                    skill_scope: skill.scope,
                    skill_path: skill.path.clone(),
                });
                result.items.push(ResponseItem::from(SkillInstructions {
                    name: skill.name.clone(),
                    path: skill.path.to_string_lossy().into_owned(),
                    contents,
                }));
            }
            Err(err) => {
                emit_skill_injected_metric(otel, skill, "error");
                let message = format!(
                    "Failed to load skill {name} at {path}: {err:#}",
                    name = skill.name,
                    path = skill.path.display()
                );
                result.warnings.push(message);
            }
        }
    }

    analytics_client.track_skill_invocations(tracking, invocations);

    result
}

fn emit_skill_injected_metric(otel: Option<&OtelManager>, skill: &SkillMetadata, status: &str) {
    let Some(otel) = otel else {
        return;
    };

    otel.counter(
        "codex.skill.injected",
        1,
        &[("status", status), ("skill", skill.name.as_str())],
    );
}

/// Collect explicitly mentioned skills from structured and text mentions.
///
/// Structured `UserInput::Skill` selections are resolved first by path against
/// enabled skills. Text inputs are then scanned to extract `$skill-name` tokens, and we
/// iterate `skills` in their existing order to preserve prior ordering semantics.
/// Explicit links are resolved by path and plain names are only used when the match
/// is unambiguous.
///
/// Complexity: `O(T + (N_s + N_t) * S)` time, `O(S + M)` space, where:
/// `S` = number of skills, `T` = total text length, `N_s` = number of structured skill inputs,
/// `N_t` = number of text inputs, `M` = max mentions parsed from a single text input.
pub(crate) fn collect_explicit_skill_mentions(
    inputs: &[UserInput],
    skills: &[SkillMetadata],
    disabled_paths: &HashSet<PathBuf>,
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
    let mut seen_paths: HashSet<PathBuf> = HashSet::new();
    let mut blocked_plain_names: HashSet<String> = HashSet::new();

    for input in inputs {
        if let UserInput::Skill { name, path } = input {
            blocked_plain_names.insert(name.clone());
            if selection_context.disabled_paths.contains(path) || seen_paths.contains(path) {
                continue;
            }

            if let Some(skill) = selection_context
                .skills
                .iter()
                .find(|skill| skill.path.as_path() == path.as_path())
            {
                seen_paths.insert(skill.path.clone());
                seen_names.insert(skill.name.clone());
                selected.push(skill.clone());
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
    disabled_paths: &'a HashSet<PathBuf>,
    skill_name_counts: &'a HashMap<String, usize>,
    connector_slug_counts: &'a HashMap<String, usize>,
}

pub(crate) struct ToolMentions<'a> {
    names: HashSet<&'a str>,
    paths: HashSet<&'a str>,
    plain_names: HashSet<&'a str>,
}

impl<'a> ToolMentions<'a> {
    fn is_empty(&self) -> bool {
        self.names.is_empty() && self.paths.is_empty()
    }

    pub(crate) fn plain_names(&self) -> impl Iterator<Item = &'a str> + '_ {
        self.plain_names.iter().copied()
    }

    pub(crate) fn paths(&self) -> impl Iterator<Item = &'a str> + '_ {
        self.paths.iter().copied()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ToolMentionKind {
    App,
    Mcp,
    Skill,
    Other,
}

const APP_PATH_PREFIX: &str = "app://";
const MCP_PATH_PREFIX: &str = "mcp://";
const SKILL_PATH_PREFIX: &str = "skill://";
const SKILL_FILENAME: &str = "SKILL.md";

pub(crate) fn tool_kind_for_path(path: &str) -> ToolMentionKind {
    if path.starts_with(APP_PATH_PREFIX) {
        ToolMentionKind::App
    } else if path.starts_with(MCP_PATH_PREFIX) {
        ToolMentionKind::Mcp
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

pub(crate) fn app_id_from_path(path: &str) -> Option<&str> {
    path.strip_prefix(APP_PATH_PREFIX)
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
pub(crate) fn extract_tool_mentions(text: &str) -> ToolMentions<'_> {
    let text_bytes = text.as_bytes();
    let mut mentioned_names: HashSet<&str> = HashSet::new();
    let mut mentioned_paths: HashSet<&str> = HashSet::new();
    let mut plain_names: HashSet<&str> = HashSet::new();

    let mut index = 0;
    while index < text_bytes.len() {
        let byte = text_bytes[index];
        if byte == b'['
            && let Some((name, path, end_index)) =
                parse_linked_tool_mention(text, text_bytes, index)
        {
            if !is_common_env_var(name) {
                let kind = tool_kind_for_path(path);
                if !matches!(kind, ToolMentionKind::App | ToolMentionKind::Mcp) {
                    mentioned_names.insert(name);
                }
                mentioned_paths.insert(path);
            }
            index = end_index;
            continue;
        }

        if byte != b'$' {
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
    }
}

/// Select mentioned skills while preserving the order of `skills`.
fn select_skills_from_mentions(
    selection_context: &SkillSelectionContext<'_>,
    blocked_plain_names: &HashSet<String>,
    mentions: &ToolMentions<'_>,
    seen_names: &mut HashSet<String>,
    seen_paths: &mut HashSet<PathBuf>,
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
                ToolMentionKind::App | ToolMentionKind::Mcp
            )
        })
        .map(normalize_skill_path)
        .collect();

    for skill in selection_context.skills {
        if selection_context.disabled_paths.contains(&skill.path)
            || seen_paths.contains(&skill.path)
        {
            continue;
        }

        let path_str = skill.path.to_string_lossy();
        if mention_skill_paths.contains(path_str.as_ref()) {
            seen_paths.insert(skill.path.clone());
            seen_names.insert(skill.name.clone());
            selected.push(skill.clone());
        }
    }

    for skill in selection_context.skills {
        if selection_context.disabled_paths.contains(&skill.path)
            || seen_paths.contains(&skill.path)
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
            seen_paths.insert(skill.path.clone());
            selected.push(skill.clone());
        }
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
    matches!(byte, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'-')
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::collections::HashMap;
    use std::collections::HashSet;

    fn make_skill(name: &str, path: &str) -> SkillMetadata {
        SkillMetadata {
            name: name.to_string(),
            description: format!("{name} skill"),
            short_description: None,
            interface: None,
            dependencies: None,
            policy: None,
            path: PathBuf::from(path),
            scope: codex_protocol::protocol::SkillScope::User,
        }
    }

    fn set<'a>(items: &'a [&'a str]) -> HashSet<&'a str> {
        items.iter().copied().collect()
    }

    fn assert_mentions(text: &str, expected_names: &[&str], expected_paths: &[&str]) {
        let mentions = extract_tool_mentions(text);
        assert_eq!(mentions.names, set(expected_names));
        assert_eq!(mentions.paths, set(expected_paths));
    }

    fn collect_mentions(
        inputs: &[UserInput],
        skills: &[SkillMetadata],
        disabled_paths: &HashSet<PathBuf>,
        connector_slug_counts: &HashMap<String, usize>,
    ) -> Vec<SkillMetadata> {
        collect_explicit_skill_mentions(inputs, skills, disabled_paths, connector_slug_counts)
    }

    #[test]
    fn text_mentions_skill_requires_exact_boundary() {
        assert_eq!(
            true,
            text_mentions_skill("use $notion-research-doc please", "notion-research-doc")
        );
        assert_eq!(
            true,
            text_mentions_skill("($notion-research-doc)", "notion-research-doc")
        );
        assert_eq!(
            true,
            text_mentions_skill("$notion-research-doc.", "notion-research-doc")
        );
        assert_eq!(
            false,
            text_mentions_skill("$notion-research-docs", "notion-research-doc")
        );
        assert_eq!(
            false,
            text_mentions_skill("$notion-research-doc_extra", "notion-research-doc")
        );
    }

    #[test]
    fn text_mentions_skill_handles_end_boundary_and_near_misses() {
        assert_eq!(true, text_mentions_skill("$alpha-skill", "alpha-skill"));
        assert_eq!(false, text_mentions_skill("$alpha-skillx", "alpha-skill"));
        assert_eq!(
            true,
            text_mentions_skill("$alpha-skillx and later $alpha-skill ", "alpha-skill")
        );
    }

    #[test]
    fn text_mentions_skill_handles_many_dollars_without_looping() {
        let prefix = "$".repeat(256);
        let text = format!("{prefix} not-a-mention");
        assert_eq!(false, text_mentions_skill(&text, "alpha-skill"));
    }

    #[test]
    fn extract_tool_mentions_handles_plain_and_linked_mentions() {
        assert_mentions(
            "use $alpha and [$beta](/tmp/beta)",
            &["alpha", "beta"],
            &["/tmp/beta"],
        );
    }

    #[test]
    fn extract_tool_mentions_skips_common_env_vars() {
        assert_mentions("use $PATH and $alpha", &["alpha"], &[]);
        assert_mentions("use [$HOME](/tmp/skill)", &[], &[]);
        assert_mentions("use $XDG_CONFIG_HOME and $beta", &["beta"], &[]);
    }

    #[test]
    fn extract_tool_mentions_requires_link_syntax() {
        assert_mentions("[beta](/tmp/beta)", &[], &[]);
        assert_mentions("[$beta] /tmp/beta", &["beta"], &[]);
        assert_mentions("[$beta]()", &["beta"], &[]);
    }

    #[test]
    fn extract_tool_mentions_trims_linked_paths_and_allows_spacing() {
        assert_mentions("use [$beta]   ( /tmp/beta )", &["beta"], &["/tmp/beta"]);
    }

    #[test]
    fn extract_tool_mentions_stops_at_non_name_chars() {
        assert_mentions(
            "use $alpha.skill and $beta_extra",
            &["alpha", "beta_extra"],
            &[],
        );
    }

    #[test]
    fn collect_explicit_skill_mentions_text_respects_skill_order() {
        let alpha = make_skill("alpha-skill", "/tmp/alpha");
        let beta = make_skill("beta-skill", "/tmp/beta");
        let skills = vec![beta.clone(), alpha.clone()];
        let inputs = vec![UserInput::Text {
            text: "first $alpha-skill then $beta-skill".to_string(),
            text_elements: Vec::new(),
        }];
        let connector_counts = HashMap::new();

        let selected = collect_mentions(&inputs, &skills, &HashSet::new(), &connector_counts);

        // Text scanning should not change the previous selection ordering semantics.
        assert_eq!(selected, vec![beta, alpha]);
    }

    #[test]
    fn collect_explicit_skill_mentions_prioritizes_structured_inputs() {
        let alpha = make_skill("alpha-skill", "/tmp/alpha");
        let beta = make_skill("beta-skill", "/tmp/beta");
        let skills = vec![alpha.clone(), beta.clone()];
        let inputs = vec![
            UserInput::Text {
                text: "please run $alpha-skill".to_string(),
                text_elements: Vec::new(),
            },
            UserInput::Skill {
                name: "beta-skill".to_string(),
                path: PathBuf::from("/tmp/beta"),
            },
        ];
        let connector_counts = HashMap::new();

        let selected = collect_mentions(&inputs, &skills, &HashSet::new(), &connector_counts);

        assert_eq!(selected, vec![beta, alpha]);
    }

    #[test]
    fn collect_explicit_skill_mentions_skips_invalid_structured_and_blocks_plain_fallback() {
        let alpha = make_skill("alpha-skill", "/tmp/alpha");
        let skills = vec![alpha];
        let inputs = vec![
            UserInput::Text {
                text: "please run $alpha-skill".to_string(),
                text_elements: Vec::new(),
            },
            UserInput::Skill {
                name: "alpha-skill".to_string(),
                path: PathBuf::from("/tmp/missing"),
            },
        ];
        let connector_counts = HashMap::new();

        let selected = collect_mentions(&inputs, &skills, &HashSet::new(), &connector_counts);

        assert_eq!(selected, Vec::new());
    }

    #[test]
    fn collect_explicit_skill_mentions_skips_disabled_structured_and_blocks_plain_fallback() {
        let alpha = make_skill("alpha-skill", "/tmp/alpha");
        let skills = vec![alpha];
        let inputs = vec![
            UserInput::Text {
                text: "please run $alpha-skill".to_string(),
                text_elements: Vec::new(),
            },
            UserInput::Skill {
                name: "alpha-skill".to_string(),
                path: PathBuf::from("/tmp/alpha"),
            },
        ];
        let disabled = HashSet::from([PathBuf::from("/tmp/alpha")]);
        let connector_counts = HashMap::new();

        let selected = collect_mentions(&inputs, &skills, &disabled, &connector_counts);

        assert_eq!(selected, Vec::new());
    }

    #[test]
    fn collect_explicit_skill_mentions_dedupes_by_path() {
        let alpha = make_skill("alpha-skill", "/tmp/alpha");
        let skills = vec![alpha.clone()];
        let inputs = vec![UserInput::Text {
            text: "use [$alpha-skill](/tmp/alpha) and [$alpha-skill](/tmp/alpha)".to_string(),
            text_elements: Vec::new(),
        }];
        let connector_counts = HashMap::new();

        let selected = collect_mentions(&inputs, &skills, &HashSet::new(), &connector_counts);

        assert_eq!(selected, vec![alpha]);
    }

    #[test]
    fn collect_explicit_skill_mentions_skips_ambiguous_name() {
        let alpha = make_skill("demo-skill", "/tmp/alpha");
        let beta = make_skill("demo-skill", "/tmp/beta");
        let skills = vec![alpha, beta];
        let inputs = vec![UserInput::Text {
            text: "use $demo-skill and again $demo-skill".to_string(),
            text_elements: Vec::new(),
        }];
        let connector_counts = HashMap::new();

        let selected = collect_mentions(&inputs, &skills, &HashSet::new(), &connector_counts);

        assert_eq!(selected, Vec::new());
    }

    #[test]
    fn collect_explicit_skill_mentions_prefers_linked_path_over_name() {
        let alpha = make_skill("demo-skill", "/tmp/alpha");
        let beta = make_skill("demo-skill", "/tmp/beta");
        let skills = vec![alpha, beta.clone()];
        let inputs = vec![UserInput::Text {
            text: "use $demo-skill and [$demo-skill](/tmp/beta)".to_string(),
            text_elements: Vec::new(),
        }];
        let connector_counts = HashMap::new();

        let selected = collect_mentions(&inputs, &skills, &HashSet::new(), &connector_counts);

        assert_eq!(selected, vec![beta]);
    }

    #[test]
    fn collect_explicit_skill_mentions_skips_plain_name_when_connector_matches() {
        let alpha = make_skill("alpha-skill", "/tmp/alpha");
        let skills = vec![alpha];
        let inputs = vec![UserInput::Text {
            text: "use $alpha-skill".to_string(),
            text_elements: Vec::new(),
        }];
        let connector_counts = HashMap::from([("alpha-skill".to_string(), 1)]);

        let selected = collect_mentions(&inputs, &skills, &HashSet::new(), &connector_counts);

        assert_eq!(selected, Vec::new());
    }

    #[test]
    fn collect_explicit_skill_mentions_allows_explicit_path_with_connector_conflict() {
        let alpha = make_skill("alpha-skill", "/tmp/alpha");
        let skills = vec![alpha.clone()];
        let inputs = vec![UserInput::Text {
            text: "use [$alpha-skill](/tmp/alpha)".to_string(),
            text_elements: Vec::new(),
        }];
        let connector_counts = HashMap::from([("alpha-skill".to_string(), 1)]);

        let selected = collect_mentions(&inputs, &skills, &HashSet::new(), &connector_counts);

        assert_eq!(selected, vec![alpha]);
    }

    #[test]
    fn collect_explicit_skill_mentions_skips_when_linked_path_disabled() {
        let alpha = make_skill("demo-skill", "/tmp/alpha");
        let beta = make_skill("demo-skill", "/tmp/beta");
        let skills = vec![alpha, beta];
        let inputs = vec![UserInput::Text {
            text: "use [$demo-skill](/tmp/alpha)".to_string(),
            text_elements: Vec::new(),
        }];
        let disabled = HashSet::from([PathBuf::from("/tmp/alpha")]);
        let connector_counts = HashMap::new();

        let selected = collect_mentions(&inputs, &skills, &disabled, &connector_counts);

        assert_eq!(selected, Vec::new());
    }

    #[test]
    fn collect_explicit_skill_mentions_prefers_resource_path() {
        let alpha = make_skill("demo-skill", "/tmp/alpha");
        let beta = make_skill("demo-skill", "/tmp/beta");
        let skills = vec![alpha, beta.clone()];
        let inputs = vec![UserInput::Text {
            text: "use [$demo-skill](/tmp/beta)".to_string(),
            text_elements: Vec::new(),
        }];
        let connector_counts = HashMap::new();

        let selected = collect_mentions(&inputs, &skills, &HashSet::new(), &connector_counts);

        assert_eq!(selected, vec![beta]);
    }

    #[test]
    fn collect_explicit_skill_mentions_skips_missing_path_with_no_fallback() {
        let alpha = make_skill("demo-skill", "/tmp/alpha");
        let beta = make_skill("demo-skill", "/tmp/beta");
        let skills = vec![alpha, beta];
        let inputs = vec![UserInput::Text {
            text: "use [$demo-skill](/tmp/missing)".to_string(),
            text_elements: Vec::new(),
        }];
        let connector_counts = HashMap::new();

        let selected = collect_mentions(&inputs, &skills, &HashSet::new(), &connector_counts);

        assert_eq!(selected, Vec::new());
    }

    #[test]
    fn collect_explicit_skill_mentions_skips_missing_path_without_fallback() {
        let alpha = make_skill("demo-skill", "/tmp/alpha");
        let skills = vec![alpha];
        let inputs = vec![UserInput::Text {
            text: "use [$demo-skill](/tmp/missing)".to_string(),
            text_elements: Vec::new(),
        }];
        let connector_counts = HashMap::new();

        let selected = collect_mentions(&inputs, &skills, &HashSet::new(), &connector_counts);

        assert_eq!(selected, Vec::new());
    }
}
