use rmcp::ClientHandler;
use rmcp::RoleClient;
use rmcp::model::CancelledNotificationParam;
use rmcp::model::ClientInfo;
use rmcp::model::CreateElicitationRequestParam;
use rmcp::model::CreateElicitationResult;
use rmcp::model::CreateMessageRequestParam;
use rmcp::model::CreateMessageResult;
use rmcp::model::ElicitationAction;
use rmcp::model::LoggingLevel;
use rmcp::model::LoggingMessageNotificationParam;
use rmcp::model::ProgressNotificationParam;
use rmcp::model::ResourceUpdatedNotificationParam;
use rmcp::service::NotificationContext;
use rmcp::service::RequestContext;
use std::sync::Arc;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::warn;

use crate::SamplingHandler;

#[derive(Clone)]
pub(crate) struct LoggingClientHandler {
    client_info: ClientInfo,
    sampling_handler: Option<Arc<dyn SamplingHandler>>,
}

impl std::fmt::Debug for LoggingClientHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoggingClientHandler")
            .field("client_info", &self.client_info)
            .field(
                "sampling_handler",
                &self.sampling_handler.as_ref().map(|_| "Some(...)"),
            )
            .finish()
    }
}

impl LoggingClientHandler {
    pub(crate) fn new(client_info: ClientInfo) -> Self {
        Self {
            client_info,
            sampling_handler: None,
        }
    }

    pub(crate) fn with_sampling_handler(
        client_info: ClientInfo,
        handler: Arc<dyn SamplingHandler>,
    ) -> Self {
        Self {
            client_info,
            sampling_handler: Some(handler),
        }
    }
}

impl ClientHandler for LoggingClientHandler {
    // TODO (CODEX-3571): support elicitations.
    async fn create_elicitation(
        &self,
        request: CreateElicitationRequestParam,
        _context: RequestContext<RoleClient>,
    ) -> Result<CreateElicitationResult, rmcp::ErrorData> {
        info!(
            "MCP server requested elicitation ({}). Elicitations are not supported yet. Declining.",
            request.message
        );
        Ok(CreateElicitationResult {
            action: ElicitationAction::Decline,
            content: None,
        })
    }

    async fn create_message(
        &self,
        params: CreateMessageRequestParam,
        _context: RequestContext<RoleClient>,
    ) -> Result<CreateMessageResult, rmcp::ErrorData> {
        info!(
            "MCP server requested sampling with {} messages",
            params.messages.len()
        );

        // Log the sampling request details
        debug!(
            "Sampling parameters: max_tokens={}, temperature={:?}, system_prompt={:?}, model_preferences={:?}",
            params.max_tokens,
            params.temperature,
            params
                .system_prompt
                .as_ref()
                .map(|s| format!("{}...", &s[..s.len().min(50)])),
            params.model_preferences
        );

        // Sampling handler is required
        let Some(handler) = &self.sampling_handler else {
            error!("MCP server requested sampling but no SamplingHandler was provided");
            return Err(rmcp::ErrorData::internal_error(
                "Sampling support requires a SamplingHandler to be configured",
                None,
            ));
        };

        debug!("Delegating sampling request to provided handler");
        handler.create_message(params).await
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

    fn get_info(&self) -> ClientInfo {
        self.client_info.clone()
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
        let log_msg =
            format!("MCP server log message (level: {level:?}, logger: {logger:?}, data: {data})");

        match level {
            LoggingLevel::Emergency
            | LoggingLevel::Alert
            | LoggingLevel::Critical
            | LoggingLevel::Error => error!("{log_msg}"),
            LoggingLevel::Warning => warn!("{log_msg}"),
            LoggingLevel::Notice | LoggingLevel::Info => info!("{log_msg}"),
            LoggingLevel::Debug => debug!("{log_msg}"),
        }
    }
}
