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
use crate::catalog::SkillReadResult;
use crate::catalog::SkillResourceId;
use crate::provider::SkillReadRequest;

use super::MAX_HANDLE_BYTES;
use super::MAX_TOOL_OUTPUT_BYTES;
use super::SkillAuthorityHandle;
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
    authority: SkillAuthorityHandle,
    package: String,
    resource: String,
    cursor: Option<String>,
}

#[derive(Debug, Eq, JsonSchema, PartialEq, Serialize)]
#[schemars(deny_unknown_fields)]
struct ReadResponse {
    resource: String,
    contents: String,
    next_cursor: Option<String>,
    truncated: bool,
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
            "Read one bounded page of a resource from an enabled remote skill. Pass the exact authority and package returned by skills.list; resource identifiers remain opaque and are routed to that provider. Pass next_cursor as cursor to continue.",
        )
    }

    fn handle(&self, call: ToolCall) -> ToolExecutorFuture<'_> {
        Box::pin(async move {
            let args: ReadArgs = parse_args(&call)?;
            let authority = args.authority.into_authority()?;
            validate_handle("package", &args.package, MAX_HANDLE_BYTES)?;
            validate_handle("resource", &args.resource, MAX_HANDLE_BYTES)?;
            let cursor = args
                .cursor
                .as_deref()
                .unwrap_or("0")
                .parse::<usize>()
                .map_err(|_| {
                    FunctionCallError::RespondToModel("invalid skills.read cursor".to_string())
                })?;

            let catalog = self.context.catalog(&call.turn_id).await;
            let package_is_available = catalog.entries.iter().any(|entry| {
                entry.enabled && entry.authority == authority && entry.id.0 == args.package
            });
            if !package_is_available {
                return Err(FunctionCallError::RespondToModel(
                    "skill package is not available from the requested authority".to_string(),
                ));
            }

            let requested_resource = SkillResourceId::new(args.resource);
            let result = self
                .context
                .providers
                .read(SkillReadRequest {
                    authority,
                    package: SkillPackageId(args.package),
                    resource: requested_resource.clone(),
                    host: None,
                    mcp_resources: self.context.mcp_resources.clone(),
                })
                .await
                .map_err(|err| FunctionCallError::RespondToModel(err.message))?;
            if result.resource != requested_resource {
                return Err(FunctionCallError::Fatal(
                    "skill provider returned a different resource".to_string(),
                ));
            }

            external_json_output(&bounded_response(result, cursor)?)
        })
    }
}

fn bounded_response(
    result: SkillReadResult,
    cursor: usize,
) -> Result<ReadResponse, FunctionCallError> {
    let resource = result.resource.as_str().to_string();
    if cursor > result.contents.len() || !result.contents.is_char_boundary(cursor) {
        return Err(FunctionCallError::RespondToModel(
            "skills.read cursor is out of range".to_string(),
        ));
    }
    let empty_response = ReadResponse {
        resource: resource.clone(),
        contents: String::new(),
        next_cursor: Some(usize::MAX.to_string()),
        truncated: false,
    };
    let overhead = serde_json::to_vec(&empty_response)
        .map_err(|err| {
            FunctionCallError::Fatal(format!("failed to serialize skills.read output: {err}"))
        })?
        .len();
    let content_budget = MAX_TOOL_OUTPUT_BYTES.saturating_sub(overhead);

    let mut escaped_bytes = 0usize;
    let mut end = cursor;
    for (relative_index, ch) in result.contents[cursor..].char_indices() {
        let next_bytes = escaped_bytes.saturating_add(json_escaped_len(ch));
        if next_bytes > content_budget {
            break;
        }
        escaped_bytes = next_bytes;
        end = cursor + relative_index + ch.len_utf8();
    }
    let truncated = end < result.contents.len();

    Ok(ReadResponse {
        resource,
        contents: result.contents[cursor..end].to_string(),
        next_cursor: truncated.then(|| end.to_string()),
        truncated,
    })
}

fn json_escaped_len(ch: char) -> usize {
    match ch {
        '"' | '\\' | '\u{0008}' | '\u{000c}' | '\n' | '\r' | '\t' => 2,
        '\u{0000}'..='\u{001f}' => 6,
        _ => ch.len_utf8(),
    }
}
