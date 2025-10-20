use crate::user_input::UserInput;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Clone, Deserialize, Serialize, TS, JsonSchema)]
pub enum TurnItem {
    UserMessage(UserMessageItem),
}

#[derive(Debug, Clone, Deserialize, Serialize, TS, JsonSchema)]
pub struct UserMessageItem {
    pub id: String,
    pub content: Vec<UserInput>,
}

impl UserMessageItem {
    pub fn new(content: &[UserInput]) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            content: content.to_vec(),
        }
    }
}

impl TurnItem {
    pub fn id(&self) -> String {
        match self {
            TurnItem::UserMessage(item) => item.id.clone(),
        }
    }
}
