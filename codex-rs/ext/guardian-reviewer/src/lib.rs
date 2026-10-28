//! Owns Guardian conversation bookkeeping and synchronous review policy independently
//! of the host session runtime.
//! The host supplies review attempts and enforces the resulting decision on the bound action.

mod approval_request;
mod approval_request_json;
mod assessment;
mod blocking;
pub use blocking::ReviewBlocking;
pub use blocking::ReviewBlockingTurn;
mod circuit_breaker;
mod completion;
mod context_mode;
mod context_policy;
mod conversation;
mod instructions;
pub use context_mode::GuardianContextMode;
pub use context_policy::ReviewContextPolicy;
pub use conversation::ConversationCheckpoint;
pub use conversation::ConversationState;
pub use instructions::GUARDIAN_CONVERSATION_HISTORY_PROMPT;
pub use instructions::GUARDIAN_FOLLOWUP_REVIEW_REMINDER;
pub use instructions::reviewer_instructions;
mod deadline;
mod elicitation;
mod execution;
pub use elicitation::GuardianElicitationReview;
pub use elicitation::guardian_elicitation_review_request;
pub use elicitation::mcp_elicitation_decline_without_message;
pub use elicitation::mcp_elicitation_response_from_guardian_decision;
mod feedback;
mod metrics;
mod model;
mod node_repl_review_evidence;
mod outcome;
mod planned_action;
mod pool;
mod prompt;
mod reporting;
mod retry;
mod review;
mod review_action;
mod review_evidence;
mod review_state;
pub use review_state::CheckpointRecovery;
pub use review_state::GuardianReviewForkSnapshot;
pub use review_state::GuardianReviewHistory;
pub use review_state::GuardianReviewState;
pub use review_state::GuardianTranscriptSource;
pub use review_state::PendingNodeReplEvidenceAdmission;
pub use review_state::ReviewerSelection;
pub use review_state::admit_node_repl_evidence;
mod routing;
mod sender_evidence;
mod session;
mod settings;

pub use approval_request::GuardianApprovalRequest;
pub use approval_request::GuardianMcpAnnotations;
pub use approval_request::GuardianNetworkAccessTrigger;
pub use approval_request::guardian_assessment_action;
pub use approval_request::guardian_request_target_item_id;
pub use approval_request::guardian_request_turn_id;
pub use approval_request::guardian_reviewed_action;
pub use approval_request_json::format_guardian_action_pretty;
pub use approval_request_json::guardian_approval_request_to_json;
pub use assessment::GuardianAssessment;
pub use assessment::guardian_output_contract_prompt;
pub use assessment::guardian_output_schema;
pub use assessment::parse_guardian_assessment;
pub(crate) use circuit_breaker::AUTO_REVIEW_DENIAL_WINDOW_SIZE;
pub(crate) use circuit_breaker::GuardianRejectionCircuitBreaker;
pub(crate) use circuit_breaker::GuardianRejectionCircuitBreakerAction;
pub(crate) use circuit_breaker::GuardianRejectionCircuitBreakerPolicy;
pub use metrics::token_usage_delta;
pub use model::ReviewModel;
pub use model::select_review_model;
pub use outcome::GuardianReviewError;
pub use outcome::GuardianReviewOutcome;
pub use outcome::GuardianReviewSessionOutcome;
pub use planned_action::build_guardian_planned_action;
pub use retry::GuardianReviewSessionLimits;
pub use retry::run_with_retry;

pub const MAX_REVIEW_ATTEMPTS: i64 = 3;
/// Default overall review budget when `auto_review.timeout_ms` is omitted.
pub const REVIEW_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(90);

pub use deadline::ReviewDeadline;
pub use deadline::run_before_review_deadline;
pub use pool::ReviewSessionResult;
pub use pool::ReviewerPool;
pub use pool::ReviewerRequest;
pub use pool::ReviewerSession;
pub use pool::ReviewerTasks;
pub use pool::SessionDisposition;
pub use prompt::GUARDIAN_TRANSCRIPT_START;
pub use prompt::GuardianPromptItems;
pub use prompt::compose_guardian_prompt;
pub use prompt::compose_guardian_prompt_with_preparation;

pub use review::ReviewHost;
pub use review_action::ReviewAction;

pub use completion::ReviewCompletion;
pub use completion::complete_review;

pub use execution::ReviewTurnResult;
pub use execution::ReviewerRuntime;
pub use execution::start_review_turn;
pub use execution::wait_for_guardian_review;
pub use session::GUARDIAN_REVIEWER_NAME;
pub use session::is_basic_session_source;
pub use session::prompt_cache_key_override_for_review_session;
pub use settings::ReviewerConfig;
pub use settings::ReviewerTurn;
pub use settings::guardian_cyber_access_program;
pub use settings::reviewer_permission_profile;
pub use settings::reviewer_tool_policy;

pub use feedback::FailedReviewFeedback;
pub use feedback::ReviewFeedbackContext;
pub use feedback::ReviewFeedbackSettings;
pub use reporting::ReviewDenials;
pub use reporting::ReviewMetadata;
pub use reporting::ReviewReport;

pub use routing::ReviewRequest;
pub use routing::routes_approval_policy_to_guardian;

pub use node_repl_review_evidence::GUARDIAN_MAX_NODE_REPL_TOOL_RESULT_TOKENS;
pub use node_repl_review_evidence::NodeReplReviewEvidence;
pub use node_repl_review_evidence::NodeReplReviewEvidenceSnapshot;
pub use prompt::guardian_truncate_text;
pub use review_evidence::GuardianAuthorizationVersion;
pub use review_evidence::GuardianReviewEvidence;
pub use review_evidence::GuardianReviewEvidenceRecord;
pub use review_evidence::GuardianRootSnapshot;
pub use review_evidence::GuardianUserInputSnapshot;
pub use sender_evidence::GuardianSenderExchange;
pub use sender_evidence::guardian_sender_exchanges;
