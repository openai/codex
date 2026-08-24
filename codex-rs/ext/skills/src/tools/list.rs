use codex_extension_api::FunctionCallError;
use codex_extension_api::ToolCall;
use codex_extension_api::ToolExecutor;
use codex_extension_api::ToolExecutorFuture;
use codex_extension_api::ToolName;
use codex_extension_api::ToolSpec;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;

use crate::catalog::SkillCatalogEntry;
use crate::render::MAX_SKILL_NAME_BYTES;
use crate::render::truncate_catalog_skill_description;
use crate::render::truncate_utf8_to_bytes;
use crate::warnings::bounded_warnings;

use super::MAX_HANDLE_BYTES;
use super::MAX_SKILL_RESPONSE_BYTES;
use super::SkillToolAuthority;
use super::SkillToolAuthoritySelector;
use super::SkillToolContext;
use super::is_bounded_handle;
use super::parse_args;
use super::parse_pagination_cursor;
use super::serialized_len;
use super::skill_function_tool;
use super::skill_json_output;
use super::skill_tool_name;
use super::value_fingerprint;

const TOOL_NAME: &str = "list";
const MAX_SKILLS_PER_PAGE: usize = 20;
const OVERSIZED_ENTRY_WARNING: &str =
    "Some skills were omitted because their metadata is too large.";

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ListArgs {
    authority: SkillToolAuthoritySelector,
    cursor: Option<String>,
}

#[derive(Clone, Debug, Eq, Hash, JsonSchema, PartialEq, Serialize)]
#[schemars(deny_unknown_fields)]
struct ListedSkill {
    authority: SkillToolAuthority,
    package: String,
    name: String,
    description: String,
    main_resource: String,
}

#[derive(Debug, Eq, JsonSchema, PartialEq, Serialize)]
#[schemars(deny_unknown_fields)]
struct ListResponse {
    skills: Vec<ListedSkill>,
    warnings: Vec<String>,
    next_cursor: Option<String>,
}

#[derive(Clone)]
pub(super) struct ListTool {
    pub(super) context: SkillToolContext,
}

impl ToolExecutor<ToolCall> for ListTool {
    fn tool_name(&self) -> ToolName {
        skill_tool_name(TOOL_NAME)
    }

    fn spec(&self) -> ToolSpec {
        skill_function_tool::<ListArgs, ListResponse>(
            TOOL_NAME,
            "List skills owned by the requested authority. Returns each skill's authority, package, and main_resource. Pass the package to skills.read, and pass next_cursor back as cursor to continue.",
        )
    }

    fn handle(&self, call: ToolCall) -> ToolExecutorFuture<'_> {
        Box::pin(async move {
            let args: ListArgs = parse_args(&call)?;
            let response_byte_budget = call.response_byte_budget(MAX_SKILL_RESPONSE_BYTES);
            let catalog = self.context.catalog(&call.turn_id, args.authority).await;
            let mut omitted_oversized_entry = false;
            let canonical_skills = catalog
                .entries
                .into_iter()
                .filter(|entry| {
                    entry.is_model_visible() && args.authority.matches(&entry.authority)
                })
                .filter_map(|entry| {
                    let listed = listed_skill(entry);
                    omitted_oversized_entry |= listed.is_none();
                    listed
                })
                .collect::<Vec<_>>();
            // Older cursors contain only the catalog fingerprint and offset. New cursors
            // also record the budget whose omissions have already been reported.
            let (cursor, previous_byte_budget) = match args.cursor.as_deref() {
                Some(cursor) => match cursor.rsplit_once(':') {
                    Some((canonical_cursor, byte_budget)) => {
                        if canonical_cursor.contains(':') {
                            let byte_budget = byte_budget.parse::<usize>().map_err(|_| {
                                FunctionCallError::RespondToModel(
                                    "skills.list cursor is invalid".to_string(),
                                )
                            })?;
                            (Some(canonical_cursor), Some(byte_budget))
                        } else {
                            (Some(cursor), None)
                        }
                    }
                    None => (Some(cursor), None),
                },
                None => (None, None),
            };
            let start = parse_pagination_cursor(cursor, &canonical_skills, "skills.list")?;
            if start > canonical_skills.len() {
                return Err(FunctionCallError::RespondToModel(
                    "skills.list cursor is invalid".to_string(),
                ));
            }
            let cursor_fingerprint = value_fingerprint(&canonical_skills);
            let cursor_at =
                |offset| format!("{cursor_fingerprint:016x}:{offset}:{response_byte_budget}");
            let mut skills = Vec::with_capacity(canonical_skills.len().saturating_sub(start));
            // The final retained skill needs no cursor, even if later entries are oversized.
            for (index, skill) in canonical_skills.iter().enumerate().skip(start).rev() {
                let next_cursor = (!skills.is_empty()).then(|| cursor_at(index.saturating_add(1)));
                if single_entry_response_is_bounded(skill, response_byte_budget, next_cursor) {
                    skills.push((index, skill));
                } else {
                    omitted_oversized_entry = true;
                }
            }
            skills.reverse();
            let mut provider_warnings = if args.cursor.is_none() {
                bounded_warnings(&catalog.warnings)
            } else {
                Vec::new()
            };
            let omission_warning = (omitted_oversized_entry
                && previous_byte_budget != Some(response_byte_budget))
            .then_some(OVERSIZED_ENTRY_WARNING);
            let mut end = MAX_SKILLS_PER_PAGE.min(skills.len());
            loop {
                let mut response = ListResponse {
                    skills: skills[..end]
                        .iter()
                        .map(|(_, skill)| (*skill).clone())
                        .collect(),
                    warnings: provider_warnings
                        .iter()
                        .cloned()
                        .chain(omission_warning.map(str::to_string))
                        .collect(),
                    next_cursor: (end < skills.len())
                        .then(|| cursor_at(skills[end.saturating_sub(1)].0.saturating_add(1))),
                };
                if serialized_len(&response)? <= response_byte_budget {
                    return skill_json_output(&response, args.authority);
                }
                if end > 1 {
                    end = end.saturating_sub(1);
                } else if provider_warnings.len() > 1 {
                    provider_warnings.pop();
                } else if !response.warnings.is_empty() {
                    response.skills.clear();
                    // Recording this budget acknowledges the omission warning, even when
                    // the next call resumes at the same canonical skill offset.
                    response.next_cursor = Some(cursor_at(start));
                    if serialized_len(&response)? <= response_byte_budget {
                        return skill_json_output(&response, args.authority);
                    }
                    if omission_warning.is_some() && !provider_warnings.is_empty() {
                        response.warnings = provider_warnings;
                        // Report discovery first without acknowledging an unshown omission.
                        // Reusing this offset lets the next call report it before advancing.
                        response.next_cursor = Some(format!("{cursor_fingerprint:016x}:{start}"));
                        if serialized_len(&response)? <= response_byte_budget {
                            return skill_json_output(&response, args.authority);
                        }
                    }
                    // FunctionCallError cannot carry external-context metadata, so do not
                    // expose provider warnings through this path.
                    return Err(FunctionCallError::RespondToModel(
                        "skills.list response budget leaves no room for discovery warnings"
                            .to_string(),
                    ));
                } else {
                    return Err(FunctionCallError::RespondToModel(
                        "skill metadata is too large to list".to_string(),
                    ));
                }
            }
        })
    }
}

fn single_entry_response_is_bounded(
    skill: &ListedSkill,
    response_byte_budget: usize,
    next_cursor: Option<String>,
) -> bool {
    serialized_len(&ListResponse {
        skills: vec![skill.clone()],
        warnings: Vec::new(),
        next_cursor,
    })
    .is_ok_and(|size| size <= response_byte_budget)
}

fn listed_skill(entry: SkillCatalogEntry) -> Option<ListedSkill> {
    let authority = SkillToolAuthority::from_authority(&entry.authority)?;
    if !is_bounded_handle(&entry.authority.id, MAX_HANDLE_BYTES)
        || !is_bounded_handle(&entry.id.0, MAX_HANDLE_BYTES)
        || !is_bounded_handle(entry.main_prompt.as_str(), MAX_HANDLE_BYTES)
    {
        return None;
    }

    Some(ListedSkill {
        authority,
        package: entry.id.0,
        name: truncate_utf8_to_bytes(&entry.name, MAX_SKILL_NAME_BYTES).0,
        description: truncate_catalog_skill_description(&entry.description).into_owned(),
        main_resource: entry.main_prompt.as_str().to_string(),
    })
}
