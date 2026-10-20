//! Carries EMA startup cancellation across configuration loading and thread publication.
//! Once published, a thread's existing config refresh gate owns selective revocation.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use tokio_util::sync::CancellationToken;

#[derive(Clone, Debug, Default)]
pub struct McpEmaCancellation(Arc<State>);

#[derive(Debug, Default)]
struct State {
    startup: CancellationToken,
    owner: CancellationToken,
    lifetime: CancellationToken,
    published: AtomicBool,
}

impl McpEmaCancellation {
    /// Capture before loading configuration so reload can fence a delayed startup.
    pub fn pending(startup: CancellationToken, owner: CancellationToken) -> Self {
        let lifetime = owner.child_token();
        Self(Arc::new(State {
            startup,
            owner,
            lifetime,
            published: AtomicBool::new(false),
        }))
    }

    /// A fork gets its own publication state, even when it inherits loaded config.
    pub fn for_startup(&self) -> Self {
        let pending = Self::pending(self.0.startup.clone(), self.0.owner.clone());
        if self.is_cancelled() {
            pending.cancel();
        }
        pending
    }

    /// A validated session keeps its authority and records the current startup fence
    /// for later forks. The session publication gate must protect this replacement.
    pub fn refreshed(&self, incoming: &Self) -> Self {
        Self(Arc::new(State {
            startup: incoming.0.startup.clone(),
            owner: self.0.owner.clone(),
            lifetime: self.0.lifetime.clone(),
            published: AtomicBool::new(true),
        }))
    }

    /// Call while holding the thread registry's publication lock. A reload either
    /// cancels this pending snapshot or discovers the published thread for validation.
    pub fn publish(&self) {
        if !self.0.published.swap(true, Ordering::AcqRel) && self.0.startup.is_cancelled() {
            self.cancel();
        }
    }

    pub fn cancel(&self) {
        self.0.lifetime.cancel();
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.lifetime.is_cancelled()
            || (!self.0.published.load(Ordering::Acquire) && self.0.startup.is_cancelled())
    }

    pub fn token(&self) -> CancellationToken {
        self.0.lifetime.clone()
    }

    pub async fn cancelled(&self) {
        tokio::select! {
            biased;
            () = self.0.lifetime.cancelled() => return,
            () = self.0.startup.cancelled() => {}
        }
        if self.0.published.load(Ordering::Acquire) {
            self.0.lifetime.cancelled().await;
        }
    }
}
