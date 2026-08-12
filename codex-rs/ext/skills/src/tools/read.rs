use codex_analytics::InvocationType;
use codex_extension_api::FunctionCallError;
use codex_extension_api::ToolCall;
use codex_extension_api::ToolExecutor;
use codex_extension_api::ToolExecutorFuture;
use codex_extension_api::ToolName;
use codex_extension_api::ToolSpec;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;

use crate::catalog::SkillResourceId;
use crate::provider::SkillReadRequest;

use super::MAX_HANDLE_BYTES;
use super::SkillToolAuthority;
use super::SkillToolContext;
use super::pagination_cursor;
use super::parse_args;
use super::parse_pagination_cursor;
use super::serialized_len;
use super::skill_function_tool;
use super::skill_json_output;
use super::skill_tool_name;
use super::validate_handle;

const TOOL_NAME: &str = "read";
const MAX_READ_RESPONSE_BYTES: usize = 512 * 1024;

#[derive(Deserialize, JsonSchema)]
struct ReadArgs {
    package: String,
    resource: Option<String>,
    cursor: Option<String>,
}

#[derive(Debug, Eq, JsonSchema, PartialEq, Serialize)]
#[schemars(deny_unknown_fields)]
struct ReadResponse {
    resource: String,
    contents: String,
    next_cursor: Option<String>,
}

#[derive(Clone)]
pub(super) struct ReadTool {
    pub(super) context: SkillToolContext,
}

impl ToolExecutor<ToolCall> for ReadTool {
    fn tool_name(&self) -> ToolName {
        skill_tool_name(TOOL_NAME)
    }

    fn spec(&self) -> ToolSpec {
        skill_function_tool::<ReadArgs, ReadResponse>(
            TOOL_NAME,
            "Read one page from a skill. Pass its provided package, expanding any root alias. Omit resource to read SKILL.md; to read another file, use the same package and pass the file's complete skill:// identifier as resource. If the package is not provided, use skills.list to find it. Pass next_cursor back as cursor to continue.",
        )
    }

    fn handle(&self, call: ToolCall) -> ToolExecutorFuture<'_> {
        Box::pin(async move {
            let args: ReadArgs = parse_args(&call)?;
            validate_handle("package", &args.package, MAX_HANDLE_BYTES)?;
            if let Some(resource) = args.resource.as_deref() {
                validate_handle("resource", resource, MAX_HANDLE_BYTES)?;
            }

            let mut selected_skill = None;
            for selector in [
                super::SkillToolAuthoritySelector::Orchestrator,
                super::SkillToolAuthoritySelector::Executor,
            ] {
                let catalog = self.context.catalog(&call.turn_id, selector).await;
                if let Some(entry) = catalog.entries.into_iter().find(|entry| {
                    entry.enabled
                        && entry.id.0 == args.package
                        && SkillToolAuthority::from_authority(&entry.authority)
                            .is_some_and(|authority| authority.selector() == selector)
                }) {
                    selected_skill = Some((entry, selector));
                    break;
                }
            }
            let Some((skill_entry, output_authority)) = selected_skill else {
                return Err(FunctionCallError::RespondToModel(
                    "skill package is not available".to_string(),
                ));
            };
            let authority = skill_entry.authority.clone();
            let package = skill_entry.id.clone();
            let main_prompt = skill_entry.main_prompt.clone();
            let requested_resource = match args.resource {
                None => main_prompt.clone(),
                Some(resource) if resource == main_prompt.as_str() => main_prompt.clone(),
                Some(resource) => main_prompt
                    .bind_environment_package_resource(&package, resource.clone())
                    .unwrap_or_else(|| SkillResourceId::new(resource)),
            };
            let resolved_executor_roots = self
                .context
                .executor_query
                .as_ref()
                .map(|query| query.resolved_executor_roots.clone())
                .unwrap_or_default();
            let sandbox = requested_resource
                .environment_path()
                .and_then(|(environment_id, _)| {
                    self.context.sandbox_contexts.as_ref().and_then(|contexts| {
                        contexts.get(environment_id).map(|captured| {
                            call.environments
                                .iter()
                                .find(|environment| environment.environment_id == environment_id)
                                .map(|environment| environment.file_system_sandbox_context.clone())
                                .unwrap_or_else(|| captured.clone())
                        })
                    })
                });
            if self.context.sandbox_contexts.is_some()
                && requested_resource.environment_path().is_some()
                && sandbox.is_none()
            {
                return Err(FunctionCallError::RespondToModel(
                    "failed to read skill resource".to_string(),
                ));
            }
            let result = self
                .context
                .thread_state
                .read_skill(
                    &self.context.providers,
                    SkillReadRequest {
                        authority,
                        package,
                        resource: requested_resource.clone(),
                        resolved_executor_roots,
                        sandbox,
                        host_snapshot: None,
                        mcp_resources: self.context.mcp_resources.clone(),
                    },
                )
                .await
                .map_err(|err| {
                    tracing::warn!(
                        error = %err,
                        turn_id = %call.turn_id,
                        call_id = %call.call_id,
                        resource = requested_resource.as_str(),
                        "skills.read provider request failed"
                    );
                    FunctionCallError::RespondToModel("failed to read skill resource".to_string())
                })?;
            if result.resource != requested_resource {
                return Err(FunctionCallError::Fatal(
                    "skill provider returned a different resource".to_string(),
                ));
            }
            if output_authority == super::SkillToolAuthoritySelector::Orchestrator
                && let Some(state) = self
                    .context
                    .thread_state
                    .shadow_selection_turn(&call.turn_id)
            {
                self.context
                    .shadow_selection
                    .record_invocation(&state, main_prompt.as_str());
            }

            let start = parse_pagination_cursor(
                args.cursor.as_deref(),
                result.contents.as_str(),
                "skills.read",
            )?;
            if start > result.contents.len() || !result.contents.is_char_boundary(start) {
                return Err(FunctionCallError::RespondToModel(
                    "skills.read cursor is invalid".to_string(),
                ));
            }
            let response = page_response(result.resource.as_str(), &result.contents, start)?;
            let output = skill_json_output(&response, output_authority)?;

            if requested_resource == main_prompt
                && args.cursor.is_none()
                && let Some(analytics) = self.context.analytics.as_ref()
            {
                analytics.track_skill_invocation(
                    &skill_entry,
                    call.model.clone(),
                    call.turn_id.clone(),
                    InvocationType::Implicit,
                );
            }

            Ok(output)
        })
    }
}

fn page_response(
    resource: &str,
    contents: &str,
    start: usize,
) -> Result<ReadResponse, FunctionCallError> {
    let response = |end, next_cursor| ReadResponse {
        resource: resource.to_string(),
        contents: contents[start..end].to_string(),
        next_cursor,
    };
    let complete = response(contents.len(), None);
    if serialized_len(&complete)? <= MAX_READ_RESPONSE_BYTES {
        return Ok(complete);
    }

    let mut end = contents.len();
    while end > start {
        end = start + (end - start) / 2;
        while !contents.is_char_boundary(end) {
            end -= 1;
        }
        let candidate = response(end, Some(pagination_cursor(contents, end)));
        if serialized_len(&candidate)? <= MAX_READ_RESPONSE_BYTES {
            return Ok(candidate);
        }
    }
    Err(FunctionCallError::Fatal(
        "skill resource handle leaves no room for contents".to_string(),
    ))
}
