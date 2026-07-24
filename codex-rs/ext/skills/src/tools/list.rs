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
use super::SkillToolAuthority;
use super::SkillToolAuthoritySelector;
use super::SkillToolContext;
use super::is_bounded_handle;
use super::pagination_cursor;
use super::parse_args;
use super::parse_pagination_cursor;
use super::serialized_len;
use super::skill_function_tool;
use super::skill_json_output;
use super::skill_tool_name;

const TOOL_NAME: &str = "list";
const MAX_SKILLS_PER_PAGE: usize = 20;
const MAX_LIST_RESPONSE_BYTES: usize = 512 * 1024;
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
            "List skills owned by the requested authority. Returns the exact authority, package, and main_resource values required by skills.read. Pass next_cursor back as cursor to continue.",
        )
    }

    fn handle(&self, call: ToolCall) -> ToolExecutorFuture<'_> {
        Box::pin(async move {
            let args: ListArgs = parse_args(&call)?;
            let catalog = self.context.catalog(&call.turn_id, args.authority).await;
            let mut omitted_oversized_entry = false;
            let skills = catalog
                .entries
                .into_iter()
                .filter(|entry| {
                    entry.is_model_visible() && args.authority.matches(&entry.authority)
                })
                .filter_map(|entry| {
                    let listed = listed_skill(entry).filter(single_entry_response_is_bounded);
                    omitted_oversized_entry |= listed.is_none();
                    listed
                })
                .collect::<Vec<_>>();
            let start = parse_pagination_cursor(args.cursor.as_deref(), &skills, "skills.list")?;
            if start > skills.len() {
                return Err(FunctionCallError::RespondToModel(
                    "skills.list cursor is invalid".to_string(),
                ));
            }
            let mut warnings = if start == 0 {
                let mut warnings = catalog.warnings;
                if omitted_oversized_entry {
                    warnings.push(OVERSIZED_ENTRY_WARNING.to_string());
                }
                bounded_warnings(&warnings)
            } else {
                Vec::new()
            };
            let mut end = (start + MAX_SKILLS_PER_PAGE).min(skills.len());
            loop {
                let response = ListResponse {
                    skills: skills[start..end].to_vec(),
                    warnings: warnings.clone(),
                    next_cursor: (end < skills.len()).then(|| pagination_cursor(&skills, end)),
                };
                if serialized_len(&response)? <= MAX_LIST_RESPONSE_BYTES {
                    return skill_json_output(&response, args.authority);
                }
                if end.saturating_sub(start) > 1 {
                    end -= 1;
                } else if !warnings.is_empty() {
                    warnings.clear();
                } else {
                    return Err(FunctionCallError::RespondToModel(
                        "skill metadata is too large to list".to_string(),
                    ));
                }
            }
        })
    }
}

fn single_entry_response_is_bounded(skill: &ListedSkill) -> bool {
    serialized_len(&ListResponse {
        skills: vec![skill.clone()],
        warnings: Vec::new(),
        next_cursor: Some(pagination_cursor(skill, usize::MAX)),
    })
    .is_ok_and(|size| size <= MAX_LIST_RESPONSE_BYTES)
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
