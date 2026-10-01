//! Minimal ThreadService protobuf envelopes; field numbers are the wire contract.
//! Length-delimited nested events remain intact, including unknown protobuf fields.

#[derive(Clone, prost::Message)]
pub(crate) struct ResumeResponse {}

#[derive(Clone, prost::Message)]
pub(crate) struct AttachRequest {
    #[prost(string, tag = "1")]
    pub thread_id: String,
}

#[derive(Clone, prost::Message)]
pub(crate) struct AttachResponse {
    #[prost(oneof = "Payload", tags = "1, 2")]
    pub event: Option<Payload>,
}

#[derive(Clone, prost::Oneof)]
pub(crate) enum Payload {
    #[prost(bytes = "bytes", tag = "1")]
    Notification(bytes::Bytes),
    #[prost(bytes = "bytes", tag = "2")]
    ServerRequest(bytes::Bytes),
}
