use codex_extension_api::FunctionCallError;
use codex_extension_api::ToolCall;
use codex_extension_api::ToolExecutor;
use codex_extension_api::ToolExecutorFuture;
use codex_extension_api::ToolName;
use codex_extension_api::ToolSpec;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;

use crate::catalog::SkillPackageId;
use crate::catalog::SkillResourceId;
use crate::provider::SkillReadRequest;

use super::MAX_HANDLE_BYTES;
use super::SkillToolAuthority;
use super::SkillToolContext;
use super::external_json_output;
use super::parse_args;
use super::skill_function_tool;
use super::skill_tool_name;
use super::validate_handle;

const TOOL_NAME: &str = "read";

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ReadArgs {
    authority: SkillToolAuthority,
    package: String,
    resource: String,
}

#[derive(Debug, Eq, JsonSchema, PartialEq, Serialize)]
#[schemars(deny_unknown_fields)]
struct ReadResponse {
    resource: String,
    contents: String,
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
            "Read one complete resource from an enabled skill. Pass the exact authority and package returned by skills.list; resource identifiers remain opaque and are routed to that authority.",
        )
    }

    fn handle(&self, call: ToolCall) -> ToolExecutorFuture<'_> {
        Box::pin(async move {
            let args: ReadArgs = parse_args(&call)?;
            let authority = args.authority.into_authority();
            validate_handle("package", &args.package, MAX_HANDLE_BYTES)?;
            validate_handle("resource", &args.resource, MAX_HANDLE_BYTES)?;

            let catalog = self.context.catalog(&call.turn_id, args.authority).await;
            let Some(skill_entry) = catalog.entries.iter().find(|entry| {
                entry.enabled && entry.authority == authority && entry.id.0 == args.package
            }) else {
                return Err(FunctionCallError::RespondToModel(
                    "skill package is not available from the requested authority".to_string(),
                ));
            };
            let main_prompt = skill_entry.main_prompt.clone();

            let requested_resource = SkillResourceId::new(args.resource);
            let result = self
                .context
                .thread_state
                .read_skill(
                    &self.context.providers,
                    SkillReadRequest {
                        authority,
                        package: SkillPackageId(args.package),
                        resource: requested_resource.clone(),
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

            if let Some(state) = self
                .context
                .thread_state
                .shadow_selection_turn(&call.turn_id)
            {
                self.context
                    .shadow_selection
                    .record_invocation(&state, main_prompt.as_str());
            }

            external_json_output(&ReadResponse {
                resource: result.resource.as_str().to_string(),
                contents: result.contents,
            })
        })
    }
}
