//! Native text/image submission and interruption envelopes; omitted settings inherit thread config.
use prost::Message;

#[derive(Clone, Message)]
pub(super) struct AddRequest {
    #[prost(string, tag = "4")]
    pub client_user_message_id: String,
    #[prost(string, optional, tag = "9")]
    pub effort: Option<String>,
    #[prost(message, optional, tag = "11")]
    pub input: Option<Input>,
    #[prost(string, optional, tag = "12")]
    pub model: Option<String>,
    #[prost(string, tag = "23")]
    pub thread_id: String,
    #[prost(string, optional, tag = "25")]
    pub turn_trigger: Option<String>,
    #[prost(int32, optional, tag = "26")]
    pub access: Option<i32>,
    #[prost(int32, optional, tag = "30")]
    pub mode: Option<i32>,
}
#[derive(Clone, Message)]
pub(super) struct Input {
    #[prost(message, repeated, tag = "1")]
    pub values: Vec<UserInput>,
}
#[derive(Clone, Message)]
pub(super) struct UserInput {
    #[prost(oneof = "Value", tags = "1, 2, 3, 4, 5, 6, 7")]
    pub value: Option<Value>,
}
#[derive(Clone, prost::Oneof)]
pub(super) enum Value {
    #[prost(message, tag = "1")]
    Text(Text),
    #[prost(message, tag = "2")]
    Image(Image),
    #[prost(bytes, tag = "3")]
    LocalImage(Vec<u8>),
    #[prost(bytes, tag = "4")]
    Audio(Vec<u8>),
    #[prost(bytes, tag = "5")]
    LocalAudio(Vec<u8>),
    #[prost(bytes, tag = "6")]
    Skill(Vec<u8>),
    #[prost(bytes, tag = "7")]
    Mention(Vec<u8>),
}
#[derive(Clone, Message)]
pub(super) struct Text {
    #[prost(string, optional, tag = "1")]
    pub text: Option<String>,
    #[prost(message, optional, tag = "2")]
    pub text_elements: Option<TextElements>,
}
#[derive(Clone, Message)]
pub(super) struct TextElements {
    #[prost(message, repeated, tag = "1")]
    pub values: Vec<TextElement>,
}
#[derive(Clone, Message)]
pub(super) struct TextElement {
    #[prost(message, optional, tag = "1")]
    pub byte_range: Option<ByteRange>,
    #[prost(string, optional, tag = "2")]
    pub placeholder: Option<String>,
}
#[derive(Clone, Message)]
pub(super) struct ByteRange {
    #[prost(uint64, optional, tag = "1")]
    pub end: Option<u64>,
    #[prost(uint64, optional, tag = "2")]
    pub start: Option<u64>,
}
#[derive(Clone, Message)]
pub(super) struct Image {
    #[prost(oneof = "ImageValue", tags = "1, 2")]
    pub value: Option<ImageValue>,
}
#[derive(Clone, prost::Oneof)]
pub(super) enum ImageValue {
    #[prost(message, tag = "1")]
    Url(ImageReference),
    #[prost(message, tag = "2")]
    FileId(ImageReference),
}
// Both native image alternatives have the same field layout.
#[derive(Clone, Message)]
pub(super) struct ImageReference {
    #[prost(int32, optional, tag = "1")]
    pub detail: Option<i32>,
    #[prost(string, optional, tag = "2")]
    pub value: Option<String>,
}
#[derive(Clone, Message)]
pub(super) struct AddResponse {
    #[prost(message, optional, tag = "1")]
    pub turn: Option<crate::transcript_wire::Turn>,
    #[prost(int32, optional, tag = "2")]
    pub outcome: Option<i32>,
    #[prost(string, optional, tag = "3")]
    pub client_user_message_id: Option<String>,
    #[prost(message, optional, tag = "5")]
    pub queued_submission: Option<QueuedSubmission>,
}
#[derive(Clone, Message)]
pub(super) struct QueuedSubmission {
    #[prost(string, optional, tag = "1")]
    pub id: Option<String>,
    #[prost(string, optional, tag = "3")]
    pub client_user_message_id: Option<String>,
}
#[derive(Clone, Message)]
pub(super) struct InterruptRequest {
    #[prost(string, tag = "1")]
    pub thread_id: String,
    #[prost(string, tag = "2")]
    pub turn_id: String,
    #[prost(int32, optional, tag = "3")]
    pub access: Option<i32>,
}
