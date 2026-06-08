//! MCP elicitation request tracking and policy handling.
//!
//! RMCP clients call into this module when a server asks Codex to elicit data
//! from the user. It decides whether the request can be automatically accepted,
//! must be declined by policy, or should be surfaced as a Codex protocol event
//! and later resolved through the stored responder.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use crate::mcp::McpPermissionPromptAutoApproveContext;
use crate::mcp::mcp_permission_prompt_is_auto_approved;
use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use async_channel::Sender;
use codex_protocol::approvals::ElicitationRequest;
use codex_protocol::approvals::ElicitationRequestEvent;
use codex_protocol::approvals::McpElicitationArrayType;
use codex_protocol::approvals::McpElicitationBooleanSchema;
use codex_protocol::approvals::McpElicitationBooleanType;
use codex_protocol::approvals::McpElicitationConstOption;
use codex_protocol::approvals::McpElicitationEnumSchema;
use codex_protocol::approvals::McpElicitationLegacyTitledEnumSchema;
use codex_protocol::approvals::McpElicitationMultiSelectEnumSchema;
use codex_protocol::approvals::McpElicitationNumberSchema;
use codex_protocol::approvals::McpElicitationNumberType;
use codex_protocol::approvals::McpElicitationObjectType;
use codex_protocol::approvals::McpElicitationPrimitiveSchema;
use codex_protocol::approvals::McpElicitationSchema;
use codex_protocol::approvals::McpElicitationSingleSelectEnumSchema;
use codex_protocol::approvals::McpElicitationStringFormat;
use codex_protocol::approvals::McpElicitationStringSchema;
use codex_protocol::approvals::McpElicitationStringType;
use codex_protocol::approvals::McpElicitationTitledEnumItems;
use codex_protocol::approvals::McpElicitationTitledMultiSelectEnumSchema;
use codex_protocol::approvals::McpElicitationTitledSingleSelectEnumSchema;
use codex_protocol::approvals::McpElicitationUntitledEnumItems;
use codex_protocol::approvals::McpElicitationUntitledMultiSelectEnumSchema;
use codex_protocol::approvals::McpElicitationUntitledSingleSelectEnumSchema;
use codex_protocol::mcp::RequestId as ProtocolRequestId;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_rmcp_client::ElicitationResponse;
use codex_rmcp_client::SendElicitation;
use futures::future::BoxFuture;
use futures::future::FutureExt;
use rmcp::model::CreateElicitationRequestParams;
use rmcp::model::ElicitationAction;
use rmcp::model::RequestId;
use tokio::sync::Mutex;
use tokio::sync::oneshot;

#[derive(Debug, Clone)]
pub struct ElicitationReviewRequest {
    pub server_name: String,
    pub request_id: RequestId,
    pub elicitation: CreateElicitationRequestParams,
}

pub trait ElicitationReviewer: Send + Sync {
    fn review(
        &self,
        request: ElicitationReviewRequest,
    ) -> BoxFuture<'static, Result<Option<ElicitationResponse>>>;
}

pub type ElicitationReviewerHandle = Arc<dyn ElicitationReviewer>;

#[derive(Clone)]
pub(crate) struct ElicitationRequestManager {
    requests: Arc<Mutex<ResponderMap>>,
    pub(crate) approval_policy: Arc<StdMutex<AskForApproval>>,
    pub(crate) permission_profile: Arc<StdMutex<PermissionProfile>>,
    auto_deny: Arc<StdMutex<bool>>,
    reviewer: Option<ElicitationReviewerHandle>,
}

impl ElicitationRequestManager {
    pub(crate) fn new(
        approval_policy: AskForApproval,
        permission_profile: PermissionProfile,
        reviewer: Option<ElicitationReviewerHandle>,
    ) -> Self {
        Self {
            requests: Arc::new(Mutex::new(HashMap::new())),
            approval_policy: Arc::new(StdMutex::new(approval_policy)),
            permission_profile: Arc::new(StdMutex::new(permission_profile)),
            auto_deny: Arc::new(StdMutex::new(false)),
            reviewer,
        }
    }

    pub(crate) fn auto_deny(&self) -> bool {
        self.auto_deny
            .lock()
            .map(|auto_deny| *auto_deny)
            .unwrap_or(false)
    }

    pub(crate) fn set_auto_deny(&self, auto_deny: bool) {
        if let Ok(mut current) = self.auto_deny.lock() {
            *current = auto_deny;
        }
    }

    pub(crate) async fn resolve(
        &self,
        server_name: String,
        id: RequestId,
        response: ElicitationResponse,
    ) -> Result<()> {
        self.requests
            .lock()
            .await
            .remove(&(server_name, id))
            .ok_or_else(|| anyhow!("elicitation request not found"))?
            .send(response)
            .map_err(|e| anyhow!("failed to send elicitation response: {e:?}"))
    }

    pub(crate) fn make_sender(
        &self,
        server_name: String,
        tx_event: Sender<Event>,
    ) -> SendElicitation {
        let elicitation_requests = self.requests.clone();
        let approval_policy = self.approval_policy.clone();
        let permission_profile = self.permission_profile.clone();
        let auto_deny = self.auto_deny.clone();
        let reviewer = self.reviewer.clone();
        Box::new(move |id, elicitation| {
            let elicitation_requests = elicitation_requests.clone();
            let tx_event = tx_event.clone();
            let server_name = server_name.clone();
            let approval_policy = approval_policy.clone();
            let permission_profile = permission_profile.clone();
            let auto_deny = auto_deny.clone();
            let reviewer = reviewer.clone();
            async move {
                let auto_deny = auto_deny
                    .lock()
                    .map(|auto_deny| *auto_deny)
                    .unwrap_or(false);
                if auto_deny {
                    return Ok(ElicitationResponse {
                        action: ElicitationAction::Decline,
                        content: None,
                        meta: None,
                    });
                }

                let approval_policy = approval_policy
                    .lock()
                    .map(|policy| *policy)
                    .unwrap_or(AskForApproval::Never);
                let permission_profile = permission_profile
                    .lock()
                    .map(|profile| profile.clone())
                    .unwrap_or_default();
                if mcp_permission_prompt_is_auto_approved(
                    approval_policy,
                    &permission_profile,
                    McpPermissionPromptAutoApproveContext::default(),
                ) && can_auto_accept_elicitation(&elicitation)
                {
                    return Ok(ElicitationResponse {
                        action: ElicitationAction::Accept,
                        content: Some(serde_json::json!({})),
                        meta: None,
                    });
                }

                if elicitation_is_rejected_by_policy(approval_policy) {
                    return Ok(ElicitationResponse {
                        action: ElicitationAction::Decline,
                        content: None,
                        meta: None,
                    });
                }

                if let Some(reviewer) = reviewer.as_ref() {
                    let request = ElicitationReviewRequest {
                        server_name: server_name.clone(),
                        request_id: id.clone(),
                        elicitation: elicitation.clone(),
                    };
                    if let Some(response) = reviewer.review(request).await? {
                        return Ok(response);
                    }
                }

                let request = elicitation_request_from_rmcp(elicitation)?;
                let (tx, rx) = oneshot::channel();
                {
                    let mut lock = elicitation_requests.lock().await;
                    lock.insert((server_name.clone(), id.clone()), tx);
                }
                let _ = tx_event
                    .send(Event {
                        id: "mcp_elicitation_request".to_string(),
                        msg: EventMsg::ElicitationRequest(ElicitationRequestEvent {
                            turn_id: None,
                            server_name,
                            id: match id.clone() {
                                rmcp::model::NumberOrString::String(value) => {
                                    ProtocolRequestId::String(value.to_string())
                                }
                                rmcp::model::NumberOrString::Number(value) => {
                                    ProtocolRequestId::Integer(value)
                                }
                            },
                            request,
                        }),
                    })
                    .await;
                rx.await
                    .context("elicitation request channel closed unexpectedly")
            }
            .boxed()
        })
    }
}

pub(crate) fn elicitation_is_rejected_by_policy(approval_policy: AskForApproval) -> bool {
    match approval_policy {
        AskForApproval::Never => true,
        AskForApproval::OnFailure => false,
        AskForApproval::OnRequest => false,
        AskForApproval::UnlessTrusted => false,
        AskForApproval::Granular(granular_config) => !granular_config.allows_mcp_elicitations(),
    }
}

type ResponderMap = HashMap<(String, RequestId), oneshot::Sender<ElicitationResponse>>;

fn can_auto_accept_elicitation(elicitation: &CreateElicitationRequestParams) -> bool {
    match elicitation {
        CreateElicitationRequestParams::FormElicitationParams {
            requested_schema, ..
        } => {
            // Auto-accept confirm/approval elicitations without schema requirements.
            requested_schema.properties.is_empty()
        }
        CreateElicitationRequestParams::UrlElicitationParams { .. } => false,
    }
}

fn elicitation_request_from_rmcp(
    elicitation: CreateElicitationRequestParams,
) -> Result<ElicitationRequest> {
    match elicitation {
        CreateElicitationRequestParams::FormElicitationParams {
            meta,
            message,
            requested_schema,
        } => Ok(ElicitationRequest::Form {
            meta: meta.map(|meta| serde_json::Value::Object(meta.0)),
            message,
            requested_schema: elicitation_schema_from_rmcp(requested_schema)?,
        }),
        CreateElicitationRequestParams::UrlElicitationParams {
            meta,
            message,
            url,
            elicitation_id,
        } => Ok(ElicitationRequest::Url {
            meta: meta.map(|meta| serde_json::Value::Object(meta.0)),
            message,
            url,
            elicitation_id,
        }),
    }
}

fn elicitation_schema_from_rmcp(
    schema: rmcp::model::ElicitationSchema,
) -> Result<McpElicitationSchema> {
    if schema.title.is_some() || schema.description.is_some() {
        return Err(anyhow!(
            "top-level MCP elicitation schema title and description are not supported"
        ));
    }

    Ok(McpElicitationSchema {
        schema_uri: None,
        type_: McpElicitationObjectType::Object,
        properties: schema
            .properties
            .into_iter()
            .map(|(name, schema)| (name, primitive_schema_from_rmcp(schema)))
            .collect(),
        required: schema.required,
    })
}

fn primitive_schema_from_rmcp(
    schema: rmcp::model::PrimitiveSchema,
) -> McpElicitationPrimitiveSchema {
    match schema {
        rmcp::model::PrimitiveSchema::Enum(schema) => {
            McpElicitationPrimitiveSchema::Enum(enum_schema_from_rmcp(schema))
        }
        rmcp::model::PrimitiveSchema::String(schema) => {
            McpElicitationPrimitiveSchema::String(McpElicitationStringSchema {
                type_: McpElicitationStringType::String,
                title: schema.title.map(|value| value.into_owned()),
                description: schema.description.map(|value| value.into_owned()),
                min_length: schema.min_length,
                max_length: schema.max_length,
                format: schema.format.map(string_format_from_rmcp),
                default: schema.default,
            })
        }
        rmcp::model::PrimitiveSchema::Number(schema) => {
            McpElicitationPrimitiveSchema::Number(McpElicitationNumberSchema {
                type_: McpElicitationNumberType::Number,
                title: schema.title.map(|value| value.into_owned()),
                description: schema.description.map(|value| value.into_owned()),
                minimum: schema.minimum,
                maximum: schema.maximum,
                default: schema.default,
            })
        }
        rmcp::model::PrimitiveSchema::Integer(schema) => {
            McpElicitationPrimitiveSchema::Number(McpElicitationNumberSchema {
                type_: McpElicitationNumberType::Integer,
                title: schema.title.map(|value| value.into_owned()),
                description: schema.description.map(|value| value.into_owned()),
                minimum: schema.minimum.map(|value| value as f64),
                maximum: schema.maximum.map(|value| value as f64),
                default: schema.default.map(|value| value as f64),
            })
        }
        rmcp::model::PrimitiveSchema::Boolean(schema) => {
            McpElicitationPrimitiveSchema::Boolean(McpElicitationBooleanSchema {
                type_: McpElicitationBooleanType::Boolean,
                title: schema.title.map(|value| value.into_owned()),
                description: schema.description.map(|value| value.into_owned()),
                default: schema.default,
            })
        }
    }
}

fn string_format_from_rmcp(format: rmcp::model::StringFormat) -> McpElicitationStringFormat {
    match format {
        rmcp::model::StringFormat::Email => McpElicitationStringFormat::Email,
        rmcp::model::StringFormat::Uri => McpElicitationStringFormat::Uri,
        rmcp::model::StringFormat::Date => McpElicitationStringFormat::Date,
        rmcp::model::StringFormat::DateTime => McpElicitationStringFormat::DateTime,
    }
}

fn enum_schema_from_rmcp(schema: rmcp::model::EnumSchema) -> McpElicitationEnumSchema {
    match schema {
        rmcp::model::EnumSchema::Single(schema) => {
            McpElicitationEnumSchema::SingleSelect(single_select_enum_schema_from_rmcp(schema))
        }
        rmcp::model::EnumSchema::Multi(schema) => {
            McpElicitationEnumSchema::MultiSelect(multi_select_enum_schema_from_rmcp(schema))
        }
        rmcp::model::EnumSchema::Legacy(schema) => {
            McpElicitationEnumSchema::Legacy(McpElicitationLegacyTitledEnumSchema {
                type_: McpElicitationStringType::String,
                title: schema.title.map(|value| value.into_owned()),
                description: schema.description.map(|value| value.into_owned()),
                enum_: schema.enum_,
                enum_names: schema.enum_names,
                default: None,
            })
        }
    }
}

fn single_select_enum_schema_from_rmcp(
    schema: rmcp::model::SingleSelectEnumSchema,
) -> McpElicitationSingleSelectEnumSchema {
    match schema {
        rmcp::model::SingleSelectEnumSchema::Untitled(schema) => {
            McpElicitationSingleSelectEnumSchema::Untitled(
                McpElicitationUntitledSingleSelectEnumSchema {
                    type_: McpElicitationStringType::String,
                    title: schema.title.map(|value| value.into_owned()),
                    description: schema.description.map(|value| value.into_owned()),
                    enum_: schema.enum_,
                    default: schema.default,
                },
            )
        }
        rmcp::model::SingleSelectEnumSchema::Titled(schema) => {
            McpElicitationSingleSelectEnumSchema::Titled(
                McpElicitationTitledSingleSelectEnumSchema {
                    type_: McpElicitationStringType::String,
                    title: schema.title.map(|value| value.into_owned()),
                    description: schema.description.map(|value| value.into_owned()),
                    one_of: schema
                        .one_of
                        .into_iter()
                        .map(const_option_from_rmcp)
                        .collect(),
                    default: schema.default,
                },
            )
        }
    }
}

fn multi_select_enum_schema_from_rmcp(
    schema: rmcp::model::MultiSelectEnumSchema,
) -> McpElicitationMultiSelectEnumSchema {
    match schema {
        rmcp::model::MultiSelectEnumSchema::Untitled(schema) => {
            McpElicitationMultiSelectEnumSchema::Untitled(
                McpElicitationUntitledMultiSelectEnumSchema {
                    type_: McpElicitationArrayType::Array,
                    title: schema.title.map(|value| value.into_owned()),
                    description: schema.description.map(|value| value.into_owned()),
                    min_items: schema.min_items,
                    max_items: schema.max_items,
                    items: McpElicitationUntitledEnumItems {
                        type_: McpElicitationStringType::String,
                        enum_: schema.items.enum_,
                    },
                    default: schema.default,
                },
            )
        }
        rmcp::model::MultiSelectEnumSchema::Titled(schema) => {
            McpElicitationMultiSelectEnumSchema::Titled(McpElicitationTitledMultiSelectEnumSchema {
                type_: McpElicitationArrayType::Array,
                title: schema.title.map(|value| value.into_owned()),
                description: schema.description.map(|value| value.into_owned()),
                min_items: schema.min_items,
                max_items: schema.max_items,
                items: McpElicitationTitledEnumItems {
                    any_of: schema
                        .items
                        .any_of
                        .into_iter()
                        .map(const_option_from_rmcp)
                        .collect(),
                },
                default: schema.default,
            })
        }
    }
}

fn const_option_from_rmcp(option: rmcp::model::ConstTitle) -> McpElicitationConstOption {
    McpElicitationConstOption {
        const_: option.const_,
        title: option.title,
    }
}

#[cfg(test)]
#[path = "elicitation_tests.rs"]
mod tests;
