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

use crate::aliases::SkillLocator;
use crate::catalog::SkillResourceId;
use crate::catalog::SkillSourceKind;
use crate::provider::MAX_SKILL_RESOURCE_CONTENT_BYTES;
use crate::provider::SkillReadContext;
use crate::provider::SkillReadRequest;
use crate::state::ExecutorReadSnapshot;

use super::InstallationLocation;
use super::InstallationSource;
use super::MAX_HANDLE_BYTES;
use super::MAX_SKILL_RESPONSE_BYTES;
use super::SKILLS_NAMESPACE;
use super::SkillToolAuthority;
use super::SkillToolAuthoritySelector;
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
const MAX_INSTALLATION_METADATA_BYTES: usize = 1024;

#[derive(Deserialize, JsonSchema)]
struct ReadArgs {
    #[schemars(with = "String")]
    package: SkillLocator,
    resource: Option<String>,
    cursor: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
struct ReadInstallationArgs {
    #[serde(flatten)]
    read: ReadArgs,
    source: Option<InstallationSource>,
}

#[derive(Debug, Eq, JsonSchema, PartialEq, Serialize)]
#[schemars(deny_unknown_fields)]
struct ReadResponse<M> {
    resource: String,
    contents: String,
    #[serde(flatten)]
    metadata: M,
    next_cursor: Option<String>,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize)]
struct LegacyReadMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    skill_root: Option<String>,
}

#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize)]
struct InstallationReadMetadata {
    installations: Vec<InstallationLocation>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    installations_truncated: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(untagged)]
enum ReadMetadata {
    Legacy(LegacyReadMetadata),
    Installations(InstallationReadMetadata),
}

#[derive(Clone)]
pub(super) struct ReadTool {
    pub(super) context: SkillToolContext,
}

impl<'call> ToolExecutor<ToolCall<'call>> for ReadTool {
    fn tool_name(&self) -> ToolName {
        skill_tool_name(TOOL_NAME)
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        true
    }

    fn spec(&self) -> ToolSpec {
        if !self.context.step_state.discovery_v2 {
            return skill_function_tool::<ReadArgs, ReadResponse<LegacyReadMetadata>>(
                TOOL_NAME,
                "Read one page from a skill. Pass its provided package directly; root aliases are resolved automatically. Omit resource to read SKILL.md; to read another file, use the same package and pass the file's complete skill:// identifier as resource. For executor-backed skills, skill_root is the skill's absolute directory in the executor filesystem and can be used to locate bundled scripts. If the package is not provided, use skills.list to find it. Pass next_cursor back as cursor to continue the same snapshot while it is cached; omit cursor to read again.",
            );
        }
        skill_function_tool::<ReadInstallationArgs, ReadResponse<InstallationReadMetadata>>(
            TOOL_NAME,
            "Read one page from a skill. Pass its provided package directly; root aliases are resolved automatically. Omit resource to read SKILL.md; to read another file, use the same package and pass the file's complete skill:// identifier as resource. Use source {kind: cloud} for Cloud or {kind: executor, environment_id: ...} for one environment. Omit source to read the first matching location. The installations list contains Cloud locators (package and resource, passed directly to skills.read) or executor locations (environment_id and skill_root, the absolute directory containing instructions and supporting files such as scripts). If installations_truncated is true, use skills.installations with the same package to retrieve all locations. If the package is not provided, use skills.list to find it. Pass next_cursor back as cursor with the same package, source, and resource to continue the same snapshot while it is cached; omit cursor to read again.",
        )
    }

    fn handle<'a>(&'a self, call: ToolCall<'call>) -> ToolExecutorFuture<'a>
    where
        'call: 'a,
    {
        Box::pin(async move {
            let (args, source) = if self.context.step_state.discovery_v2 {
                let args: ReadInstallationArgs = parse_args(&call)?;
                (args.read, args.source)
            } else {
                (parse_args::<ReadArgs>(&call)?, None)
            };
            if let Some(source) = &source {
                source.validate()?;
            }
            let response_byte_budget = call.response_byte_budget(MAX_SKILL_RESPONSE_BYTES);
            validate_handle("package", args.package.as_str(), MAX_HANDLE_BYTES)?;
            if let Some(resource) = args.resource.as_deref() {
                validate_handle("resource", resource, MAX_HANDLE_BYTES)?;
            }

            let matches = self.context.matching_packages(&args.package);
            let (skill_entry, selected_source, output_authority) = matches
                .iter()
                .find_map(|(entry, matched_source)| {
                    let selected_source = match &source {
                        None => matched_source.clone(),
                        Some(filter) => std::iter::once(matched_source.clone())
                            .chain(entry.source_locations())
                            .find(|candidate| filter.matches(candidate))?,
                    };
                    let authority =
                        SkillToolAuthority::from_authority(&selected_source.authority)?.selector();
                    Some((*entry, selected_source, authority))
                })
                .ok_or_else(|| {
                    FunctionCallError::RespondToModel(if source.is_some() {
                        "skill package is not available from the requested source".to_string()
                    } else {
                        "skill package is not available".to_string()
                    })
                })?;
            let authority = selected_source.authority.clone();
            let package = selected_source.package.clone();
            let main_prompt = selected_source.main_prompt.clone();
            let requested_resource = match args.resource {
                None => main_prompt.clone(),
                Some(resource) if resource == main_prompt.as_str() => main_prompt.clone(),
                Some(resource) => main_prompt
                    .bind_environment_package_resource(&package, resource.clone())
                    .unwrap_or_else(|| SkillResourceId::new(resource)),
            };
            // Record the selected route before either cache or provider can serve the read.
            // This is an attempted read, not a successful invocation or a backend RPC.
            tracing::event!(
                name: "codex.skill_read.source_selected",
                target: "codex_otel.trace_safe",
                tracing::Level::INFO,
                event.name = "codex.skill_read.source_selected",
                conversation.id = %self.context.thread_id,
                turn_id = %call.turn_id,
                call_id = %call.call_id,
                tool_namespace = SKILLS_NAMESPACE,
                tool_name = TOOL_NAME,
                skill_source = match output_authority {
                    SkillToolAuthoritySelector::Cloud => "cloud",
                    SkillToolAuthoritySelector::Executor => "executor",
                },
                skill_name = %skill_entry.name,
                skill_plugin_id = selected_source.plugin_id.as_deref(),
                skill_resource_kind = if requested_resource == main_prompt {
                    "main_prompt"
                } else {
                    "supporting_resource"
                },
                is_continuation = args.cursor.is_some(),
                "Selected skill read source"
            );
            let (context, access_key) = match output_authority {
                SkillToolAuthoritySelector::Cloud => (
                    SkillReadContext::Cloud {
                        mcp_resources: self.context.mcp_resources.clone(),
                    },
                    None,
                ),
                SkillToolAuthoritySelector::Executor => {
                    let fs = requested_resource
                        .environment_path()
                        .and_then(|(id, _)| {
                            call.environments
                                .iter()
                                .find(|env| env.environment_id == id)
                        })
                        .map(codex_extension_api::ToolEnvironment::fs)
                        .ok_or_else(|| {
                            FunctionCallError::RespondToModel(
                                "skill environment is not available for this callback".to_string(),
                            )
                        })?;
                    (SkillReadContext::Executor { fs }, Some(fs.cache_key()))
                }
            };
            let cursor_start = |contents: &str| {
                if self.context.step_state.discovery_v2 {
                    parse_pagination_cursor(
                        args.cursor.as_deref(),
                        &(&source, &package, contents),
                        "skills.read",
                    )
                } else {
                    parse_pagination_cursor(args.cursor.as_deref(), contents, "skills.read")
                }
            };
            // Reuse the snapshot to avoid reading the whole file for each page. File edits
            // are not detected on cache hits, so a continuation can return old contents.
            // Snapshots have no expiry and are not cleared after the final page.
            let cached = args.cursor.as_deref().and_then(|_| {
                let access = access_key.as_ref()?;
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
                        && snapshot.access == *access
                })?;
                let start = cursor_start(&snapshot.result.contents).ok()?;
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
                                context,
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
                    let start = cursor_start(&result.contents)?;
                    (Arc::new(result), start)
                }
            };
            if start > result.contents.len() || !result.contents.is_char_boundary(start) {
                return Err(FunctionCallError::RespondToModel(
                    "skills.read cursor is invalid".to_string(),
                ));
            }
            // Shadow candidates use the merged identity, not the selected read route.
            if skill_entry.authority.kind == SkillSourceKind::Cloud
                && let Some(state) = self
                    .context
                    .thread_state
                    .shadow_selection_turn(&call.turn_id)
            {
                state.record_invocation(skill_entry.main_prompt.as_str());
            }
            let metadata = if self.context.step_state.discovery_v2 {
                let mut installations = Vec::new();
                let mut installations_truncated = false;
                let mut installation_bytes = 2; // JSON array brackets.
                let installation_budget =
                    MAX_INSTALLATION_METADATA_BYTES.min(response_byte_budget / 4);
                // Prefer the installation used by this read before other sources fill the budget.
                for source in
                    std::iter::once(selected_source.clone()).chain(skill_entry.source_locations())
                {
                    let Some(installation) = InstallationLocation::from_source(&source) else {
                        continue;
                    };
                    if installations.contains(&installation) {
                        continue;
                    }
                    let size =
                        serialized_len(&installation)? + usize::from(!installations.is_empty());
                    if installation_bytes + size > installation_budget {
                        installations_truncated = true;
                        continue;
                    }
                    installation_bytes += size;
                    installations.push(installation);
                }
                installations_truncated |= matches.iter().any(|(entry, _)| {
                    entry
                        .source_locations()
                        .filter_map(|source| InstallationLocation::from_source(&source))
                        .any(|location| !installations.contains(&location))
                });
                ReadMetadata::Installations(InstallationReadMetadata {
                    installations,
                    installations_truncated,
                })
            } else {
                let skill_root = if output_authority == SkillToolAuthoritySelector::Executor {
                    main_prompt
                        .environment_path()
                        .and_then(|(_, path)| path.parent())
                        .map(|path| path.inferred_native_path_string())
                } else {
                    None
                };
                ReadMetadata::Legacy(LegacyReadMetadata { skill_root })
            };
            let response = page_response(
                result.resource.as_str(),
                &result.contents,
                &metadata,
                start,
                response_byte_budget,
                |offset| {
                    if self.context.step_state.discovery_v2 {
                        pagination_cursor(&(&source, &package, result.contents.as_str()), offset)
                    } else {
                        pagination_cursor(result.contents.as_str(), offset)
                    }
                },
            )?;
            let response_authority = if matches!(
                &response.metadata,
                ReadMetadata::Installations(metadata)
                    if metadata.installations.iter().any(|location| {
                        matches!(location, InstallationLocation::Cloud { .. })
                    })
            ) {
                SkillToolAuthoritySelector::Cloud
            } else {
                output_authority
            };
            let output = skill_json_output(&response, response_authority)?;
            if output_authority == super::SkillToolAuthoritySelector::Executor
                && response.next_cursor.is_some()
                && result.contents.len() <= MAX_SKILL_RESOURCE_CONTENT_BYTES
                && let Some(access) = access_key
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
                        access,
                        result,
                    });
            }

            if requested_resource == main_prompt
                && args.cursor.is_none()
                && let Some(analytics) = self.context.analytics.as_ref()
            {
                analytics.track_skill_invocation(
                    skill_entry,
                    &selected_source,
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
    metadata: &ReadMetadata,
    start: usize,
    max_response_bytes: usize,
    cursor_at: impl Fn(usize) -> String,
) -> Result<ReadResponse<ReadMetadata>, FunctionCallError> {
    let response = |end, next_cursor| ReadResponse {
        resource: resource.to_string(),
        contents: contents[start..end].to_string(),
        metadata: metadata.clone(),
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
        let candidate = response(end, Some(cursor_at(end)));
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
