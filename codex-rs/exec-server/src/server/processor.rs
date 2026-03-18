use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCRequest;
use tracing::debug;

use crate::connection::JsonRpcConnection;
use crate::connection::JsonRpcConnectionEvent;
use crate::protocol::INITIALIZE_METHOD;
use crate::protocol::INITIALIZED_METHOD;
use crate::protocol::InitializeParams;
use crate::server::ExecServerHandler;
use crate::server::jsonrpc::invalid_params;
use crate::server::jsonrpc::invalid_request_message;
use crate::server::jsonrpc::method_not_found;
use crate::server::jsonrpc::response_message;
use tracing::warn;

pub(crate) async fn run_connection(connection: JsonRpcConnection) {
    let (json_outgoing_tx, mut incoming_rx, _connection_tasks) = connection.into_parts();
    let handler = ExecServerHandler::new();

    while let Some(event) = incoming_rx.recv().await {
        match event {
            JsonRpcConnectionEvent::Message(message) => {
                let response = match handle_connection_message(&handler, message).await {
                    Ok(response) => response,
                    Err(err) => {
                        tracing::warn!(
                            "closing exec-server connection after protocol error: {err}"
                        );
                        break;
                    }
                };
                let Some(response) = response else {
                    continue;
                };
                if json_outgoing_tx.send(response).await.is_err() {
                    break;
                }
            }
            JsonRpcConnectionEvent::MalformedMessage { reason } => {
                warn!("ignoring malformed exec-server message: {reason}");
                if json_outgoing_tx
                    .send(invalid_request_message(reason))
                    .await
                    .is_err()
                {
                    break;
                }
            }
            JsonRpcConnectionEvent::Disconnected { reason } => {
                if let Some(reason) = reason {
                    debug!("exec-server connection disconnected: {reason}");
                }
                break;
            }
        }
    }

    handler.shutdown().await;
}

pub(crate) async fn handle_connection_message(
    handler: &ExecServerHandler,
    message: JSONRPCMessage,
) -> Result<Option<JSONRPCMessage>, String> {
    match message {
        JSONRPCMessage::Request(request) => Ok(Some(dispatch_request(handler, request))),
        JSONRPCMessage::Notification(notification) => {
            handle_notification(handler, notification)?;
            Ok(None)
        }
        JSONRPCMessage::Response(response) => Err(format!(
            "unexpected client response for request id {:?}",
            response.id
        )),
        JSONRPCMessage::Error(error) => Err(format!(
            "unexpected client error for request id {:?}",
            error.id
        )),
    }
}

fn dispatch_request(handler: &ExecServerHandler, request: JSONRPCRequest) -> JSONRPCMessage {
    let JSONRPCRequest {
        id,
        method,
        params,
        trace: _,
    } = request;

    match method.as_str() {
        INITIALIZE_METHOD => {
            let result = serde_json::from_value::<InitializeParams>(
                params.unwrap_or(serde_json::Value::Null),
            )
            .map_err(|err| invalid_params(err.to_string()))
            .and_then(|_params| handler.initialize())
            .and_then(|response| {
                serde_json::to_value(response).map_err(|err| invalid_params(err.to_string()))
            });
            response_message(id, result)
        }
        other => response_message(
            id,
            Err(method_not_found(format!(
                "exec-server stub does not implement `{other}` yet"
            ))),
        ),
    }
}

fn handle_notification(
    handler: &ExecServerHandler,
    notification: JSONRPCNotification,
) -> Result<(), String> {
    match notification.method.as_str() {
        INITIALIZED_METHOD => handler.initialized(),
        other => Err(format!("unexpected notification method: {other}")),
    }
}
