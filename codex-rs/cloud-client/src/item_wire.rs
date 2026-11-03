//! Native item lifecycle envelopes and the item fields used by typed events.
use prost::Message;

#[derive(Clone, Message)]
pub(super) struct Started {
    #[prost(message, optional, tag = "1")]
    pub item: Option<Item>,
    #[prost(int64, optional, tag = "2")]
    pub at_ms: Option<i64>,
    #[prost(string, optional, tag = "3")]
    pub thread_id: Option<String>,
    #[prost(string, optional, tag = "4")]
    pub turn_id: Option<String>,
}
#[derive(Clone, Message)]
pub(super) struct Completed {
    #[prost(int64, optional, tag = "1")]
    pub at_ms: Option<i64>,
    #[prost(message, optional, tag = "2")]
    pub item: Option<Item>,
    #[prost(string, optional, tag = "3")]
    pub thread_id: Option<String>,
    #[prost(string, optional, tag = "4")]
    pub turn_id: Option<String>,
}
#[derive(Clone, Message)]
pub(super) struct Item {
    #[prost(
        oneof = "Value",
        tags = "1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19"
    )]
    pub value: Option<Value>,
}
#[derive(Clone, prost::Oneof)]
pub(super) enum Value {
    #[prost(message, tag = "1")]
    UserMessage(UserMessage),
    #[prost(bytes, tag = "2")]
    HookPrompt(Vec<u8>),
    #[prost(message, tag = "3")]
    AgentMessage(AgentMessage),
    #[prost(bytes, tag = "4")]
    FunctionCallOutput(Vec<u8>),
    #[prost(bytes, tag = "5")]
    Plan(Vec<u8>),
    #[prost(message, tag = "6")]
    Reasoning(Reasoning),
    #[prost(message, tag = "7")]
    Command(Command),
    #[prost(message, tag = "8")]
    FileChange(FileChange),
    #[prost(bytes, tag = "9")]
    Mcp(Vec<u8>),
    #[prost(bytes, tag = "10")]
    DynamicTool(Vec<u8>),
    #[prost(bytes, tag = "11")]
    CollabAgent(Vec<u8>),
    #[prost(bytes, tag = "12")]
    SubAgent(Vec<u8>),
    #[prost(bytes, tag = "13")]
    WebSearch(Vec<u8>),
    #[prost(bytes, tag = "14")]
    ImageView(Vec<u8>),
    #[prost(bytes, tag = "15")]
    Sleep(Vec<u8>),
    #[prost(bytes, tag = "16")]
    ImageGeneration(Vec<u8>),
    #[prost(bytes, tag = "17")]
    EnterReview(Vec<u8>),
    #[prost(bytes, tag = "18")]
    ExitReview(Vec<u8>),
    #[prost(bytes, tag = "19")]
    Compaction(Vec<u8>),
}
#[derive(Clone, Message)]
pub(super) struct AgentMessage {
    #[prost(string, optional, tag = "2")]
    pub id: Option<String>,
    #[prost(message, optional, tag = "5")]
    pub questions: Option<AgentQuestionList>,
    #[prost(string, optional, tag = "6")]
    pub text: Option<String>,
}
#[derive(Clone, Message)]
pub(super) struct AgentQuestionList {
    #[prost(message, repeated, tag = "1")]
    pub values: Vec<AgentQuestion>,
}
#[derive(Clone, Message)]
pub(super) struct AgentQuestion {
    #[prost(message, optional, tag = "1")]
    pub options: Option<StringList>,
    #[prost(string, optional, tag = "2")]
    pub title: Option<String>,
}
#[derive(Clone, Message)]
pub(super) struct Command {
    #[prost(string, optional, tag = "1")]
    pub output: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub command: Option<String>,
    #[prost(message, optional, tag = "3")]
    pub actions: Option<CommandActionList>,
    #[prost(string, optional, tag = "4")]
    pub cwd: Option<String>,
    #[prost(int64, optional, tag = "5")]
    pub duration_ms: Option<i64>,
    #[prost(int64, optional, tag = "6")]
    pub exit_code: Option<i64>,
    #[prost(string, optional, tag = "7")]
    pub id: Option<String>,
    #[prost(int32, optional, tag = "11")]
    pub source: Option<i32>,
    #[prost(int32, optional, tag = "12")]
    pub status: Option<i32>,
}
#[derive(Clone, Message)]
pub(super) struct Reasoning {
    #[prost(message, optional, tag = "1")]
    pub content: Option<StringList>,
    #[prost(string, optional, tag = "2")]
    pub id: Option<String>,
    #[prost(message, optional, tag = "3")]
    pub summary: Option<StringList>,
}
#[derive(Clone, Message)]
pub(super) struct StringList {
    #[prost(string, repeated, tag = "1")]
    pub values: Vec<String>,
}
#[derive(Clone, Message)]
pub(super) struct CommandActionList {
    #[prost(message, repeated, tag = "1")]
    pub values: Vec<CommandAction>,
}
#[derive(Clone, Message)]
pub(super) struct CommandAction {
    #[prost(oneof = "command_action::Value", tags = "1, 2, 3, 4")]
    pub value: Option<command_action::Value>,
}
pub(super) mod command_action {
    #[derive(Clone, prost::Oneof)]
    pub enum Value {
        #[prost(message, tag = "1")]
        Read(super::ReadAction),
        #[prost(message, tag = "2")]
        ListFiles(super::ListFilesAction),
        #[prost(message, tag = "3")]
        Search(super::SearchAction),
        #[prost(message, tag = "4")]
        Unknown(super::UnknownAction),
    }
}
#[derive(Clone, Message)]
pub(super) struct ReadAction {
    #[prost(string, optional, tag = "1")]
    pub command: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub name: Option<String>,
    #[prost(string, optional, tag = "3")]
    pub path: Option<String>,
}
#[derive(Clone, Message)]
pub(super) struct ListFilesAction {
    #[prost(string, optional, tag = "1")]
    pub command: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub path: Option<String>,
}
#[derive(Clone, Message)]
pub(super) struct SearchAction {
    #[prost(string, optional, tag = "1")]
    pub command: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub path: Option<String>,
    #[prost(string, optional, tag = "3")]
    pub query: Option<String>,
}
#[derive(Clone, Message)]
pub(super) struct UnknownAction {
    #[prost(string, optional, tag = "1")]
    pub command: Option<String>,
}
#[derive(Clone, Message)]
pub(super) struct FileChange {
    #[prost(message, optional, tag = "1")]
    pub changes: Option<crate::transcript_wire::Changes>,
    #[prost(string, optional, tag = "2")]
    pub id: Option<String>,
    #[prost(int32, optional, tag = "3")]
    pub status: Option<i32>,
}

#[derive(Clone, Message)]
pub(super) struct UserMessage {
    #[prost(string, optional, tag = "1")]
    pub client_id: Option<String>,
    #[prost(message, optional, tag = "2")]
    pub content: Option<crate::turns_wire::Input>,
    #[prost(string, optional, tag = "3")]
    pub id: Option<String>,
}
