//! Code-review flow state for `ChatWidget`.

use crate::auto_review_denials::RecentAutoReviewDenials;
use crate::token_usage::TokenUsageInfo;

#[derive(Debug, Default)]
pub(super) struct ReviewState {
    pub(super) recent_auto_review_denials: RecentAutoReviewDenials,
    /// Simple review mode flag; used to adjust layout and banners.
    pub(super) is_review_mode: bool,
    /// Snapshot of token usage to restore after review mode exits.
    pub(super) pre_review_token_info: Option<Option<TokenUsageInfo>>,
}
