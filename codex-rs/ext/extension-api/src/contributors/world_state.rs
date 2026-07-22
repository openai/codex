use std::sync::Arc;

use codex_exec_server_protocol::ExecutorCapabilityDiscoverySnapshot;
use codex_protocol::ThreadId;
use codex_protocol::capabilities::SelectedCapabilityRoot;
use codex_protocol::protocol::TurnEnvironmentSelection;
use serde_json::Value;

use crate::ExtensionData;

/// Host state available while an extension contributes one sampling step's World State.
pub struct WorldStateContributionInput<'a> {
    pub thread_id: ThreadId,
    pub turn_id: &'a str,
    pub environments: &'a [TurnEnvironmentSelection],
    /// Selected roots whose stable environments are ready in this sampling step.
    pub ready_selected_capability_roots: &'a [SelectedCapabilityRoot],
    /// Executor-materialized capability files shared by all consumers in this exact step.
    pub executor_capability_discovery: Option<&'a ExecutorCapabilityDiscoverySnapshot>,
    pub session_store: &'a ExtensionData,
    pub thread_store: &'a ExtensionData,
    pub turn_store: &'a ExtensionData,
}

/// What the harness knows about the previous value of one extension-owned section.
pub enum PreviousWorldStateSection<'a> {
    Absent,
    Unknown,
    Known(&'a Value),
}

/// Plain model-visible data rendered by an extension-owned World State section.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderedWorldStateFragment {
    role: &'static str,
    markers: (&'static str, &'static str),
    body: String,
}

impl RenderedWorldStateFragment {
    pub fn new(
        role: &'static str,
        markers: (&'static str, &'static str),
        body: impl Into<String>,
    ) -> Self {
        Self {
            role,
            markers,
            body: body.into(),
        }
    }

    pub fn role(&self) -> &'static str {
        self.role
    }

    pub fn markers(&self) -> (&'static str, &'static str) {
        self.markers
    }

    pub fn body(&self) -> &str {
        &self.body
    }
}

type RenderDiff = dyn for<'a> Fn(PreviousWorldStateSection<'a>) -> Option<RenderedWorldStateFragment>
    + Send
    + Sync;
type LegacyFragmentMatcher = dyn Fn(&str, &str) -> bool + Send + Sync;

/// One extension-owned World State section captured for a sampling step.
///
/// The extension owns the stable ID, comparison snapshot, and diff rendering. The harness owns
/// persistence and the concrete model-context fragment envelope.
#[derive(Clone)]
pub struct WorldStateSectionContribution {
    id: &'static str,
    snapshot: Value,
    render_diff: Arc<RenderDiff>,
    matches_legacy_fragment: Arc<LegacyFragmentMatcher>,
    matches_retained_fragment: Option<Arc<LegacyFragmentMatcher>>,
}

impl WorldStateSectionContribution {
    pub fn new(
        id: &'static str,
        snapshot: Value,
        render_diff: impl for<'a> Fn(
            PreviousWorldStateSection<'a>,
        ) -> Option<RenderedWorldStateFragment>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        Self {
            id,
            snapshot,
            render_diff: Arc::new(render_diff),
            matches_legacy_fragment: Arc::new(|_, _| false),
            matches_retained_fragment: None,
        }
    }

    pub fn with_legacy_matcher(
        mut self,
        matcher: impl Fn(&str, &str) -> bool + Send + Sync + 'static,
    ) -> Self {
        self.matches_legacy_fragment = Arc::new(matcher);
        self
    }

    /// Requires a matching model-visible fragment whenever a persisted snapshot is reused.
    pub fn with_retained_fragment_matcher(
        mut self,
        matcher: impl Fn(&str, &str) -> bool + Send + Sync + 'static,
    ) -> Self {
        self.matches_retained_fragment = Some(Arc::new(matcher));
        self
    }

    pub fn id(&self) -> &'static str {
        self.id
    }

    pub fn snapshot(&self) -> &Value {
        &self.snapshot
    }

    pub fn render_diff(
        &self,
        previous: PreviousWorldStateSection<'_>,
    ) -> Option<RenderedWorldStateFragment> {
        (self.render_diff)(previous)
    }

    pub fn matches_legacy_fragment(&self, role: &str, text: &str) -> bool {
        (self.matches_legacy_fragment)(role, text)
    }

    pub fn has_retained_fragment_matcher(&self) -> bool {
        self.matches_retained_fragment.is_some()
    }

    pub fn matches_retained_fragment(&self, role: &str, text: &str) -> bool {
        self.matches_retained_fragment
            .as_ref()
            .is_some_and(|matcher| matcher(role, text))
    }
}
