//! Context adapter types used by the host-installed synchronous Guardian extension.
//! The extension supplies agent startup and owns the reviewer pool and its lifecycle.

pub use crate::guardian::GuardianReviewSession;
pub use crate::guardian::GuardianReviewState;
pub use crate::guardian::PreparedGuardianContext;
pub use crate::guardian::prepare_review_prewarm;
