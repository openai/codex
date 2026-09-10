//! Shared context sections for synchronous Guardian review and asynchronous scoring.
//!
//! Transcript collection and bounded host-owned history are also available directly,
//! without section composition.
//! Contributor failures abort collection without returning partial context.
//! Sections preserve source-specific evidence and share prompt framing, while
//! profiles retain the consumer-specific transcript policy. Hosts own full/delta
//! cursors, compaction and request lifecycles.
//! Registered contributors declare their scope once and are collected only for
//! matching context consumers. History and collection settings are borrowed for
//! each request so the default registry can be reused without retaining state.

use std::sync::Arc;
use std::sync::LazyLock;

use codex_protocol::models::ResponseItem;

use authorization::RootConversationSection;
use authorization::TrustedUserAnswersSection;
use retained_instructions::RetainedUserInstructionsSection;
use transcript::ConversationTranscriptSection;

pub use action::ActionPresentation;
pub use action::PlannedAction;
pub use action::PlannedActionKind;
pub use authorization::GuardianRootMessage;
pub use section::ContextSection;

pub use entry::ConversationTranscriptEntry;
pub use entry::ConversationTranscriptEntryKind;
pub use history::TranscriptHistory;
pub use transcript::ConversationTranscriptConfig;
pub use transcript::ConversationTranscriptOptions;
pub use transcript::MANUAL_APPROVAL_DEVELOPER_PREFIX;
pub use transcript::TranscriptEntryLimits;
pub use transcript::TranscriptRetentionConfig;
pub use transcript::collect_transcript;
pub use truncation::truncate_text;

mod verified_answers;
pub use verified_answers::RenderedVerifiedAnswers;
pub use verified_answers::render_verified_answer;
pub use verified_answers::render_verified_answers;

mod retained_instructions;

mod action;
mod enforcement;
pub(crate) use enforcement::BudgetPriority;
pub use enforcement::Budgeted;
pub use enforcement::HistoryTruncation;
pub(crate) use enforcement::Retention;
mod budget;
mod composition;
pub use budget::DEFAULT_MAX_INPUT_TOKENS;
pub use budget::REQUEST_TOKENS_BOUNDARIES;
pub use budget::REQUEST_TOKENS_METRIC;
pub use budget::RequestBudget;
pub use budget::SECTION_COST_BOUNDARIES;
pub use budget::SECTION_COST_METRIC;
pub use budget::SectionCost;
pub use budget::effective_input_token_limit;
pub use budget::estimate_input_tokens;
mod profile;
pub use composition::CollectedContext;
pub use composition::ComposedContext;
pub use composition::ContextPresentation;
pub use composition::RenderedTranscript;
pub use profile::ContextProfile;
mod authorization;
mod entry;
mod history;
mod images;
mod node_repl;
pub use node_repl::NodeReplContext;
pub use node_repl::NodeReplResponse;
pub use node_repl::NodeReplReviewEvidenceMode;
pub use node_repl::RenderedNodeReplEvidence;
mod permissions;
pub use images::TranscriptImageInput;
pub use images::TranscriptImages;
mod trusted_skills;
mod trusted_tool;
pub use trusted_skills::TrustedSkills;
pub use trusted_tool::TrustedTool;
mod reviews;
pub use reviews::MAX_PREVIOUS_REVIEWS;
pub use reviews::PreviousReviews;
pub use reviews::RenderedReviewEvidence;
pub use reviews::ReviewEvidence;
pub use reviews::render_review_evidence;
pub use truncation::TruncationObservation;
mod section;
pub use permissions::PermissionContext;
mod transcript;
mod truncation;

/// Consumer for which a Guardian context is composed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContextTarget {
    /// The reusable synchronous Guardian reviewer.
    Sync,
    /// The asynchronous Guardian action scorer.
    Async,
}

/// Consumers to which a context section contributes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SectionScope {
    /// Include the section in both synchronous review and asynchronous scoring.
    Shared,
    /// Include the section only in synchronous review.
    SyncOnly,
    /// Include the section only in asynchronous scoring.
    AsyncOnly,
}

impl SectionScope {
    /// Whether this section is included for the requested context consumer.
    pub fn includes(self, target: ContextTarget) -> bool {
        match self {
            Self::Shared => true,
            Self::SyncOnly => matches!(target, ContextTarget::Sync),
            Self::AsyncOnly => matches!(target, ContextTarget::Async),
        }
    }
}

/// Borrowed host inputs available while one Guardian context section is built.
#[derive(Clone, Copy)]
pub struct SectionInput<'a> {
    /// Consumer for which the host is collecting context sections.
    pub target: ContextTarget,
    /// Parent conversation history available to this contribution.
    pub history: &'a dyn SectionHistory,
    /// Evidence sources and per-entry limits for this collection.
    pub transcript: &'a ConversationTranscriptConfig,
    /// Bounded root evidence resolved by the host; empty when not applicable.
    pub root_conversation: &'a [GuardianRootMessage],
    /// Bounded, role-labeled answers selected from the host-owned context snapshot.
    pub trusted_user_answers: &'a [String],
    /// Exact action JSON and reason, already bounded by the requesting host.
    pub planned_action: Option<&'a PlannedAction>,
    /// Sync-only restrictions resolved from the parent execution environment.
    pub permissions: Option<&'a PermissionContext>,
    /// Size-validated, host-attested reviews selected against the action's authorization snapshot.
    pub previous_reviews: Option<&'a PreviousReviews>,
    /// Metadata verified by the host for the exact action being classified.
    pub trusted_tool: Option<&'a TrustedTool>,
    /// Current-turn and delegated skill paths verified and bounded by the host.
    pub trusted_skill_paths: &'a [String],
    /// Optional consumer image policy; no history images are added implicitly.
    pub images: Option<TranscriptImageInput<'a>>,
    /// Sync-only frozen REPL snapshot selected by the host's delivery cursor.
    pub node_repl: Option<&'a NodeReplContext<'a>>,
}

/// Supplies repeatable, zero-copy access to a host-owned conversation snapshot.
///
/// Implementations return a fresh iterator for every call so independently
/// registered contributors can inspect the same history without cloning its
/// response items or taking ownership away from the host.
pub trait SectionHistory: Send + Sync {
    /// Returns borrowed response items in their original conversation order.
    fn items(&self) -> Box<dyn Iterator<Item = &ResponseItem> + Send + '_>;

    /// Bounded host-owned facts from the same snapshot as the current items.
    fn retained_context(&self) -> Option<&codex_history::RetainedContext> {
        None
    }
}

impl SectionHistory for Vec<ResponseItem> {
    fn items(&self) -> Box<dyn Iterator<Item = &ResponseItem> + Send + '_> {
        Box::new(self.iter())
    }
}

impl<const LENGTH: usize> SectionHistory for [ResponseItem; LENGTH] {
    fn items(&self) -> Box<dyn Iterator<Item = &ResponseItem> + Send + '_> {
        Box::new(self.iter())
    }
}

/// Supplies one independently scoped section to Guardian context assembly.
///
/// Implementations declare whether they apply to synchronous review,
/// asynchronous scoring, or both. The registry filters contributors by scope
/// before invoking them. Contributors distinguish sections that do not apply
/// from required evidence that could not be collected.
/// Keep request-specific settings and history in [`SectionInput`] so the same
/// contributor can serve concurrent reviews without retaining stale state.
pub trait SectionContributor: Send + Sync {
    /// Guardian consumers that should receive this contribution.
    fn scope(&self) -> SectionScope;

    /// Builds this section using the host's current conversation snapshot.
    ///
    /// Return `Ok(None)` only when this section is optional or does not apply.
    /// Missing required evidence must return `Err`; callers must not review a
    /// partial context as though collection succeeded.
    fn contribute(&self, input: &SectionInput<'_>) -> Result<Option<ContextSection>, SectionError>;
}

/// A section could not provide the evidence needed for a valid review context.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SectionError {
    /// Evidence required by this contributor for the current input is missing.
    MissingRequiredEvidence { section: &'static str },
    /// A section cannot be delivered by the requested consumer.
    UnsupportedDelivery { section: &'static str },
    /// Supplied evidence exceeds the section's count or rendered-size limit.
    EvidenceLimitExceeded { section: &'static str },
}

impl std::fmt::Display for SectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingRequiredEvidence { section } => {
                write!(formatter, "missing required evidence for section {section}")
            }
            Self::UnsupportedDelivery { section } => {
                write!(formatter, "unsupported delivery for section {section}")
            }
            Self::EvidenceLimitExceeded { section } => {
                write!(formatter, "evidence exceeds limits for section {section}")
            }
        }
    }
}

impl std::error::Error for SectionError {}

/// Ordered collection of independently scoped Guardian section contributors.
#[derive(Clone, Default)]
pub struct SectionRegistry {
    contributors: Vec<Arc<dyn SectionContributor>>,
}

/// Shared, process-lifetime registry of built-in Guardian sections.
///
/// Contributors store no conversation or configuration state. Each collection
/// borrows the current history and settings from [`SectionInput`], so callers
/// can reuse this registry across threads, model changes, and review targets.
pub fn default_registry() -> &'static SectionRegistry {
    static REGISTRY: LazyLock<SectionRegistry> = LazyLock::new(|| {
        let mut registry = SectionRegistry::default();
        registry.register(reviews::PreviousReviewsSection);
        registry.register(trusted_tool::TrustedToolSection);
        registry.register(trusted_skills::TrustedSkillsSection);
        registry.register(RootConversationSection);
        registry.register(RetainedUserInstructionsSection);
        registry.register(TrustedUserAnswersSection);
        registry.register(ConversationTranscriptSection);
        registry.register(images::TranscriptImagesSection);
        registry.register(node_repl::NodeReplEvidenceSection);
        registry.register(permissions::PermissionContextSection);
        registry.register(action::PlannedActionSection);
        registry
    });
    &REGISTRY
}

impl SectionRegistry {
    /// Adds a contributor to the end of the section collection order.
    pub fn register(&mut self, contributor: impl SectionContributor + 'static) {
        self.contributors.push(Arc::new(contributor));
    }

    /// Collects evidence for host transcript selection and shared composition.
    pub fn prepare(&self, input: &SectionInput<'_>) -> Result<CollectedContext, SectionError> {
        Ok(CollectedContext {
            sections: self.collect(input)?,
        })
    }

    /// Collects applicable sections in their original registration order.
    ///
    /// Stops at the first error without returning any partial context. The host
    /// decides whether to fall back to synchronous review or deny approval.
    pub fn collect(&self, input: &SectionInput<'_>) -> Result<Vec<ContextSection>, SectionError> {
        self.contributors
            .iter()
            .filter(|contributor| contributor.scope().includes(input.target))
            .filter_map(|contributor| contributor.contribute(input).transpose())
            .collect()
    }
}

#[cfg(test)]
#[path = "registry_tests.rs"]
mod registry_tests;
