use super::*;

impl DirectSchemaProto<proto::V2ForcedChatgptWorkspaceIds>
    for codex_app_server_protocol::ForcedChatgptWorkspaceIds
{
    fn decode_schema(payload: proto::V2ForcedChatgptWorkspaceIds) -> Result<Self, Status> {
        match payload
            .value
            .ok_or_else(|| missing("ForcedChatgptWorkspaceIds.value"))?
        {
            proto::v2_forced_chatgpt_workspace_ids::Value::StringValue(value) => {
                Ok(Self::Single(value))
            }
            proto::v2_forced_chatgpt_workspace_ids::Value::Variant2(values) => {
                Ok(Self::Multiple(values.values))
            }
        }
    }

    fn encode_schema(self) -> Result<proto::V2ForcedChatgptWorkspaceIds, Status> {
        let value = match self {
            Self::Single(value) => {
                proto::v2_forced_chatgpt_workspace_ids::Value::StringValue(value)
            }
            Self::Multiple(values) => proto::v2_forced_chatgpt_workspace_ids::Value::Variant2(
                proto::V2ThreadStartParamsRuntimeWorkspaceRootsList { values },
            ),
        };
        Ok(proto::V2ForcedChatgptWorkspaceIds { value: Some(value) })
    }
}

impl DirectSchemaProto<proto::V2RequestId> for codex_app_server_protocol::RequestId {
    fn decode_schema(payload: proto::V2RequestId) -> Result<Self, Status> {
        match payload.value.ok_or_else(|| missing("RequestId.value"))? {
            proto::v2_request_id::Value::StringValue(value) => Ok(Self::String(value)),
            proto::v2_request_id::Value::Int64Value(value) => Ok(Self::Integer(value)),
        }
    }

    fn encode_schema(self) -> Result<proto::V2RequestId, Status> {
        let value = match self {
            Self::String(value) => proto::v2_request_id::Value::StringValue(value),
            Self::Integer(value) => proto::v2_request_id::Value::Int64Value(value),
        };
        Ok(proto::V2RequestId { value: Some(value) })
    }
}

impl DirectSchemaProto<proto::V2ResourceContent> for codex_protocol::mcp::ResourceContent {
    fn decode_schema(payload: proto::V2ResourceContent) -> Result<Self, Status> {
        let proto::V2ResourceContent {
            meta,
            mime_type,
            text,
            uri,
            blob,
        } = payload;
        let meta = meta
            .map(super::super::super::grpc_api_conversions::decode_dynamic_value)
            .transpose()?;

        match (text, blob) {
            (Some(text), None) => Ok(Self::Text {
                uri,
                mime_type,
                text,
                meta,
            }),
            (None, Some(blob)) => Ok(Self::Blob {
                uri,
                mime_type,
                blob,
                meta,
            }),
            (Some(_), Some(_)) => Err(invalid("ResourceContent", "both `text` and `blob` are set")),
            (None, None) => Err(missing("ResourceContent.text or ResourceContent.blob")),
        }
    }

    fn encode_schema(self) -> Result<proto::V2ResourceContent, Status> {
        match self {
            Self::Text {
                uri,
                mime_type,
                text,
                meta,
            } => Ok(proto::V2ResourceContent {
                meta: meta
                    .map(super::super::super::grpc_api_conversions::encode_dynamic_value)
                    .transpose()?,
                mime_type,
                text: Some(text),
                uri,
                blob: None,
            }),
            Self::Blob {
                uri,
                mime_type,
                blob,
                meta,
            } => Ok(proto::V2ResourceContent {
                meta: meta
                    .map(super::super::super::grpc_api_conversions::encode_dynamic_value)
                    .transpose()?,
                mime_type,
                text: None,
                uri,
                blob: Some(blob),
            }),
        }
    }
}

impl DirectSchemaProto<proto::V2ThreadListCwdFilter>
    for codex_app_server_protocol::ThreadListCwdFilter
{
    fn decode_schema(payload: proto::V2ThreadListCwdFilter) -> Result<Self, Status> {
        match payload
            .value
            .ok_or_else(|| missing("ThreadListCwdFilter.value"))?
        {
            proto::v2_thread_list_cwd_filter::Value::StringValue(value) => Ok(Self::One(value)),
            proto::v2_thread_list_cwd_filter::Value::Variant2(values) => {
                Ok(Self::Many(values.values))
            }
        }
    }

    fn encode_schema(self) -> Result<proto::V2ThreadListCwdFilter, Status> {
        let value = match self {
            Self::One(value) => proto::v2_thread_list_cwd_filter::Value::StringValue(value),
            Self::Many(values) => proto::v2_thread_list_cwd_filter::Value::Variant2(
                proto::V2ThreadStartParamsRuntimeWorkspaceRootsList { values },
            ),
        };
        Ok(proto::V2ThreadListCwdFilter { value: Some(value) })
    }
}
