use codex_backend_client::ApiKeyTurnCost;
use codex_backend_client::ApiKeyTurnCostStatus;
use codex_backend_client::Client as BackendClient;
use codex_backend_client::RequestError;
use codex_config::types::OtelExporterKind;
use codex_core::config::Config;
use codex_login::AuthManager;
use codex_model_provider::SharedModelProvider;
use codex_model_provider::create_model_provider;
use codex_otel::SessionTelemetry;
use codex_protocol::ThreadId;
use codex_protocol::auth::AuthMode;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::warn;

const POLL_INTERVAL: Duration = Duration::from_secs(150);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const OBSERVATION_CHANNEL_CAPACITY: usize = 16_384;
const MAX_TRACKED_TURNS: usize = 4_096;
const MAX_QUERY_TURNS: usize = 100;
const MAX_STALLED_POLL_ATTEMPTS: u8 = 5;

pub(crate) struct TurnCostWorker {
    handle: TurnCostWorkerHandle,
    shutdown: CancellationToken,
    _task: JoinHandle<()>,
}

#[derive(Clone)]
pub(crate) struct TurnCostWorkerHandle {
    sender: mpsc::Sender<TurnCostObservation>,
    backend: TurnCostBackend,
    config: Arc<Config>,
}

enum TurnCostObservationKind {
    Started {
        session_telemetry: Box<SessionTelemetry>,
    },
    ResponseCompleted,
    Finished {
        interrupted: bool,
    },
}

struct TurnCostObservation {
    thread_id: ThreadId,
    turn_id: String,
    kind: TurnCostObservationKind,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TurnCostStatus {
    Running,
    Completed,
    Interrupted,
}

struct TurnCostEntry {
    thread_id: ThreadId,
    session_telemetry: SessionTelemetry,
    expected_response_count: u64,
    status: TurnCostStatus,
    next_poll_at: Instant,
    attempt_count: u8,
}

struct WorkerRuntime {
    config: Arc<Config>,
    backend: TurnCostBackend,
    turns: HashMap<String, TurnCostEntry>,
}

#[derive(Clone)]
enum TurnCostBackend {
    OpenAiApiKey(Arc<AuthManager>),
    ModelProvider(SharedModelProvider),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BackendAvailability {
    AwaitingAuthChange,
    RetryProbe,
    Ready,
    Disabled,
}

impl TurnCostWorker {
    pub(crate) fn spawn(config: Arc<Config>, auth_manager: Arc<AuthManager>) -> Option<Self> {
        let has_otel_log_exporter = matches!(
            config.otel.exporter,
            OtelExporterKind::OtlpHttp { .. } | OtelExporterKind::OtlpGrpc { .. }
        );
        let has_otel_metrics_exporter = matches!(
            config.otel.metrics_exporter,
            OtelExporterKind::OtlpHttp { .. } | OtelExporterKind::OtlpGrpc { .. }
        );
        if !(has_otel_log_exporter || has_otel_metrics_exporter)
            || config.model_provider.is_amazon_bedrock()
        {
            return None;
        }
        let is_openai = config.model_provider.is_openai();
        let backend = if is_openai {
            TurnCostBackend::OpenAiApiKey(Arc::clone(&auth_manager))
        } else {
            TurnCostBackend::ModelProvider(create_model_provider(
                config.model_provider.clone(),
                Some(Arc::clone(&auth_manager)),
            ))
        };
        let (sender, receiver) = mpsc::channel(OBSERVATION_CHANNEL_CAPACITY);
        let shutdown = CancellationToken::new();
        let runtime = WorkerRuntime {
            config: Arc::clone(&config),
            backend: backend.clone(),
            turns: HashMap::new(),
        };
        let worker_shutdown = shutdown.clone();
        let task = tokio::spawn(async move {
            runtime.run(receiver, worker_shutdown).await;
        });
        Some(Self {
            handle: TurnCostWorkerHandle {
                sender,
                backend,
                config,
            },
            shutdown,
            _task: task,
        })
    }

    pub(crate) fn handle(&self) -> TurnCostWorkerHandle {
        self.handle.clone()
    }

    pub(crate) fn shutdown(&self) {
        self.shutdown.cancel();
    }
}

impl Drop for TurnCostWorker {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl TurnCostWorkerHandle {
    pub(crate) fn observe_event(
        &self,
        thread_id: ThreadId,
        thread_config: &Config,
        event: &Event,
        session_telemetry: impl FnOnce() -> SessionTelemetry,
    ) {
        if thread_config.model_provider != self.config.model_provider {
            return;
        }
        if let TurnCostBackend::OpenAiApiKey(auth_manager) = &self.backend {
            let Some(auth) = auth_manager.auth_cached() else {
                return;
            };
            if !auth.is_api_key_auth() {
                return;
            }
        }
        let kind = match &event.msg {
            EventMsg::TurnStarted(_) => TurnCostObservationKind::Started {
                session_telemetry: Box::new(session_telemetry()),
            },
            EventMsg::RawResponseCompleted(_) => TurnCostObservationKind::ResponseCompleted,
            EventMsg::TurnComplete(_) => TurnCostObservationKind::Finished { interrupted: false },
            EventMsg::TurnAborted(_) => TurnCostObservationKind::Finished { interrupted: true },
            _ => return,
        };
        let _ = self.sender.try_send(TurnCostObservation {
            thread_id,
            turn_id: event.id.clone(),
            kind,
        });
    }
}

impl WorkerRuntime {
    async fn run(self, receiver: mpsc::Receiver<TurnCostObservation>, shutdown: CancellationToken) {
        let auth_changes = match &self.backend {
            TurnCostBackend::OpenAiApiKey(auth_manager) => {
                Some(auth_manager.auth_change_receiver())
            }
            TurnCostBackend::ModelProvider(_) => None,
        };
        let backend_availability = self.probe_backend().await;
        self.run_with_backend_availability(receiver, shutdown, auth_changes, backend_availability)
            .await;
    }

    async fn run_with_backend_availability(
        mut self,
        mut receiver: mpsc::Receiver<TurnCostObservation>,
        shutdown: CancellationToken,
        mut auth_changes: Option<tokio::sync::watch::Receiver<u64>>,
        mut backend_availability: BackendAvailability,
    ) {
        let mut ticker = tokio::time::interval(POLL_INTERVAL);
        ticker.tick().await;
        loop {
            tokio::select! {
                biased;
                _ = shutdown.cancelled() => break,
                changed = async {
                    match auth_changes.as_mut() {
                        Some(auth_changes) => auth_changes.changed().await,
                        None => std::future::pending().await,
                    }
                } => {
                    if changed.is_err() {
                        break;
                    }
                    self.turns.clear();
                    while receiver.try_recv().is_ok() {}
                    backend_availability = self.probe_backend().await;
                }
                observation = receiver.recv() => {
                    let Some(observation) = observation else {
                        break;
                    };
                    if !matches!(
                        backend_availability,
                        BackendAvailability::Ready | BackendAvailability::RetryProbe
                    ) {
                        continue;
                    }
                    self.record_observation(observation);
                }
                _ = ticker.tick() => {
                    match backend_availability {
                        BackendAvailability::Ready => self.poll_due().await,
                        BackendAvailability::RetryProbe => {
                            backend_availability = self.probe_backend().await;
                        }
                        BackendAvailability::AwaitingAuthChange
                        | BackendAvailability::Disabled => {}
                    }
                }
            }
        }
    }

    async fn probe_backend(&self) -> BackendAvailability {
        let probe_turn_ids = [uuid::Uuid::new_v4().to_string()];
        match tokio::time::timeout(REQUEST_TIMEOUT, self.query_turn_costs(&probe_turn_ids)).await {
            Ok(Ok(Some(_))) => BackendAvailability::Ready,
            Ok(Ok(None)) => match self.backend {
                TurnCostBackend::OpenAiApiKey(_) => BackendAvailability::AwaitingAuthChange,
                TurnCostBackend::ModelProvider(_) => BackendAvailability::Disabled,
            },
            Ok(Err(error)) => match error.status().map(|status| status.as_u16()) {
                Some(401 | 403) if matches!(self.backend, TurnCostBackend::OpenAiApiKey(_)) => {
                    tracing::debug!(
                        "turn cost worker waiting for auth change after backend availability check: {error}"
                    );
                    BackendAvailability::AwaitingAuthChange
                }
                Some(401 | 403 | 429) => BackendAvailability::RetryProbe,
                Some(400..=499) => {
                    tracing::debug!(
                        "turn cost worker disabled by backend availability check: {error}"
                    );
                    BackendAvailability::Disabled
                }
                _ => {
                    tracing::debug!(
                        "turn cost worker backend availability check failed temporarily: {error}"
                    );
                    BackendAvailability::RetryProbe
                }
            },
            Err(_) => {
                tracing::debug!(
                    "turn cost worker backend availability check timed out; will retry"
                );
                BackendAvailability::RetryProbe
            }
        }
    }

    fn record_observation(&mut self, observation: TurnCostObservation) {
        match observation.kind {
            TurnCostObservationKind::Started { session_telemetry } => {
                if self.turns.len() < MAX_TRACKED_TURNS {
                    self.turns
                        .entry(observation.turn_id)
                        .or_insert(TurnCostEntry {
                            thread_id: observation.thread_id,
                            session_telemetry: *session_telemetry,
                            expected_response_count: 0,
                            status: TurnCostStatus::Running,
                            next_poll_at: Instant::now(),
                            attempt_count: 0,
                        });
                }
            }
            TurnCostObservationKind::ResponseCompleted => {
                if let Some(entry) = self.turns.get_mut(&observation.turn_id)
                    && entry.status == TurnCostStatus::Running
                {
                    entry.expected_response_count = entry.expected_response_count.saturating_add(1);
                }
            }
            TurnCostObservationKind::Finished { interrupted } => {
                let Some(entry) = self.turns.get_mut(&observation.turn_id) else {
                    return;
                };
                if entry.status != TurnCostStatus::Running {
                    return;
                }
                entry.status = if interrupted {
                    TurnCostStatus::Interrupted
                } else {
                    TurnCostStatus::Completed
                };
                entry.next_poll_at = Instant::now();
            }
        }
    }

    async fn poll_due(&mut self) {
        let now = Instant::now();
        let due_turn_ids: Vec<String> = self
            .turns
            .iter()
            .filter(|(_, entry)| {
                entry.status != TurnCostStatus::Running && entry.next_poll_at <= now
            })
            .take(MAX_QUERY_TURNS)
            .map(|(turn_id, _)| turn_id.clone())
            .collect();
        if !due_turn_ids.is_empty() {
            self.poll_api_key_entries(&due_turn_ids).await;
        }
    }

    async fn poll_api_key_entries(&mut self, turn_ids: &[String]) {
        let costs =
            match tokio::time::timeout(REQUEST_TIMEOUT, self.query_turn_costs(turn_ids)).await {
                Ok(Ok(Some(costs))) => costs,
                Ok(Ok(None)) => return,
                Ok(Err(error)) => {
                    warn!("failed to query API-key turn costs: {error}");
                    self.retry_entries(turn_ids);
                    return;
                }
                Err(_) => {
                    warn!("timed out querying API-key turn costs");
                    self.retry_entries(turn_ids);
                    return;
                }
            };
        let costs_by_turn: HashMap<String, ApiKeyTurnCost> = costs
            .into_iter()
            .map(|cost| (cost.turn_id.clone(), cost))
            .collect();
        for turn_id in turn_ids {
            let Some(cost) = costs_by_turn.get(turn_id) else {
                self.retry_entry(turn_id);
                continue;
            };
            self.process_api_key_cost(turn_id, cost);
        }
    }

    async fn query_turn_costs(
        &self,
        turn_ids: &[String],
    ) -> Result<Option<Vec<ApiKeyTurnCost>>, RequestError> {
        match &self.backend {
            TurnCostBackend::OpenAiApiKey(auth_manager) => {
                let Some(auth) = auth_manager.auth().await else {
                    return Ok(None);
                };
                if !auth.is_api_key_auth() {
                    return Ok(None);
                }
                let provider = self
                    .config
                    .model_provider
                    .to_api_provider(Some(AuthMode::ApiKey))
                    .map_err(|error| RequestError::Other(error.into()))?;
                let client = BackendClient::from_auth(
                    self.config.chatgpt_base_url.clone(),
                    &auth,
                    self.config.http_client_factory(),
                );
                client
                    .query_api_key_turn_costs(turn_ids, &provider.headers)
                    .await
                    .map(Some)
            }
            TurnCostBackend::ModelProvider(model_provider) => {
                if model_provider.info().requires_openai_auth {
                    let Some(auth) = model_provider.auth().await else {
                        return Ok(None);
                    };
                    if !auth.is_api_key_auth() {
                        return Ok(None);
                    }
                }
                let provider = model_provider
                    .api_provider()
                    .await
                    .map_err(|error| RequestError::Other(error.into()))?;
                let auth = model_provider
                    .api_auth()
                    .await
                    .map_err(|error| RequestError::Other(error.into()))?;
                let endpoint = provider.url_for_path("analytics/codex/turn-costs");
                let client = BackendClient::new(
                    provider.base_url.clone(),
                    self.config.http_client_factory(),
                )
                .with_auth_provider(auth);
                client
                    .query_api_key_turn_costs_at(&endpoint, turn_ids, &provider.headers)
                    .await
                    .map(Some)
            }
        }
    }

    fn process_api_key_cost(&mut self, turn_id: &str, cost: &ApiKeyTurnCost) {
        if cost.status != ApiKeyTurnCostStatus::Priced {
            self.retry_entry(turn_id);
            return;
        }
        let response_count = cost
            .responses
            .as_ref()
            .map(|responses| responses.len() as u64)
            .or(cost.event_count);
        let (Some(total_usd), Some(response_count)) = (cost.total_usd.as_deref(), response_count)
        else {
            self.retry_entry(turn_id);
            return;
        };
        let Some(entry) = self.turns.get(turn_id) else {
            return;
        };
        if response_count < entry.expected_response_count {
            self.retry_entry(turn_id);
            return;
        }
        let mut session_telemetry = entry.session_telemetry.clone();
        if let Some(model) = cost.model.as_deref() {
            session_telemetry = session_telemetry.with_model(model, model);
        }
        let Some(entry) = self.turns.remove(turn_id) else {
            return;
        };
        session_telemetry.record_turn_cost(
            turn_id,
            total_usd,
            entry.status == TurnCostStatus::Interrupted,
            cost.speed.as_deref(),
            cost.reasoning_effort.as_deref(),
        );
    }

    fn retry_entries(&mut self, turn_ids: &[String]) {
        for turn_id in turn_ids {
            self.retry_entry(turn_id);
        }
    }

    fn retry_entry(&mut self, turn_id: &str) {
        let Some(entry) = self.turns.get_mut(turn_id) else {
            return;
        };
        entry.attempt_count = entry.attempt_count.saturating_add(1);
        if entry.attempt_count >= MAX_STALLED_POLL_ATTEMPTS {
            warn!(
                thread_id = %entry.thread_id,
                turn_id,
                attempts = MAX_STALLED_POLL_ATTEMPTS,
                "dropping turn cost event after repeated unsuccessful polls"
            );
            self.turns.remove(turn_id);
            return;
        }
        entry.next_poll_at = Instant::now() + POLL_INTERVAL;
    }
}

#[cfg(test)]
#[path = "turn_cost_worker_tests.rs"]
mod tests;
