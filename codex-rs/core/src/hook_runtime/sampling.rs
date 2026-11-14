//! A sampling loop keeps one request ID across retries. Each call to open a
//! response stream pairs its own hooks; transport recovery within that call
//! does not start another hook pair.

use super::emit_hook_completed_events;
use super::emit_hook_started_events;
use super::hooks_for_model_request;
use super::run_model_request_hooks;
use super::thread_spawn_subagent_hook_context;
use super::tool_hook_cwd;
use crate::client::OpenedModelResponse;
use crate::client_common::SamplingStream;
use crate::session::session::Session;
use crate::session::step_context::StepContext;
use codex_hooks::ModelRequestHookEvent;
use codex_hooks::ModelRequestHookInput;
use codex_hooks::ModelRequestHookOutcome;
use codex_hooks::SamplingKind;
use codex_protocol::error::Result;
use codex_protocol::protocol::HookCompletedEvent;
use codex_protocol::protocol::HookEventName;
use codex_protocol::protocol::HookExecutionMode;
use codex_protocol::protocol::HookRunStatus;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub(crate) struct SamplingRequest {
    id: Uuid,
    attempts: u32,
    kind: SamplingKind,
}

impl SamplingRequest {
    pub(crate) fn new(kind: SamplingKind) -> Self {
        Self {
            id: Uuid::new_v4(),
            attempts: 0,
            kind,
        }
    }

    pub(crate) fn open(
        &mut self,
        session: &Arc<Session>,
        step: &Arc<StepContext>,
        model: &str,
        owner_cancellation: &CancellationToken,
        send: impl Future<Output = Result<OpenedModelResponse>> + Send,
    ) -> impl Future<Output = Result<SamplingStream>> + Send {
        // Keep the transport and hook state out of the turn's cancellation and retry futures.
        Box::pin(self.open_attempt(session, step, model, owner_cancellation, send))
    }

    async fn open_attempt(
        &mut self,
        session: &Arc<Session>,
        step: &Arc<StepContext>,
        model: &str,
        owner_cancellation: &CancellationToken,
        send: impl Future<Output = Result<OpenedModelResponse>>,
    ) -> Result<SamplingStream> {
        self.attempts += 1;
        let sampling_turn = match self.kind {
            SamplingKind::Sampling => Some(Arc::clone(&step.turn)),
            SamplingKind::ForegroundCompaction | SamplingKind::BackgroundCompaction => None,
        };
        let settlement = Some((Arc::clone(session), Arc::clone(&step.turn)));
        let hooks = hooks_for_model_request(session, step);
        let pre_mode = hooks.execution_mode_for_event(HookEventName::PreModelRequest);
        let post_mode = hooks.execution_mode_for_event(HookEventName::PostModelRequest);
        if pre_mode.is_none() && post_mode.is_none() {
            return send.await?.deliver(&mut None, sampling_turn, settlement);
        }
        let subagent = thread_spawn_subagent_hook_context(session, &step.turn);
        let input = ModelRequestHookInput {
            lifecycle_id: uuid::Uuid::new_v4(),
            kind: self.kind,
            request_id: self.id.to_string(),
            attempt_number: self.attempts,
            session_id: session.session_id().to_string(),
            thread_id: session.thread_id().to_string(),
            turn_id: step.turn.sub_id.clone(),
            cwd: tool_hook_cwd(&step.environments, &step.turn),
            transcript_path: session.hook_transcript_path().await,
            model: model.to_owned(),
            event: ModelRequestHookEvent::PreModelRequest,
            agent_id: subagent.as_ref().map(|context| context.agent_id.clone()),
            agent_type: subagent.map(|context| context.agent_type),
        };
        // Hook input preparation may wait. Do not start a new hook pair after a stop;
        // the HTTP/WS send independently rechecks after a pre-hook has finished.
        session.check_rollout_budget()?;
        let mut attempt = post_mode.map(|mode| SamplingAttempt {
            owner_cancellation: owner_cancellation.clone(),
            post: Some(PostHooks {
                hooks: Arc::clone(&hooks),
                step: Arc::clone(step),
                input: input.clone(),
                mode,
            }),
            session: Arc::clone(session),
        });
        if pre_mode.is_some() {
            run_model_request_hooks(session, step, &hooks, input).await;
        }
        let stream = send
            .await
            .and_then(|opened| opened.deliver(&mut attempt, sampling_turn, settlement));
        if let Err(error) = &stream
            && let Some(mut attempt) = attempt
            && let Some(hooks) = attempt.start_post_hooks(ModelRequestHookEvent::failed(error))
        {
            hooks.await;
        }
        stream
    }
}

/// The sampling future owns the synchronous handlers. Observer-only and
/// pre-response cancellation notifications run on the session's hook executor.
struct PostHooks {
    hooks: Arc<codex_hooks::Hooks>,
    step: Arc<StepContext>,
    input: ModelRequestHookInput,
    mode: HookExecutionMode,
}

impl PostHooks {
    async fn run(
        mut self,
        session: &Arc<Session>,
        event: ModelRequestHookEvent,
    ) -> ModelRequestHookOutcome {
        self.input.event = event;
        run_model_request_hooks(session, &self.step, &self.hooks, self.input).await
    }

    async fn run_sync(
        mut self,
        session: Arc<Session>,
        event: ModelRequestHookEvent,
    ) -> ModelRequestHookOutcome {
        let (mut done, wait) = oneshot::channel();
        self.input.event = event;
        // Dropping the waiter cancels execution while the task finishes publishing
        // lifecycle events. It must run independently of the async hook limit.
        tokio::spawn(async move {
            if done.is_closed() {
                return;
            }
            let turn = &self.step.turn;
            let runs = self.hooks.preview_model_request(&self.input);
            let started = Instant::now();
            emit_hook_started_events(&session, turn, runs.clone()).await;
            let mut finished: Vec<HookCompletedEvent> = Vec::new();
            let (events, outcome) = tokio::select! {
                biased;
                () = done.closed() => {
                    let completed_at = chrono::Utc::now().timestamp();
                    let duration_ms = i64::try_from(started.elapsed().as_millis()).unwrap_or(i64::MAX);
                    for mut run in runs {
                        if run.execution_mode != HookExecutionMode::Sync
                            || finished.iter().any(|event| event.run.id == run.id)
                        {
                            continue;
                        }
                        run.status = HookRunStatus::Stopped;
                        run.status_message = Some("Hook cancelled because the turn was interrupted".into());
                        run.completed_at = Some(completed_at);
                        run.duration_ms = Some(duration_ms);
                        finished.push(HookCompletedEvent {
                            turn_id: Some(turn.sub_id.clone()),
                            run,
                        });
                    }
                    (finished, ModelRequestHookOutcome::default())
                }
                events = self.hooks.run_model_request(self.input, |event| finished.push(event.clone())) => events,
            };
            emit_hook_completed_events(&session, turn, events).await;
            let _ = done.send(outcome);
        });
        wait.await.unwrap_or_else(|_| ModelRequestHookOutcome {
            invalid_price: Some("synchronous hook task exited without reporting its charge".into()),
            ..Default::default()
        })
    }

    fn schedule(self, session: Arc<Session>, event: ModelRequestHookEvent) {
        Arc::clone(&self.hooks).schedule_async_task(async move {
            self.run(&session, event).await;
        });
    }
}

pub(crate) struct SamplingAttempt {
    /// The turn owns foreground responses; the session owns speculative responses.
    pub(crate) owner_cancellation: CancellationToken,
    session: Arc<Session>,
    post: Option<PostHooks>,
}

impl Drop for SamplingAttempt {
    fn drop(&mut self) {
        if let Some(post) = self.post.take() {
            post.schedule(
                Arc::clone(&self.session),
                ModelRequestHookEvent::cancelled(),
            );
        }
    }
}

impl SamplingAttempt {
    /// Observers run on the session executor. The request owns the synchronous
    /// group: if the turn stops waiting, dropping this future cancels it.
    pub(crate) fn start_post_hooks(
        &mut self,
        event: ModelRequestHookEvent,
    ) -> Option<impl Future<Output = ModelRequestHookOutcome> + Send + 'static> {
        let post = self.post.take()?;
        let session = Arc::clone(&self.session);
        if post.mode == HookExecutionMode::Sync {
            Some(post.run_sync(session, event))
        } else {
            post.schedule(session, event);
            None
        }
    }
}
