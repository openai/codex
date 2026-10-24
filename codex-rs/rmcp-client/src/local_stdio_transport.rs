//! Protocol framing and child lifetime for locally spawned MCP servers.
//!
//! Process creation is platform-specific; protocol selection and shutdown are shared.

use std::future::Future;
use std::io;
use std::time::Duration;

use futures::FutureExt;
use rmcp::model::ClientRequest;
use rmcp::model::ErrorCode;
use rmcp::model::JsonRpcMessage;
use rmcp::model::RequestId;
use rmcp::model::ServerResult;
use rmcp::service::RoleClient;
use rmcp::service::RxJsonRpcMessage;
use rmcp::service::TxJsonRpcMessage;
use rmcp::transport::Transport;
use rmcp::transport::async_rw::AsyncRwTransport;
use tokio::io::AsyncRead;
use tokio::process::ChildStderr;
use tokio::process::ChildStdin;
use tokio::sync::oneshot;

use crate::bounded_stdio_transport::BoundedStdioTransport;
use crate::protocol_mode::McpProtocolMode;
use codex_utils_pty::Child;

pub(super) struct LocalStdioTransport {
    child: Option<Box<Child>>,
    transport: StdioTransport,
    discovery: Option<Discovery>,
}

pub(super) enum DiscoveryOutcome {
    Reply,
    RetryLegacy,
    Late {
        reply: Box<RxJsonRpcMessage<RoleClient>>,
        after_reoffer: bool,
    },
}

struct Discovery {
    finished: Option<oneshot::Sender<DiscoveryOutcome>>,
    outcome: Option<DiscoveryOutcome>,
    retry_at: Option<tokio::time::Instant>,
    probe_id: Option<RequestId>,
    initialize_id: Option<RequestId>,
}

impl Drop for Discovery {
    fn drop(&mut self) {
        // A version re-offer can supersede an earlier reply. Publish only when startup ends.
        if let Some(tx) = self.finished.take()
            && let Some(outcome) = self.outcome.take()
        {
            let _ = tx.send(outcome);
        }
    }
}

impl Discovery {
    /// Track discovery replies and skip messages that do not answer the active handshake.
    fn handle_reply(&mut self, message: &mut Option<RxJsonRpcMessage<RoleClient>>) -> bool {
        let Some(reply) = message.as_ref() else {
            self.outcome.get_or_insert(DiscoveryOutcome::RetryLegacy);
            return false;
        };
        let (id, modern) = match reply {
            JsonRpcMessage::Response(response) => (
                Some(&response.id),
                matches!(response.result, ServerResult::DiscoverResult(_)),
            ),
            JsonRpcMessage::Error(error) => (
                error.id.as_ref(),
                matches!(
                    error.error.code,
                    ErrorCode::UNSUPPORTED_PROTOCOL_VERSION
                        | ErrorCode::HEADER_MISMATCH
                        | ErrorCode::MISSING_REQUIRED_CLIENT_CAPABILITY
                ),
            ),
            _ => return false,
        };
        let matches_id = |sent: &RequestId| match (sent, id) {
            (RequestId::Number(sent), Some(RequestId::String(received))) => {
                received.parse::<i64>().ok() == Some(*sent)
            }
            (_, Some(received)) => sent == received,
            (_, None) => false,
        };
        let legacy = self.initialize_id.is_some();
        let matches_probe = self.probe_id.as_ref().is_some_and(matches_id);
        let idless = self.probe_id.is_some() && id.is_none();
        if legacy
            && (matches_probe || idless)
            && matches!(reply, JsonRpcMessage::Error(error) if error.error.code == ErrorCode::METHOD_NOT_FOUND)
        {
            // If initialize also fails, try it as the first request on a fresh helper.
            self.outcome.get_or_insert(DiscoveryOutcome::RetryLegacy);
        }
        // With one request rmcp accepts an ID-less error. With two, only a modern
        // protocol error can be classified as a discovery reply without an ID.
        if matches_probe || (idless && (!legacy || modern)) {
            self.probe_id = None;
            if legacy {
                // Auto sent initialize before receiving the reply to its earlier probe.
                if !modern && matches!(reply, JsonRpcMessage::Error(_)) {
                    return true;
                }
                // Stop the obsolete initialize and let the caller classify the reply.
                self.outcome = message.take().map(|reply| DiscoveryOutcome::Late {
                    reply: Box::new(reply),
                    after_reoffer: matches!(self.outcome, Some(DiscoveryOutcome::Reply)),
                });
                return false;
            }
            if modern {
                self.retry_at = None;
                self.outcome = Some(DiscoveryOutcome::Reply);
            }
        } else if self.probe_id.is_some()
            && id.is_some()
            && !self.initialize_id.as_ref().is_some_and(matches_id)
        {
            // rmcp would abort the handshake before reading the reply to discover or initialize.
            return true;
        } else if legacy && matches!(reply, JsonRpcMessage::Response(_)) {
            // Once initialize wins, rmcp owns the session and can ignore the obsolete probe.
            self.probe_id = None;
            self.retry_at = None;
            self.outcome = Some(DiscoveryOutcome::Reply);
        }
        false
    }
}

enum StdioTransport {
    /// Preserve rmcp's existing framing for servers using the initialize handshake.
    Legacy(AsyncRwTransport<RoleClient, Box<dyn AsyncRead + Unpin + Send>, ChildStdin>),
    /// Bound frames and skip messages unknown to the client during 2026-07-28 discovery.
    Modern(BoundedStdioTransport),
    Closed,
}

impl LocalStdioTransport {
    pub(super) fn new(
        mut child: Child,
        program_name: String,
        protocol_mode: McpProtocolMode,
    ) -> io::Result<(Self, Option<ChildStderr>)> {
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| io::Error::other("MCP server stdin was not piped"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("MCP server stdout was not piped"))?;
        let stderr = child.stderr.take();
        let transport = match protocol_mode {
            McpProtocolMode::Legacy => {
                StdioTransport::Legacy(AsyncRwTransport::new(Box::new(stdout), stdin))
            }
            McpProtocolMode::V20260728 | McpProtocolMode::DiscoverLocalStdio => {
                StdioTransport::Modern(BoundedStdioTransport::new(stdin, stdout, program_name))
            }
        };
        Ok((
            Self {
                child: Some(Box::new(child)),
                transport,
                discovery: None,
            },
            stderr,
        ))
    }

    pub(super) fn watch_discovery(
        &mut self,
        timeout: Option<Duration>,
    ) -> oneshot::Receiver<DiscoveryOutcome> {
        let (tx, rx) = oneshot::channel();
        self.discovery = Some(Discovery {
            finished: Some(tx),
            outcome: None,
            // Aim for three seconds of discovery. Reserve at least a quarter of short
            // timeouts and half of longer ones for a fresh initialize.
            retry_at: timeout.and_then(|limit| {
                let probe_timeout = Duration::from_secs(3).clamp(limit / 2, limit - limit / 4);
                tokio::time::Instant::now().checked_add(probe_timeout)
            }),
            probe_id: None,
            initialize_id: None,
        });
        rx
    }

    #[cfg(unix)]
    pub(super) fn take_child(&mut self) -> Option<Box<Child>> {
        self.child.take()
    }
}

impl Transport<RoleClient> for LocalStdioTransport {
    type Error = io::Error;

    fn send(
        &mut self,
        item: TxJsonRpcMessage<RoleClient>,
    ) -> impl Future<Output = io::Result<()>> + Send + 'static {
        if self.discovery.as_ref().is_some_and(|probe| {
            probe
                .finished
                .as_ref()
                .is_none_or(oneshot::Sender::is_closed)
        }) {
            self.discovery = None;
        }
        if let Some(probe) = &mut self.discovery
            && let JsonRpcMessage::Request(request) = &item
            && matches!(request.request, ClientRequest::DiscoverRequest(_))
        {
            probe.probe_id = Some(request.id.clone());
        }
        if let Some(probe) = &mut self.discovery
            && let JsonRpcMessage::Request(request) = &item
            && matches!(request.request, ClientRequest::InitializeRequest(_))
            && matches!(self.transport, StdioTransport::Modern(_))
        {
            let StdioTransport::Modern(transport) =
                std::mem::replace(&mut self.transport, StdioTransport::Closed)
            else {
                unreachable!();
            };
            match transport.into_legacy_pipes() {
                Ok((stdout, stdin)) => {
                    probe.initialize_id = Some(request.id.clone());
                    self.transport = StdioTransport::Legacy(AsyncRwTransport::new(stdout, stdin));
                }
                Err(error) => return futures::future::err(error).boxed(),
            }
        }
        match &mut self.transport {
            StdioTransport::Legacy(transport) => transport.send(item).boxed(),
            StdioTransport::Modern(transport) => transport.send(item).boxed(),
            StdioTransport::Closed => futures::future::err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "MCP stdin closed",
            ))
            .boxed(),
        }
    }

    async fn receive(&mut self) -> Option<RxJsonRpcMessage<RoleClient>> {
        loop {
            let receive = async {
                match &mut self.transport {
                    StdioTransport::Legacy(transport) => transport.receive().await,
                    StdioTransport::Modern(transport) => transport.receive().await,
                    StdioTransport::Closed => None,
                }
            };
            let mut message = match self.discovery.as_ref().and_then(|probe| probe.retry_at) {
                Some(retry_at) => tokio::time::timeout_at(retry_at, receive)
                    .await
                    .unwrap_or_default(),
                None => receive.await,
            };
            if self
                .discovery
                .as_mut()
                .is_some_and(|probe| probe.handle_reply(&mut message))
            {
                continue;
            }
            return message;
        }
    }

    async fn close(&mut self) -> io::Result<()> {
        match &mut self.transport {
            StdioTransport::Legacy(transport) => transport.close().await?,
            StdioTransport::Modern(transport) => transport.close().await?,
            StdioTransport::Closed => {}
        }
        let Some(child) = &mut self.child else {
            return Ok(());
        };
        match tokio::time::timeout(Duration::from_secs(3), child.wait()).await {
            Ok(status) => status.map(|_| ()),
            Err(_) => child.kill().await,
        }
    }
}
