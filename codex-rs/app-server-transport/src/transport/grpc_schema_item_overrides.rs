use super::*;

use super::super::super::grpc_api_conversions::decode_dynamic_value;
use super::super::super::grpc_api_conversions::encode_dynamic_value;
use codex_app_server_protocol::CollabAgentTool;
use codex_app_server_protocol::CollabAgentToolCallStatus;
use codex_app_server_protocol::CommandExecutionSource;
use codex_app_server_protocol::CommandExecutionStatus;
use codex_app_server_protocol::DynamicToolCallStatus;
use codex_app_server_protocol::McpToolCallResult;
use codex_app_server_protocol::McpToolCallStatus;
use codex_app_server_protocol::PatchApplyStatus;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::UserInput;

macro_rules! direct_string_enum {
    ($type:ty, $field:literal, { $($wire:literal => $variant:ident),+ $(,)? }) => {
        impl DirectSchemaProto<String> for $type {
            fn decode_schema(payload: String) -> Result<Self, Status> {
                match payload.as_str() {
                    $($wire => Ok(Self::$variant),)+
                    value => Err(invalid($field, format!("unknown value `{value}`"))),
                }
            }

            fn encode_schema(self) -> Result<String, Status> {
                Ok(match self {
                    $(Self::$variant => $wire.to_owned(),)+
                })
            }
        }
    };
}

direct_string_enum!(
    CommandExecutionStatus,
    "ThreadItem.commandExecution.status",
    {
        "inProgress" => InProgress,
        "completed" => Completed,
        "failed" => Failed,
        "declined" => Declined,
    }
);

direct_string_enum!(PatchApplyStatus, "ThreadItem.fileChange.status", {
    "inProgress" => InProgress,
    "completed" => Completed,
    "failed" => Failed,
    "declined" => Declined,
});

direct_string_enum!(McpToolCallStatus, "ThreadItem.mcpToolCall.status", {
    "inProgress" => InProgress,
    "completed" => Completed,
    "failed" => Failed,
});

direct_string_enum!(
    DynamicToolCallStatus,
    "ThreadItem.dynamicToolCall.status",
    {
        "inProgress" => InProgress,
        "completed" => Completed,
        "failed" => Failed,
    }
);

direct_string_enum!(CollabAgentTool, "ThreadItem.collabAgentToolCall.tool", {
    "spawnAgent" => SpawnAgent,
    "sendInput" => SendInput,
    "resumeAgent" => ResumeAgent,
    "wait" => Wait,
    "closeAgent" => CloseAgent,
});

direct_string_enum!(
    CollabAgentToolCallStatus,
    "ThreadItem.collabAgentToolCall.status",
    {
        "inProgress" => InProgress,
        "completed" => Completed,
        "failed" => Failed,
    }
);

fn decode_command_execution_source(
    payload: proto::V2CommandExecutionSource,
) -> Result<CommandExecutionSource, Status> {
    match payload.value.as_str() {
        "agent" => Ok(CommandExecutionSource::Agent),
        "userShell" => Ok(CommandExecutionSource::UserShell),
        "unifiedExecStartup" => Ok(CommandExecutionSource::UnifiedExecStartup),
        "unifiedExecInteraction" => Ok(CommandExecutionSource::UnifiedExecInteraction),
        value => Err(invalid(
            "ThreadItem.commandExecution.source",
            format!("unknown value `{value}`"),
        )),
    }
}

fn encode_command_execution_source(
    source: CommandExecutionSource,
) -> proto::V2CommandExecutionSource {
    let value = match source {
        CommandExecutionSource::Agent => "agent",
        CommandExecutionSource::UserShell => "userShell",
        CommandExecutionSource::UnifiedExecStartup => "unifiedExecStartup",
        CommandExecutionSource::UnifiedExecInteraction => "unifiedExecInteraction",
    };
    proto::V2CommandExecutionSource {
        value: value.to_owned(),
    }
}

impl DirectSchemaProto<proto::V2ThreadItemContentItem> for UserInput {
    fn decode_schema(payload: proto::V2ThreadItemContentItem) -> Result<Self, Status> {
        match payload
            .value
            .ok_or_else(|| missing("ThreadItem.content[].value"))?
        {
            proto::v2_thread_item_content_item::Value::Variant1(value) => {
                DirectSchemaProto::decode_schema(value)
            }
            proto::v2_thread_item_content_item::Value::StringValue(_) => Err(invalid(
                "ThreadItem.userMessage.content",
                "expected a user input item",
            )),
        }
    }

    fn encode_schema(self) -> Result<proto::V2ThreadItemContentItem, Status> {
        Ok(proto::V2ThreadItemContentItem {
            value: Some(proto::v2_thread_item_content_item::Value::Variant1(
                DirectSchemaProto::encode_schema(self)?,
            )),
        })
    }
}

impl DirectSchemaProto<proto::V2ThreadItemContentItem> for String {
    fn decode_schema(payload: proto::V2ThreadItemContentItem) -> Result<Self, Status> {
        match payload
            .value
            .ok_or_else(|| missing("ThreadItem.content[].value"))?
        {
            proto::v2_thread_item_content_item::Value::StringValue(value) => Ok(value),
            proto::v2_thread_item_content_item::Value::Variant1(_) => Err(invalid(
                "ThreadItem.reasoning.content",
                "expected a reasoning content string",
            )),
        }
    }

    fn encode_schema(self) -> Result<proto::V2ThreadItemContentItem, Status> {
        Ok(proto::V2ThreadItemContentItem {
            value: Some(proto::v2_thread_item_content_item::Value::StringValue(self)),
        })
    }
}

impl DirectSchemaProto<proto::V2ThreadItemResult> for Box<McpToolCallResult> {
    fn decode_schema(payload: proto::V2ThreadItemResult) -> Result<Self, Status> {
        match payload
            .value
            .ok_or_else(|| missing("ThreadItem.result.value"))?
        {
            proto::v2_thread_item_result::Value::Variant1(value) => {
                Ok(Box::new(DirectSchemaProto::decode_schema(value)?))
            }
            proto::v2_thread_item_result::Value::StringValue(_) => Err(invalid(
                "ThreadItem.mcpToolCall.result",
                "expected an MCP tool call result",
            )),
        }
    }

    fn encode_schema(self) -> Result<proto::V2ThreadItemResult, Status> {
        Ok(proto::V2ThreadItemResult {
            value: Some(proto::v2_thread_item_result::Value::Variant1(
                DirectSchemaProto::encode_schema(*self)?,
            )),
        })
    }
}

impl DirectSchemaProto<proto::V2ThreadItemResult> for String {
    fn decode_schema(payload: proto::V2ThreadItemResult) -> Result<Self, Status> {
        match payload
            .value
            .ok_or_else(|| missing("ThreadItem.result.value"))?
        {
            proto::v2_thread_item_result::Value::StringValue(value) => Ok(value),
            proto::v2_thread_item_result::Value::Variant1(_) => Err(invalid(
                "ThreadItem.imageGeneration.result",
                "expected an image generation result string",
            )),
        }
    }

    fn encode_schema(self) -> Result<proto::V2ThreadItemResult, Status> {
        Ok(proto::V2ThreadItemResult {
            value: Some(proto::v2_thread_item_result::Value::StringValue(self)),
        })
    }
}

fn decode_absolute_path<T>(
    value: proto::V2AbsolutePathBuf,
    field: &'static str,
) -> Result<T, Status>
where
    T: TryFrom<String>,
    T::Error: std::fmt::Display,
{
    decode_newtype_string(value.value, field)
}

fn encode_absolute_path(
    value: impl AsRef<std::path::Path>,
    field: &'static str,
) -> Result<proto::V2AbsolutePathBuf, Status> {
    let value = value
        .as_ref()
        .to_str()
        .ok_or_else(|| encode_error(field, "path is not valid UTF-8"))?
        .to_owned();
    Ok(proto::V2AbsolutePathBuf { value })
}

impl DirectSchemaProto<proto::V2ThreadItem> for ThreadItem {
    fn decode_schema(payload: proto::V2ThreadItem) -> Result<Self, Status> {
        match payload.r#type.as_str() {
            "userMessage" => Ok(Self::UserMessage {
                id: payload.id,
                client_id: payload.client_id,
                content: payload
                    .content
                    .ok_or_else(|| missing("ThreadItem.userMessage.content"))?
                    .values
                    .into_iter()
                    .map(DirectSchemaProto::decode_schema)
                    .collect::<Result<_, _>>()?,
            }),
            "hookPrompt" => Ok(Self::HookPrompt {
                id: payload.id,
                fragments: payload
                    .fragments
                    .ok_or_else(|| missing("ThreadItem.hookPrompt.fragments"))?
                    .values
                    .into_iter()
                    .map(DirectSchemaProto::decode_schema)
                    .collect::<Result<_, _>>()?,
            }),
            "agentMessage" => Ok(Self::AgentMessage {
                id: payload.id,
                text: payload
                    .text
                    .ok_or_else(|| missing("ThreadItem.agentMessage.text"))?,
                phase: payload
                    .phase
                    .map(DirectSchemaProto::decode_schema)
                    .transpose()?,
                memory_citation: payload
                    .memory_citation
                    .map(DirectSchemaProto::decode_schema)
                    .transpose()?,
            }),
            "plan" => Ok(Self::Plan {
                id: payload.id,
                text: payload
                    .text
                    .ok_or_else(|| missing("ThreadItem.plan.text"))?,
            }),
            "reasoning" => Ok(Self::Reasoning {
                id: payload.id,
                summary: payload
                    .summary
                    .map(|value| value.values)
                    .unwrap_or_default(),
                content: payload
                    .content
                    .map(|value| {
                        value
                            .values
                            .into_iter()
                            .map(DirectSchemaProto::decode_schema)
                            .collect()
                    })
                    .transpose()?
                    .unwrap_or_default(),
            }),
            "commandExecution" => Ok(Self::CommandExecution {
                id: payload.id,
                command: payload
                    .command
                    .ok_or_else(|| missing("ThreadItem.commandExecution.command"))?,
                cwd: decode_absolute_path(
                    payload
                        .cwd
                        .ok_or_else(|| missing("ThreadItem.commandExecution.cwd"))?,
                    "ThreadItem.commandExecution.cwd",
                )?,
                process_id: payload.process_id,
                source: payload
                    .source
                    .map(decode_command_execution_source)
                    .transpose()?
                    .unwrap_or_default(),
                status: DirectSchemaProto::decode_schema(
                    payload
                        .status
                        .ok_or_else(|| missing("ThreadItem.commandExecution.status"))?,
                )?,
                command_actions: payload
                    .command_actions
                    .ok_or_else(|| missing("ThreadItem.commandExecution.commandActions"))?
                    .values
                    .into_iter()
                    .map(DirectSchemaProto::decode_schema)
                    .collect::<Result<_, _>>()?,
                aggregated_output: payload.aggregated_output,
                exit_code: payload
                    .exit_code
                    .map(|value| decode_integer(value, "ThreadItem.commandExecution.exitCode"))
                    .transpose()?,
                duration_ms: payload.duration_ms,
            }),
            "fileChange" => Ok(Self::FileChange {
                id: payload.id,
                changes: payload
                    .changes
                    .ok_or_else(|| missing("ThreadItem.fileChange.changes"))?
                    .values
                    .into_iter()
                    .map(DirectSchemaProto::decode_schema)
                    .collect::<Result<_, _>>()?,
                status: DirectSchemaProto::decode_schema(
                    payload
                        .status
                        .ok_or_else(|| missing("ThreadItem.fileChange.status"))?,
                )?,
            }),
            "mcpToolCall" => Ok(Self::McpToolCall {
                id: payload.id,
                server: payload
                    .server
                    .ok_or_else(|| missing("ThreadItem.mcpToolCall.server"))?,
                tool: payload
                    .tool
                    .ok_or_else(|| missing("ThreadItem.mcpToolCall.tool"))?,
                status: DirectSchemaProto::decode_schema(
                    payload
                        .status
                        .ok_or_else(|| missing("ThreadItem.mcpToolCall.status"))?,
                )?,
                arguments: decode_dynamic_value(
                    payload
                        .arguments
                        .ok_or_else(|| missing("ThreadItem.mcpToolCall.arguments"))?,
                )?,
                mcp_app_resource_uri: payload.mcp_app_resource_uri,
                plugin_id: payload.plugin_id,
                result: payload
                    .result
                    .map(DirectSchemaProto::decode_schema)
                    .transpose()?,
                error: payload
                    .error
                    .map(DirectSchemaProto::decode_schema)
                    .transpose()?,
                duration_ms: payload.duration_ms,
            }),
            "dynamicToolCall" => Ok(Self::DynamicToolCall {
                id: payload.id,
                namespace: payload.namespace,
                tool: payload
                    .tool
                    .ok_or_else(|| missing("ThreadItem.dynamicToolCall.tool"))?,
                arguments: decode_dynamic_value(
                    payload
                        .arguments
                        .ok_or_else(|| missing("ThreadItem.dynamicToolCall.arguments"))?,
                )?,
                status: DirectSchemaProto::decode_schema(
                    payload
                        .status
                        .ok_or_else(|| missing("ThreadItem.dynamicToolCall.status"))?,
                )?,
                content_items: payload
                    .content_items
                    .map(|value| {
                        value
                            .values
                            .into_iter()
                            .map(DirectSchemaProto::decode_schema)
                            .collect()
                    })
                    .transpose()?,
                success: payload.success,
                duration_ms: payload.duration_ms,
            }),
            "collabAgentToolCall" => Ok(Self::CollabAgentToolCall {
                id: payload.id,
                tool: DirectSchemaProto::decode_schema(
                    payload
                        .tool
                        .ok_or_else(|| missing("ThreadItem.collabAgentToolCall.tool"))?,
                )?,
                status: DirectSchemaProto::decode_schema(
                    payload
                        .status
                        .ok_or_else(|| missing("ThreadItem.collabAgentToolCall.status"))?,
                )?,
                sender_thread_id: payload
                    .sender_thread_id
                    .ok_or_else(|| missing("ThreadItem.collabAgentToolCall.senderThreadId"))?,
                receiver_thread_ids: payload
                    .receiver_thread_ids
                    .ok_or_else(|| missing("ThreadItem.collabAgentToolCall.receiverThreadIds"))?
                    .values,
                prompt: payload.prompt,
                model: payload.model,
                reasoning_effort: payload
                    .reasoning_effort
                    .map(|value| {
                        value.value.parse().map_err(|error| {
                            invalid("ThreadItem.collabAgentToolCall.reasoningEffort", error)
                        })
                    })
                    .transpose()?,
                agents_states: payload
                    .agents_states
                    .ok_or_else(|| missing("ThreadItem.collabAgentToolCall.agentsStates"))?
                    .values
                    .into_iter()
                    .map(|(key, value)| Ok((key, DirectSchemaProto::decode_schema(value)?)))
                    .collect::<Result<_, Status>>()?,
            }),
            "webSearch" => Ok(Self::WebSearch {
                id: payload.id,
                query: payload
                    .query
                    .ok_or_else(|| missing("ThreadItem.webSearch.query"))?,
                action: payload
                    .action
                    .map(DirectSchemaProto::decode_schema)
                    .transpose()?,
            }),
            "imageView" => Ok(Self::ImageView {
                id: payload.id,
                path: decode_absolute_path(
                    payload
                        .path
                        .ok_or_else(|| missing("ThreadItem.imageView.path"))?,
                    "ThreadItem.imageView.path",
                )?,
            }),
            "imageGeneration" => Ok(Self::ImageGeneration {
                id: payload.id,
                status: payload
                    .status
                    .ok_or_else(|| missing("ThreadItem.imageGeneration.status"))?,
                revised_prompt: payload.revised_prompt,
                result: DirectSchemaProto::decode_schema(
                    payload
                        .result
                        .ok_or_else(|| missing("ThreadItem.imageGeneration.result"))?,
                )?,
                saved_path: payload
                    .saved_path
                    .map(|value| {
                        decode_absolute_path(value, "ThreadItem.imageGeneration.savedPath")
                    })
                    .transpose()?,
            }),
            "enteredReviewMode" => Ok(Self::EnteredReviewMode {
                id: payload.id,
                review: payload
                    .review
                    .ok_or_else(|| missing("ThreadItem.enteredReviewMode.review"))?,
            }),
            "exitedReviewMode" => Ok(Self::ExitedReviewMode {
                id: payload.id,
                review: payload
                    .review
                    .ok_or_else(|| missing("ThreadItem.exitedReviewMode.review"))?,
            }),
            "contextCompaction" => Ok(Self::ContextCompaction { id: payload.id }),
            value => Err(invalid("ThreadItem.type", format!("unknown tag `{value}`"))),
        }
    }

    fn encode_schema(self) -> Result<proto::V2ThreadItem, Status> {
        match self {
            Self::UserMessage {
                id,
                client_id,
                content,
            } => Ok(proto::V2ThreadItem {
                r#type: "userMessage".to_owned(),
                id,
                client_id,
                content: Some(proto::V2ThreadItemContentList {
                    values: content
                        .into_iter()
                        .map(DirectSchemaProto::encode_schema)
                        .collect::<Result<_, _>>()?,
                }),
                ..Default::default()
            }),
            Self::HookPrompt { id, fragments } => Ok(proto::V2ThreadItem {
                r#type: "hookPrompt".to_owned(),
                id,
                fragments: Some(proto::V2ThreadItemFragmentsList {
                    values: fragments
                        .into_iter()
                        .map(DirectSchemaProto::encode_schema)
                        .collect::<Result<_, _>>()?,
                }),
                ..Default::default()
            }),
            Self::AgentMessage {
                id,
                text,
                phase,
                memory_citation,
            } => Ok(proto::V2ThreadItem {
                r#type: "agentMessage".to_owned(),
                id,
                text: Some(text),
                phase: phase.map(DirectSchemaProto::encode_schema).transpose()?,
                memory_citation: memory_citation
                    .map(DirectSchemaProto::encode_schema)
                    .transpose()?,
                ..Default::default()
            }),
            Self::Plan { id, text } => Ok(proto::V2ThreadItem {
                r#type: "plan".to_owned(),
                id,
                text: Some(text),
                ..Default::default()
            }),
            Self::Reasoning {
                id,
                summary,
                content,
            } => Ok(proto::V2ThreadItem {
                r#type: "reasoning".to_owned(),
                id,
                summary: Some(proto::V2ThreadStartParamsRuntimeWorkspaceRootsList {
                    values: summary,
                }),
                content: Some(proto::V2ThreadItemContentList {
                    values: content
                        .into_iter()
                        .map(DirectSchemaProto::encode_schema)
                        .collect::<Result<_, _>>()?,
                }),
                ..Default::default()
            }),
            Self::CommandExecution {
                id,
                command,
                cwd,
                process_id,
                source,
                status,
                command_actions,
                aggregated_output,
                exit_code,
                duration_ms,
            } => Ok(proto::V2ThreadItem {
                r#type: "commandExecution".to_owned(),
                id,
                command: Some(command),
                cwd: Some(encode_absolute_path(
                    cwd,
                    "ThreadItem.commandExecution.cwd",
                )?),
                process_id,
                source: Some(encode_command_execution_source(source)),
                status: Some(DirectSchemaProto::encode_schema(status)?),
                command_actions: Some(proto::V2ThreadItemCommandActionsList {
                    values: command_actions
                        .into_iter()
                        .map(DirectSchemaProto::encode_schema)
                        .collect::<Result<_, _>>()?,
                }),
                aggregated_output,
                exit_code: exit_code
                    .map(|value| encode_integer(value, "ThreadItem.commandExecution.exitCode"))
                    .transpose()?,
                duration_ms,
                ..Default::default()
            }),
            Self::FileChange {
                id,
                changes,
                status,
            } => Ok(proto::V2ThreadItem {
                r#type: "fileChange".to_owned(),
                id,
                changes: Some(proto::V2ThreadItemChangesList {
                    values: changes
                        .into_iter()
                        .map(DirectSchemaProto::encode_schema)
                        .collect::<Result<_, _>>()?,
                }),
                status: Some(DirectSchemaProto::encode_schema(status)?),
                ..Default::default()
            }),
            Self::McpToolCall {
                id,
                server,
                tool,
                status,
                arguments,
                mcp_app_resource_uri,
                plugin_id,
                result,
                error,
                duration_ms,
            } => Ok(proto::V2ThreadItem {
                r#type: "mcpToolCall".to_owned(),
                id,
                server: Some(server),
                tool: Some(tool),
                status: Some(DirectSchemaProto::encode_schema(status)?),
                arguments: Some(encode_dynamic_value(arguments)?),
                mcp_app_resource_uri,
                plugin_id,
                result: result.map(DirectSchemaProto::encode_schema).transpose()?,
                error: error.map(DirectSchemaProto::encode_schema).transpose()?,
                duration_ms,
                ..Default::default()
            }),
            Self::DynamicToolCall {
                id,
                namespace,
                tool,
                arguments,
                status,
                content_items,
                success,
                duration_ms,
            } => Ok(proto::V2ThreadItem {
                r#type: "dynamicToolCall".to_owned(),
                id,
                namespace,
                tool: Some(tool),
                arguments: Some(encode_dynamic_value(arguments)?),
                status: Some(DirectSchemaProto::encode_schema(status)?),
                content_items: content_items
                    .map(|values| {
                        Ok::<_, Status>(proto::V2ThreadItemContentItemsList {
                            values: values
                                .into_iter()
                                .map(DirectSchemaProto::encode_schema)
                                .collect::<Result<_, Status>>()?,
                        })
                    })
                    .transpose()?,
                success,
                duration_ms,
                ..Default::default()
            }),
            Self::CollabAgentToolCall {
                id,
                tool,
                status,
                sender_thread_id,
                receiver_thread_ids,
                prompt,
                model,
                reasoning_effort,
                agents_states,
            } => Ok(proto::V2ThreadItem {
                r#type: "collabAgentToolCall".to_owned(),
                id,
                tool: Some(DirectSchemaProto::encode_schema(tool)?),
                status: Some(DirectSchemaProto::encode_schema(status)?),
                sender_thread_id: Some(sender_thread_id),
                receiver_thread_ids: Some(proto::V2ThreadStartParamsRuntimeWorkspaceRootsList {
                    values: receiver_thread_ids,
                }),
                prompt,
                model,
                reasoning_effort: reasoning_effort.map(|value| proto::V2ReasoningEffort {
                    value: value.to_string(),
                }),
                agents_states: Some(proto::V2ThreadItemAgentsStatesMap {
                    values: agents_states
                        .into_iter()
                        .map(|(key, value)| Ok((key, DirectSchemaProto::encode_schema(value)?)))
                        .collect::<Result<_, Status>>()?,
                }),
                ..Default::default()
            }),
            Self::WebSearch { id, query, action } => Ok(proto::V2ThreadItem {
                r#type: "webSearch".to_owned(),
                id,
                query: Some(query),
                action: action.map(DirectSchemaProto::encode_schema).transpose()?,
                ..Default::default()
            }),
            Self::ImageView { id, path } => Ok(proto::V2ThreadItem {
                r#type: "imageView".to_owned(),
                id,
                path: Some(encode_absolute_path(path, "ThreadItem.imageView.path")?),
                ..Default::default()
            }),
            Self::ImageGeneration {
                id,
                status,
                revised_prompt,
                result,
                saved_path,
            } => Ok(proto::V2ThreadItem {
                r#type: "imageGeneration".to_owned(),
                id,
                status: Some(status),
                revised_prompt,
                result: Some(DirectSchemaProto::encode_schema(result)?),
                saved_path: saved_path
                    .map(|value| {
                        encode_absolute_path(value, "ThreadItem.imageGeneration.savedPath")
                    })
                    .transpose()?,
                ..Default::default()
            }),
            Self::EnteredReviewMode { id, review } => Ok(proto::V2ThreadItem {
                r#type: "enteredReviewMode".to_owned(),
                id,
                review: Some(review),
                ..Default::default()
            }),
            Self::ExitedReviewMode { id, review } => Ok(proto::V2ThreadItem {
                r#type: "exitedReviewMode".to_owned(),
                id,
                review: Some(review),
                ..Default::default()
            }),
            Self::ContextCompaction { id } => Ok(proto::V2ThreadItem {
                r#type: "contextCompaction".to_owned(),
                id,
                ..Default::default()
            }),
        }
    }
}
