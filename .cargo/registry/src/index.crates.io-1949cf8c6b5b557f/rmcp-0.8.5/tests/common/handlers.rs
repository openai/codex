use std::{
    future::Future,
    sync::{Arc, Mutex},
};

use rmcp::{
    ClientHandler, ErrorData as McpError, RoleClient, RoleServer, ServerHandler,
    model::*,
    service::{NotificationContext, RequestContext},
};
use serde_json::json;
use tokio::sync::Notify;

#[derive(Clone)]
pub struct TestClientHandler {
    pub honor_this_server: bool,
    pub honor_all_servers: bool,
    pub receive_signal: Arc<Notify>,
    pub received_messages: Arc<Mutex<Vec<LoggingMessageNotificationParam>>>,
}

impl TestClientHandler {
    #[allow(dead_code)]
    pub fn new(honor_this_server: bool, honor_all_servers: bool) -> Self {
        Self {
            honor_this_server,
            honor_all_servers,
            receive_signal: Arc::new(Notify::new()),
            received_messages: Arc::new(Mutex::new(Vec::new())),
        }
    }

    #[allow(dead_code)]
    pub fn with_notification(
        honor_this_server: bool,
        honor_all_servers: bool,
        receive_signal: Arc<Notify>,
        received_messages: Arc<Mutex<Vec<LoggingMessageNotificationParam>>>,
    ) -> Self {
        Self {
            honor_this_server,
            honor_all_servers,
            receive_signal,
            received_messages,
        }
    }
}

impl ClientHandler for TestClientHandler {
    async fn create_message(
        &self,
        params: CreateMessageRequestParam,
        _context: RequestContext<RoleClient>,
    ) -> Result<CreateMessageResult, McpError> {
        // First validate that there's at least one User message
        if !params.messages.iter().any(|msg| msg.role == Role::User) {
            return Err(McpError::invalid_request(
                "Message sequence must contain at least one user message",
                Some(json!({"messages": params.messages})),
            ));
        }

        // Create response based on context inclusion
        let response = match params.include_context {
            Some(ContextInclusion::ThisServer) if self.honor_this_server => {
                "Test response with context: test context"
            }
            Some(ContextInclusion::AllServers) if self.honor_all_servers => {
                "Test response with context: test context"
            }
            _ => "Test response without context",
        };

        Ok(CreateMessageResult {
            message: SamplingMessage {
                role: Role::Assistant,
                content: Content::text(response.to_string()),
            },
            model: "test-model".to_string(),
            stop_reason: Some(CreateMessageResult::STOP_REASON_END_TURN.to_string()),
        })
    }

    fn on_logging_message(
        &self,
        params: LoggingMessageNotificationParam,
        _context: NotificationContext<RoleClient>,
    ) -> impl Future<Output = ()> + Send + '_ {
        let receive_signal = self.receive_signal.clone();
        let received_messages = self.received_messages.clone();

        async move {
            println!("Client: Received log message: {:?}", params);
            let mut messages = received_messages.lock().unwrap();
            messages.push(params);
            receive_signal.notify_one();
        }
    }
}

pub struct TestServer {}

impl TestServer {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {}
    }
}

impl ServerHandler for TestServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder().enable_logging().build(),
            ..Default::default()
        }
    }

    fn set_level(
        &self,
        request: SetLevelRequestParam,
        context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<(), McpError>> + Send + '_ {
        let peer = context.peer;
        async move {
            let (data, logger) = match request.level {
                LoggingLevel::Error => (
                    serde_json::json!({
                        "message": "Failed to process request",
                        "error_code": "E1001",
                        "error_details": "Connection timeout",
                        "timestamp": chrono::Utc::now().to_rfc3339(),
                    }),
                    Some("error_handler".to_string()),
                ),
                LoggingLevel::Debug => (
                    serde_json::json!({
                        "message": "Processing request",
                        "function": "handle_request",
                        "line": 42,
                        "context": {
                            "request_id": "req-123",
                            "user_id": "user-456"
                        },
                        "timestamp": chrono::Utc::now().to_rfc3339(),
                    }),
                    Some("debug_logger".to_string()),
                ),
                LoggingLevel::Info => (
                    serde_json::json!({
                        "message": "System status update",
                        "status": "healthy",
                        "metrics": {
                            "requests_per_second": 150,
                            "average_latency_ms": 45,
                            "error_rate": 0.01
                        },
                        "timestamp": chrono::Utc::now().to_rfc3339(),
                    }),
                    Some("monitoring".to_string()),
                ),
                _ => (
                    serde_json::json!({
                        "message": format!("Message at level {:?}", request.level),
                        "timestamp": chrono::Utc::now().to_rfc3339(),
                    }),
                    None,
                ),
            };

            if let Err(e) = peer
                .notify_logging_message(LoggingMessageNotificationParam {
                    level: request.level,
                    data,
                    logger,
                })
                .await
            {
                panic!("Failed to send notification: {}", e);
            }
            Ok(())
        }
    }
}
