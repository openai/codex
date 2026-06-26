use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use codex_rmcp_client::Elicitation;
use codex_rmcp_client::ElicitationResponse;
use rmcp::model::ClientCapabilities;
use rmcp::model::ClientResult;
use rmcp::model::CreateElicitationRequest;
use rmcp::model::CustomRequest;
use rmcp::model::ElicitationCapability;
use rmcp::model::FormElicitationCapability;
use rmcp::model::GetMeta;
use rmcp::model::JsonObject;
use rmcp::model::Meta;
use rmcp::model::RequestId;
use rmcp::model::ServerRequest;
use rmcp::model::UrlElicitationCapability;
use rmcp::service::Peer;
use rmcp::service::RoleServer;
use tokio::sync::OwnedSemaphorePermit;
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;

const OPENAI_FORM_METHOD: &str = "openai/form";

/// Correlates an upstream Apps elicitation with the downstream MCP client whose tool call
/// triggered it.
///
/// Each hosted Apps connection belongs to one downstream MCP session. Calls within that session
/// are serialized while its peer is installed, giving upstream elicitations an unambiguous route
/// without blocking other sessions or teaching the generic MCP manager about Apps or connectors.
pub(crate) struct AppsElicitationBridge {
    call_permit: Arc<Semaphore>,
    downstream: Mutex<Option<Arc<ActiveDownstream>>>,
}

struct ActiveDownstream {
    peer: Peer<RoleServer>,
    cancelled: CancellationToken,
}

impl AppsElicitationBridge {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            call_permit: Arc::new(Semaphore::new(1)),
            downstream: Mutex::new(None),
        })
    }

    pub(crate) fn upstream_capabilities(auth_elicitation_enabled: bool) -> ClientCapabilities {
        let mut capabilities = ClientCapabilities::default();
        if auth_elicitation_enabled {
            capabilities.elicitation = Some(ElicitationCapability {
                form: Some(FormElicitationCapability::default()),
                url: Some(UrlElicitationCapability::default()),
            });
        }
        capabilities.extensions = Some(BTreeMap::from([(
            OPENAI_FORM_METHOD.to_string(),
            JsonObject::new(),
        )]));
        capabilities
    }

    pub(crate) async fn begin_call(
        self: &Arc<Self>,
        downstream: Peer<RoleServer>,
    ) -> Result<AppsElicitationCallGuard> {
        let permit = match Arc::clone(&self.call_permit).try_acquire_owned() {
            Ok(permit) => permit,
            Err(tokio::sync::TryAcquireError::NoPermits) => Arc::clone(&self.call_permit)
                .acquire_owned()
                .await
                .context("Codex Apps elicitation bridge is closed")?,
            Err(tokio::sync::TryAcquireError::Closed) => {
                bail!("Codex Apps elicitation bridge is closed")
            }
        };
        let active = Arc::new(ActiveDownstream {
            peer: downstream,
            cancelled: CancellationToken::new(),
        });
        *self.lock_downstream() = Some(Arc::clone(&active));
        Ok(AppsElicitationCallGuard {
            bridge: Arc::clone(self),
            active,
            _permit: permit,
        })
    }

    pub(crate) async fn forward(
        &self,
        upstream_request_id: RequestId,
        elicitation: Elicitation,
    ) -> Result<ElicitationResponse> {
        let Some(active) = self.lock_downstream().clone() else {
            tracing::debug!(
                request_id = %request_id_string(&upstream_request_id),
                "cancelling Codex Apps elicitation without an active downstream request"
            );
            return Ok(cancelled_response());
        };

        let request = match elicitation {
            Elicitation::Mcp(params) => {
                if !supports_mcp_elicitation(&active.peer, &params) {
                    tracing::debug!(
                        request_id = %request_id_string(&upstream_request_id),
                        "cancelling Codex Apps elicitation unsupported by the downstream client"
                    );
                    return Ok(cancelled_response());
                }
                ServerRequest::CreateElicitationRequest(CreateElicitationRequest::new(params))
            }
            Elicitation::OpenAiForm {
                meta,
                message,
                requested_schema,
            } => {
                if !supports_openai_form(&active.peer) {
                    tracing::debug!(
                        request_id = %request_id_string(&upstream_request_id),
                        "cancelling Codex Apps openai/form elicitation unsupported by the downstream client"
                    );
                    return Ok(cancelled_response());
                }
                let params = serde_json::Map::from_iter([
                    ("message".to_string(), serde_json::Value::String(message)),
                    ("requestedSchema".to_string(), requested_schema),
                ]);
                let mut request =
                    CustomRequest::new(OPENAI_FORM_METHOD, Some(serde_json::Value::Object(params)));
                if let Some(meta) = meta {
                    let meta = meta
                        .as_object()
                        .cloned()
                        .context("Codex Apps openai/form elicitation metadata must be an object")?;
                    request.get_meta_mut().extend(Meta(meta));
                }
                ServerRequest::CustomRequest(request)
            }
        };
        let result = tokio::select! {
            result = active.peer.send_request(request) => result.with_context(|| {
                format!(
                    "failed to forward Codex Apps elicitation `{}` to the downstream MCP client",
                    request_id_string(&upstream_request_id)
                )
            })?,
            _ = active.cancelled.cancelled() => return Ok(cancelled_response()),
        };

        response_from_client_result(result).with_context(|| {
            format!(
                "invalid response to Codex Apps elicitation `{}` from the downstream MCP client",
                request_id_string(&upstream_request_id)
            )
        })
    }

    fn lock_downstream(&self) -> std::sync::MutexGuard<'_, Option<Arc<ActiveDownstream>>> {
        self.downstream
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

fn supports_mcp_elicitation(
    peer: &Peer<RoleServer>,
    params: &rmcp::model::CreateElicitationRequestParams,
) -> bool {
    let Some(info) = peer.peer_info() else {
        return false;
    };
    let Some(capability) = info.capabilities.elicitation.as_ref() else {
        return false;
    };
    match params {
        rmcp::model::CreateElicitationRequestParams::FormElicitationParams { .. } => {
            capability.form.is_some()
        }
        rmcp::model::CreateElicitationRequestParams::UrlElicitationParams { .. } => {
            capability.url.is_some()
        }
    }
}

pub(crate) fn supports_url_elicitation(peer: &Peer<RoleServer>) -> bool {
    peer.peer_info().is_some_and(|info| {
        info.capabilities
            .elicitation
            .as_ref()
            .is_some_and(|capability| capability.url.is_some())
    })
}

fn supports_openai_form(peer: &Peer<RoleServer>) -> bool {
    peer.peer_info().is_some_and(|info| {
        info.capabilities
            .extensions
            .as_ref()
            .is_some_and(|extensions| extensions.contains_key(OPENAI_FORM_METHOD))
    })
}

pub(crate) struct AppsElicitationCallGuard {
    bridge: Arc<AppsElicitationBridge>,
    active: Arc<ActiveDownstream>,
    _permit: OwnedSemaphorePermit,
}

impl Drop for AppsElicitationCallGuard {
    fn drop(&mut self) {
        self.active.cancelled.cancel();
        self.bridge.lock_downstream().take();
    }
}

fn cancelled_response() -> ElicitationResponse {
    ElicitationResponse {
        action: rmcp::model::ElicitationAction::Cancel,
        content: None,
        meta: None,
    }
}

fn response_from_client_result(result: ClientResult) -> Result<ElicitationResponse> {
    match result {
        ClientResult::CreateElicitationResult(result) => Ok(ElicitationResponse {
            action: result.action,
            content: result.content,
            meta: result.meta.map(|meta| serde_json::Value::Object(meta.0)),
        }),
        ClientResult::CustomResult(result) => serde_json::from_value(result.0)
            .context("downstream MCP client returned an invalid elicitation response"),
        unexpected => bail!(
            "downstream MCP client returned an unexpected elicitation response: {unexpected:?}"
        ),
    }
}

fn request_id_string(id: &RequestId) -> String {
    match id {
        rmcp::model::NumberOrString::String(value) => value.to_string(),
        rmcp::model::NumberOrString::Number(value) => value.to_string(),
    }
}
