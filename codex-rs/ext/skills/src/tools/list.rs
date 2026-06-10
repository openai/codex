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
use crate::render::truncate_utf8_to_bytes;

use super::MAX_HANDLE_BYTES;
use super::MAX_TOOL_OUTPUT_BYTES;
use super::SkillAuthorityHandle;
use super::SkillToolContext;
use super::external_json_output;
use super::is_bounded_handle;
use super::parse_args;
use super::skill_function_tool;
use super::skill_tool_name;

const TOOL_NAME: &str = "list";
const MAX_CATALOG_SKILLS: usize = 100;
const MAX_PAGE_SKILLS: usize = 20;
const MAX_DESCRIPTION_BYTES: usize = 512;
const MAX_WARNINGS: usize = 4;
const MAX_WARNING_BYTES: usize = 256;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ListArgs {
    cursor: Option<String>,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize)]
#[schemars(deny_unknown_fields)]
struct ListedSkill {
    authority: SkillAuthorityHandle,
    package: String,
    name: String,
    description: String,
    main_resource: String,
}

#[derive(Debug, Eq, JsonSchema, PartialEq, Serialize)]
#[schemars(deny_unknown_fields)]
struct ListResponse {
    skills: Vec<ListedSkill>,
    next_cursor: Option<String>,
    warnings: Vec<String>,
    truncated: bool,
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
            "List enabled remote skills and the opaque authority, package, and main-resource handles required by skills.read.",
        )
    }

    fn handle(&self, call: ToolCall) -> ToolExecutorFuture<'_> {
        Box::pin(async move {
            let args: ListArgs = parse_args(&call)?;
            let catalog = self.context.catalog(&call.turn_id).await;
            let (warnings, warnings_truncated) = bounded_warnings(catalog.warnings);

            let mut metadata_truncated = false;
            let mut skills = Vec::new();
            for entry in catalog.entries.into_iter().filter(|entry| entry.enabled) {
                let Some((skill, truncated)) = listed_skill(entry) else {
                    metadata_truncated = true;
                    continue;
                };
                metadata_truncated |= truncated;
                if skills.len() >= MAX_CATALOG_SKILLS {
                    metadata_truncated = true;
                    continue;
                }
                skills.push(skill);
            }

            let offset = args
                .cursor
                .as_deref()
                .unwrap_or("0")
                .parse::<usize>()
                .map_err(|_| {
                    FunctionCallError::RespondToModel("invalid skills.list cursor".to_string())
                })?;
            if offset > skills.len() {
                return Err(FunctionCallError::RespondToModel(
                    "skills.list cursor is out of range".to_string(),
                ));
            }
            let mut base_truncated = metadata_truncated || warnings_truncated;
            let mut response = ListResponse {
                skills: Vec::new(),
                next_cursor: None,
                warnings,
                truncated: base_truncated,
            };
            let mut index = offset;
            while index < skills.len() && response.skills.len() < MAX_PAGE_SKILLS {
                response.skills.push(skills[index].clone());
                response.next_cursor = (index + 1 < skills.len()).then(|| (index + 1).to_string());
                response.truncated = base_truncated || response.next_cursor.is_some();
                if serialized_len(&response)? > MAX_TOOL_OUTPUT_BYTES {
                    response.skills.pop();
                    if response.skills.is_empty() {
                        index += 1;
                        base_truncated = true;
                        response.next_cursor = (index < skills.len()).then(|| index.to_string());
                        response.truncated = true;
                        continue;
                    }
                    response.next_cursor = Some(index.to_string());
                    response.truncated = true;
                    break;
                }
                index += 1;
            }
            if index < skills.len() && response.next_cursor.is_none() {
                response.next_cursor = Some(index.to_string());
            }
            response.truncated = base_truncated || response.next_cursor.is_some();

            external_json_output(&response)
        })
    }
}

fn listed_skill(entry: SkillCatalogEntry) -> Option<(ListedSkill, bool)> {
    let authority = SkillAuthorityHandle::from_authority(&entry.authority)?;
    if !authority.is_bounded()
        || !is_bounded_handle(&entry.id.0, MAX_HANDLE_BYTES)
        || !is_bounded_handle(entry.main_prompt.as_str(), MAX_HANDLE_BYTES)
    {
        return None;
    }

    let (name, name_truncated) = truncate_utf8_to_bytes(&entry.name, MAX_SKILL_NAME_BYTES);
    let (description, description_truncated) =
        truncate_utf8_to_bytes(&entry.description, MAX_DESCRIPTION_BYTES);
    Some((
        ListedSkill {
            authority,
            package: entry.id.0,
            name,
            description,
            main_resource: entry.main_prompt.as_str().to_string(),
        },
        name_truncated || description_truncated,
    ))
}

fn bounded_warnings(warnings: Vec<String>) -> (Vec<String>, bool) {
    let mut truncated = warnings.len() > MAX_WARNINGS;
    let warnings = warnings
        .into_iter()
        .take(MAX_WARNINGS)
        .map(|warning| {
            let (warning, warning_truncated) = truncate_utf8_to_bytes(&warning, MAX_WARNING_BYTES);
            truncated |= warning_truncated;
            warning
        })
        .collect();
    (warnings, truncated)
}

fn serialized_len(response: &ListResponse) -> Result<usize, FunctionCallError> {
    serde_json::to_vec(response)
        .map(|value| value.len())
        .map_err(|err| {
            FunctionCallError::Fatal(format!("failed to serialize skills.list output: {err}"))
        })
}
