use std::sync::Arc;

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
use crate::provider::MAX_SKILL_RESOURCE_CONTENT_BYTES;
use crate::provider::SkillReadRequest;
use crate::render::build_alias_plan;
use crate::state::ExecutorReadSnapshot;

use super::MAX_HANDLE_BYTES;
use super::MAX_SKILL_RESPONSE_BYTES;
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
    #[serde(skip_serializing_if = "Option::is_none")]
    skill_root: Option<String>,
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
            "Read one page from a skill. Pass its provided package directly; root aliases are resolved automatically. Omit resource to read SKILL.md; to read another file, use the same package and pass the file's complete skill:// identifier as resource. For executor-backed skills, skill_root is the skill's absolute directory in the executor filesystem and can be used to locate bundled scripts. If the package is not provided, use skills.list to find it. Pass next_cursor back as cursor to continue the same snapshot while it is cached; omit cursor to read again.",
        )
    }

    fn handle(&self, call: ToolCall) -> ToolExecutorFuture<'_> {
        Box::pin(async move {
            let args: ReadArgs = parse_args(&call)?;
            let response_byte_budget = call.response_byte_budget(MAX_SKILL_RESPONSE_BYTES);
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
                let alias_plan = build_alias_plan(
                    &catalog
                        .entries
                        .iter()
                        .filter(|entry| entry.is_model_visible())
                        .collect::<Vec<_>>(),
                );
                if let Some(entry) = catalog.entries.into_iter().find(|entry| {
                    entry.enabled
                        && (entry.id.0 == args.package
                            || alias_plan
                                .as_ref()
                                .and_then(|plan| plan.shorten(&entry.id.0))
                                .is_some_and(|alias| alias == args.package))
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
            let executor_environment = resolved_executor_roots
                .iter()
                .find(|root| root.selected_root().id == authority.id)
                .map(|root| Arc::downgrade(root.environment()));
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
            // Reuse the snapshot to avoid reading the whole file for each page. File edits
            // are not detected on cache hits, so a continuation can return old contents.
            // Snapshots have no expiry and are not cleared after the final page.
            let cached = args.cursor.as_deref().and_then(|cursor| {
                let environment = executor_environment.as_ref()?;
                let snapshot = self
                    .context
                    .thread_state
                    .executor_read_snapshot
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let snapshot = snapshot.as_ref().filter(|snapshot| {
                    snapshot.authority == authority
                        && snapshot.package == package
                        && snapshot.result.resource == requested_resource
                        && snapshot.environment.ptr_eq(environment)
                        && snapshot.sandbox == sandbox
                })?;
                let start = parse_pagination_cursor(
                    Some(cursor),
                    snapshot.result.contents.as_str(),
                    "skills.read",
                )
                .ok()?;
                Some((Arc::clone(&snapshot.result), start))
            });
            let (result, start) = match cached {
                Some(cached) => cached,
                None => {
                    let result = self
                        .context
                        .thread_state
                        .read_skill(
                            &self.context.providers,
                            SkillReadRequest {
                                authority: authority.clone(),
                                package: package.clone(),
                                resource: requested_resource.clone(),
                                resolved_executor_roots,
                                sandbox: sandbox.clone(),
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
                            FunctionCallError::RespondToModel(
                                "failed to read skill resource".to_string(),
                            )
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
                    (Arc::new(result), start)
                }
            };
            if start > result.contents.len() || !result.contents.is_char_boundary(start) {
                return Err(FunctionCallError::RespondToModel(
                    "skills.read cursor is invalid".to_string(),
                ));
            }
            let skill_root = if output_authority == super::SkillToolAuthoritySelector::Executor {
                main_prompt
                    .environment_path()
                    .and_then(|(_, path)| path.parent())
                    .map(|path| path.inferred_native_path_string())
            } else {
                None
            };
            let response = page_response(
                result.resource.as_str(),
                &result.contents,
                skill_root.as_deref(),
                start,
                response_byte_budget,
            )?;
            let output = skill_json_output(&response, output_authority)?;
            if output_authority == super::SkillToolAuthoritySelector::Executor
                && response.next_cursor.is_some()
                && result.contents.len() <= MAX_SKILL_RESOURCE_CONTENT_BYTES
                && let Some(environment) = executor_environment
            {
                *self
                    .context
                    .thread_state
                    .executor_read_snapshot
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) =
                    Some(ExecutorReadSnapshot {
                        authority,
                        package,
                        environment,
                        sandbox,
                        result,
                    });
            }

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
    skill_root: Option<&str>,
    start: usize,
    max_response_bytes: usize,
) -> Result<ReadResponse, FunctionCallError> {
    let response = |end, next_cursor| ReadResponse {
        resource: resource.to_string(),
        contents: contents[start..end].to_string(),
        skill_root: skill_root.map(str::to_string),
        next_cursor,
    };
    let complete = response(contents.len(), None);
    if serialized_len(&complete)? <= max_response_bytes {
        return Ok(complete);
    }

    let mut lower = start;
    let mut upper = contents.len();
    let mut best = None;
    while lower < upper {
        // Probe strictly above lower so a multibyte character cannot stall the search.
        let end = contents.ceil_char_boundary(lower.midpoint(upper).saturating_add(1));
        let candidate = response(end, Some(pagination_cursor(contents, end)));
        if serialized_len(&candidate)? <= max_response_bytes {
            lower = end;
            best = Some(candidate);
        } else {
            upper = contents.floor_char_boundary(end.saturating_sub(1));
        }
    }
    best.ok_or_else(|| {
        FunctionCallError::RespondToModel(
            "skills.read response budget leaves no room for contents".to_string(),
        )
    })
}
