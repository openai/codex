//! Native user-input envelopes shared by item decoding and later turn submission.
use prost::Message;

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
    #[prost(bytes, tag = "2")]
    Image(Vec<u8>),
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
}
