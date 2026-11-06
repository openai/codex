//! Retrieves installation locations from the prepared catalog without reading skill contents.

use std::collections::HashSet;
use std::sync::Arc;

use codex_extension_api::ExtensionMetrics;
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

use super::InstallationLocation;
use super::InstallationSource;
use super::MAX_HANDLE_BYTES;
use super::SkillToolAuthoritySelector;
use super::SkillToolContext;
use super::parse_args;
use super::parse_pagination_cursor;
use super::serialized_len;
use super::skill_function_tool;
use super::skill_json_output;
use super::skill_tool_name;
use super::validate_handle;
use super::value_fingerprint;

const TOOL_NAME: &str = "installations";
const MAX_INSTALLATIONS_RESPONSE_BYTES: usize = 8 * 1024;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct InstallationsArgs {
    #[schemars(with = "String")]
    package: SkillLocator,
    source: Option<InstallationSource>,
    cursor: Option<String>,
}

#[derive(JsonSchema, Serialize)]
#[schemars(deny_unknown_fields)]
struct InstallationsResponse {
    installations: Vec<InstallationLocation>,
    next_cursor: Option<String>,
}

pub(super) struct InstallationsTool {
    pub(super) context: SkillToolContext,
    pub(super) metrics: Option<Arc<dyn ExtensionMetrics>>,
}

impl<'call> ToolExecutor<ToolCall<'call>> for InstallationsTool {
    fn tool_name(&self) -> ToolName {
        skill_tool_name(TOOL_NAME)
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        true
    }

    fn spec(&self) -> ToolSpec {
        skill_function_tool::<InstallationsArgs, InstallationsResponse>(
            TOOL_NAME,
            "Find a skill's enabled Cloud and executor locations, including those omitted by skills.read. Pass its provided package directly; root aliases are resolved automatically. Use source {kind: cloud} for Cloud or {kind: executor, environment_id: ...} for one environment. Omit source for all locations. Cloud locations return package and resource: pass both directly to skills.read to read the main instruction file, usually SKILL.md. Executor locations return environment_id and skill_root, the absolute directory containing instructions and supporting files such as scripts. Pass next_cursor back as cursor with the same package and source to continue listing locations. Omit cursor to start again. An empty list without next_cursor means no enabled location matches.",
        )
    }

    fn handle<'a>(&'a self, call: ToolCall<'call>) -> ToolExecutorFuture<'a>
    where
        'call: 'a,
    {
        Box::pin(async move {
            let mut outcome = "internal_error";
            let mut source_filter = "unparsed";
            let result = (|| {
                let args: InstallationsArgs =
                    parse_args(&call).inspect_err(|_| outcome = "invalid_arguments")?;
                source_filter = match &args.source {
                    None => "all",
                    Some(InstallationSource::Cloud) => "cloud",
                    Some(InstallationSource::Executor { .. }) => "executor",
                };
                validate_handle("package", args.package.as_str(), MAX_HANDLE_BYTES)
                    .inspect_err(|_| outcome = "invalid_package")?;
                if let Some(source) = &args.source {
                    source
                        .validate()
                        .inspect_err(|_| outcome = "invalid_source")?;
                }
                let matches = self
                    .context
                    .matching_packages(&args.package)
                    .inspect_err(|_| outcome = "ambiguous_name")?;
                let (entry, _) = matches.first().ok_or_else(|| {
                    outcome = if args.package.alias_source().is_some()
                        && self
                            .context
                            .step_state
                            .catalog
                            .aliases
                            .resolve(&args.package)
                            .is_none()
                    {
                        "unresolved_alias"
                    } else {
                        "package_unavailable"
                    };
                    FunctionCallError::RespondToModel("skill package is not available".to_string())
                })?;
                let mut installations = Vec::new();
                let mut seen = HashSet::new();
                for installation in matches
                    .iter()
                    .flat_map(|(entry, matched_source)| {
                        std::iter::once(matched_source.clone()).chain(entry.source_locations())
                    })
                    .filter(|source| {
                        args.source
                            .as_ref()
                            .is_none_or(|filter| filter.matches(source))
                    })
                    .filter_map(|source| InstallationLocation::from_source(&source))
                {
                    if seen.insert(installation.clone()) {
                        installations.push(installation);
                    }
                }
                let identity = (&entry.id.0, &args.source, &installations);
                let start = parse_pagination_cursor(
                    args.cursor.as_deref(),
                    &identity,
                    "skills.installations",
                )
                .inspect_err(|_| outcome = "invalid_cursor")?;
                if start > installations.len() {
                    outcome = "invalid_cursor";
                    return Err(FunctionCallError::RespondToModel(
                        "skills.installations cursor is invalid".to_string(),
                    ));
                }
                let fingerprint = value_fingerprint(&identity);
                let cursor_at = |offset| format!("{fingerprint:016x}:{offset}");
                let budget = call.response_byte_budget(MAX_INSTALLATIONS_RESPONSE_BYTES);
                let mut response = InstallationsResponse {
                    installations: Vec::new(),
                    next_cursor: None,
                };
                let installation_count = installations.len();
                for (index, installation) in installations.into_iter().enumerate().skip(start) {
                    response.installations.push(installation);
                    response.next_cursor =
                        (index + 1 < installation_count).then(|| cursor_at(index + 1));
                    if serialized_len(&response)? > budget {
                        response.installations.pop();
                        response.next_cursor = Some(cursor_at(index));
                        break;
                    }
                }
                if (start < installation_count && response.installations.is_empty())
                    || serialized_len(&response)? > budget
                {
                    outcome = "response_budget";
                    return Err(FunctionCallError::RespondToModel(
                        "skills.installations response budget leaves no room for an installation"
                            .to_string(),
                    ));
                }
                let authority = if response
                    .installations
                    .iter()
                    .any(|location| matches!(location, InstallationLocation::Cloud { .. }))
                {
                    SkillToolAuthoritySelector::Cloud
                } else {
                    SkillToolAuthoritySelector::Executor
                };
                let output = skill_json_output(&response, authority)?;
                outcome = if response.installations.is_empty() {
                    "empty"
                } else {
                    "success"
                };
                Ok(output)
            })();
            if let Some(metrics) = &self.metrics {
                metrics.counter(
                    "codex.skills.installations.result",
                    /*inc*/ 1,
                    &[
                        ("outcome", outcome),
                        ("source_filter", source_filter),
                        ("discovery_version", "v2"),
                    ],
                );
            }
            result
        })
    }
}
