use std::sync::Arc;

use tokio::sync::mpsc;
use tracing::debug;
use tracing::warn;

use crate::connection::CHANNEL_CAPACITY;
use crate::connection::JsonRpcConnection;
use crate::connection::JsonRpcConnectionEvent;
use crate::rpc::RpcNotificationSender;
use crate::rpc::RpcServerOutboundMessage;
use crate::rpc::encode_server_message;
use crate::rpc::invalid_request;
use crate::rpc::method_not_found;
use crate::server::ExecServerHandler;
use crate::server::registry::build_router;
use crate::server::session_registry::SessionRegistry;

#[derive(Clone)]
pub(crate) struct ConnectionProcessor {
    session_registry: Arc<SessionRegistry>,
}

impl ConnectionProcessor {
    pub(crate) fn new() -> Self {
        Self {
            session_registry: SessionRegistry::new(),
        }
    }

    pub(crate) async fn run_connection(&self, connection: JsonRpcConnection) {
        run_connection(connection, Arc::clone(&self.session_registry)).await;
    }
}

async fn run_connection(connection: JsonRpcConnection, session_registry: Arc<SessionRegistry>) {
    let router = Arc::new(build_router());
    let (json_outgoing_tx, mut incoming_rx, connection_tasks) = connection.into_parts();
    let (outgoing_tx, mut outgoing_rx) =
        mpsc::channel::<RpcServerOutboundMessage>(CHANNEL_CAPACITY);
    let notifications = RpcNotificationSender::new(outgoing_tx.clone());
    let handler = Arc::new(ExecServerHandler::new(session_registry, notifications));

    let outbound_task = tokio::spawn(async move {
        while let Some(message) = outgoing_rx.recv().await {
            let json_message = match encode_server_message(message) {
                Ok(json_message) => json_message,
                Err(err) => {
                    warn!("failed to serialize exec-server outbound message: {err}");
                    break;
                }
            };
            if json_outgoing_tx.send(json_message).await.is_err() {
                break;
            }
        }
    });

    // Process inbound events sequentially to preserve initialize/initialized ordering.
    while let Some(event) = incoming_rx.recv().await {
        if !handler.is_session_attached() {
            debug!("exec-server connection evicted after session resume");
            break;
        }
        match event {
            JsonRpcConnectionEvent::MalformedMessage { reason } => {
                warn!("ignoring malformed exec-server message: {reason}");
                if outgoing_tx
                    .send(RpcServerOutboundMessage::Error {
                        request_id: codex_app_server_protocol::RequestId::Integer(-1),
                        error: invalid_request(reason),
                    })
                    .await
                    .is_err()
                {
                    break;
                }
            }
            JsonRpcConnectionEvent::Message(message) => match message {
                codex_app_server_protocol::JSONRPCMessage::Request(request) => {
                    if let Some(route) = router.request_route(request.method.as_str()) {
                        let message = route(Arc::clone(&handler), request).await;
                        if outgoing_tx.send(message).await.is_err() {
                            break;
                        }
                    } else if outgoing_tx
                        .send(RpcServerOutboundMessage::Error {
                            request_id: request.id,
                            error: method_not_found(format!(
                                "exec-server stub does not implement `{}` yet",
                                request.method
                            )),
                        })
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                codex_app_server_protocol::JSONRPCMessage::Notification(notification) => {
                    let Some(route) = router.notification_route(notification.method.as_str())
                    else {
                        warn!(
                            "closing exec-server connection after unexpected notification: {}",
                            notification.method
                        );
                        break;
                    };
                    if let Err(err) = route(Arc::clone(&handler), notification).await {
                        warn!("closing exec-server connection after protocol error: {err}");
                        break;
                    }
                }
                codex_app_server_protocol::JSONRPCMessage::Response(response) => {
                    warn!(
                        "closing exec-server connection after unexpected client response: {:?}",
                        response.id
                    );
                    break;
                }
                codex_app_server_protocol::JSONRPCMessage::Error(error) => {
                    warn!(
                        "closing exec-server connection after unexpected client error: {:?}",
                        error.id
                    );
                    break;
                }
            },
            JsonRpcConnectionEvent::Disconnected { reason } => {
                if let Some(reason) = reason {
                    debug!("exec-server connection disconnected: {reason}");
                }
                break;
            }
        }
    }

    handler.shutdown().await;
    drop(outgoing_tx);
    for task in connection_tasks {
        task.abort();
        let _ = task.await;
    }
    let _ = outbound_task.await;
}
