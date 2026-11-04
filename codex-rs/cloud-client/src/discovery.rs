//! Typed, paginated cloud thread discovery over the native ThreadService.

use crate::Client;
use crate::Error;
use crate::ThreadSnapshot;
use bytes::Bytes;
use prost::Message;
use std::collections::HashMap;
use std::fmt;

#[derive(Clone, Copy, Debug, Default)]
pub enum ThreadSortKey {
    #[default]
    CreatedAt,
    UpdatedAt,
    RecencyAt,
}

#[derive(Clone, Copy, Debug, Default)]
pub enum ThreadSortDirection {
    Asc,
    #[default]
    Desc,
}

/// Lists unarchived threads with the server's default source selection.
/// Reuse the same originators and order when continuing with an opaque cursor.
#[derive(Clone, Default)]
pub struct ThreadListQuery {
    pub cursor: Option<String>,
    pub limit: Option<u32>,
    pub sort_key: ThreadSortKey,
    pub sort_direction: ThreadSortDirection,
    /// Empty means all originators, including conversations created by other clients.
    pub originators: Vec<String>,
}

/// One bounded page in server order; no automatic pagination or deduplication.
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadPage {
    pub data: Vec<ThreadSnapshot>,
    pub read_states: Option<HashMap<String, ThreadReadState>>,
    pub next_cursor: Option<String>,
    pub backwards_cursor: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadReadState {
    pub first_unread: Option<ThreadUnreadPosition>,
    pub revision: String,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ThreadUnreadPosition {
    ThreadStart,
    Turn {
        #[serde(rename = "turnId")]
        turn_id: String,
    },
}

macro_rules! redacted_debug {
    ($($ty:ty),*) => {$(impl fmt::Debug for $ty {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(concat!(stringify!($ty), "([redacted])"))
        }
    })*};
}
redacted_debug!(ThreadListQuery, ThreadPage);

impl Client {
    /// Returns one bounded native page. Read failures are not retried automatically.
    pub async fn list_threads(&self, query: &ThreadListQuery) -> Result<ThreadPage, Error> {
        let bytes = self
            .raw_unary(
                wire::ListThreadsRequest {
                    cursor: query.cursor.clone(),
                    limit: query.limit,
                    sort_key: Some(match query.sort_key {
                        ThreadSortKey::CreatedAt => 1,
                        ThreadSortKey::UpdatedAt => 2,
                        ThreadSortKey::RecencyAt => 3,
                    }),
                    sort_direction: Some(match query.sort_direction {
                        ThreadSortDirection::Asc => 1,
                        ThreadSortDirection::Desc => 2,
                    }),
                    archived: Some(false),
                    originators: Some(wire::StringList {
                        values: query.originators.clone(),
                    }),
                },
                "/oaiproto.codex_app_server_backend.ThreadService/ListThreads",
            )
            .await?;
        let response =
            wire::ListThreadsResponse::decode(bytes).map_err(|_| Error::InvalidResponse)?;
        Ok(ThreadPage {
            data: response
                .data
                .ok_or(Error::InvalidResponse)?
                .values
                .into_iter()
                .map(ThreadSnapshot::decode)
                .collect::<Result<_, _>>()?,
            read_states: response
                .read_states
                .map(|states| {
                    states
                        .values
                        .into_iter()
                        .map(|(thread_id, state)| {
                            let revision = state.revision.ok_or(Error::InvalidResponse)?;
                            if revision.is_empty() {
                                return Err(Error::InvalidResponse);
                            }
                            let first_unread = state
                                .first_unread
                                .map(|position| match position.value {
                                    Some(wire::thread_unread_position::Value::ThreadStart(_)) => {
                                        Ok(ThreadUnreadPosition::ThreadStart)
                                    }
                                    Some(wire::thread_unread_position::Value::Turn(turn)) => {
                                        let turn_id = turn.turn_id.ok_or(Error::InvalidResponse)?;
                                        if turn_id.is_empty() {
                                            Err(Error::InvalidResponse)
                                        } else {
                                            Ok(ThreadUnreadPosition::Turn { turn_id })
                                        }
                                    }
                                    None => Err(Error::InvalidResponse),
                                })
                                .transpose()?;
                            Ok((
                                thread_id,
                                ThreadReadState {
                                    first_unread,
                                    revision,
                                },
                            ))
                        })
                        .collect::<Result<_, _>>()
                })
                .transpose()?,
            next_cursor: response.next_cursor,
            backwards_cursor: response.backwards_cursor,
        })
    }
}

mod wire {
    use super::*;

    #[derive(Clone, Message)]
    pub(super) struct ListThreadsRequest {
        #[prost(string, optional, tag = "1")]
        pub cursor: Option<String>,
        #[prost(uint32, optional, tag = "2")]
        pub limit: Option<u32>,
        #[prost(int32, optional, tag = "3")]
        pub sort_key: Option<i32>,
        #[prost(int32, optional, tag = "4")]
        pub sort_direction: Option<i32>,
        #[prost(bool, optional, tag = "5")]
        pub archived: Option<bool>,
        #[prost(message, optional, tag = "6")]
        pub originators: Option<StringList>,
    }
    #[derive(Clone, Message)]
    pub(super) struct StringList {
        #[prost(string, repeated, tag = "1")]
        pub values: Vec<String>,
    }
    #[derive(Clone, Message)]
    pub(super) struct ListThreadsResponse {
        #[prost(message, optional, tag = "1")]
        pub data: Option<ThreadList>,
        #[prost(string, optional, tag = "2")]
        pub next_cursor: Option<String>,
        #[prost(string, optional, tag = "3")]
        pub backwards_cursor: Option<String>,
        #[prost(message, optional, tag = "4")]
        pub read_states: Option<ThreadReadStateMap>,
    }
    #[derive(Clone, Message)]
    pub(super) struct ThreadList {
        #[prost(bytes = "bytes", repeated, tag = "1")]
        pub values: Vec<Bytes>,
    }
    #[derive(Clone, Message)]
    pub(super) struct ThreadReadStateMap {
        #[prost(map = "string, message", tag = "1")]
        pub values: HashMap<String, ThreadReadState>,
    }
    #[derive(Clone, Message, PartialEq)]
    pub(super) struct ThreadReadState {
        #[prost(message, optional, tag = "1")]
        pub first_unread: Option<ThreadUnreadPosition>,
        #[prost(string, optional, tag = "2")]
        pub revision: Option<String>,
    }
    #[derive(Clone, Message, PartialEq)]
    pub(super) struct ThreadUnreadPosition {
        #[prost(oneof = "thread_unread_position::Value", tags = "1, 2")]
        pub value: Option<thread_unread_position::Value>,
    }
    pub(super) mod thread_unread_position {
        #[derive(Clone, prost::Oneof, PartialEq)]
        pub enum Value {
            #[prost(message, tag = "1")]
            ThreadStart(super::ThreadUnreadPositionThreadStart),
            #[prost(message, tag = "2")]
            Turn(super::ThreadUnreadPositionTurn),
        }
    }
    #[derive(Clone, Message, PartialEq)]
    pub(super) struct ThreadUnreadPositionThreadStart {}
    #[derive(Clone, Message, PartialEq)]
    pub(super) struct ThreadUnreadPositionTurn {
        #[prost(string, optional, tag = "1")]
        pub turn_id: Option<String>,
    }
}
