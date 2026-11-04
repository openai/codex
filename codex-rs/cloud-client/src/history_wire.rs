//! Native paginated history envelopes. Raw rows and unary responses retain unknown fields.
use bytes::Bytes;
use prost::Message;

#[derive(Clone, Message)]
pub(super) struct TurnsRequest {
    #[prost(string, tag = "1")]
    pub thread_id: String,
    #[prost(string, optional, tag = "2")]
    pub cursor: Option<String>,
    #[prost(uint32, optional, tag = "3")]
    pub limit: Option<u32>,
    #[prost(int32, optional, tag = "4")]
    pub order: Option<i32>,
    #[prost(int32, optional, tag = "6")]
    pub access: Option<i32>,
}
#[derive(Clone, Message)]
pub(super) struct ItemsRequest {
    #[prost(string, tag = "1")]
    pub thread_id: String,
    #[prost(string, optional, tag = "2")]
    pub turn_id: Option<String>,
    #[prost(string, optional, tag = "3")]
    pub cursor: Option<String>,
    #[prost(uint32, optional, tag = "4")]
    pub limit: Option<u32>,
    #[prost(int32, optional, tag = "5")]
    pub order: Option<i32>,
    #[prost(int32, optional, tag = "6")]
    pub access: Option<i32>,
}
#[derive(Clone, Message)]
pub(super) struct Page {
    #[prost(message, optional, tag = "1")]
    pub data: Option<Rows>,
    #[prost(string, optional, tag = "2")]
    pub next_cursor: Option<String>,
    #[prost(string, optional, tag = "3")]
    pub backwards_cursor: Option<String>,
}
#[derive(Clone, Message)]
pub(super) struct Rows {
    #[prost(bytes = "bytes", repeated, tag = "1")]
    pub values: Vec<Bytes>,
}
#[derive(Clone, Message)]
pub(super) struct ItemEntry {
    #[prost(string, optional, tag = "1")]
    pub turn_id: Option<String>,
    #[prost(message, optional, tag = "2")]
    pub item: Option<crate::item_wire::Item>,
    #[prost(int64, optional, tag = "3")]
    pub started_at_ms: Option<i64>,
    #[prost(int64, optional, tag = "4")]
    pub completed_at_ms: Option<i64>,
}
