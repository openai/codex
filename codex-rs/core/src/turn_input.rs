use codex_protocol::models::ResponseInputItem;
use codex_protocol::user_input::UserInput;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum TurnInput {
    User(Vec<UserInput>),
    ResponseItems(Vec<ResponseInputItem>),
}

impl TurnInput {
    pub(crate) fn is_empty(&self) -> bool {
        match self {
            Self::User(items) => items.is_empty(),
            Self::ResponseItems(items) => items.is_empty(),
        }
    }

    pub(crate) fn into_user(self) -> Option<Vec<UserInput>> {
        match self {
            Self::User(items) => Some(items),
            Self::ResponseItems(_) => None,
        }
    }
}

impl From<Vec<UserInput>> for TurnInput {
    fn from(items: Vec<UserInput>) -> Self {
        Self::User(items)
    }
}

impl From<Vec<ResponseInputItem>> for TurnInput {
    fn from(items: Vec<ResponseInputItem>) -> Self {
        Self::ResponseItems(items)
    }
}
