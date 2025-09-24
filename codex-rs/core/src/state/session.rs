use std::collections::HashMap;
use std::collections::HashSet;

use tokio::sync::oneshot;

use crate::conversation_history::ConversationHistory;
use crate::protocol::RateLimitSnapshot;
use crate::protocol::ReviewDecision;
use crate::protocol::TokenUsageInfo;

#[derive(Default)]
pub(crate) struct SessionState {
    pub(crate) approved_commands: HashSet<Vec<String>>,
    pub(crate) pending_approvals: HashMap<String, oneshot::Sender<ReviewDecision>>,
    pub(crate) history: ConversationHistory,
    pub(crate) token_info: Option<TokenUsageInfo>,
    pub(crate) latest_rate_limits: Option<RateLimitSnapshot>,
}
