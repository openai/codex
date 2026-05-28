use std::sync::Arc;

use codex_protocol::protocol::SessionSource;

use crate::ExtensionData;
use crate::ResponseInjectionItem;
use crate::ResponseItemInjector;

/// Idle-turn scheduling policy declared by an extension contributor.
///
/// The default policy allows idle turns only outside plan mode.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IdleTurnPolicy {
    allow_plan_mode: bool,
}

impl IdleTurnPolicy {
    pub const fn allow_plan_mode() -> Self {
        Self {
            allow_plan_mode: true,
        }
    }

    pub fn allows_plan_mode(self) -> bool {
        self.allow_plan_mode
    }
}

/// Input supplied when the host starts a runtime for a thread.
pub struct ThreadStartInput<'a, C> {
    /// Host configuration visible at thread start.
    pub config: &'a C,
    /// Source for this thread's session.
    pub session_source: &'a SessionSource,
    /// Whether persistent state is available for this thread.
    pub persistent_thread_state_available: bool,
    /// Host-provided helper for injecting model-visible input into this
    /// thread's active turn.
    pub response_item_injector: Arc<dyn ResponseItemInjector>,
    /// Store scoped to the host session runtime.
    pub session_store: &'a ExtensionData,
    /// Store scoped to this thread runtime.
    pub thread_store: &'a ExtensionData,
}

/// Input supplied when the host resumes an existing thread.
pub struct ThreadResumeInput<'a> {
    /// Store scoped to the host session runtime.
    pub session_store: &'a ExtensionData,
    /// Store scoped to this thread runtime.
    pub thread_store: &'a ExtensionData,
}

/// Input supplied when the host has no immediately pending thread work.
pub struct ThreadIdleInput<'a> {
    /// Store scoped to the host session runtime.
    pub session_store: &'a ExtensionData,
    /// Store scoped to this thread runtime.
    pub thread_store: &'a ExtensionData,
}

/// Extension request to start a new idle turn.
pub struct ThreadIdleRequest {
    /// Extension-owned input that the host injects as model-visible input.
    pub item: ResponseInjectionItem,
    /// Opaque extension-owned key used to reject stale requests before start.
    pub validation_key: Option<String>,
}

impl ThreadIdleRequest {
    pub fn new(item: impl Into<ResponseInjectionItem>) -> Self {
        Self {
            item: item.into(),
            validation_key: None,
        }
    }

    pub fn with_validation_key(mut self, validation_key: impl Into<String>) -> Self {
        self.validation_key = Some(validation_key.into());
        self
    }
}

/// Input supplied before the host starts an extension-requested idle turn.
pub struct ThreadIdleTurnStartInput<'a> {
    /// Request returned by this contributor for the candidate idle turn.
    pub request: &'a ThreadIdleRequest,
    /// Store scoped to the host session runtime.
    pub session_store: &'a ExtensionData,
    /// Store scoped to this thread runtime.
    pub thread_store: &'a ExtensionData,
}

/// Input supplied when the host stops a thread runtime.
pub struct ThreadStopInput<'a> {
    /// Store scoped to the host session runtime.
    pub session_store: &'a ExtensionData,
    /// Store scoped to this thread runtime.
    pub thread_store: &'a ExtensionData,
}
