use std::num::NonZeroUsize;
use std::num::ParseIntError;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Instant;

use codex_exec_server_protocol::JSONRPCError;
use codex_exec_server_protocol::JSONRPCNotification;
use codex_exec_server_protocol::JSONRPCRequest;
use codex_exec_server_protocol::JSONRPCResponse;
use codex_exec_server_protocol::RequestId;
use tokio::sync::Semaphore;
use tokio::sync::mpsc;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::Instrument;
use tracing::debug;
use tracing::warn;

use crate::protocol::ENVIRONMENT_INFO_METHOD;
use crate::protocol::ENVIRONMENT_STATUS_METHOD;
use crate::protocol::EXEC_SIGNAL_METHOD;
use crate::protocol::EXEC_TERMINATE_METHOD;
use crate::protocol::FS_CLOSE_METHOD;
use crate::protocol::INITIALIZE_METHOD;
use crate::protocol::INITIALIZED_METHOD;
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
    lanes: Option<RequestLanes>,
    tasks: JoinSet<RequestTaskResult>,
    initialized: bool,
}

impl RequestDispatcher {
    pub(super) fn new(
        router: Arc<RpcRouter<ExecServerHandler>>,
        handler: Arc<ExecServerHandler>,
        outgoing_tx: mpsc::Sender<RpcServerOutboundMessage>,
        disconnected_rx: watch::Receiver<bool>,
        requests: RpcServerRequestSender,
        telemetry: ExecServerTelemetry,
        mode: RequestDispatchMode,
    ) -> Self {
        let lanes = match mode {
            RequestDispatchMode::Inline => None,
            RequestDispatchMode::Concurrent {
                max_concurrent_requests,
            } => Some(RequestLanes {
                ordinary: Arc::new(Semaphore::new(max_concurrent_requests.get())),
                control: Arc::new(Semaphore::new(max_concurrent_requests.get())),
            }),
        };

        Self {
            router,
            handler,
            outgoing_tx,
            disconnected_rx,
            requests,
            telemetry,
            lanes,
            tasks: JoinSet::new(),
            initialized: false,
        }
    }

    pub(super) fn has_tasks(&self) -> bool {
        !self.tasks.is_empty()
    }

    pub(super) async fn join_next(&mut self) -> RequestTaskResult {
        match self.tasks.join_next().await {
            Some(Ok(result)) => result,
            Some(Err(error)) => {
                warn!("exec-server request task failed: {error}");
                RequestTaskResult::ConnectionClosed
            }
            None => RequestTaskResult::Completed,
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
        let is_initialized = notification.method == INITIALIZED_METHOD;
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
        if is_initialized {
            self.initialized = true;
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

    pub(super) async fn dispatch_request(
        &mut self,
        request: JSONRPCRequest,
        request_span: tracing::Span,
        queued_at: Instant,
    ) -> RequestTaskResult {
        let started_at = Instant::now();
        let Some((method, route)) = self.router.request_route(request.method.as_str()) else {
            let method = "unknown";
            self.telemetry
                .request_queue_completed(method, queued_at.elapsed());
            request_span.record("otel.name", method);
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
                request_span.record("result", "disconnected");
                self.telemetry
                    .request_completed(method, "disconnected", started_at.elapsed());
                return RequestTaskResult::ConnectionClosed;
            }
            request_span.record("result", "error");
            self.telemetry
                .request_completed(method, "error", started_at.elapsed());
            return RequestTaskResult::Completed;
        };

        request_span.record("otel.name", method);
        let route_setup_started_at = Instant::now();
        let route = route(Arc::clone(&self.handler), request);
        let route_setup_duration = route_setup_started_at.elapsed();
        let outgoing_tx = self.outgoing_tx.clone();
        let mut disconnected_rx = self.disconnected_rx.clone();
        let telemetry = self.telemetry.clone();
        let task = async move {
            telemetry.request_queue_completed(
                method,
                queued_at.elapsed().saturating_sub(route_setup_duration),
            );
            let message = tokio::select! {
                message = route.instrument(request_span.clone()) => message,
                _ = disconnected_rx.changed() => {
                    request_span.record("result", "disconnected");
                    telemetry.request_completed(method, "disconnected", started_at.elapsed());
                    return RequestTaskResult::ConnectionClosed;
                }
            };
            let result = request_result(&message);
            let response_sent = match message {
                Some(message) => tokio::select! {
                    result = outgoing_tx.send(message) => result.is_ok(),
                    _ = disconnected_rx.changed() => false,
                },
                None => true,
            };
            if !response_sent {
                request_span.record("result", "disconnected");
                telemetry.request_completed(method, "disconnected", started_at.elapsed());
                return RequestTaskResult::ConnectionClosed;
            }
            request_span.record("result", result);
            telemetry.request_completed(method, result, started_at.elapsed());
            RequestTaskResult::Completed
        };

        let Some(RequestLanes { ordinary, control }) = &self.lanes else {
            // Keep requests ordered when concurrent dispatch is not enabled.
            return task.await;
        };
        // Finish the handshake before concurrent requests can observe session state.
        if method == INITIALIZE_METHOD || !self.initialized {
            return task.await;
        }

        // Reserve capacity for health checks and cleanup while ordinary requests are blocked.
        let admission = if matches!(
            method,
            ENVIRONMENT_INFO_METHOD
                | ENVIRONMENT_STATUS_METHOD
                | EXEC_SIGNAL_METHOD
                | EXEC_TERMINATE_METHOD
                | FS_CLOSE_METHOD
        ) {
            Arc::clone(control)
        } else {
            Arc::clone(ordinary)
        };

        // TODO(anp) bound queued request bytes without blocking later responses or cleanup.
        self.tasks.spawn(async move {
            let Ok(_permit) = admission.acquire_owned().await else {
                return RequestTaskResult::ConnectionClosed;
            };
            task.await
        });
        RequestTaskResult::Completed
    }

    pub(super) async fn shutdown(mut self) {
        self.tasks.abort_all();
        while self.tasks.join_next().await.is_some() {}
    }
}

/// Per-connection request dispatch policy for local and remote exec-servers.
#[derive(Clone, Copy, Debug)]
pub enum RequestDispatchMode {
    Inline,
    Concurrent {
        max_concurrent_requests: ConcurrentRequestLimit,
    },
}

/// A valid request concurrency limit accepted by Tokio's semaphore.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConcurrentRequestLimit(usize);

impl ConcurrentRequestLimit {
    /// Returns a limit when it enables concurrency and fits Tokio's semaphore.
    pub fn new(max_concurrent_requests: usize) -> Option<Self> {
        if !(2..=Semaphore::MAX_PERMITS).contains(&max_concurrent_requests) {
            return None;
        }

        Some(Self(max_concurrent_requests))
    }

    /// Returns the validated number of concurrent requests.
    pub fn get(self) -> usize {
        self.0
    }
}

impl FromStr for RequestDispatchMode {
    type Err = ParseIntError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let max_concurrent_requests = value.parse::<NonZeroUsize>()?.get();
        if max_concurrent_requests == 1 {
            Ok(Self::Inline)
        } else {
            Ok(Self::Concurrent {
                max_concurrent_requests: ConcurrentRequestLimit(
                    max_concurrent_requests.min(Semaphore::MAX_PERMITS),
                ),
            })
        }
    }
}

struct RequestLanes {
    ordinary: Arc<Semaphore>,
    control: Arc<Semaphore>,
}

#[derive(Eq, PartialEq)]
pub(super) enum RequestTaskResult {
    Completed,
    ConnectionClosed,
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
