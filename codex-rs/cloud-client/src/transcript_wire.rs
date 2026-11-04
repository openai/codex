//! Mirrors the ASB notification fields exposed by this client so it can build without
//! the monorepo's generated bindings. Keep the tag numbers in sync with `Notification.value`
//! in `native_notification.proto`. Only the final oneof variant is decoded on a second pass.
use prost::Message;

type SelectedNotification<'a> = (u32, SelectedMessages<'a>);

pub(super) struct SelectedMessages<'a> {
    tag: u32,
    input: &'a [u8],
}

// Known oneof members in native_notification.proto replace earlier variants, even
// when unsupported. Fields outside that schema leave the selected variant intact.
pub(super) fn notification(
    input: &[u8],
) -> Result<Option<SelectedNotification<'_>>, prost::DecodeError> {
    let mut cursor = input;
    let mut selected = None;
    while !cursor.is_empty() {
        let start = input.len() - cursor.len();
        let (tag, kind) = prost::encoding::decode_key(&mut cursor)?;
        if (1..=89).contains(&tag) {
            prost::encoding::check_wire_type(prost::encoding::WireType::LengthDelimited, kind)?;
            if selected.is_none_or(|(previous, _)| previous != tag) {
                selected = Some((tag, start));
            }
        }
        prost::encoding::skip_field(kind, tag, &mut cursor, Default::default())?;
    }
    Ok(selected.map(|(tag, start)| {
        (
            tag,
            SelectedMessages {
                tag,
                input: &input[start..],
            },
        )
    }))
}

pub(super) fn merge_messages<T: Message + Default>(
    values: SelectedMessages<'_>,
) -> Result<T, prost::DecodeError> {
    let mut message = T::default();
    let mut input = values.input;
    while !input.is_empty() {
        let (tag, kind) = prost::encoding::decode_key(&mut input)?;
        if tag == values.tag {
            // The first pass validated the wire type and bounded every length.
            let length = prost::encoding::decode_varint(&mut input)? as usize;
            let (value, tail) = input.split_at(length);
            message.merge(value)?;
            input = tail;
        } else {
            prost::encoding::skip_field(kind, tag, &mut input, Default::default())?;
        }
    }
    Ok(message)
}

#[derive(Clone, Message)]
pub(super) struct Envelope<T: Message + Default + std::fmt::Debug> {
    #[prost(int64, optional, tag = "1")]
    pub emitted_at_ms: Option<i64>,
    #[prost(message, optional, tag = "2")]
    pub params: Option<T>,
}
#[derive(Clone, Message)]
pub(super) struct StatusUpdate {
    #[prost(message, optional, tag = "1")]
    pub status: Option<crate::activation_wire::ThreadStatus>,
    #[prost(string, optional, tag = "2")]
    pub thread_id: Option<String>,
}
#[derive(Clone, Message)]
pub(super) struct PatchUpdate {
    #[prost(message, optional, tag = "1")]
    pub changes: Option<Changes>,
    #[prost(string, optional, tag = "2")]
    pub item_id: Option<String>,
    #[prost(string, optional, tag = "3")]
    pub thread_id: Option<String>,
    #[prost(string, optional, tag = "4")]
    pub turn_id: Option<String>,
}
#[derive(Clone, Message)]
pub(super) struct Delta {
    #[prost(string, optional, tag = "1")]
    pub delta: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub item_id: Option<String>,
    #[prost(string, optional, tag = "3")]
    pub thread_id: Option<String>,
    #[prost(string, optional, tag = "4")]
    pub turn_id: Option<String>,
}
#[derive(Clone, Message)]
pub(super) struct Changes {
    #[prost(message, repeated, tag = "1")]
    pub values: Vec<Change>,
}
#[derive(Clone, Message)]
pub(super) struct Change {
    #[prost(string, optional, tag = "1")]
    pub diff: Option<String>,
    #[prost(message, optional, tag = "2")]
    pub kind: Option<ChangeKind>,
    #[prost(string, optional, tag = "3")]
    pub path: Option<String>,
}
#[derive(Clone, Message)]
pub(super) struct ChangeKind {
    #[prost(oneof = "Kind", tags = "1, 2, 3")]
    pub value: Option<Kind>,
}
#[derive(Clone, prost::Oneof)]
pub(super) enum Kind {
    #[prost(message, tag = "1")]
    Add(Empty),
    #[prost(message, tag = "2")]
    Delete(Empty),
    #[prost(message, tag = "3")]
    Update(Move),
}
#[derive(Clone, Message)]
pub(super) struct Empty {}
#[derive(Clone, Message)]
pub(super) struct Move {
    #[prost(string, optional, tag = "1")]
    pub move_path: Option<String>,
}
#[derive(Clone, Message)]
pub(super) struct TurnUpdate {
    #[prost(string, optional, tag = "1")]
    pub thread_id: Option<String>,
    #[prost(message, optional, tag = "2")]
    pub turn: Option<Turn>,
}
#[derive(Clone, Message)]
pub(super) struct Turn {
    #[prost(int64, optional, tag = "1")]
    pub completed_at: Option<i64>,
    #[prost(int64, optional, tag = "2")]
    pub duration_ms: Option<i64>,
    #[prost(message, optional, tag = "3")]
    pub error: Option<TurnError>,
    #[prost(string, optional, tag = "4")]
    pub id: Option<String>,
    #[prost(int64, optional, tag = "7")]
    pub started_at: Option<i64>,
    #[prost(int32, optional, tag = "8")]
    pub status: Option<i32>,
}
#[derive(Clone, Message)]
pub(super) struct TurnError {
    #[prost(string, optional, tag = "1")]
    pub additional_details: Option<String>,
    #[prost(string, optional, tag = "3")]
    pub message: Option<String>,
}
