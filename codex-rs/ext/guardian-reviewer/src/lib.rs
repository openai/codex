//! Owns synchronous Guardian review policy independently of the host session runtime.
//! The host supplies review attempts and enforces the resulting decision on the bound action.

mod assessment;
mod circuit_breaker;
mod completion;
mod deadline;
mod execution;
mod feedback;
mod metrics;
mod model;
mod outcome;
mod pool;
mod reporting;
mod retry;
mod review;
mod settings;

pub use assessment::GuardianAssessment;
pub use assessment::guardian_output_contract_prompt;
pub use assessment::guardian_output_schema;
pub use assessment::parse_guardian_assessment;
pub use circuit_breaker::AUTO_REVIEW_DENIAL_WINDOW_SIZE;
pub use circuit_breaker::GuardianRejectionCircuitBreaker;
pub use circuit_breaker::GuardianRejectionCircuitBreakerAction;
pub use circuit_breaker::GuardianRejectionCircuitBreakerPolicy;
pub use model::ReviewModel;
pub use model::select_review_model;
pub use outcome::GuardianReviewError;
pub use outcome::GuardianReviewOutcome;
pub use outcome::GuardianReviewSessionOutcome;
pub use retry::GuardianReviewSessionLimits;
pub use retry::run_with_retry;

pub const MAX_REVIEW_ATTEMPTS: i64 = 3;
pub const REVIEW_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(90);

pub use deadline::run_before_review_deadline;
pub use deadline::run_before_review_deadline_with_cancel;
pub use pool::ReviewerPool;
pub use pool::ReviewerRequest;
pub use pool::ReviewerSession;
pub use pool::ReviewerSessionFactory;
pub use pool::SessionDisposition;

pub use review::ReviewHost;
pub use review::SynchronousReview;

pub use completion::ReviewCompletion;
pub use completion::complete_review;
pub use completion::guardian_timeout_message;

pub use execution::ReviewerRuntime;
pub use execution::start_review_turn;
pub use execution::wait_for_guardian_review;
pub use settings::ReviewerConfigOverrides;
pub use settings::ReviewerTurn;
pub use settings::reviewer_config_overrides;

pub use feedback::FailedReviewFeedback;
pub use feedback::ReviewFeedbackContext;
pub use feedback::ReviewFeedbackSettings;
pub use reporting::ReviewDenials;
pub use reporting::ReviewMetadata;
pub use reporting::ReviewReport;
