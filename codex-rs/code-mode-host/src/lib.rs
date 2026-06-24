use std::collections::HashMap;
use std::collections::HashSet;
use std::io;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use anyhow::Context;
use anyhow::Result;
use codex_code_mode::InProcessCodeModeSession;
use codex_code_mode_protocol::host::CapabilitySet;
use codex_code_mode_protocol::host::ClientToHost;
use codex_code_mode_protocol::host::FramedReader;
use codex_code_mode_protocol::host::FramedWriter;
use codex_code_mode_protocol::host::HandshakeRejectReason;
use codex_code_mode_protocol::host::HostHello;
use codex_code_mode_protocol::host::HostRequest;
use codex_code_mode_protocol::host::HostResponse;
use codex_code_mode_protocol::host::HostToClient;
use codex_code_mode_protocol::host::MAX_FRAME_BYTES;
use codex_code_mode_protocol::host::ProtocolVersion;
use codex_code_mode_protocol::host::RequestId;
use codex_code_mode_protocol::host::SessionId;
use codex_code_mode_protocol::host::SupportedProtocolVersions;
use codex_code_mode_protocol::host::WireResult;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio_util::task::TaskTracker;

use self::delegate::RemoteDelegate;
use self::peer::HostPeer;

mod delegate;
mod peer;

/// Runs one code-mode host connection over the process standard streams.
pub async fn run_stdio() -> Result<()> {
    run(tokio::io::stdin(), tokio::io::stdout()).await
}

/// Runs one code-mode host connection over an ordered input/output pair.
async fn run<R, W>(reader: R, writer: W) -> Result<()>
where
    R: AsyncRead + Send + Unpin + 'static,
    W: AsyncWrite + Send + Unpin + 'static,
{
    let mut reader = FramedReader::new(reader);
    let mut writer = FramedWriter::new(writer);
    if !negotiate(&mut reader, &mut writer).await? {
        return Ok(());
    }

    let (outgoing_tx, mut outgoing_rx) = mpsc::unbounded_channel();
    let peer = Arc::new(HostPeer::new(outgoing_tx));
    let state = Arc::new(HostState {
        sessions: Mutex::new(HashMap::new()),
        seen_session_ids: Mutex::new(HashSet::new()),
        request_tasks: TaskTracker::new(),
        closing: AtomicBool::new(false),
        peer: Arc::clone(&peer),
    });
    let (writer_finished_tx, mut writer_finished_rx) = oneshot::channel();
    let writer_task = tokio::spawn(async move {
        let result = async {
            while let Some(message) = outgoing_rx.recv().await {
                let write_result = match writer.write(&message).await {
                    Err(error) if error.kind() == io::ErrorKind::InvalidInput => {
                        let error_message = format!(
                            "code-mode host response exceeded the {MAX_FRAME_BYTES}-byte IPC frame limit: {error}"
                        );
                        let fallback = match &message {
                            HostToClient::Response { id, .. } => Some(HostToClient::Response {
                                id: *id,
                                result: WireResult::Err {
                                    message: error_message,
                                },
                            }),
                            HostToClient::InitialResponse { id, .. } => {
                                Some(HostToClient::InitialResponse {
                                    id: *id,
                                    result: WireResult::Err {
                                        message: error_message,
                                    },
                                })
                            }
                            HostToClient::HostHello(_)
                            | HostToClient::HandshakeRejected { .. }
                            | HostToClient::DelegateRequest { .. }
                            | HostToClient::CancelDelegateRequest { .. }
                            | HostToClient::CellClosed { .. } => None,
                        };
                        match fallback {
                            Some(fallback) => writer.write(&fallback).await,
                            None => Err(error),
                        }
                    }
                    result => result,
                };
                write_result.context("failed to write code-mode host message")?;
            }
            Ok::<(), anyhow::Error>(())
        }
        .await;
        let status = match &result {
            Ok(()) => "code-mode host writer stopped unexpectedly".to_string(),
            Err(error) => format!("{error:#}"),
        };
        let _ = writer_finished_tx.send(status);
        result
    });

    let input_result = async {
        loop {
            let message = tokio::select! {
                biased;
                writer_status = &mut writer_finished_rx => {
                    let message = writer_status.unwrap_or_else(|_| {
                        "code-mode host writer task stopped without reporting status".to_string()
                    });
                    anyhow::bail!(message);
                }
                message = reader.read::<ClientToHost>() => {
                    let Some(message) = message
                        .context("failed to read code-mode client message")?
                    else {
                        break;
                    };
                    message
                }
            };
            match message {
                ClientToHost::ClientHello(_) => {
                    anyhow::bail!("received a second code-mode client hello");
                }
                ClientToHost::Request { id, request } => {
                    state.spawn_request(id, request);
                }
                ClientToHost::DelegateResponse { id, result } => {
                    peer.complete(id, result.into_result()).await;
                }
            }
        }
        Ok::<(), anyhow::Error>(())
    }
    .await;

    peer.disconnect();
    state.disconnect().await;
    drop(state);
    drop(peer);
    let writer_result = writer_task.await.context("code-mode writer task failed")?;
    input_result?;
    writer_result
}

async fn negotiate<R, W>(reader: &mut FramedReader<R>, writer: &mut FramedWriter<W>) -> Result<bool>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let Some(first_message) = reader
        .read::<ClientToHost>()
        .await
        .context("failed to read code-mode client hello")?
    else {
        return Ok(false);
    };
    let ClientToHost::ClientHello(client_hello) = first_message else {
        writer
            .write(&HostToClient::HandshakeRejected {
                reason: HandshakeRejectReason::InvalidHello {
                    message: "first message must be connection/hello".to_string(),
                },
            })
            .await
            .context("failed to reject invalid code-mode client hello")?;
        return Ok(false);
    };

    let supported_versions = SupportedProtocolVersions::try_new([ProtocolVersion::V1])?;
    if !client_hello
        .supported_versions()
        .contains(ProtocolVersion::V1)
    {
        writer
            .write(&HostToClient::HandshakeRejected {
                reason: HandshakeRejectReason::NoCompatibleVersion { supported_versions },
            })
            .await
            .context("failed to reject incompatible code-mode client")?;
        return Ok(false);
    }

    let host_capabilities = CapabilitySet::empty();
    if let Some(capability) = client_hello
        .required_capabilities()
        .iter()
        .find(|capability| !host_capabilities.contains(capability))
    {
        writer
            .write(&HostToClient::HandshakeRejected {
                reason: HandshakeRejectReason::MissingRequiredCapability {
                    capability: capability.clone(),
                },
            })
            .await
            .context("failed to reject unsupported code-mode capability")?;
        return Ok(false);
    }

    writer
        .write(&HostToClient::HostHello(HostHello::new(
            ProtocolVersion::V1,
            host_capabilities,
        )))
        .await
        .context("failed to write code-mode host hello")?;
    Ok(true)
}

struct HostState {
    sessions: Mutex<HashMap<SessionId, Arc<InProcessCodeModeSession>>>,
    seen_session_ids: Mutex<HashSet<SessionId>>,
    request_tasks: TaskTracker,
    closing: AtomicBool,
    peer: Arc<HostPeer>,
}

impl HostState {
    fn spawn_request(self: &Arc<Self>, request_id: RequestId, request: HostRequest) {
        let state = Arc::clone(self);
        self.request_tasks.spawn(async move {
            state.handle_request(request_id, request).await;
        });
    }

    async fn handle_request(&self, request_id: RequestId, request: HostRequest) {
        if self.closing.load(Ordering::Acquire) {
            self.respond(
                request_id,
                Err("code-mode host is shutting down".to_string()),
            );
            return;
        }
        match request {
            HostRequest::OpenSession { session_id } => {
                let result = self
                    .open_session(session_id.clone())
                    .map(|()| HostResponse::SessionReady { session_id });
                self.respond(request_id, result);
            }
            HostRequest::Execute {
                session_id,
                request,
            } => {
                let request = match request.try_into() {
                    Ok(request) => request,
                    Err(err) => {
                        self.respond(
                            request_id,
                            Err(format!("invalid code-mode execute request: {err}")),
                        );
                        return;
                    }
                };
                let result = match self.session(&session_id) {
                    Ok(session) => session.execute(request).await,
                    Err(err) => {
                        self.respond(request_id, Err(err));
                        return;
                    }
                };
                match result {
                    Ok(started) => {
                        let cell_id = started.cell_id.clone();
                        self.respond(
                            request_id,
                            Ok(HostResponse::ExecutionStarted {
                                cell_id: cell_id.into(),
                            }),
                        );
                        self.peer.start_cell(session_id, request_id, started);
                    }
                    Err(err) => self.respond(request_id, Err(err)),
                }
            }
            HostRequest::Wait {
                session_id,
                request,
            } => {
                let result = match self.session(&session_id) {
                    Ok(session) => session.wait(request.into()).await.map(|outcome| {
                        HostResponse::WaitCompleted {
                            outcome: outcome.into(),
                        }
                    }),
                    Err(err) => Err(err),
                };
                self.respond(request_id, result);
            }
            HostRequest::Terminate {
                session_id,
                cell_id,
            } => {
                let result = match self.session(&session_id) {
                    Ok(session) => session.terminate(cell_id.into()).await.map(|outcome| {
                        HostResponse::WaitCompleted {
                            outcome: outcome.into(),
                        }
                    }),
                    Err(err) => Err(err),
                };
                self.respond(request_id, result);
            }
            HostRequest::ShutdownSession { session_id } => {
                let session = self
                    .sessions
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .remove(&session_id);
                let result = match session {
                    Some(session) => session
                        .shutdown()
                        .await
                        .map(|()| HostResponse::SessionClosed { session_id }),
                    None => Err(format!("unknown code-mode session {session_id}")),
                };
                self.respond(request_id, result);
            }
        }
    }

    fn open_session(&self, session_id: SessionId) -> Result<(), String> {
        let mut sessions = self.sessions.lock().unwrap_or_else(PoisonError::into_inner);
        if self.closing.load(Ordering::Acquire) {
            return Err("code-mode host is shutting down".to_string());
        }
        if !self
            .seen_session_ids
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(session_id.clone())
        {
            return Err(format!("code-mode session ID `{session_id}` was reused"));
        }
        let delegate = Arc::new(RemoteDelegate::new(
            session_id.clone(),
            Arc::clone(&self.peer),
        ));
        sessions.insert(
            session_id,
            Arc::new(InProcessCodeModeSession::with_delegate(delegate)),
        );
        Ok(())
    }

    fn session(&self, session_id: &SessionId) -> Result<Arc<InProcessCodeModeSession>, String> {
        self.sessions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(session_id)
            .cloned()
            .ok_or_else(|| format!("unknown code-mode session {session_id}"))
    }

    fn respond(&self, id: RequestId, result: Result<HostResponse, String>) {
        self.peer.send(HostToClient::Response {
            id,
            result: WireResult::from_result(result),
        });
    }

    async fn disconnect(&self) {
        self.closing.store(true, Ordering::Release);
        let sessions = self
            .sessions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .drain()
            .map(|(_, session)| session)
            .collect::<Vec<_>>();
        for session in sessions {
            let _ = session.shutdown().await;
        }
        self.request_tasks.close();
        self.request_tasks.wait().await;
    }
}

#[cfg(test)]
#[path = "host_tests.rs"]
mod tests;
