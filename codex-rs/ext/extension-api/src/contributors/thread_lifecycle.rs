use std::sync::Arc;

use crate::ExtensionData;
use crate::HiddenContextMarker;
use crate::ResponseItemInjector;
use codex_protocol::protocol::ThreadSettingsSnapshot;

/// Input supplied when the host starts a runtime for a thread.
pub struct ThreadStartInput<'a, C> {
    /// Host configuration visible at thread start.
    pub config: &'a C,
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
    /// Current host-owned thread settings at resume time.
    pub thread_settings: &'a ThreadSettingsSnapshot,
    /// Store scoped to the host session runtime.
    pub session_store: &'a ExtensionData,
    /// Store scoped to this thread runtime.
    pub thread_store: &'a ExtensionData,
}

/// Input supplied when the host has no immediately pending thread work.
pub struct ThreadIdleInput<'a> {
    /// Current host-owned thread settings for the idle thread.
    pub thread_settings: &'a ThreadSettingsSnapshot,
    /// Store scoped to the host session runtime.
    pub session_store: &'a ExtensionData,
    /// Store scoped to this thread runtime.
    pub thread_store: &'a ExtensionData,
}

/// Extension request to start a new idle turn.
pub struct ThreadIdleRequest {
    /// Hidden prompt body that the host wraps as extension-owned context.
    pub prompt: String,
    /// Marker pair used to wrap the prompt as hidden extension-owned context.
    pub context_marker: HiddenContextMarker,
    /// Opaque extension-owned key used to reject stale requests before start.
    pub validation_key: Option<String>,
}

impl ThreadIdleRequest {
    pub fn new(context_marker: HiddenContextMarker, prompt: impl Into<String>) -> Self {
        Self {
            prompt: prompt.into(),
            context_marker,
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
    /// Host-owned thread settings for the default turn being started.
    pub thread_settings: &'a ThreadSettingsSnapshot,
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
