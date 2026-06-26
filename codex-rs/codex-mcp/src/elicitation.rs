//! MCP elicitation request tracking and policy handling.
//!
//! RMCP clients call into this module when a server asks Codex to elicit data
//! from the user. It decides whether the request can be automatically accepted,
//! must be declined by policy, or should be surfaced as a Codex protocol event
//! and later resolved through the stored responder.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use crate::mcp::McpPermissionPromptAutoApproveContext;
use crate::mcp::mcp_permission_prompt_is_auto_approved;
use crate::server::McpElicitationRuntimeMetadata;
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
use tokio::sync::oneshot;

#[derive(Debug, Clone)]
pub struct ElicitationReviewRequest {
    pub server_name: String,
    pub request_id: RequestId,
    pub elicitation: Elicitation,
    pub server_runtime_metadata: McpElicitationRuntimeMetadata,
}

pub trait ElicitationReviewer: Send + Sync {
    fn review(
        &self,
        request: ElicitationReviewRequest,
    ) -> BoxFuture<'static, Result<Option<ElicitationResponse>>>;
}

pub type ElicitationReviewerHandle = Arc<dyn ElicitationReviewer>;

#[derive(Clone, Default)]
pub(crate) struct McpElicitationState {
    auto_deny: Arc<AtomicBool>,
    next_request_id: Arc<AtomicU64>,
    requests: Arc<StdMutex<ResponderMap>>,
}

impl McpElicitationState {
    pub(crate) fn auto_deny(&self) -> bool {
        self.auto_deny.load(Ordering::Acquire)
    }

    pub(crate) fn set_auto_deny(&self, auto_deny: bool) {
        self.auto_deny.store(auto_deny, Ordering::Release);
    }

    fn next_request_id(&self) -> RequestId {
        let id = self.next_request_id.fetch_add(1, Ordering::Relaxed);
        RequestId::String(Arc::from(format!("codex-mcp-elicitation-{id}")))
    }

    fn register(&self, server_name: String, id: RequestId) -> Result<PendingElicitationResponse> {
        let key = (server_name, id);
        let (tx, rx) = oneshot::channel();
        let mut requests = self
            .requests
            .lock()
            .map_err(|_| anyhow!("elicitation request router lock poisoned"))?;
        if requests.contains_key(&key) {
            return Err(anyhow!("duplicate elicitation request identifier"));
        }
        requests.insert(key.clone(), tx);
        drop(requests);
        Ok(PendingElicitationResponse {
            key,
            requests: Arc::clone(&self.requests),
            response: rx,
        })
    }

    pub(crate) fn resolve(
        &self,
        server_name: String,
        id: RequestId,
        response: ElicitationResponse,
    ) -> Result<()> {
        let sender = self
            .requests
            .lock()
            .map_err(|_| anyhow!("elicitation request router lock poisoned"))?
            .remove(&(server_name, id))
            .ok_or_else(|| anyhow!("elicitation request not found"))?;
        sender
            .send(response)
            .map_err(|error| anyhow!("failed to send elicitation response: {error:?}"))
    }
}

struct PendingElicitationResponse {
    key: (String, RequestId),
    requests: Arc<StdMutex<ResponderMap>>,
    response: oneshot::Receiver<ElicitationResponse>,
}

impl PendingElicitationResponse {
    async fn wait(mut self) -> Result<ElicitationResponse> {
        (&mut self.response)
            .await
            .context("elicitation request channel closed unexpectedly")
    }
}

impl Drop for PendingElicitationResponse {
    fn drop(&mut self) {
        if let Ok(mut requests) = self.requests.lock() {
            requests.remove(&self.key);
        }
    }
}

#[derive(Clone)]
pub(crate) struct ElicitationRequestManager {
    pub(crate) approval_policy: Arc<StdMutex<AskForApproval>>,
    pub(crate) permission_profile: Arc<StdMutex<PermissionProfile>>,
    state: McpElicitationState,
    reviewer: Option<ElicitationReviewerHandle>,
    server_runtime_metadata: McpElicitationRuntimeMetadata,
}

impl ElicitationRequestManager {
    #[cfg(test)]
    pub(crate) fn new(
        approval_policy: AskForApproval,
        permission_profile: PermissionProfile,
        reviewer: Option<ElicitationReviewerHandle>,
    ) -> Self {
        Self::new_with_state(
            approval_policy,
            permission_profile,
            reviewer,
            McpElicitationRuntimeMetadata::default(),
            McpElicitationState::default(),
        )
    }

    pub(crate) fn new_with_state(
        approval_policy: AskForApproval,
        permission_profile: PermissionProfile,
        reviewer: Option<ElicitationReviewerHandle>,
        server_runtime_metadata: McpElicitationRuntimeMetadata,
        state: McpElicitationState,
    ) -> Self {
        Self {
            approval_policy: Arc::new(StdMutex::new(approval_policy)),
            permission_profile: Arc::new(StdMutex::new(permission_profile)),
            state,
            reviewer,
            server_runtime_metadata,
        }
    }

    pub(crate) fn make_sender(
        &self,
        server_name: String,
        tx_event: Sender<Event>,
    ) -> SendElicitation {
        let approval_policy = self.approval_policy.clone();
        let permission_profile = self.permission_profile.clone();
        let state = self.state.clone();
        let reviewer = self.reviewer.clone();
        let server_runtime_metadata = self.server_runtime_metadata.clone();
        Box::new(move |_upstream_id, elicitation| {
            let tx_event = tx_event.clone();
            let server_name = server_name.clone();
            let approval_policy = approval_policy.clone();
            let permission_profile = permission_profile.clone();
            let state = state.clone();
            let reviewer = reviewer.clone();
            let server_runtime_metadata = server_runtime_metadata.clone();
            async move {
                if state.auto_deny() {
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

                let request_id = state.next_request_id();
                if let Some(reviewer) = reviewer.as_ref() {
                    let request = ElicitationReviewRequest {
                        server_name: server_name.clone(),
                        request_id: request_id.clone(),
                        elicitation: elicitation.clone(),
                        server_runtime_metadata,
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
                let pending = state.register(server_name.clone(), request_id.clone())?;
                tx_event
                    .send(Event {
                        id: "mcp_elicitation_request".to_string(),
                        msg: EventMsg::ElicitationRequest(ElicitationRequestEvent {
                            turn_id: None,
                            server_name,
                            id: match request_id {
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
                    .await
                    .context("failed to send MCP elicitation request event")?;
                pending.wait().await
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

#[cfg(test)]
#[path = "elicitation_tests.rs"]
mod tests;
