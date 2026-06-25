//! MCP elicitation request tracking and policy handling.
//!
//! RMCP clients call into this module when a server asks Codex to elicit data
//! from the user. It decides whether the request can be automatically accepted,
//! must be declined by policy, or should be surfaced as a Codex protocol event
//! and later resolved through the stored responder.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use crate::mcp::McpPermissionPromptAutoApproveContext;
use crate::mcp::mcp_permission_prompt_is_auto_approved;
use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use async_channel::Sender;
use codex_protocol::approvals::ElicitationRequest;
use codex_protocol::approvals::ElicitationRequestEvent;
use codex_protocol::mcp::RequestId as ProtocolRequestId;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_rmcp_client::Elicitation;
use codex_rmcp_client::ElicitationResponse;
use codex_rmcp_client::SendElicitation;
use futures::future::BoxFuture;
use futures::future::FutureExt;
use rmcp::model::ElicitationAction;
use rmcp::model::RequestId;
use tokio::sync::Mutex;
use tokio::sync::oneshot;

#[derive(Debug, Clone)]
pub struct ElicitationReviewRequest {
    pub server_name: String,
    pub request_id: RequestId,
    pub elicitation: Elicitation,
}

pub trait ElicitationReviewer: Send + Sync {
    fn review(
        &self,
        request: ElicitationReviewRequest,
    ) -> BoxFuture<'static, Result<Option<ElicitationResponse>>>;
}

pub type ElicitationReviewerHandle = Arc<dyn ElicitationReviewer>;

static NEXT_ELICITATION_ROUTE_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone)]
pub(crate) struct ElicitationRequestManager {
    requests: Arc<Mutex<ResponderMap>>,
    pub(crate) approval_policy: Arc<StdMutex<AskForApproval>>,
    pub(crate) permission_profile: Arc<StdMutex<PermissionProfile>>,
    auto_deny: Arc<StdMutex<bool>>,
    reviewer: Option<ElicitationReviewerHandle>,
    route_id: u64,
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
            route_id: NEXT_ELICITATION_ROUTE_ID.fetch_add(1, Ordering::Relaxed),
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
        if self.try_resolve(server_name, id, response).await? {
            Ok(())
        } else {
            Err(anyhow!("elicitation request not found"))
        }
    }

    pub(crate) async fn try_resolve(
        &self,
        server_name: String,
        id: RequestId,
        response: ElicitationResponse,
    ) -> Result<bool> {
        let Some(responder) = self.requests.lock().await.remove(&(server_name, id)) else {
            return Ok(false);
        };
        responder
            .send(response)
            .map_err(|e| anyhow!("failed to send elicitation response: {e:?}"))?;
        Ok(true)
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
        let route_id = self.route_id;
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

                let request = match elicitation {
                    Elicitation::Mcp(
                        rmcp::model::CreateElicitationRequestParams::FormElicitationParams {
                            meta,
                            message,
                            requested_schema,
                        },
                    ) => ElicitationRequest::Form {
                        meta: meta
                            .map(serde_json::to_value)
                            .transpose()
                            .context("failed to serialize MCP elicitation metadata")?,
                        message,
                        requested_schema: serde_json::to_value(requested_schema)
                            .context("failed to serialize MCP elicitation schema")?,
                    },
                    Elicitation::Mcp(
                        rmcp::model::CreateElicitationRequestParams::UrlElicitationParams {
                            meta,
                            message,
                            url,
                            elicitation_id,
                        },
                    ) => ElicitationRequest::Url {
                        meta: meta
                            .map(serde_json::to_value)
                            .transpose()
                            .context("failed to serialize MCP elicitation metadata")?,
                        message,
                        url,
                        elicitation_id,
                    },
                    Elicitation::OpenAiForm {
                        meta,
                        message,
                        requested_schema,
                    } => ElicitationRequest::OpenAiForm {
                        meta,
                        message,
                        requested_schema,
                    },
                };
                let (tx, rx) = oneshot::channel();
                let response_id = RequestId::String(Arc::from(format!("{route_id}:{id}")));
                {
                    let mut lock = elicitation_requests.lock().await;
                    lock.insert((server_name.clone(), response_id.clone()), tx);
                }
                let _ = tx_event
                    .send(Event {
                        id: "mcp_elicitation_request".to_string(),
                        msg: EventMsg::ElicitationRequest(ElicitationRequestEvent {
                            turn_id: None,
                            server_name,
                            id: match response_id {
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
        AskForApproval::OnRequest => false,
        AskForApproval::UnlessTrusted => false,
        AskForApproval::Granular(granular_config) => !granular_config.allows_mcp_elicitations(),
    }
}

type ResponderMap = HashMap<(String, RequestId), oneshot::Sender<ElicitationResponse>>;

fn can_auto_accept_elicitation(elicitation: &Elicitation) -> bool {
    match elicitation {
        Elicitation::Mcp(rmcp::model::CreateElicitationRequestParams::FormElicitationParams {
            requested_schema,
            ..
        }) => {
            // Auto-accept confirm/approval elicitations without schema requirements.
            requested_schema.properties.is_empty()
        }
        Elicitation::Mcp(rmcp::model::CreateElicitationRequestParams::UrlElicitationParams {
            ..
        })
        | Elicitation::OpenAiForm { .. } => false,
    }
}
