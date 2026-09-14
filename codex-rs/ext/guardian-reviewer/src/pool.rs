//! Owns the reusable reviewer and temporary forks for one parent thread.
//! Guardian supplies agent startup; the host supplies captured context. Selection stays serialized;
//! concurrent reviews fork committed context and shutdown joins every tracked session.
//! Startup and fork futures stay boxed to bound the orchestration stack frames.

use std::future::Future;
use std::sync::Arc;

use codex_analytics::GuardianReviewAnalyticsResult;
use codex_analytics::GuardianReviewSessionKind;
use codex_extension_api::ExtensionFuture;
use tokio::sync::Mutex;
use tokio::sync::Semaphore;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

use crate::GuardianReviewSessionOutcome;
use crate::run_before_review_deadline;
use crate::run_before_review_deadline_with_cancel;

/// A host-owned reviewer session. Context and snapshots remain opaque to the pool.
/// Shutdown must cancel the runtime and await its termination.
pub trait ReviewerSession: Send + Sync + 'static {
    type Setup: Send + Sync + 'static;
    type Context: Clone + PartialEq + Send + Sync;
    type Snapshot: Send + Sync;

    fn context(&self) -> &Self::Context;
    fn cancel(&self);
    fn snapshot(&self) -> impl Future<Output = Option<Self::Snapshot>> + Send;
    fn commit_snapshot(&self) -> impl Future<Output = ()> + Send;
    fn shutdown(&self) -> impl Future<Output = ()> + Send;
}

/// Executes one approval on a selected session. The host must drain the submitted
/// turn before returning Reusable, and must keep the issuing action and permissions bound.
pub trait ReviewerRequest: Send + Sync {
    type Session: ReviewerSession;

    fn setup(&self) -> Arc<<Self::Session as ReviewerSession>::Setup>;
    fn context(
        &self,
        previous: Option<&Self::Session>,
    ) -> <Self::Session as ReviewerSession>::Context;
    fn deadline(&self) -> Instant;
    fn cancellation(&self) -> Option<&CancellationToken>;
    fn run(
        &self,
        session: &Self::Session,
        kind: GuardianReviewSessionKind,
    ) -> impl Future<
        Output = (
            GuardianReviewSessionOutcome,
            SessionDisposition,
            GuardianReviewAnalyticsResult,
        ),
    > + Send;
}

/// Whether the host drained the session sufficiently for another review to use it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionDisposition {
    Reusable,
    Discard,
}

/// Per-parent reviewer state. The same pool serves prewarm, review, invalidation and shutdown.
pub struct ReviewerPool<S: ReviewerSession> {
    state: Arc<Mutex<PoolState<S>>>,
    cancellation: CancellationToken,
    spawn: Box<SpawnReviewer<S>>,
}

type SpawnReviewer<S> = dyn Fn(
        Arc<<S as ReviewerSession>::Setup>,
        <S as ReviewerSession>::Context,
        GuardianReviewSessionKind,
        Option<<S as ReviewerSession>::Snapshot>,
        CancellationToken,
    ) -> ExtensionFuture<'static, anyhow::Result<S>>
    + Send
    + Sync;

struct PoolState<S: ReviewerSession> {
    trunk: Option<Arc<Trunk<S>>>,
    ephemeral_reviews: Vec<Arc<S>>,
}

struct Trunk<S: ReviewerSession> {
    session: Arc<S>,
    review_lock: Semaphore,
}

impl<S: ReviewerSession> ReviewerPool<S> {
    /// Installs Guardian's startup function once. It must finish or clean up partial startup
    /// even when the caller drops its future, and preserve the supplied cancellation token.
    pub fn new(
        spawn: impl Fn(
            Arc<S::Setup>,
            S::Context,
            GuardianReviewSessionKind,
            Option<S::Snapshot>,
            CancellationToken,
        ) -> ExtensionFuture<'static, anyhow::Result<S>>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        Self {
            state: Arc::new(Mutex::new(PoolState {
                trunk: None,
                ephemeral_reviews: Vec::new(),
            })),
            cancellation: CancellationToken::new(),
            spawn: Box::new(spawn),
        }
    }
}

impl<S: ReviewerSession> ReviewerPool<S> {
    /// Returns the current reviewer handle for host inspection and feedback collection.
    pub async fn trunk(&self) -> Option<Arc<S>> {
        self.state
            .lock()
            .await
            .trunk
            .as_ref()
            .map(|trunk| Arc::clone(&trunk.session))
    }

    /// Prepares the first reviewer without replacing a review that won the startup race.
    pub async fn prewarm(&self, setup: Arc<S::Setup>, context: S::Context) -> anyhow::Result<()> {
        let cancellation = self.cancellation.child_token();
        let guard = cancellation.clone().drop_guard();
        let session = (self.spawn)(
            setup,
            context,
            GuardianReviewSessionKind::TrunkNew,
            /*snapshot*/ None,
            cancellation.clone(),
        )
        .await?;
        let mut state = self.state.lock().await;
        if !cancellation.is_cancelled() && state.trunk.is_none() {
            state.trunk = Some(Arc::new(Trunk {
                session: Arc::new(session),
                review_lock: Semaphore::new(/*permits*/ 1),
            }));
            drop(guard.disarm());
        }
        Ok(())
    }

    /// Permanently stops this parent's reviewer pool and waits for tracked runtimes.
    pub async fn shutdown(&self) {
        self.cancellation.cancel();
        self.invalidate().await;
    }

    /// Drops reusable context after parent history rollback or another host invalidation.
    pub async fn invalidate(&self) {
        let (trunk, ephemeral) = {
            let mut state = self.state.lock().await;
            (
                state.trunk.take(),
                std::mem::take(&mut state.ephemeral_reviews),
            )
        };
        for session in trunk
            .into_iter()
            .map(|trunk| Arc::clone(&trunk.session))
            .chain(ephemeral)
        {
            if self.cancellation.is_cancelled() {
                session.shutdown().await;
            } else {
                session.cancel();
                shutdown_in_background(session);
            }
        }
    }

    /// Selects one reviewer; busy or incompatible trunks use an isolated temporary session.
    #[expect(
        clippy::await_holding_invalid_type,
        reason = "reviewer selection and spawning stay serialized"
    )]
    pub async fn review<R>(
        &self,
        request: R,
    ) -> (GuardianReviewSessionOutcome, GuardianReviewAnalyticsResult)
    where
        R: ReviewerRequest<Session = S>,
    {
        let mut spawned_trunk = false;
        let (trunk, context) = match run_before_review_deadline(
            request.deadline(),
            request.cancellation(),
            self.state.lock(),
        )
        .await
        {
            Ok(mut state) => {
                let context =
                    request.context(state.trunk.as_ref().map(|trunk| trunk.session.as_ref()));
                if let Some(trunk) = state.trunk.as_ref()
                    && trunk.session.context() != &context
                    && trunk.review_lock.try_acquire().is_ok()
                    && let Some(stale) = state.trunk.take()
                {
                    shutdown_in_background(Arc::clone(&stale.session));
                }
                if state.trunk.is_none() {
                    let cancellation = self.cancellation.child_token();
                    let session = match run_before_review_deadline_with_cancel(
                        request.deadline(),
                        request.cancellation(),
                        &cancellation,
                        (self.spawn)(
                            request.setup(),
                            context.clone(),
                            GuardianReviewSessionKind::TrunkNew,
                            /*snapshot*/ None,
                            cancellation.clone(),
                        ),
                    )
                    .await
                    {
                        Ok(Ok(session)) => Arc::new(session),
                        Ok(Err(error)) => {
                            return (
                                GuardianReviewSessionOutcome::PromptBuildFailed(error),
                                GuardianReviewAnalyticsResult::without_session(),
                            );
                        }
                        Err(outcome) => {
                            return (outcome, GuardianReviewAnalyticsResult::without_session());
                        }
                    };
                    state.trunk = Some(Arc::new(Trunk {
                        session,
                        review_lock: Semaphore::new(/*permits*/ 1),
                    }));
                    spawned_trunk = true;
                }
                (state.trunk.as_ref().cloned(), context)
            }
            Err(outcome) => return (outcome, GuardianReviewAnalyticsResult::without_session()),
        };
        let Some(trunk) = trunk else {
            return (
                GuardianReviewSessionOutcome::Completed(Err(anyhow::anyhow!(
                    "guardian review session was not available after spawn"
                ))),
                GuardianReviewAnalyticsResult::without_session(),
            );
        };
        if trunk.session.context() != &context {
            return Box::pin(self.review_ephemeral(&request, context, /*snapshot*/ None)).await;
        }
        let guard = match trunk.review_lock.try_acquire() {
            Ok(guard) => guard,
            Err(_) => {
                return Box::pin(self.review_ephemeral(
                    &request,
                    context,
                    trunk.session.snapshot().await,
                ))
                .await;
            }
        };
        let kind = if spawned_trunk {
            GuardianReviewSessionKind::TrunkNew
        } else {
            GuardianReviewSessionKind::TrunkReused
        };
        let (outcome, disposition, analytics) = request.run(&trunk.session, kind).await;
        if disposition == SessionDisposition::Reusable
            && matches!(outcome, GuardianReviewSessionOutcome::Completed(_))
        {
            trunk.session.commit_snapshot().await;
        }
        drop(guard);
        if disposition == SessionDisposition::Discard {
            let mut state = self.state.lock().await;
            if state
                .trunk
                .as_ref()
                .is_some_and(|current| Arc::ptr_eq(current, &trunk))
                && let Some(removed) = state.trunk.take()
            {
                shutdown_in_background(Arc::clone(&removed.session));
            }
        }
        (outcome, analytics)
    }

    async fn review_ephemeral<R>(
        &self,
        request: &R,
        context: S::Context,
        snapshot: Option<S::Snapshot>,
    ) -> (GuardianReviewSessionOutcome, GuardianReviewAnalyticsResult)
    where
        R: ReviewerRequest<Session = S>,
    {
        let cancellation = self.cancellation.child_token();
        let session = match run_before_review_deadline_with_cancel(
            request.deadline(),
            request.cancellation(),
            &cancellation,
            (self.spawn)(
                request.setup(),
                context,
                GuardianReviewSessionKind::EphemeralForked,
                snapshot,
                cancellation.clone(),
            ),
        )
        .await
        {
            Ok(Ok(session)) => Arc::new(session),
            Ok(Err(error)) => {
                return (
                    GuardianReviewSessionOutcome::PromptBuildFailed(error),
                    GuardianReviewAnalyticsResult::without_session(),
                );
            }
            Err(outcome) => return (outcome, GuardianReviewAnalyticsResult::without_session()),
        };
        self.state
            .lock()
            .await
            .ephemeral_reviews
            .push(Arc::clone(&session));
        let mut cleanup = EphemeralCleanup {
            state: Arc::clone(&self.state),
            session: Some(Arc::clone(&session)),
        };
        let (outcome, _, analytics) = request
            .run(&session, GuardianReviewSessionKind::EphemeralForked)
            .await;
        let removed = {
            let mut state = self.state.lock().await;
            state
                .ephemeral_reviews
                .iter()
                .position(|active| Arc::ptr_eq(active, &session))
                .map(|index| state.ephemeral_reviews.swap_remove(index))
        };
        if let Some(removed) = removed {
            cleanup.session = None;
            shutdown_in_background(removed);
        }
        (outcome, analytics)
    }
}

fn shutdown_in_background<S: ReviewerSession>(session: Arc<S>) {
    drop(tokio::spawn(async move {
        session.shutdown().await;
    }));
}

struct EphemeralCleanup<S: ReviewerSession> {
    state: Arc<Mutex<PoolState<S>>>,
    session: Option<Arc<S>>,
}

impl<S: ReviewerSession> Drop for EphemeralCleanup<S> {
    fn drop(&mut self) {
        let Some(session) = self.session.take() else {
            return;
        };
        let state = Arc::clone(&self.state);
        drop(tokio::spawn(async move {
            let removed = {
                let mut state = state.lock().await;
                state
                    .ephemeral_reviews
                    .iter()
                    .position(|active| Arc::ptr_eq(active, &session))
                    .map(|index| state.ephemeral_reviews.swap_remove(index))
            };
            if let Some(removed) = removed {
                removed.shutdown().await;
            }
        }));
    }
}
