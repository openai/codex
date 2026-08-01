use std::sync::Arc;
use std::time::Instant;

use codex_exec_server_protocol::JSONRPCError;
use codex_exec_server_protocol::JSONRPCNotification;
use codex_exec_server_protocol::JSONRPCRequest;
use codex_exec_server_protocol::JSONRPCResponse;
use codex_exec_server_protocol::RequestId;
use tokio::sync::mpsc;
use tokio::sync::watch;
use tracing::Instrument;
use tracing::debug;
use tracing::warn;

use crate::rpc::RpcCallError;
use crate::rpc::RpcRouter;
use crate::rpc::RpcServerOutboundMessage;
use crate::rpc::invalid_request;
use crate::rpc::method_not_found;
use crate::rpc_server_requests::RpcServerRequestSender;
use crate::server::ExecServerHandler;
use crate::telemetry::ExecServerTelemetry;

pub(super) struct RequestDispatcher {
    router: Arc<RpcRouter<ExecServerHandler>>,
    handler: Arc<ExecServerHandler>,
    outgoing_tx: mpsc::Sender<RpcServerOutboundMessage>,
    disconnected_rx: watch::Receiver<bool>,
    requests: RpcServerRequestSender,
    telemetry: ExecServerTelemetry,
}

impl RequestDispatcher {
    pub(super) fn new(
        router: Arc<RpcRouter<ExecServerHandler>>,
        handler: Arc<ExecServerHandler>,
        outgoing_tx: mpsc::Sender<RpcServerOutboundMessage>,
        disconnected_rx: watch::Receiver<bool>,
        requests: RpcServerRequestSender,
        telemetry: ExecServerTelemetry,
    ) -> Self {
        Self {
            router,
            handler,
            outgoing_tx,
            disconnected_rx,
            requests,
            telemetry,
        }
    }

    pub(super) async fn handle_malformed_message(&self, reason: String) -> RequestTaskResult {
        warn!("ignoring malformed exec-server message: {reason}");
        if self
            .outgoing_tx
            .send(RpcServerOutboundMessage::Error {
                request_id: RequestId::Integer(-1),
                error: invalid_request(reason),
            })
            .await
            .is_err()
        {
            return RequestTaskResult::ConnectionClosed;
        }

        RequestTaskResult::Completed
    }

    pub(super) async fn handle_notification(
        &mut self,
        notification: JSONRPCNotification,
    ) -> RequestTaskResult {
        let Some(route) = self.router.notification_route(notification.method.as_str()) else {
            warn!(
                "closing exec-server connection after unexpected notification: {}",
                notification.method
            );
            return RequestTaskResult::ConnectionClosed;
        };
        let result = tokio::select! {
            result = route(Arc::clone(&self.handler), notification) => result,
            _ = self.disconnected_rx.changed() => {
                debug!("exec-server transport disconnected while handling notification");
                return RequestTaskResult::ConnectionClosed;
            }
        };
        if let Err(error) = result {
            warn!("closing exec-server connection after protocol error: {error}");
            return RequestTaskResult::ConnectionClosed;
        }

        RequestTaskResult::Completed
    }

    pub(super) fn handle_response(&self, response: JSONRPCResponse) -> RequestTaskResult {
        if !self
            .requests
            .complete(response.id.clone(), Ok(response.result))
        {
            warn!(
                "closing exec-server connection after unexpected client response: {:?}",
                response.id
            );
            return RequestTaskResult::ConnectionClosed;
        }

        RequestTaskResult::Completed
    }

    pub(super) fn handle_error(&self, error: JSONRPCError) -> RequestTaskResult {
        if !self
            .requests
            .complete(error.id.clone(), Err(RpcCallError::Server(error.error)))
        {
            warn!(
                "closing exec-server connection after unexpected client error: {:?}",
                error.id
            );
            return RequestTaskResult::ConnectionClosed;
        }

        RequestTaskResult::Completed
    }

    pub(super) async fn dispatch_request(&mut self, request: JSONRPCRequest) -> RequestTaskResult {
        let started_at = Instant::now();
        let Some((method, route)) = self.router.request_route(request.method.as_str()) else {
            let method = "unknown";
            let span = request_span(method, &request);
            if self
                .outgoing_tx
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
                span.record("result", "disconnected");
                self.telemetry
                    .request_completed(method, "disconnected", started_at.elapsed());
                return RequestTaskResult::ConnectionClosed;
            }
            span.record("result", "error");
            self.telemetry
                .request_completed(method, "error", started_at.elapsed());
            return RequestTaskResult::Completed;
        };

        let span = request_span(method, &request);
        let message = tokio::select! {
            message = route(Arc::clone(&self.handler), request).instrument(span.clone()) => message,
            _ = self.disconnected_rx.changed() => {
                span.record("result", "disconnected");
                self.telemetry
                    .request_completed(method, "disconnected", started_at.elapsed());
                debug!("exec-server transport disconnected while handling request");
                return RequestTaskResult::ConnectionClosed;
            }
        };
        let result = request_result(&message);
        if let Some(message) = message
            && self.outgoing_tx.send(message).await.is_err()
        {
            span.record("result", "disconnected");
            self.telemetry
                .request_completed(method, "disconnected", started_at.elapsed());
            return RequestTaskResult::ConnectionClosed;
        }
        span.record("result", result);
        self.telemetry
            .request_completed(method, result, started_at.elapsed());
        drop(span);

        RequestTaskResult::Completed
    }
}

#[derive(Eq, PartialEq)]
pub(super) enum RequestTaskResult {
    Completed,
    ConnectionClosed,
}

fn request_span(span_name: &str, request: &JSONRPCRequest) -> tracing::Span {
    let method = request.method.as_str();
    let span = tracing::info_span!(
        "codex.exec_server.request",
        otel.kind = "server",
        otel.name = span_name,
        method,
        result = tracing::field::Empty,
    );
    if let Some(trace) = &request.trace
        && !codex_otel::set_parent_from_w3c_trace_context(&span, trace)
    {
        warn!(method, "ignoring invalid inbound exec-server trace carrier");
    }
    span
}

fn request_result(message: &Option<RpcServerOutboundMessage>) -> &'static str {
    match message {
        Some(RpcServerOutboundMessage::Error { .. }) => "error",
        Some(
            RpcServerOutboundMessage::Request(_)
            | RpcServerOutboundMessage::Response { .. }
            | RpcServerOutboundMessage::Notification(_),
        )
        | None => "success",
    }
}

#[cfg(test)]
#[path = "request_dispatcher_tests.rs"]
mod tests;
