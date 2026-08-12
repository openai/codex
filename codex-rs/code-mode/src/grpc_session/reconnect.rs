use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use codex_code_mode_protocol::CellId;
use codex_code_mode_protocol::CodeModeSession;
use codex_code_mode_protocol::CodeModeSessionCellExecutionLimits;
use codex_code_mode_protocol::CodeModeSessionDelegate;
use codex_code_mode_protocol::CodeModeSessionResultFuture;
use codex_code_mode_protocol::ExecuteRequest;
use codex_code_mode_protocol::StartedCell;
use codex_code_mode_protocol::WaitOutcome;
use codex_code_mode_protocol::WaitRequest;
use tokio::sync::Semaphore;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use super::GrpcCodeModeSession;
use super::GrpcCodeModeSessionProvider;
use super::SHUTDOWN_ERROR;
use super::generation;
use super::generation::GenerationDelegate;
use crate::remote_session::ShutdownResultReceiver;
use crate::remote_session::wait_for_watch;

pub(super) struct ReconnectableSession {
    inner: Arc<ReconnectInner>,
}

struct ReconnectInner {
    provider: GrpcCodeModeSessionProvider,
    delegate: Arc<dyn CodeModeSessionDelegate>,
    limits: CodeModeSessionCellExecutionLimits,
    binding: Mutex<Option<SessionBinding>>,
    opening_permit: Semaphore,
    next_generation: AtomicU64,
    shutdown_requested: CancellationToken,
    shutdown_result: Mutex<Option<ShutdownResultReceiver>>,
}

#[derive(Clone)]
struct SessionBinding {
    session: Arc<GrpcCodeModeSession>,
    generation: u64,
}

impl ReconnectableSession {
    pub(super) fn new(
        provider: GrpcCodeModeSessionProvider,
        delegate: Arc<dyn CodeModeSessionDelegate>,
        limits: CodeModeSessionCellExecutionLimits,
    ) -> Self {
        Self {
            inner: Arc::new(ReconnectInner {
                provider,
                delegate,
                limits,
                binding: Mutex::new(None),
                opening_permit: Semaphore::new(/*permits*/ 1),
                next_generation: AtomicU64::new(1),
                shutdown_requested: CancellationToken::new(),
                shutdown_result: Mutex::new(None),
            }),
        }
    }

    pub(super) async fn initialize(&self) -> Result<(), String> {
        self.inner.get_or_open_binding().await.map(|_| ())
    }
}

impl CodeModeSession for ReconnectableSession {
    fn execute<'a>(
        &'a self,
        request: ExecuteRequest,
    ) -> CodeModeSessionResultFuture<'a, StartedCell> {
        Box::pin(async move {
            let binding = self.inner.get_or_open_binding().await?;
            let started = binding.session.execute(request).await?;
            Ok(generation::public_started_cell(binding.generation, started))
        })
    }

    fn wait<'a>(&'a self, request: WaitRequest) -> CodeModeSessionResultFuture<'a, WaitOutcome> {
        Box::pin(async move {
            let binding = self.inner.get_or_open_binding().await?;
            let request = WaitRequest {
                cell_id: generation::remote_cell_id(binding.generation, &request.cell_id)?,
                yield_time_ms: request.yield_time_ms,
            };
            let outcome = binding.session.wait(request).await?;
            Ok(generation::public_wait_outcome(binding.generation, outcome))
        })
    }

    fn terminate<'a>(&'a self, cell_id: CellId) -> CodeModeSessionResultFuture<'a, WaitOutcome> {
        Box::pin(async move {
            let binding = self.inner.get_or_open_binding().await?;
            let cell_id = generation::remote_cell_id(binding.generation, &cell_id)?;
            let outcome = binding.session.terminate(cell_id).await?;
            Ok(generation::public_wait_outcome(binding.generation, outcome))
        })
    }

    fn shutdown<'a>(&'a self) -> CodeModeSessionResultFuture<'a, ()> {
        Box::pin(wait_for_watch(self.inner.request_shutdown()))
    }
}

impl Drop for ReconnectableSession {
    fn drop(&mut self) {
        if tokio::runtime::Handle::try_current().is_ok() {
            self.inner.request_shutdown();
        }
    }
}

impl ReconnectInner {
    async fn get_or_open_binding(&self) -> Result<SessionBinding, String> {
        if self.shutdown_requested.is_cancelled() {
            return Err(SHUTDOWN_ERROR.to_string());
        }
        if let Some(binding) = self.live_binding() {
            return Ok(binding);
        }

        let _opening_permit = tokio::select! {
            biased;
            _ = self.shutdown_requested.cancelled() => {
                return Err(SHUTDOWN_ERROR.to_string());
            }
            permit = self.opening_permit.acquire() => permit
                .map_err(|_| "gRPC code-mode session opening coordinator closed".to_string())?,
        };
        if self.shutdown_requested.is_cancelled() {
            return Err(SHUTDOWN_ERROR.to_string());
        }
        if let Some(binding) = self.live_binding() {
            return Ok(binding);
        }

        let previous_binding = self
            .binding
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        if let Some(binding) = previous_binding {
            wait_for_watch(binding.session.inner.request_shutdown()).await?;
        }

        let generation = self.next_generation.fetch_add(1, Ordering::Relaxed);
        let delegate = Arc::new(GenerationDelegate {
            delegate: Arc::clone(&self.delegate),
            generation,
        });
        let session = tokio::select! {
            biased;
            _ = self.shutdown_requested.cancelled() => {
                return Err(SHUTDOWN_ERROR.to_string());
            }
            session = self.provider.open_binding(delegate, self.limits.clone()) => session?,
        };
        let binding = SessionBinding {
            session,
            generation,
        };
        let published = {
            let mut current = self.binding.lock().unwrap_or_else(PoisonError::into_inner);
            if self.shutdown_requested.is_cancelled() {
                false
            } else {
                *current = Some(binding.clone());
                true
            }
        };
        if !published {
            let _ = wait_for_watch(binding.session.inner.request_shutdown()).await;
            return Err(SHUTDOWN_ERROR.to_string());
        }
        Ok(binding)
    }

    fn live_binding(&self) -> Option<SessionBinding> {
        self.binding
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .filter(|binding| !binding.session.inner.stopped.is_cancelled())
            .cloned()
    }

    fn request_shutdown(self: &Arc<Self>) -> ShutdownResultReceiver {
        {
            let binding = self.binding.lock().unwrap_or_else(PoisonError::into_inner);
            self.shutdown_requested.cancel();
            if let Some(binding) = binding.as_ref() {
                binding.session.inner.request_shutdown();
            }
        }
        let mut result = self
            .shutdown_result
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(receiver) = result.as_ref() {
            return receiver.clone();
        }

        let (sender, receiver) = watch::channel(None);
        *result = Some(receiver.clone());
        let inner = Arc::clone(self);
        tokio::spawn(async move {
            let opening_permit = match inner.opening_permit.acquire().await {
                Ok(permit) => permit,
                Err(_) => {
                    sender.send_replace(Some(Err(
                        "gRPC code-mode session opening coordinator closed".to_string(),
                    )));
                    return;
                }
            };
            let binding = inner
                .binding
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .take();
            drop(opening_permit);
            let result = match binding {
                Some(binding) => wait_for_watch(binding.session.inner.request_shutdown()).await,
                None => Ok(()),
            };
            sender.send_replace(Some(result));
        });
        receiver
    }
}
