use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;

use crate::SkillLoadOutcome;
use crate::SkillMetadata;
use crate::build_skill_name_counts;
use codex_analytics::AnalyticsEventsClient;
use codex_analytics::InvocationType;
use codex_analytics::SkillInvocation;
use codex_analytics::TrackEventsContext;
use codex_exec_server::LOCAL_FS;
use codex_otel::SessionTelemetry;
use codex_otel::sanitize_metric_tag_value;
use codex_protocol::user_input::UserInput;
pub use codex_skills::ToolMentionKind;
pub use codex_skills::ToolMentions;
pub use codex_skills::app_id_from_path;
pub use codex_skills::extract_tool_mentions;
pub use codex_skills::extract_tool_mentions_with_sigil;
pub use codex_skills::normalize_skill_path;
pub use codex_skills::plugin_config_name_from_path;
pub use codex_skills::tool_kind_for_path;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use codex_utils_string::take_bytes_at_char_boundary;

use crate::MAX_SKILL_PROMPT_BYTES;

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

/// Host skill prompts that have already been injected by an extension for this
/// turn.
///
/// Core uses this to keep the legacy skill-injection path from sending the same
/// host `SKILL.md` body again while the skills extension is being wired in.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InjectedHostSkillPrompts {
    paths: HashSet<String>,
}

/// Marks a turn whose skills extension projects the host skill catalog through
/// WorldState.
///
/// Core uses this to keep its legacy thread-start catalog from duplicating the
/// extension-owned catalog.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HostSkillsCatalogInWorldState;

impl InjectedHostSkillPrompts {
    pub fn insert_path(&mut self, path: impl Into<String>) {
        let path = path.into();
        self.paths.insert(normalize_host_skill_path(&path));
        self.paths.insert(path);
    }

    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    pub fn contains_path(&self, path: &str) -> bool {
        self.paths.contains(path) || self.paths.contains(&normalize_host_skill_path(path))
    }
}

#[tracing::instrument(
    level = "trace",
    skip_all,
    fields(mentioned_skill_count = mentioned_skills.len())
)]
pub async fn build_skill_injections(
    mentioned_skills: &[SkillMetadata],
    loaded_skills: Option<&SkillLoadOutcome>,
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
        let fs = loaded_skills
            .and_then(|outcome| outcome.file_system_for_skill(skill))
            .unwrap_or_else(|| Arc::clone(&LOCAL_FS));
        let path = PathUri::from_abs_path(&skill.path_to_skills_md);
        match fs.read_file_text(&path, /*sandbox*/ None).await {
            Ok(contents) => {
                let (contents, truncated) =
                    if loaded_skills.is_some_and(|outcome| outcome.is_agent_plugin_skill(skill)) {
                        bounded_skill_prompt_contents(&contents)
                    } else {
                        (contents, false)
                    };
                if truncated {
                    result.warnings.push(format!(
                        "Skill `{}` exceeded the main prompt context limit and was truncated.",
                        skill.name
                    ));
                }
                emit_skill_injected_metric(otel, skill, "ok");
                invocations.push(SkillInvocation {
                    skill_name: skill.name.clone(),
                    skill_scope: skill.scope,
                    skill_path: skill.path_to_skills_md.to_path_buf(),
                    plugin_id: skill.plugin_id.clone(),
                    remote_plugin_id: skill.remote_plugin_id.clone(),
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

fn bounded_skill_prompt_contents(contents: &str) -> (String, bool) {
    let bounded = take_bytes_at_char_boundary(contents, MAX_SKILL_PROMPT_BYTES);
    (bounded.to_string(), bounded.len() < contents.len())
}

fn normalize_host_skill_path(path: &str) -> String {
    normalize_skill_path(path).replace('\\', "/")
}

fn emit_skill_injected_metric(
    otel: Option<&SessionTelemetry>,
    skill: &SkillMetadata,
    status: &str,
) {
    let Some(otel) = otel else {
        return;
    };
    let skill_name_tag = sanitize_metric_tag_value(skill.name.as_str());

    otel.counter(
        "codex.skill.injected",
        /*inc*/ 1,
        &[
            ("status", status),
            ("skill", skill_name_tag.as_str()),
            ("invoke_type", "explicit"),
        ],
    );
}

/// Collect explicitly mentioned skills from structured and text mentions.
///
/// Structured `UserInput::Skill` selections are resolved first by path against
/// enabled skills. Text inputs are then scanned to extract `$skill-name` tokens, and we
/// iterate loaded skills in their existing order to preserve prior ordering semantics.
/// Explicit paths match either a skill's canonical identity or its logical discovery
/// path, and plain names are only used when the match is unambiguous.
///
/// Complexity: `O(T + (N_s + N_t) * S)` time, `O(S + M)` space, where:
/// `S` = number of skills, `T` = total text length, `N_s` = number of structured skill inputs,
/// `N_t` = number of text inputs, `M` = max mentions parsed from a single text input.
pub fn collect_explicit_skill_mentions(
    inputs: &[UserInput],
    loaded_skills: &SkillLoadOutcome,
    connector_slug_counts: &HashMap<String, usize>,
) -> Vec<SkillMetadata> {
    let skill_name_counts =
        build_skill_name_counts(&loaded_skills.skills, &loaded_skills.disabled_paths).0;

    let selection_context = SkillSelectionContext {
        loaded_skills,
        skill_name_counts: &skill_name_counts,
        connector_slug_counts,
    };
    let mut selected: Vec<SkillMetadata> = Vec::new();
    let mut seen_names: HashSet<String> = HashSet::new();
    let mut seen_paths: HashSet<AbsolutePathBuf> = HashSet::new();
    let mut blocked_plain_names: HashSet<String> = HashSet::new();

    for input in inputs {
        if let UserInput::Skill { name, path, .. } = input {
            blocked_plain_names.insert(name.clone());
            let Ok(path) = AbsolutePathBuf::relative_to_current_dir(path) else {
                continue;
            };

            let Some(skill) = selection_context.loaded_skills.skills.iter().find(|skill| {
                skill.path_to_skills_md == path
                    || selection_context
                        .loaded_skills
                        .skill_discovery_path_for_path(&skill.path_to_skills_md)
                        .is_some_and(|discovery_path| discovery_path == &path)
            }) else {
                continue;
            };

            if !selection_context.loaded_skills.is_skill_enabled(skill)
                || seen_paths.contains(&skill.path_to_skills_md)
            {
                continue;
            }

            seen_paths.insert(skill.path_to_skills_md.clone());
            seen_names.insert(skill.name.clone());
            selected.push(skill.clone());
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
    loaded_skills: &'a SkillLoadOutcome,
    skill_name_counts: &'a HashMap<String, usize>,
    connector_slug_counts: &'a HashMap<String, usize>,
}

/// Select mentioned skills while preserving the order of `skills`.
fn select_skills_from_mentions(
    selection_context: &SkillSelectionContext<'_>,
    blocked_plain_names: &HashSet<String>,
    mentions: &ToolMentions<'_>,
    seen_names: &mut HashSet<String>,
    seen_paths: &mut HashSet<AbsolutePathBuf>,
    selected: &mut Vec<SkillMetadata>,
) {
    if mentions.is_empty() {
        return;
    }

    let mention_skill_paths: HashSet<String> = mentions
        .paths()
        .filter(|path| {
            !matches!(
                tool_kind_for_path(path),
                ToolMentionKind::App | ToolMentionKind::Mcp | ToolMentionKind::Plugin
            )
        })
        .map(normalize_host_skill_path)
        .collect();

    for skill in &selection_context.loaded_skills.skills {
        if !selection_context.loaded_skills.is_skill_enabled(skill)
            || seen_paths.contains(&skill.path_to_skills_md)
        {
            continue;
        }

        let canonical_path = normalize_host_skill_path(&skill.path_to_skills_md.to_string_lossy());
        let matches_discovery_path = selection_context
            .loaded_skills
            .skill_discovery_path_for_path(&skill.path_to_skills_md)
            .is_some_and(|discovery_path| {
                mention_skill_paths.contains(&normalize_host_skill_path(
                    &discovery_path.to_string_lossy(),
                ))
            });
        if mention_skill_paths.contains(&canonical_path) || matches_discovery_path {
            seen_paths.insert(skill.path_to_skills_md.clone());
            seen_names.insert(skill.name.clone());
            selected.push(skill.clone());
        }
    }

    for skill in &selection_context.loaded_skills.skills {
        if !selection_context.loaded_skills.is_skill_enabled(skill)
            || seen_paths.contains(&skill.path_to_skills_md)
        {
            continue;
        }

        if blocked_plain_names.contains(skill.name.as_str()) {
            continue;
        }
        if !mentions.contains_plain_name(skill.name.as_str()) {
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
            seen_paths.insert(skill.path_to_skills_md.clone());
            selected.push(skill.clone());
        }
    }
}

#[cfg(test)]
#[path = "injection_tests.rs"]
mod tests;
