use codex_extension_api::ToolCall;
use codex_extension_api::ToolExecutor;
use codex_extension_api::ToolExecutorFuture;
use codex_extension_api::ToolName;
use codex_extension_api::ToolSpec;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;

use crate::render::truncate_catalog_skill_description;
use crate::render::truncate_utf8_to_bytes;
use codex_core_skills::runtime::SkillCatalogEntry;

use super::MAX_HANDLE_BYTES;
use super::SkillToolAuthority;
use super::SkillToolContext;
use super::external_json_output;
use super::is_bounded_handle;
use super::parse_args;
use super::skill_function_tool;
use super::skill_tool_name;

const TOOL_NAME: &str = "list";
const MAX_WARNINGS: usize = 4;
const MAX_WARNING_BYTES: usize = 256;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ListArgs {
    authority: SkillToolAuthority,
}

#[derive(Debug, Eq, JsonSchema, PartialEq, Serialize)]
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
            "List enabled skills owned by the requested authority. Only orchestrator-owned skills are currently supported. Returns the opaque package and main-resource handles required by skills.read.",
        )
    }

    fn handle(&self, call: ToolCall) -> ToolExecutorFuture<'_> {
        Box::pin(async move {
            let args: ListArgs = parse_args(&call)?;
            let authority = args.authority.into_authority();
            let source = self.context.source();
            let catalog = self.context.catalog(source.as_ref()).await;
            let response = ListResponse {
                skills: catalog
                    .entries
                    .into_iter()
                    .filter(|entry| entry.enabled && entry.authority == authority)
                    .filter_map(listed_skill)
                    .collect(),
                warnings: bounded_warnings(catalog.warnings),
            };

            external_json_output(&response)
        })
    }
}

fn listed_skill(entry: SkillCatalogEntry) -> Option<ListedSkill> {
    let authority = SkillToolAuthority::from_authority(&entry.authority)?;
    if !is_bounded_handle(&entry.id.0, MAX_HANDLE_BYTES)
        || !is_bounded_handle(entry.main_prompt.as_str(), MAX_HANDLE_BYTES)
    {
        return None;
    }

    Some(ListedSkill {
        authority,
        package: entry.id.0,
        name: entry.name,
        description: truncate_catalog_skill_description(&entry.description).into_owned(),
        main_resource: entry.main_prompt.as_str().to_string(),
    })
}

fn bounded_warnings(warnings: Vec<String>) -> Vec<String> {
    warnings
        .into_iter()
        .take(MAX_WARNINGS)
        .map(|warning| {
            let (warning, _) = truncate_utf8_to_bytes(&warning, MAX_WARNING_BYTES);
            warning
        })
        .collect()
}
