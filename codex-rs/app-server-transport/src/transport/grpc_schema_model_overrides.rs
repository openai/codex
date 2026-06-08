use super::*;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ImageDetail;
use codex_protocol::models::LocalShellAction;
use codex_protocol::models::LocalShellExecAction;
use codex_protocol::models::LocalShellStatus;
use codex_protocol::models::MessagePhase;
use codex_protocol::models::ReasoningItemContent;
use codex_protocol::models::ReasoningItemReasoningSummary;
use codex_protocol::models::ResponseItem;
use codex_protocol::models::WebSearchAction;

impl DirectSchemaProto<proto::V2ContentItem> for ContentItem {
    fn decode_schema(payload: proto::V2ContentItem) -> Result<Self, Status> {
        match payload.r#type.as_str() {
            "input_text" => Ok(Self::InputText {
                text: payload.text.ok_or_else(|| missing("ContentItem.text"))?,
            }),
            "input_image" => Ok(Self::InputImage {
                image_url: payload
                    .image_url
                    .ok_or_else(|| missing("ContentItem.image_url"))?,
                detail: payload
                    .detail
                    .map(<ImageDetail as DirectSchemaProto<proto::V2ImageDetail>>::decode_schema)
                    .transpose()?,
            }),
            "output_text" => Ok(Self::OutputText {
                text: payload.text.ok_or_else(|| missing("ContentItem.text"))?,
            }),
            value => Err(invalid(
                "ContentItem.type",
                format!("unknown tag `{value}`"),
            )),
        }
    }

    fn encode_schema(self) -> Result<proto::V2ContentItem, Status> {
        Ok(match self {
            Self::InputText { text } => proto::V2ContentItem {
                text: Some(text),
                r#type: "input_text".to_owned(),
                ..Default::default()
            },
            Self::InputImage { image_url, detail } => proto::V2ContentItem {
                r#type: "input_image".to_owned(),
                detail: detail
                    .map(<ImageDetail as DirectSchemaProto<proto::V2ImageDetail>>::encode_schema)
                    .transpose()?,
                image_url: Some(image_url),
                ..Default::default()
            },
            Self::OutputText { text } => proto::V2ContentItem {
                text: Some(text),
                r#type: "output_text".to_owned(),
                ..Default::default()
            },
        })
    }
}

impl DirectSchemaProto<proto::V2ReasoningItemContent> for ReasoningItemContent {
    fn decode_schema(payload: proto::V2ReasoningItemContent) -> Result<Self, Status> {
        match payload.r#type.as_str() {
            "reasoning_text" => Ok(Self::ReasoningText { text: payload.text }),
            "text" => Ok(Self::Text { text: payload.text }),
            value => Err(invalid(
                "ReasoningItemContent.type",
                format!("unknown tag `{value}`"),
            )),
        }
    }

    fn encode_schema(self) -> Result<proto::V2ReasoningItemContent, Status> {
        Ok(match self {
            Self::ReasoningText { text } => proto::V2ReasoningItemContent {
                text,
                r#type: "reasoning_text".to_owned(),
            },
            Self::Text { text } => proto::V2ReasoningItemContent {
                text,
                r#type: "text".to_owned(),
            },
        })
    }
}

impl DirectSchemaProto<proto::V2ReasoningItemReasoningSummary> for ReasoningItemReasoningSummary {
    fn decode_schema(payload: proto::V2ReasoningItemReasoningSummary) -> Result<Self, Status> {
        match payload.r#type.as_str() {
            "summary_text" => Ok(Self::SummaryText { text: payload.text }),
            value => Err(invalid(
                "ReasoningItemReasoningSummary.type",
                format!("unknown tag `{value}`"),
            )),
        }
    }

    fn encode_schema(self) -> Result<proto::V2ReasoningItemReasoningSummary, Status> {
        let Self::SummaryText { text } = self;
        Ok(proto::V2ReasoningItemReasoningSummary {
            text,
            r#type: "summary_text".to_owned(),
        })
    }
}

impl DirectSchemaProto<String> for LocalShellStatus {
    fn decode_schema(payload: String) -> Result<Self, Status> {
        match payload.as_str() {
            "completed" => Ok(Self::Completed),
            "in_progress" => Ok(Self::InProgress),
            "incomplete" => Ok(Self::Incomplete),
            value => Err(invalid(
                "LocalShellStatus",
                format!("unknown value `{value}`"),
            )),
        }
    }

    fn encode_schema(self) -> Result<String, Status> {
        Ok(match self {
            Self::Completed => "completed",
            Self::InProgress => "in_progress",
            Self::Incomplete => "incomplete",
        }
        .to_owned())
    }
}

impl DirectSchemaProto<proto::V2LocalShellAction> for LocalShellAction {
    fn decode_schema(payload: proto::V2LocalShellAction) -> Result<Self, Status> {
        match payload.r#type.as_str() {
            "exec" => Ok(Self::Exec(LocalShellExecAction {
                command: payload.command,
                timeout_ms: payload.timeout_ms,
                working_directory: payload.working_directory,
                env: payload.env.map(|env| env.values),
                user: payload.user,
            })),
            value => Err(invalid(
                "LocalShellAction.type",
                format!("unknown tag `{value}`"),
            )),
        }
    }

    fn encode_schema(self) -> Result<proto::V2LocalShellAction, Status> {
        let Self::Exec(action) = self;
        Ok(proto::V2LocalShellAction {
            command: action.command,
            env: action
                .env
                .map(|values| proto::V2LocalShellActionEnvMap { values }),
            timeout_ms: action.timeout_ms,
            r#type: "exec".to_owned(),
            user: action.user,
            working_directory: action.working_directory,
        })
    }
}

impl DirectSchemaProto<proto::V2FunctionCallOutputContentItem> for FunctionCallOutputContentItem {
    fn decode_schema(payload: proto::V2FunctionCallOutputContentItem) -> Result<Self, Status> {
        match payload.r#type.as_str() {
            "input_text" => Ok(Self::InputText {
                text: payload
                    .text
                    .ok_or_else(|| missing("FunctionCallOutputContentItem.text"))?,
            }),
            "input_image" => Ok(Self::InputImage {
                image_url: payload
                    .image_url
                    .ok_or_else(|| missing("FunctionCallOutputContentItem.image_url"))?,
                detail: payload
                    .detail
                    .map(<ImageDetail as DirectSchemaProto<proto::V2ImageDetail>>::decode_schema)
                    .transpose()?,
            }),
            "encrypted_content" => Ok(Self::EncryptedContent {
                encrypted_content: payload
                    .encrypted_content
                    .ok_or_else(|| missing("FunctionCallOutputContentItem.encrypted_content"))?,
            }),
            value => Err(invalid(
                "FunctionCallOutputContentItem.type",
                format!("unknown tag `{value}`"),
            )),
        }
    }

    fn encode_schema(self) -> Result<proto::V2FunctionCallOutputContentItem, Status> {
        Ok(match self {
            Self::InputText { text } => proto::V2FunctionCallOutputContentItem {
                text: Some(text),
                r#type: "input_text".to_owned(),
                ..Default::default()
            },
            Self::InputImage { image_url, detail } => proto::V2FunctionCallOutputContentItem {
                r#type: "input_image".to_owned(),
                detail: detail
                    .map(<ImageDetail as DirectSchemaProto<proto::V2ImageDetail>>::encode_schema)
                    .transpose()?,
                image_url: Some(image_url),
                ..Default::default()
            },
            Self::EncryptedContent { encrypted_content } => {
                proto::V2FunctionCallOutputContentItem {
                    r#type: "encrypted_content".to_owned(),
                    encrypted_content: Some(encrypted_content),
                    ..Default::default()
                }
            }
        })
    }
}

impl DirectSchemaProto<proto::V2FunctionCallOutputBody> for FunctionCallOutputBody {
    fn decode_schema(payload: proto::V2FunctionCallOutputBody) -> Result<Self, Status> {
        use proto::v2_function_call_output_body::Value;

        match payload
            .value
            .ok_or_else(|| missing("FunctionCallOutputBody.value"))?
        {
            Value::StringValue(text) => Ok(Self::Text(text)),
            Value::Variant2(items) => Ok(Self::ContentItems(
                items
                    .values
                    .into_iter()
                    .map(
                        <FunctionCallOutputContentItem as DirectSchemaProto<
                            proto::V2FunctionCallOutputContentItem,
                        >>::decode_schema,
                    )
                    .collect::<Result<Vec<_>, _>>()?,
            )),
        }
    }

    fn encode_schema(self) -> Result<proto::V2FunctionCallOutputBody, Status> {
        use proto::v2_function_call_output_body::Value;

        let value = match self {
            Self::Text(text) => Value::StringValue(text),
            Self::ContentItems(items) => Value::Variant2(proto::V2FunctionCallOutputBodyVariant2 {
                values: items
                    .into_iter()
                    .map(
                        <FunctionCallOutputContentItem as DirectSchemaProto<
                            proto::V2FunctionCallOutputContentItem,
                        >>::encode_schema,
                    )
                    .collect::<Result<Vec<_>, _>>()?,
            }),
        };
        Ok(proto::V2FunctionCallOutputBody { value: Some(value) })
    }
}

impl DirectSchemaProto<proto::V2ResponsesApiWebSearchAction> for WebSearchAction {
    fn decode_schema(payload: proto::V2ResponsesApiWebSearchAction) -> Result<Self, Status> {
        Ok(match payload.r#type.as_str() {
            "search" => Self::Search {
                query: payload.query,
                queries: payload.queries.map(|queries| queries.values),
            },
            "open_page" => Self::OpenPage { url: payload.url },
            "find_in_page" => Self::FindInPage {
                url: payload.url,
                pattern: payload.pattern,
            },
            _ => Self::Other,
        })
    }

    fn encode_schema(self) -> Result<proto::V2ResponsesApiWebSearchAction, Status> {
        Ok(match self {
            Self::Search { query, queries } => proto::V2ResponsesApiWebSearchAction {
                queries: queries
                    .map(|values| proto::V2ThreadStartParamsRuntimeWorkspaceRootsList { values }),
                query,
                r#type: "search".to_owned(),
                ..Default::default()
            },
            Self::OpenPage { url } => proto::V2ResponsesApiWebSearchAction {
                r#type: "open_page".to_owned(),
                url,
                ..Default::default()
            },
            Self::FindInPage { url, pattern } => proto::V2ResponsesApiWebSearchAction {
                r#type: "find_in_page".to_owned(),
                url,
                pattern,
                ..Default::default()
            },
            Self::Other => proto::V2ResponsesApiWebSearchAction {
                r#type: "other".to_owned(),
                ..Default::default()
            },
        })
    }
}

impl DirectSchemaProto<proto::V2ResponseItem> for ResponseItem {
    fn decode_schema(payload: proto::V2ResponseItem) -> Result<Self, Status> {
        match payload.r#type.as_str() {
            "message" => {
                let content = payload
                    .content
                    .ok_or_else(|| missing("ResponseItem.content"))?;
                let content = content
                    .values
                    .into_iter()
                    .map(|item| {
                        use proto::v2_response_item_content_item::Value;

                        match item
                            .value
                            .ok_or_else(|| missing("ResponseItem.content[].value"))?
                        {
                            Value::Variant1(item) => <ContentItem as DirectSchemaProto<
                                proto::V2ContentItem,
                            >>::decode_schema(
                                item
                            ),
                            Value::Variant2(_) => Err(invalid(
                                "ResponseItem.content[]",
                                "reasoning content in a message item",
                            )),
                        }
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                Ok(Self::Message {
                    id: payload.id,
                    role: payload
                        .role
                        .ok_or_else(|| missing("ResponseItem.role"))?,
                    content,
                    phase: payload
                        .phase
                        .map(
                            <MessagePhase as DirectSchemaProto<
                                proto::V2MessagePhase,
                            >>::decode_schema,
                        )
                        .transpose()?,
                })
            }
            "reasoning" => {
                let content = payload
                    .content
                    .map(|content| {
                        content
                            .values
                            .into_iter()
                            .map(|item| {
                                use proto::v2_response_item_content_item::Value;

                                match item
                                    .value
                                    .ok_or_else(|| missing("ResponseItem.content[].value"))?
                                {
                                    Value::Variant1(_) => Err(invalid(
                                        "ResponseItem.content[]",
                                        "message content in a reasoning item",
                                    )),
                                    Value::Variant2(item) => {
                                        <ReasoningItemContent as DirectSchemaProto<
                                            proto::V2ReasoningItemContent,
                                        >>::decode_schema(
                                            item
                                        )
                                    }
                                }
                            })
                            .collect::<Result<Vec<_>, _>>()
                    })
                    .transpose()?;
                let summary = payload
                    .summary
                    .ok_or_else(|| missing("ResponseItem.summary"))?
                    .values
                    .into_iter()
                    .map(
                        <ReasoningItemReasoningSummary as DirectSchemaProto<
                            proto::V2ReasoningItemReasoningSummary,
                        >>::decode_schema,
                    )
                    .collect::<Result<Vec<_>, _>>()?;

                Ok(Self::Reasoning {
                    id: payload.id.unwrap_or_default(),
                    summary,
                    content,
                    encrypted_content: payload.encrypted_content,
                })
            }
            "local_shell_call" => {
                use proto::v2_response_item_action::Value;

                let action = payload
                    .action
                    .ok_or_else(|| missing("ResponseItem.action"))?
                    .value
                    .ok_or_else(|| missing("ResponseItem.action.value"))?;
                let action = match action {
                    Value::Variant1(action) => <LocalShellAction as DirectSchemaProto<
                        proto::V2LocalShellAction,
                    >>::decode_schema(action)?,
                    Value::Variant2(_) => {
                        return Err(invalid(
                            "ResponseItem.action",
                            "web-search action in a local-shell item",
                        ));
                    }
                };

                Ok(Self::LocalShellCall {
                    id: payload.id,
                    call_id: payload.call_id,
                    status: <LocalShellStatus as DirectSchemaProto<String>>::decode_schema(
                        payload
                            .status
                            .ok_or_else(|| missing("ResponseItem.status"))?,
                    )?,
                    action,
                })
            }
            "function_call" => {
                use proto::v2_response_item_arguments::Value;

                let arguments = payload
                    .arguments
                    .ok_or_else(|| missing("ResponseItem.arguments"))?
                    .value
                    .ok_or_else(|| missing("ResponseItem.arguments.value"))?;
                let arguments = match arguments {
                    Value::StringValue(arguments) => arguments,
                    Value::Variant2(_) => {
                        return Err(invalid(
                            "ResponseItem.arguments",
                            "dynamic arguments in a function-call item",
                        ));
                    }
                };

                Ok(Self::FunctionCall {
                    id: payload.id,
                    name: payload.name.ok_or_else(|| missing("ResponseItem.name"))?,
                    namespace: payload.namespace,
                    arguments,
                    call_id: payload
                        .call_id
                        .ok_or_else(|| missing("ResponseItem.call_id"))?,
                })
            }
            "tool_search_call" => {
                use proto::v2_response_item_arguments::Value;

                let arguments = payload
                    .arguments
                    .ok_or_else(|| missing("ResponseItem.arguments"))?
                    .value
                    .ok_or_else(|| missing("ResponseItem.arguments.value"))?;
                let arguments = match arguments {
                    Value::StringValue(_) => {
                        return Err(invalid(
                            "ResponseItem.arguments",
                            "string arguments in a tool-search item",
                        ));
                    }
                    Value::Variant2(arguments) => {
                        super::super::super::grpc_api_conversions::decode_dynamic_value(arguments)?
                    }
                };

                Ok(Self::ToolSearchCall {
                    id: payload.id,
                    call_id: payload.call_id,
                    status: payload.status,
                    execution: payload
                        .execution
                        .ok_or_else(|| missing("ResponseItem.execution"))?,
                    arguments,
                })
            }
            "function_call_output" => Ok(Self::FunctionCallOutput {
                call_id: payload
                    .call_id
                    .ok_or_else(|| missing("ResponseItem.call_id"))?,
                output: FunctionCallOutputPayload {
                    body: <FunctionCallOutputBody as DirectSchemaProto<
                        proto::V2FunctionCallOutputBody,
                    >>::decode_schema(
                        payload
                            .output
                            .ok_or_else(|| missing("ResponseItem.output"))?,
                    )?,
                    success: None,
                },
            }),
            "custom_tool_call" => Ok(Self::CustomToolCall {
                id: payload.id,
                status: payload.status,
                call_id: payload
                    .call_id
                    .ok_or_else(|| missing("ResponseItem.call_id"))?,
                name: payload.name.ok_or_else(|| missing("ResponseItem.name"))?,
                input: payload.input.ok_or_else(|| missing("ResponseItem.input"))?,
            }),
            "custom_tool_call_output" => Ok(Self::CustomToolCallOutput {
                call_id: payload
                    .call_id
                    .ok_or_else(|| missing("ResponseItem.call_id"))?,
                name: payload.name,
                output: FunctionCallOutputPayload {
                    body: <FunctionCallOutputBody as DirectSchemaProto<
                        proto::V2FunctionCallOutputBody,
                    >>::decode_schema(
                        payload
                            .output
                            .ok_or_else(|| missing("ResponseItem.output"))?,
                    )?,
                    success: None,
                },
            }),
            "tool_search_output" => Ok(Self::ToolSearchOutput {
                call_id: payload.call_id,
                status: payload
                    .status
                    .ok_or_else(|| missing("ResponseItem.status"))?,
                execution: payload
                    .execution
                    .ok_or_else(|| missing("ResponseItem.execution"))?,
                tools: payload
                    .tools
                    .ok_or_else(|| missing("ResponseItem.tools"))?
                    .values
                    .into_iter()
                    .map(super::super::super::grpc_api_conversions::decode_dynamic_value)
                    .collect::<Result<Vec<_>, _>>()?,
            }),
            "web_search_call" => {
                let action = payload
                    .action
                    .map(|action| {
                        use proto::v2_response_item_action::Value;

                        match action
                            .value
                            .ok_or_else(|| missing("ResponseItem.action.value"))?
                        {
                            Value::Variant1(_) => Err(invalid(
                                "ResponseItem.action",
                                "local-shell action in a web-search item",
                            )),
                            Value::Variant2(action) => {
                                <WebSearchAction as DirectSchemaProto<
                                    proto::V2ResponsesApiWebSearchAction,
                                >>::decode_schema(action)
                            }
                        }
                    })
                    .transpose()?;

                Ok(Self::WebSearchCall {
                    id: payload.id,
                    status: payload.status,
                    action,
                })
            }
            "image_generation_call" => Ok(Self::ImageGenerationCall {
                id: payload.id.ok_or_else(|| missing("ResponseItem.id"))?,
                status: payload
                    .status
                    .ok_or_else(|| missing("ResponseItem.status"))?,
                revised_prompt: payload.revised_prompt,
                result: payload
                    .result
                    .ok_or_else(|| missing("ResponseItem.result"))?,
            }),
            "compaction" | "compaction_summary" => Ok(Self::Compaction {
                encrypted_content: payload
                    .encrypted_content
                    .ok_or_else(|| missing("ResponseItem.encrypted_content"))?,
            }),
            "compaction_trigger" => Ok(Self::CompactionTrigger),
            "context_compaction" => Ok(Self::ContextCompaction {
                encrypted_content: payload.encrypted_content,
            }),
            _ => Ok(Self::Other),
        }
    }

    fn encode_schema(self) -> Result<proto::V2ResponseItem, Status> {
        Ok(match self {
            Self::Message {
                id,
                role,
                content,
                phase,
            } => proto::V2ResponseItem {
                content: Some(proto::V2ResponseItemContentList {
                    values: content
                        .into_iter()
                        .map(|item| {
                            use proto::v2_response_item_content_item::Value;

                            Ok(proto::V2ResponseItemContentItem {
                                value: Some(Value::Variant1(<ContentItem as DirectSchemaProto<
                                    proto::V2ContentItem,
                                >>::encode_schema(
                                    item
                                )?)),
                            })
                        })
                        .collect::<Result<Vec<_>, Status>>()?,
                }),
                id,
                phase: phase
                    .map(<MessagePhase as DirectSchemaProto<proto::V2MessagePhase>>::encode_schema)
                    .transpose()?,
                role: Some(role),
                r#type: "message".to_owned(),
                ..Default::default()
            },
            Self::Reasoning {
                id,
                summary,
                content,
                encrypted_content,
            } => proto::V2ResponseItem {
                content: content
                    .map(|content| {
                        Ok::<_, Status>(proto::V2ResponseItemContentList {
                            values: content
                                .into_iter()
                                .map(|item| {
                                    use proto::v2_response_item_content_item::Value;

                                    Ok(proto::V2ResponseItemContentItem {
                                        value: Some(Value::Variant2(
                                            <ReasoningItemContent as DirectSchemaProto<
                                                proto::V2ReasoningItemContent,
                                            >>::encode_schema(
                                                item
                                            )?,
                                        )),
                                    })
                                })
                                .collect::<Result<Vec<_>, Status>>()?,
                        })
                    })
                    .transpose()?,
                id: Some(id),
                r#type: "reasoning".to_owned(),
                encrypted_content,
                summary: Some(proto::V2ResponseItemSummaryList {
                    values: summary
                        .into_iter()
                        .map(
                            <ReasoningItemReasoningSummary as DirectSchemaProto<
                                proto::V2ReasoningItemReasoningSummary,
                            >>::encode_schema,
                        )
                        .collect::<Result<Vec<_>, _>>()?,
                }),
                ..Default::default()
            },
            Self::LocalShellCall {
                id,
                call_id,
                status,
                action,
            } => {
                use proto::v2_response_item_action::Value;

                proto::V2ResponseItem {
                    id,
                    r#type: "local_shell_call".to_owned(),
                    action: Some(proto::V2ResponseItemAction {
                        value: Some(Value::Variant1(<LocalShellAction as DirectSchemaProto<
                            proto::V2LocalShellAction,
                        >>::encode_schema(
                            action
                        )?)),
                    }),
                    call_id,
                    status: Some(
                        <LocalShellStatus as DirectSchemaProto<String>>::encode_schema(status)?,
                    ),
                    ..Default::default()
                }
            }
            Self::FunctionCall {
                id,
                name,
                namespace,
                arguments,
                call_id,
            } => {
                use proto::v2_response_item_arguments::Value;

                proto::V2ResponseItem {
                    id,
                    r#type: "function_call".to_owned(),
                    call_id: Some(call_id),
                    arguments: Some(proto::V2ResponseItemArguments {
                        value: Some(Value::StringValue(arguments)),
                    }),
                    name: Some(name),
                    namespace,
                    ..Default::default()
                }
            }
            Self::ToolSearchCall {
                id,
                call_id,
                status,
                execution,
                arguments,
            } => {
                use proto::v2_response_item_arguments::Value;

                proto::V2ResponseItem {
                    id,
                    r#type: "tool_search_call".to_owned(),
                    call_id,
                    status,
                    arguments: Some(proto::V2ResponseItemArguments {
                        value: Some(Value::Variant2(
                            super::super::super::grpc_api_conversions::encode_dynamic_value(
                                arguments,
                            )?,
                        )),
                    }),
                    execution: Some(execution),
                    ..Default::default()
                }
            }
            Self::FunctionCallOutput { call_id, output } => {
                let FunctionCallOutputPayload { body, success: _ } = output;
                proto::V2ResponseItem {
                    r#type: "function_call_output".to_owned(),
                    call_id: Some(call_id),
                    output: Some(<FunctionCallOutputBody as DirectSchemaProto<
                        proto::V2FunctionCallOutputBody,
                    >>::encode_schema(body)?),
                    ..Default::default()
                }
            }
            Self::CustomToolCall {
                id,
                status,
                call_id,
                name,
                input,
            } => proto::V2ResponseItem {
                id,
                r#type: "custom_tool_call".to_owned(),
                call_id: Some(call_id),
                status,
                name: Some(name),
                input: Some(input),
                ..Default::default()
            },
            Self::CustomToolCallOutput {
                call_id,
                name,
                output,
            } => {
                let FunctionCallOutputPayload { body, success: _ } = output;
                proto::V2ResponseItem {
                    r#type: "custom_tool_call_output".to_owned(),
                    call_id: Some(call_id),
                    name,
                    output: Some(<FunctionCallOutputBody as DirectSchemaProto<
                        proto::V2FunctionCallOutputBody,
                    >>::encode_schema(body)?),
                    ..Default::default()
                }
            }
            Self::ToolSearchOutput {
                call_id,
                status,
                execution,
                tools,
            } => proto::V2ResponseItem {
                r#type: "tool_search_output".to_owned(),
                call_id,
                status: Some(status),
                execution: Some(execution),
                tools: Some(proto::V2ResponseItemToolsList {
                    values: tools
                        .into_iter()
                        .map(super::super::super::grpc_api_conversions::encode_dynamic_value)
                        .collect::<Result<Vec<_>, _>>()?,
                }),
                ..Default::default()
            },
            Self::WebSearchCall { id, status, action } => {
                use proto::v2_response_item_action::Value;

                proto::V2ResponseItem {
                    id,
                    r#type: "web_search_call".to_owned(),
                    status,
                    action: action
                        .map(|action| {
                            Ok::<_, Status>(proto::V2ResponseItemAction {
                                value: Some(Value::Variant2(
                                    <WebSearchAction as DirectSchemaProto<
                                        proto::V2ResponsesApiWebSearchAction,
                                    >>::encode_schema(action)?,
                                )),
                            })
                        })
                        .transpose()?,
                    ..Default::default()
                }
            }
            Self::ImageGenerationCall {
                id,
                status,
                revised_prompt,
                result,
            } => proto::V2ResponseItem {
                id: Some(id),
                r#type: "image_generation_call".to_owned(),
                status: Some(status),
                result: Some(result),
                revised_prompt,
                ..Default::default()
            },
            Self::Compaction { encrypted_content } => proto::V2ResponseItem {
                r#type: "compaction".to_owned(),
                encrypted_content: Some(encrypted_content),
                ..Default::default()
            },
            Self::CompactionTrigger => proto::V2ResponseItem {
                r#type: "compaction_trigger".to_owned(),
                ..Default::default()
            },
            Self::ContextCompaction { encrypted_content } => proto::V2ResponseItem {
                r#type: "context_compaction".to_owned(),
                encrypted_content,
                ..Default::default()
            },
            Self::Other => proto::V2ResponseItem {
                r#type: "other".to_owned(),
                ..Default::default()
            },
        })
    }
}
