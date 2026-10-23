//! Tool choices supplied by consumers. Each tool has one fixed contract.

/// Shared tools a consumer can expose through `message_board_tools`.
/// Tool selection does not grant storage permissions to the underlying connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageBoardTool {
    CreateChannel,
    /// Creates a channel or updates its description, without subscriptions or notifications.
    UpsertChannel,
    GetChannels,
    ListThreads,
    SearchPosts,
    ReadThread,
    ReadPost,
    Subscribe,
    Unsubscribe,
    /// Posts, subscribes the author unless opted out, and notifies recipients.
    Post,
    /// Writes evidence without automatic subscriptions or notifications.
    Append,
}

impl MessageBoardTool {
    /// Existing agent discussion tools, in their original registration order.
    pub const DISCUSSION: [Self; 9] = [
        Self::CreateChannel,
        Self::GetChannels,
        Self::ListThreads,
        Self::SearchPosts,
        Self::ReadThread,
        Self::ReadPost,
        Self::Subscribe,
        Self::Unsubscribe,
        Self::Post,
    ];

    pub(super) fn name(self) -> &'static str {
        match self {
            Self::CreateChannel => "create_channel",
            Self::UpsertChannel => "upsert_channel",
            Self::GetChannels => "get_channels",
            Self::ListThreads => "list_threads",
            Self::SearchPosts => "search_posts",
            Self::ReadThread => "read_thread",
            Self::ReadPost => "read_post",
            Self::Subscribe => "subscribe",
            Self::Unsubscribe => "unsubscribe",
            Self::Post => "post",
            Self::Append => "append",
        }
    }
}
