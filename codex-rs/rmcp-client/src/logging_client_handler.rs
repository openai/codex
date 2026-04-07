use std::sync::Arc;

use rmcp::RoleClient;
use rmcp::model::CancelledNotificationParam;
use rmcp::model::ClientInfo;
use rmcp::model::ClientResult;
use rmcp::model::CreateMessageRequestMethod;
use rmcp::model::CustomResult;
use rmcp::model::ElicitationAction;
use rmcp::model::ErrorCode;
use rmcp::model::LoggingLevel;
use rmcp::model::LoggingMessageNotificationParam;
use rmcp::model::Meta;
use rmcp::model::ProgressNotificationParam;
use rmcp::model::RequestParamsMeta;
use rmcp::model::ResourceUpdatedNotificationParam;
use rmcp::model::ServerNotification;
use rmcp::model::ServerRequest;
use rmcp::service::NotificationContext;
use rmcp::service::RequestContext;
use rmcp::service::Service;
use serde_json::Value;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::warn;

use crate::rmcp_client::Elicitation;
use crate::rmcp_client::ElicitationResponse;
use crate::rmcp_client::SendElicitation;

const MCP_PROGRESS_TOKEN_META_KEY: &str = "progressToken";

#[derive(Clone)]
pub(crate) struct LoggingClientHandler {
    client_info: ClientInfo,
    send_elicitation: Arc<SendElicitation>,
}

impl LoggingClientHandler {
    pub(crate) fn new(client_info: ClientInfo, send_elicitation: SendElicitation) -> Self {
        Self {
            client_info,
            send_elicitation: Arc::new(send_elicitation),
        }
    }
}

impl Service<RoleClient> for LoggingClientHandler {
    async fn handle_request(
        &self,
        request: ServerRequest,
        context: RequestContext<RoleClient>,
    ) -> Result<ClientResult, rmcp::ErrorData> {
        match request {
            ServerRequest::PingRequest(_) => Ok(ClientResult::empty(())),
            ServerRequest::CreateMessageRequest(_) => Err(rmcp::ErrorData::method_not_found::<
                CreateMessageRequestMethod,
            >()),
            ServerRequest::ListRootsRequest(_) => {
                Ok(ClientResult::ListRootsResult(Default::default()))
            }
            ServerRequest::CreateElicitationRequest(request) => self
                .create_elicitation(request.params, context)
                .await
                // RMCP's typed CreateElicitationResult does not model result-level `_meta`.
                .map(elicitation_response_result)
                .map(ClientResult::CustomResult),
            ServerRequest::CustomRequest(request) => Err(rmcp::ErrorData::new(
                ErrorCode::METHOD_NOT_FOUND,
                request.method,
                None,
            )),
        }
    }

    async fn handle_notification(
        &self,
        notification: ServerNotification,
        context: NotificationContext<RoleClient>,
    ) -> Result<(), rmcp::ErrorData> {
        match notification {
            ServerNotification::CancelledNotification(notification) => {
                self.on_cancelled(notification.params, context).await;
            }
            ServerNotification::ProgressNotification(notification) => {
                self.on_progress(notification.params, context).await;
            }
            ServerNotification::LoggingMessageNotification(notification) => {
                self.on_logging_message(notification.params, context).await;
            }
            ServerNotification::ResourceUpdatedNotification(notification) => {
                self.on_resource_updated(notification.params, context).await;
            }
            ServerNotification::ResourceListChangedNotification(_) => {
                self.on_resource_list_changed(context).await;
            }
            ServerNotification::ToolListChangedNotification(_) => {
                self.on_tool_list_changed(context).await;
            }
            ServerNotification::PromptListChangedNotification(_) => {
                self.on_prompt_list_changed(context).await;
            }
            ServerNotification::ElicitationCompletionNotification(_) => {}
            ServerNotification::CustomNotification(_) => {}
        }
        Ok(())
    }

    fn get_info(&self) -> ClientInfo {
        self.client_info.clone()
    }
}

impl LoggingClientHandler {
    async fn create_elicitation(
        &self,
        request: Elicitation,
        context: RequestContext<RoleClient>,
    ) -> Result<ElicitationResponse, rmcp::ErrorData> {
        let RequestContext { id, meta, .. } = context;
        let request = restore_context_meta(request, meta);
        (self.send_elicitation)(id, request)
            .await
            .map_err(|err| rmcp::ErrorData::internal_error(err.to_string(), None))
    }

    async fn on_cancelled(
        &self,
        params: CancelledNotificationParam,
        _context: NotificationContext<RoleClient>,
    ) {
        info!(
            "MCP server cancelled request (request_id: {}, reason: {:?})",
            params.request_id, params.reason
        );
    }

    async fn on_progress(
        &self,
        params: ProgressNotificationParam,
        _context: NotificationContext<RoleClient>,
    ) {
        info!(
            "MCP server progress notification (token: {:?}, progress: {}, total: {:?}, message: {:?})",
            params.progress_token, params.progress, params.total, params.message
        );
    }

    async fn on_resource_updated(
        &self,
        params: ResourceUpdatedNotificationParam,
        _context: NotificationContext<RoleClient>,
    ) {
        info!("MCP server resource updated (uri: {})", params.uri);
    }

    async fn on_resource_list_changed(&self, _context: NotificationContext<RoleClient>) {
        info!("MCP server resource list changed");
    }

    async fn on_tool_list_changed(&self, _context: NotificationContext<RoleClient>) {
        info!("MCP server tool list changed");
    }

    async fn on_prompt_list_changed(&self, _context: NotificationContext<RoleClient>) {
        info!("MCP server prompt list changed");
    }

    async fn on_logging_message(
        &self,
        params: LoggingMessageNotificationParam,
        _context: NotificationContext<RoleClient>,
    ) {
        let LoggingMessageNotificationParam {
            level,
            logger,
            data,
        } = params;
        let logger = logger.as_deref();
        match level {
            LoggingLevel::Emergency
            | LoggingLevel::Alert
            | LoggingLevel::Critical
            | LoggingLevel::Error => {
                error!(
                    "MCP server log message (level: {:?}, logger: {:?}, data: {})",
                    level, logger, data
                );
            }
            LoggingLevel::Warning => {
                warn!(
                    "MCP server log message (level: {:?}, logger: {:?}, data: {})",
                    level, logger, data
                );
            }
            LoggingLevel::Notice | LoggingLevel::Info => {
                info!(
                    "MCP server log message (level: {:?}, logger: {:?}, data: {})",
                    level, logger, data
                );
            }
            LoggingLevel::Debug => {
                debug!(
                    "MCP server log message (level: {:?}, logger: {:?}, data: {})",
                    level, logger, data
                );
            }
        }
    }
}

fn restore_context_meta(mut request: Elicitation, mut context_meta: Meta) -> Elicitation {
    // RMCP lifts JSON-RPC `_meta` into RequestContext before invoking handlers.
    context_meta.remove(MCP_PROGRESS_TOKEN_META_KEY);
    if context_meta.is_empty() {
        return request;
    }

    match request.meta_mut() {
        Some(meta) => {
            meta.extend(context_meta);
        }
        meta @ None => {
            *meta = Some(context_meta);
        }
    }
    request
}

fn elicitation_response_result(response: ElicitationResponse) -> CustomResult {
    let ElicitationResponse {
        action,
        content,
        meta,
    } = response;
    let action = match action {
        ElicitationAction::Accept => "accept",
        ElicitationAction::Decline => "decline",
        ElicitationAction::Cancel => "cancel",
    };

    let mut result =
        serde_json::Map::from_iter([("action".to_string(), Value::String(action.to_string()))]);
    if let Some(content) = content {
        result.insert("content".to_string(), content);
    }
    if let Some(meta) = meta {
        result.insert("_meta".to_string(), meta);
    }

    CustomResult(Value::Object(result))
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rmcp::model::BooleanSchema;
    use rmcp::model::CreateElicitationRequestParams;
    use rmcp::model::ElicitationSchema;
    use rmcp::model::PrimitiveSchema;
    use serde_json::Value;
    use serde_json::json;

    use super::*;

    #[test]
    fn restore_context_meta_adds_elicitation_meta() {
        let request = restore_context_meta(
            form_request(None),
            meta(json!({
                "progressToken": "progress-token",
                "persist": ["session", "always"],
            })),
        );

        assert_eq!(
            request,
            form_request(Some(meta(json!({
                "persist": ["session", "always"],
            }))))
        );
    }

    #[test]
    fn restore_context_meta_ignores_progress_only_meta() {
        let request = restore_context_meta(
            form_request(None),
            meta(json!({
                "progressToken": "progress-token",
            })),
        );

        assert_eq!(request, form_request(None));
    }

    #[test]
    fn elicitation_response_result_serializes_response_meta() {
        let result = rmcp::model::ClientResult::CustomResult(elicitation_response_result(
            ElicitationResponse {
                action: ElicitationAction::Accept,
                content: Some(json!({ "confirmed": true })),
                meta: Some(json!({ "persist": "always" })),
            },
        ));

        assert_eq!(
            serde_json::to_value(result).expect("client result should serialize"),
            json!({
                "action": "accept",
                "content": { "confirmed": true },
                "_meta": { "persist": "always" },
            })
        );
    }

    #[test]
    fn elicitation_response_result_omits_absent_content_and_meta() {
        let result = rmcp::model::ClientResult::CustomResult(elicitation_response_result(
            ElicitationResponse {
                action: ElicitationAction::Decline,
                content: None,
                meta: None,
            },
        ));

        assert_eq!(
            serde_json::to_value(result).expect("client result should serialize"),
            json!({ "action": "decline" })
        );
    }

    fn form_request(meta: Option<Meta>) -> CreateElicitationRequestParams {
        CreateElicitationRequestParams::FormElicitationParams {
            meta,
            message: "Confirm?".to_string(),
            requested_schema: ElicitationSchema::builder()
                .required_property("confirmed", PrimitiveSchema::Boolean(BooleanSchema::new()))
                .build()
                .expect("schema should build"),
        }
    }

    fn meta(value: Value) -> Meta {
        let Value::Object(map) = value else {
            panic!("meta must be an object");
        };
        Meta(map)
    }
}
