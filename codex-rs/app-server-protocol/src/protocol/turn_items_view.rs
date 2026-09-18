//! Projects a turn's items without copying payloads excluded by the requested view.

use super::v2::ThreadItem;
use super::v2::TurnItemsView;

impl TurnItemsView {
    /// Copies only the items included in this view, preserving summary selection order.
    pub fn project_items(self, items: &[ThreadItem]) -> Vec<ThreadItem> {
        match self {
            Self::NotLoaded => Vec::new(),
            Self::Full => items.to_vec(),
            Self::Summary => {
                let first_user_message = items
                    .iter()
                    .find(|item| matches!(item, ThreadItem::UserMessage { .. }));
                let final_agent_message = items
                    .iter()
                    .rev()
                    .find(|item| matches!(item, ThreadItem::AgentMessage { .. }));
                match (first_user_message, final_agent_message) {
                    (Some(user_message), Some(agent_message))
                        if user_message.id() != agent_message.id() =>
                    {
                        vec![user_message.clone(), agent_message.clone()]
                    }
                    (Some(user_message), _) => vec![user_message.clone()],
                    (None, Some(agent_message)) => vec![agent_message.clone()],
                    (None, None) => Vec::new(),
                }
            }
        }
    }
}
