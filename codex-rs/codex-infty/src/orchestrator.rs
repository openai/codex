use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use codex_core::CodexAuth;
use codex_core::CodexConversation;
use codex_core::ConversationManager;
use codex_core::cross_session::CrossSessionHub;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_protocol::ConversationId;
use tokio::signal;
use tokio_stream::StreamExt;
use tokio_util::sync::CancellationToken;
use tracing::warn;

use crate::progress::ProgressReporter;
use crate::prompts;
use crate::roles::Role;
use crate::roles::director::DirectionRequestPayload;
use crate::roles::director::DirectorRole;
use crate::roles::solver::SolverRequest;
use crate::roles::solver::SolverRole;
use crate::roles::solver::SolverSignal;
use crate::roles::solver::parse_solver_signal;
use crate::roles::verifier::VerificationRequestPayload;
use crate::roles::verifier_pool::VerifierPool;
use crate::run_store::RoleMetadata;
use crate::run_store::RunStore;
use crate::session;
use crate::signals::AggregatedVerifierVerdict;
use crate::types::RoleConfig;
use crate::types::RoleSession;
use crate::types::RunExecutionOptions;
use crate::types::RunOutcome;
use crate::types::RunParams;
use crate::types::RunSessions;

#[derive(Default)]
struct LoopState {
    waiting_for_signal: bool,
    pending_solver_turn_completion: bool,
}

struct SessionCleanup {
    conversation_id: ConversationId,
    conversation: Arc<CodexConversation>,
}

impl SessionCleanup {
    fn new(session: &RoleSession) -> Self {
        Self {
            conversation_id: session.conversation_id,
            conversation: Arc::clone(&session.conversation),
        }
    }
}

pub struct InftyOrchestrator {
    hub: Arc<CrossSessionHub>,
    conversation_manager: ConversationManager,
    runs_root: PathBuf,
    progress: Option<Arc<dyn ProgressReporter>>,
}

impl InftyOrchestrator {
    fn progress_ref(&self) -> Option<&dyn ProgressReporter> {
        self.progress.as_deref()
    }
    pub fn new(auth: CodexAuth) -> Result<Self> {
        let runs_root = crate::default_runs_root()?;
        Ok(Self::with_runs_root(auth, runs_root))
    }

    pub fn with_runs_root(auth: CodexAuth, runs_root: impl Into<PathBuf>) -> Self {
        Self {
            hub: Arc::new(CrossSessionHub::new()),
            conversation_manager: ConversationManager::with_auth(auth),
            runs_root: runs_root.into(),
            progress: None,
        }
    }

    pub fn runs_root(&self) -> &PathBuf {
        &self.runs_root
    }

    pub fn hub(&self) -> Arc<CrossSessionHub> {
        Arc::clone(&self.hub)
    }

    pub fn with_progress(mut self, reporter: Arc<dyn ProgressReporter>) -> Self {
        self.progress = Some(reporter);
        self
    }

    pub async fn execute_new_run(
        &self,
        params: RunParams,
        options: RunExecutionOptions,
    ) -> Result<RunOutcome> {
        let sessions = self.spawn_run(params).await?;
        self.drive_run(sessions, options).await
    }

    // resumable runs are disabled; execute_existing_run removed

    pub async fn spawn_run(&self, params: RunParams) -> Result<RunSessions> {
        let RunParams {
            run_id,
            run_root,
            solver,
            director,
            verifiers,
        } = params;

        let run_path = run_root.unwrap_or_else(|| self.runs_root.join(&run_id));
        let role_metadata = collect_role_metadata(&solver, &director, &verifiers);
        let mut store = RunStore::initialize(&run_path, &run_id, &role_metadata)?;
        let mut cleanup = Vec::new();

        let solver_session = match self
            .spawn_and_register_role(&run_id, &run_path, &solver, &mut store, &mut cleanup)
            .await
        {
            Ok(session) => session,
            Err(err) => {
                self.cleanup_failed_spawn(cleanup, &run_path).await;
                return Err(err);
            }
        };

        let director_session = match self
            .spawn_and_register_role(&run_id, &run_path, &director, &mut store, &mut cleanup)
            .await
        {
            Ok(session) => session,
            Err(err) => {
                self.cleanup_failed_spawn(cleanup, &run_path).await;
                return Err(err);
            }
        };

        let mut verifier_sessions = Vec::with_capacity(verifiers.len());
        for verifier in verifiers {
            let session = match self
                .spawn_and_register_role(&run_id, &run_path, &verifier, &mut store, &mut cleanup)
                .await
            {
                Ok(session) => session,
                Err(err) => {
                    self.cleanup_failed_spawn(cleanup, &run_path).await;
                    return Err(err);
                }
            };
            verifier_sessions.push(session);
        }

        Ok(RunSessions {
            run_id,
            solver: solver_session,
            director: director_session,
            verifiers: verifier_sessions,
            store,
        })
    }

    // resumable runs are disabled; resume_run removed

    async fn drive_run(
        &self,
        mut sessions: RunSessions,
        options: RunExecutionOptions,
    ) -> Result<RunOutcome> {
        let result = self.inner_drive_run(&mut sessions, &options).await;
        let cleanup = collect_session_cleanup(&sessions);
        self.shutdown_sessions(cleanup).await;
        result
    }

    async fn inner_drive_run(
        &self,
        sessions: &mut RunSessions,
        options: &RunExecutionOptions,
    ) -> Result<RunOutcome> {
        let solver_role = SolverRole::new(
            Arc::clone(&self.hub),
            sessions.run_id.clone(),
            sessions.solver.role.clone(),
            sessions.solver.conversation_id,
            self.progress.clone(),
        );
        let director_role = DirectorRole::new(
            Arc::clone(&self.hub),
            sessions.run_id.clone(),
            sessions.director.role.clone(),
            options.director_timeout,
            self.progress.clone(),
        );
        let mut verifier_pool = VerifierPool::from_sessions(
            Arc::clone(&self.hub),
            sessions,
            options.verifier_timeout,
            self.progress.clone(),
        );

        let mut solver_events = solver_role.stream_events()?;
        let mut state = LoopState::default();
        self.maybe_post_objective(&solver_role, sessions, &mut state, options)
            .await?;

        // Cancellation token that propagates Ctrl+C to nested awaits
        let cancel = CancellationToken::new();
        let cancel_on_ctrl_c = cancel.clone();
        tokio::spawn(async move {
            let _ = signal::ctrl_c().await;
            cancel_on_ctrl_c.cancel();
        });

        'event_loop: loop {
            tokio::select! {
                maybe_event = solver_events.next() => {
                    let Some(event) = maybe_event else {
                        break 'event_loop;
                    };
                    if let Some(p) = self.progress_ref() { p.solver_event(&event.event.msg); }
                    match &event.event.msg {
                        EventMsg::AgentMessage(agent_msg) => {
                            if let Some(p) = self.progress_ref() { p.solver_agent_message(agent_msg); }
                            if let Some(signal) = parse_solver_signal(&agent_msg.message) {
                                state.waiting_for_signal = false;
                                match signal {
                                    SolverSignal::DirectionRequest { prompt } => {
                                        let prompt = crate::utils::required_trimmed(
                                            prompt,
                                            "solver direction_request missing prompt text",
                                        )?;
                                        if let Some(p) = self.progress_ref() { p.direction_request(&prompt); }
                                        self
                                            .handle_direction_request(
                                                &prompt,
                                                options,
                                                &director_role,
                                                &solver_role,
                                                cancel.clone(),
                                            )
                                            .await?;
                                        sessions.store.touch()?;
                                        state.pending_solver_turn_completion = true;
                                    }
                                    SolverSignal::FinalDelivery {
                                        deliverable_path,
                                        summary,
                                    } => {
                                        let deliverable_path = crate::utils::required_trimmed(
                                            deliverable_path,
                                            "solver final_delivery missing deliverable_path",
                                        )?;
                                        if deliverable_path.is_empty() { bail!("solver final_delivery provided empty path"); }

                                        // Minimal behavior: if the provided path cannot be resolved,
                                        // send a placeholder claim so verifiers can fail it.
                                        let resolved = crate::utils::resolve_deliverable_path(
                                            sessions.store.path(),
                                            &deliverable_path,
                                        )
                                        .unwrap_or_else(|_| std::path::PathBuf::from("file not existing"));

                                        let summary_clean = crate::utils::trim_to_non_empty(summary);
                                        let summary_ref = summary_clean.as_deref();
                                        if let Some(p) = self.progress_ref() { p.final_delivery(&resolved, summary_ref); }
                                        let verified = self
                                            .run_final_verification(
                                                sessions,
                                                &mut verifier_pool,
                                                &resolved,
                                                summary_ref,
                                                options,
                                                &solver_role,
                                                cancel.clone(),
                                            )
                                            .await?;
                                        if !verified { state.pending_solver_turn_completion = true; continue; }
                                        sessions.store.touch()?;
                                        return Ok(RunOutcome {
                                            run_id: sessions.run_id.clone(),
                                            deliverable_path: resolved,
                                            summary: summary_clean,
                                            raw_message: agent_msg.message.clone(),
                                        });
                                    }
                                }
                            }
                        }
                        EventMsg::TaskComplete(..) => {
                            if state.waiting_for_signal {
                                // The solver completed its turn without issuing a signal; ask for one now.
                                solver_role.request_finalization_signal().await?;
                            } else if state.pending_solver_turn_completion {
                                // We handled a signal earlier in the loop; this completion corresponds to it.
                                state.pending_solver_turn_completion = false;
                            }
                        }
                        EventMsg::Error(error) => {
                            tracing::error!("Error: {:?}", error);
                        }
                        EventMsg::StreamError(error) => {
                            tracing::error!("Stream error: {:?}", error);
                        }
                        e => {
                            tracing::info!("Unhandled event: {:?}", e); // todo move to trace
                        }
                    }
                }
                _ = cancel.cancelled() => {
                    if let Some(progress) = self.progress.as_ref() { progress.run_interrupted(); }
                    // Proactively interrupt any in-flight role turns for fast shutdown.
                    let _ = sessions.solver.conversation.submit(Op::Interrupt).await;
                    let _ = sessions.director.conversation.submit(Op::Interrupt).await;
                    for v in &sessions.verifiers { let _ = v.conversation.submit(Op::Interrupt).await; }
                    // Cleanup is handled by the caller (drive_run) to avoid double-shutdown
                    bail!("run interrupted by Ctrl+C");
                }
            }
        }

        Err(anyhow!(
            "run {} ended before emitting final_delivery message",
            sessions.run_id
        ))
    }

    async fn maybe_post_objective(
        &self,
        solver: &crate::roles::solver::SolverRole,
        sessions: &mut RunSessions,
        state: &mut LoopState,
        options: &RunExecutionOptions,
    ) -> Result<()> {
        if let Some(objective) = options.objective.as_deref()
            && !objective.trim().is_empty()
        {
            solver
                .post(objective, Some(SolverRole::solver_signal_schema()))
                .await?;
            sessions.store.touch()?;
            state.waiting_for_signal = true;
            if let Some(p) = self.progress_ref() {
                p.objective_posted(objective);
            }
        }
        Ok(())
    }

    async fn handle_direction_request(
        &self,
        prompt: &str,
        options: &RunExecutionOptions,
        director_role: &DirectorRole,
        solver_role: &SolverRole,
        cancel: CancellationToken,
    ) -> Result<()> {
        let request = DirectionRequestPayload::new(prompt, options.objective.as_deref());
        let directive_payload = tokio::select! {
            r = director_role.call(&request) => {
                r.context("director response was not valid directive JSON")?
            }
            _ = cancel.cancelled() => {
                bail!("interrupted")
            }
        };
        if let Some(progress) = self.progress.as_ref() {
            progress.director_response(&directive_payload);
        }
        let req = SolverRequest::from(directive_payload);
        tokio::select! {
            r = solver_role.call(&req) => { r?; }
            _ = cancel.cancelled() => { bail!("interrupted"); }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_final_verification(
        &self,
        sessions: &mut RunSessions,
        verifier_pool: &mut VerifierPool,
        deliverable_path: &Path,
        summary: Option<&str>,
        options: &RunExecutionOptions,
        solver_role: &SolverRole,
        cancel: CancellationToken,
    ) -> Result<bool> {
        let relative = deliverable_path
            .strip_prefix(sessions.store.path())
            .ok()
            .and_then(|p| p.to_str().map(|s| s.to_string()));
        let claim_path = relative.unwrap_or_else(|| deliverable_path.display().to_string());

        let objective = crate::utils::objective_as_str(options);

        let request = VerificationRequestPayload::new(claim_path.as_str(), summary, objective);
        if verifier_pool.is_empty() {
            return Ok(true);
        }
        let round = tokio::select! {
            r = verifier_pool.collect_round(&request) => { r? }
            _ = cancel.cancelled() => { bail!("interrupted"); }
        };
        verifier_pool
            .rotate_passing(sessions, &self.conversation_manager, &round.passing_roles)
            .await?;
        let summary_result = round.summary;
        self.emit_verification_summary(&summary_result);
        let req = SolverRequest::from(&summary_result);
        tokio::select! {
            r = solver_role.call(&req) => { r?; }
            _ = cancel.cancelled() => { bail!("interrupted"); }
        }
        Ok(summary_result.overall.is_pass())
    }

    fn emit_verification_summary(&self, summary: &AggregatedVerifierVerdict) {
        if let Some(progress) = self.progress.as_ref() {
            progress.verification_summary(summary);
        }
    }

    async fn cleanup_failed_spawn(&self, sessions: Vec<SessionCleanup>, run_path: &Path) {
        self.shutdown_sessions(sessions).await;
        if run_path.exists()
            && let Err(err) = fs::remove_dir_all(run_path)
        {
            warn!(
                path = %run_path.display(),
                ?err,
                "failed to remove run directory after spawn failure"
            );
        }
    }

    // resumable runs are disabled; cleanup_failed_resume removed

    async fn shutdown_sessions(&self, sessions: Vec<SessionCleanup>) {
        for session in sessions {
            if let Err(err) = session.conversation.submit(Op::Shutdown).await {
                warn!(
                    %session.conversation_id,
                    ?err,
                    "failed to shutdown session during cleanup"
                );
            }
            let _ = self
                .conversation_manager
                .remove_conversation(&session.conversation_id)
                .await;
        }
    }

    async fn spawn_and_register_role(
        &self,
        run_id: &str,
        run_path: &Path,
        role_config: &RoleConfig,
        store: &mut RunStore,
        cleanup: &mut Vec<SessionCleanup>,
    ) -> Result<RoleSession> {
        let session = session::spawn_role(
            Arc::clone(&self.hub),
            &self.conversation_manager,
            run_id,
            run_path,
            role_config.clone(),
            prompts::ensure_instructions,
        )
        .await?;
        cleanup.push(SessionCleanup::new(&session));
        store.update_rollout_path(&session.role, session.rollout_path.clone())?;
        if let Some(path) = role_config.config_path.clone() {
            store.set_role_config_path(&session.role, path)?;
        }
        Ok(session)
    }

    // resumable runs are disabled; resume_and_register_role removed
}

impl InftyOrchestrator {
    /// Test-only helper to run a single verification round against all verifiers,
    /// applying the replacement policy (replace passes, keep failures).
    pub async fn verify_round_for_test(
        &self,
        sessions: &mut RunSessions,
        claim_path: &str,
        options: &RunExecutionOptions,
    ) -> Result<AggregatedVerifierVerdict> {
        let mut pool = VerifierPool::from_sessions(
            Arc::clone(&self.hub),
            sessions,
            options.verifier_timeout,
            self.progress.clone(),
        );
        let req = VerificationRequestPayload::new(claim_path, None, None);
        let round = pool.collect_round(&req).await?;
        pool.rotate_passing(sessions, &self.conversation_manager, &round.passing_roles)
            .await?;
        Ok(round.summary)
    }
}

fn collect_role_metadata(
    solver: &RoleConfig,
    director: &RoleConfig,
    verifiers: &[RoleConfig],
) -> Vec<RoleMetadata> {
    solver_and_director_metadata(solver, director)
        .into_iter()
        .chain(verifiers.iter().map(|verifier| RoleMetadata {
            role: verifier.role.clone(),
            rollout_path: None,
            config_path: verifier.config_path.clone(),
        }))
        .collect()
}

fn solver_and_director_metadata(solver: &RoleConfig, director: &RoleConfig) -> Vec<RoleMetadata> {
    vec![
        RoleMetadata {
            role: solver.role.clone(),
            rollout_path: None,
            config_path: solver.config_path.clone(),
        },
        RoleMetadata {
            role: director.role.clone(),
            rollout_path: None,
            config_path: director.config_path.clone(),
        },
    ]
}

fn collect_session_cleanup(sessions: &RunSessions) -> Vec<SessionCleanup> {
    let mut cleanup = Vec::with_capacity(2 + sessions.verifiers.len());
    cleanup.push(SessionCleanup::new(&sessions.solver));
    cleanup.push(SessionCleanup::new(&sessions.director));
    cleanup.extend(sessions.verifiers.iter().map(SessionCleanup::new));
    cleanup
}
