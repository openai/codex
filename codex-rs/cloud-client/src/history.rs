//! Bounded history page reads. Pagination cursors are not live-event replay offsets.
use crate::Client;
use crate::Error;
use crate::ItemSummary;
use crate::TurnSummary;
use crate::history_wire as wire;
use crate::transcript::DecodeFailure;
use crate::transcript::required;
use bytes::Bytes;
use prost::Message;
use std::fmt;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HistoryOrder {
    Ascending,
    #[default]
    Descending,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HistoryAccess {
    #[default]
    Default,
    Shared,
}
/// A cursor belongs to its original thread and filters. Keep the order for
/// `next_cursor` and reverse it for `backwards_cursor`.
#[derive(Clone, Default)]
pub struct HistoryQuery {
    pub cursor: Option<String>,
    pub limit: Option<u32>,
    pub order: HistoryOrder,
    pub access: HistoryAccess,
}
#[derive(Clone)]
pub struct ListTurnsRequest {
    pub thread_id: String,
    pub page: HistoryQuery,
}
#[derive(Clone)]
pub struct ListItemsRequest {
    pub thread_id: String,
    pub turn_id: Option<String>,
    pub page: HistoryQuery,
}
/// One server page in server order. No automatic pagination, deduplication or retries.
pub struct HistoryPage<T> {
    pub thread_id: String,
    pub data: Vec<T>,
    pub next_cursor: Option<String>,
    pub backwards_cursor: Option<String>,
    payload: Bytes,
}
impl<T> HistoryPage<T> {
    pub fn payload(&self) -> &Bytes {
        &self.payload
    }
}
pub struct HistoryTurn {
    pub summary: TurnSummary,
    payload: Bytes,
}
impl HistoryTurn {
    /// Original Turn protobuf; includes metadata omitted from the summary.
    pub fn payload(&self) -> &Bytes {
        &self.payload
    }
}
pub struct HistoryItem {
    pub turn_id: String,
    /// Unsupported items and command sources have no summary and retain their original entry.
    pub summary: Option<ItemSummary>,
    pub started_at_ms: Option<i64>,
    pub completed_at_ms: Option<i64>,
    payload: Bytes,
}
impl HistoryItem {
    /// Original ThreadItemEntry protobuf, including the item and untyped metadata.
    pub fn payload(&self) -> &Bytes {
        &self.payload
    }
}
macro_rules! redacted_debug {
    ($($ty:ty),*) => {$(impl fmt::Debug for $ty {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(concat!(stringify!($ty), "([redacted])")) }
    })*};
}
redacted_debug!(
    HistoryQuery,
    ListTurnsRequest,
    ListItemsRequest,
    HistoryTurn,
    HistoryItem
);
impl<T> fmt::Debug for HistoryPage<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("HistoryPage([redacted])")
    }
}
impl HistoryQuery {
    fn order(&self) -> Option<i32> {
        Some(match self.order {
            HistoryOrder::Ascending => 1,
            HistoryOrder::Descending => 2,
        })
    }
    fn access(&self) -> Option<i32> {
        match self.access {
            HistoryAccess::Default => None,
            HistoryAccess::Shared => Some(1),
        }
    }
}
impl Client {
    /// Reads stored turn metadata; use `list_items` for paginated item content.
    pub async fn list_turns(
        &self,
        request: &ListTurnsRequest,
    ) -> Result<HistoryPage<HistoryTurn>, Error> {
        let payload = self
            .raw_unary(
                wire::TurnsRequest {
                    thread_id: request.thread_id.clone(),
                    cursor: request.page.cursor.clone(),
                    limit: request.page.limit,
                    order: request.page.order(),
                    access: request.page.access(),
                },
                "/oaiproto.codex_app_server_backend.ThreadService/ListTurns",
            )
            .await?;
        decode_page(&request.thread_id, payload, |payload| {
            let turn = crate::transcript_wire::Turn::decode(payload.clone())?;
            Ok(HistoryTurn {
                summary: turn.try_into()?,
                payload,
            })
        })
        .map_err(|_| Error::InvalidResponse)
    }
    /// Reads one page, across the thread or restricted to one turn. This is not an
    /// atomic snapshot synchronized with Attach; do not append buffered deltas to it.
    pub async fn list_items(
        &self,
        request: &ListItemsRequest,
    ) -> Result<HistoryPage<HistoryItem>, Error> {
        let payload = self
            .raw_unary(
                wire::ItemsRequest {
                    thread_id: request.thread_id.clone(),
                    turn_id: request.turn_id.clone(),
                    cursor: request.page.cursor.clone(),
                    limit: request.page.limit,
                    order: request.page.order(),
                    access: request.page.access(),
                },
                "/oaiproto.codex_app_server_backend.ThreadService/ListItems",
            )
            .await?;
        decode_page(&request.thread_id, payload, |payload| {
            let entry = wire::ItemEntry::decode(payload.clone())?;
            let turn_id = required(entry.turn_id)?;
            if request
                .turn_id
                .as_ref()
                .is_some_and(|expected| expected != &turn_id)
            {
                return Err(DecodeFailure::InvalidResponse);
            }
            Ok(HistoryItem {
                turn_id,
                summary: crate::items::summary(required(entry.item)?)?,
                started_at_ms: entry.started_at_ms,
                completed_at_ms: entry.completed_at_ms,
                payload,
            })
        })
        .map_err(|_| Error::InvalidResponse)
    }
}
fn decode_page<T>(
    thread_id: &str,
    payload: Bytes,
    decode: impl Fn(Bytes) -> Result<T, DecodeFailure>,
) -> Result<HistoryPage<T>, DecodeFailure> {
    let page = wire::Page::decode(payload.clone())?;
    let data = required(page.data)?
        .values
        .into_iter()
        .map(decode)
        .collect::<Result<_, _>>()?;
    Ok(HistoryPage {
        thread_id: thread_id.to_owned(),
        data,
        next_cursor: page.next_cursor,
        backwards_cursor: page.backwards_cursor,
        payload,
    })
}
