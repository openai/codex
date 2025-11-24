use std::{io::Write, path::Path};

use serde::Deserialize;
use thiserror::Error;
use uuid::Uuid;

use super::v7 as protocol;

use protocol::{
    Attachment, AttachmentType, Event, MonitorCheckIn, SessionAggregates, SessionUpdate,
    Transaction,
};

/// Raised if a envelope cannot be parsed from a given input.
#[derive(Debug, Error)]
pub enum EnvelopeError {
    /// Unexpected end of file
    #[error("unexpected end of file")]
    UnexpectedEof,
    /// Missing envelope header
    #[error("missing envelope header")]
    MissingHeader,
    /// Missing item header
    #[error("missing item header")]
    MissingItemHeader,
    /// Missing newline after header or payload
    #[error("missing newline after header or payload")]
    MissingNewline,
    /// Invalid envelope header
    #[error("invalid envelope header")]
    InvalidHeader(#[source] serde_json::Error),
    /// Invalid item header
    #[error("invalid item header")]
    InvalidItemHeader(#[source] serde_json::Error),
    /// Invalid item payload
    #[error("invalid item payload")]
    InvalidItemPayload(#[source] serde_json::Error),
}

#[derive(Deserialize)]
struct EnvelopeHeader {
    event_id: Option<Uuid>,
}

/// An Envelope Item Type.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[non_exhaustive]
enum EnvelopeItemType {
    /// An Event Item type.
    #[serde(rename = "event")]
    Event,
    /// A Session Item type.
    #[serde(rename = "session")]
    SessionUpdate,
    /// A Session Aggregates Item type.
    #[serde(rename = "sessions")]
    SessionAggregates,
    /// A Transaction Item type.
    #[serde(rename = "transaction")]
    Transaction,
    /// An Attachment Item type.
    #[serde(rename = "attachment")]
    Attachment,
    /// A Monitor Check In Item Type
    #[serde(rename = "check_in")]
    MonitorCheckIn,
    /// A Metrics Item type.
    #[cfg(feature = "metrics")]
    #[serde(rename = "statsd")]
    Metrics,
}

/// An Envelope Item Header.
#[derive(Clone, Debug, Deserialize)]
struct EnvelopeItemHeader {
    r#type: EnvelopeItemType,
    length: Option<usize>,
    // Fields below apply only to Attachment Item type
    filename: Option<String>,
    attachment_type: Option<AttachmentType>,
    content_type: Option<String>,
}

/// An Envelope Item.
///
/// See the [documentation on Items](https://develop.sentry.dev/sdk/envelopes/#items)
/// for more details.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
#[allow(clippy::large_enum_variant)]
pub enum EnvelopeItem {
    /// An Event Item.
    ///
    /// See the [Event Item documentation](https://develop.sentry.dev/sdk/envelopes/#event)
    /// for more details.
    Event(Event<'static>),
    /// A Session Item.
    ///
    /// See the [Session Item documentation](https://develop.sentry.dev/sdk/envelopes/#session)
    /// for more details.
    SessionUpdate(SessionUpdate<'static>),
    /// A Session Aggregates Item.
    ///
    /// See the [Session Aggregates Item documentation](https://develop.sentry.dev/sdk/envelopes/#sessions)
    /// for more details.
    SessionAggregates(SessionAggregates<'static>),
    /// A Transaction Item.
    ///
    /// See the [Transaction Item documentation](https://develop.sentry.dev/sdk/envelopes/#transaction)
    /// for more details.
    Transaction(Transaction<'static>),
    /// An Attachment Item.
    ///
    /// See the [Attachment Item documentation](https://develop.sentry.dev/sdk/envelopes/#attachment)
    /// for more details.
    Attachment(Attachment),
    /// A MonitorCheckIn item.
    MonitorCheckIn(MonitorCheckIn),
    /// A Metrics Item.
    #[cfg(feature = "metrics")]
    Statsd(Vec<u8>),
    /// This is a sentinel item used to `filter` raw envelopes.
    Raw,
    // TODO:
    // etc…
}

impl From<Event<'static>> for EnvelopeItem {
    fn from(event: Event<'static>) -> Self {
        EnvelopeItem::Event(event)
    }
}

impl From<SessionUpdate<'static>> for EnvelopeItem {
    fn from(session: SessionUpdate<'static>) -> Self {
        EnvelopeItem::SessionUpdate(session)
    }
}

impl From<SessionAggregates<'static>> for EnvelopeItem {
    fn from(aggregates: SessionAggregates<'static>) -> Self {
        EnvelopeItem::SessionAggregates(aggregates)
    }
}

impl From<Transaction<'static>> for EnvelopeItem {
    fn from(transaction: Transaction<'static>) -> Self {
        EnvelopeItem::Transaction(transaction)
    }
}

impl From<Attachment> for EnvelopeItem {
    fn from(attachment: Attachment) -> Self {
        EnvelopeItem::Attachment(attachment)
    }
}

impl From<MonitorCheckIn> for EnvelopeItem {
    fn from(check_in: MonitorCheckIn) -> Self {
        EnvelopeItem::MonitorCheckIn(check_in)
    }
}

/// An Iterator over the items of an Envelope.
#[derive(Clone)]
pub struct EnvelopeItemIter<'s> {
    inner: std::slice::Iter<'s, EnvelopeItem>,
}

impl<'s> Iterator for EnvelopeItemIter<'s> {
    type Item = &'s EnvelopeItem;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// The items contained in an [`Envelope`].
///
/// This may be a vector of [`EnvelopeItem`]s (the standard case)
/// or a binary blob.
#[derive(Debug, Clone, PartialEq)]
enum Items {
    EnvelopeItems(Vec<EnvelopeItem>),
    Raw(Vec<u8>),
}

impl Default for Items {
    fn default() -> Self {
        Self::EnvelopeItems(Default::default())
    }
}

impl Items {
    fn is_empty(&self) -> bool {
        match self {
            Items::EnvelopeItems(items) => items.is_empty(),
            Items::Raw(bytes) => bytes.is_empty(),
        }
    }
}

/// A Sentry Envelope.
///
/// An Envelope is the data format that Sentry uses for Ingestion. It can contain
/// multiple Items, some of which are related, such as Events, and Event Attachments.
/// Other Items, such as Sessions are independent.
///
/// See the [documentation on Envelopes](https://develop.sentry.dev/sdk/envelopes/)
/// for more details.
#[derive(Clone, Default, Debug, PartialEq)]
pub struct Envelope {
    event_id: Option<Uuid>,
    items: Items,
}

impl Envelope {
    /// Creates a new empty Envelope.
    pub fn new() -> Envelope {
        Default::default()
    }

    /// Add a new Envelope Item.
    pub fn add_item<I>(&mut self, item: I)
    where
        I: Into<EnvelopeItem>,
    {
        let item = item.into();

        let Items::EnvelopeItems(ref mut items) = self.items else {
            if item != EnvelopeItem::Raw {
                eprintln!(
                    "WARNING: This envelope contains raw items. Adding an item is not supported."
                );
            }
            return;
        };

        if self.event_id.is_none() {
            if let EnvelopeItem::Event(ref event) = item {
                self.event_id = Some(event.event_id);
            } else if let EnvelopeItem::Transaction(ref transaction) = item {
                self.event_id = Some(transaction.event_id);
            }
        }
        items.push(item);
    }

    /// Create an [`Iterator`] over all the [`EnvelopeItem`]s.
    pub fn items(&self) -> EnvelopeItemIter {
        let inner = match &self.items {
            Items::EnvelopeItems(items) => items.iter(),
            Items::Raw(_) => [].iter(),
        };

        EnvelopeItemIter { inner }
    }

    /// Returns the Envelopes Uuid, if any.
    pub fn uuid(&self) -> Option<&Uuid> {
        self.event_id.as_ref()
    }

    /// Returns the [`Event`] contained in this Envelope, if any.
    ///
    /// [`Event`]: struct.Event.html
    pub fn event(&self) -> Option<&Event<'static>> {
        let Items::EnvelopeItems(ref items) = self.items else {
            return None;
        };

        items.iter().find_map(|item| match item {
            EnvelopeItem::Event(event) => Some(event),
            _ => None,
        })
    }

    /// Filters the Envelope's [`EnvelopeItem`]s based on a predicate,
    /// and returns a new Envelope containing only the filtered items.
    ///
    /// Retains the [`EnvelopeItem`]s for which the predicate returns `true`.
    /// Additionally, [`EnvelopeItem::Attachment`]s are only kept if the Envelope
    /// contains an [`EnvelopeItem::Event`] or [`EnvelopeItem::Transaction`].
    ///
    /// [`None`] is returned if no items remain in the Envelope after filtering.
    pub fn filter<P>(self, mut predicate: P) -> Option<Self>
    where
        P: FnMut(&EnvelopeItem) -> bool,
    {
        let Items::EnvelopeItems(items) = self.items else {
            return if predicate(&EnvelopeItem::Raw) {
                Some(self)
            } else {
                None
            };
        };

        let mut filtered = Envelope::new();
        for item in items {
            if predicate(&item) {
                filtered.add_item(item);
            }
        }

        // filter again, removing attachments which do not make any sense without
        // an event/transaction
        if filtered.uuid().is_none() {
            if let Items::EnvelopeItems(ref mut items) = filtered.items {
                items.retain(|item| !matches!(item, EnvelopeItem::Attachment(..)))
            }
        }

        if filtered.items.is_empty() {
            None
        } else {
            Some(filtered)
        }
    }

    /// Serialize the Envelope into the given [`Write`].
    ///
    /// [`Write`]: https://doc.rust-lang.org/std/io/trait.Write.html
    pub fn to_writer<W>(&self, mut writer: W) -> std::io::Result<()>
    where
        W: Write,
    {
        let items = match &self.items {
            Items::Raw(bytes) => return writer.write_all(bytes).map(|_| ()),
            Items::EnvelopeItems(items) => items,
        };

        // write the headers:
        let event_id = self.uuid();
        match event_id {
            Some(uuid) => writeln!(writer, r#"{{"event_id":"{uuid}"}}"#)?,
            _ => writeln!(writer, "{{}}")?,
        }

        let mut item_buf = Vec::new();
        // write each item:
        for item in items {
            // we write them to a temporary buffer first, since we need their length
            match item {
                EnvelopeItem::Event(event) => serde_json::to_writer(&mut item_buf, event)?,
                EnvelopeItem::SessionUpdate(session) => {
                    serde_json::to_writer(&mut item_buf, session)?
                }
                EnvelopeItem::SessionAggregates(aggregates) => {
                    serde_json::to_writer(&mut item_buf, aggregates)?
                }
                EnvelopeItem::Transaction(transaction) => {
                    serde_json::to_writer(&mut item_buf, transaction)?
                }
                EnvelopeItem::Attachment(attachment) => {
                    attachment.to_writer(&mut writer)?;
                    writeln!(writer)?;
                    continue;
                }
                EnvelopeItem::MonitorCheckIn(check_in) => {
                    serde_json::to_writer(&mut item_buf, check_in)?
                }
                #[cfg(feature = "metrics")]
                EnvelopeItem::Statsd(statsd) => item_buf.extend_from_slice(statsd),
                EnvelopeItem::Raw => {
                    continue;
                }
            }
            let item_type = match item {
                EnvelopeItem::Event(_) => "event",
                EnvelopeItem::SessionUpdate(_) => "session",
                EnvelopeItem::SessionAggregates(_) => "sessions",
                EnvelopeItem::Transaction(_) => "transaction",
                EnvelopeItem::MonitorCheckIn(_) => "check_in",
                #[cfg(feature = "metrics")]
                EnvelopeItem::Statsd(_) => "statsd",
                EnvelopeItem::Attachment(_) | EnvelopeItem::Raw => unreachable!(),
            };
            writeln!(
                writer,
                r#"{{"type":"{}","length":{}}}"#,
                item_type,
                item_buf.len()
            )?;
            writer.write_all(&item_buf)?;
            writeln!(writer)?;
            item_buf.clear();
        }

        Ok(())
    }

    /// Creates a new Envelope from slice.
    pub fn from_slice(slice: &[u8]) -> Result<Envelope, EnvelopeError> {
        let (header, offset) = Self::parse_header(slice)?;
        let items = Self::parse_items(slice, offset)?;

        let mut envelope = Envelope {
            event_id: header.event_id,
            ..Default::default()
        };

        for item in items {
            envelope.add_item(item);
        }

        Ok(envelope)
    }

    /// Creates a new raw Envelope from the given buffer.
    pub fn from_bytes_raw(bytes: Vec<u8>) -> Result<Self, EnvelopeError> {
        Ok(Self {
            event_id: None,
            items: Items::Raw(bytes),
        })
    }

    /// Creates a new Envelope from path.
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Envelope, EnvelopeError> {
        let bytes = std::fs::read(path).map_err(|_| EnvelopeError::UnexpectedEof)?;
        Envelope::from_slice(&bytes)
    }

    /// Creates a new Envelope from path without attempting to parse anything.
    ///
    /// The resulting Envelope will have no `event_id` and the file contents will
    /// be contained verbatim in the `items` field.
    pub fn from_path_raw<P: AsRef<Path>>(path: P) -> Result<Self, EnvelopeError> {
        let bytes = std::fs::read(path).map_err(|_| EnvelopeError::UnexpectedEof)?;
        Self::from_bytes_raw(bytes)
    }

    fn parse_header(slice: &[u8]) -> Result<(EnvelopeHeader, usize), EnvelopeError> {
        let mut stream = serde_json::Deserializer::from_slice(slice).into_iter();

        let header: EnvelopeHeader = match stream.next() {
            None => return Err(EnvelopeError::MissingHeader),
            Some(Err(error)) => return Err(EnvelopeError::InvalidHeader(error)),
            Some(Ok(header)) => header,
        };

        // Each header is terminated by a UNIX newline.
        Self::require_termination(slice, stream.byte_offset())?;

        Ok((header, stream.byte_offset() + 1))
    }

    fn parse_items(slice: &[u8], mut offset: usize) -> Result<Vec<EnvelopeItem>, EnvelopeError> {
        let mut items = Vec::new();

        while offset < slice.len() {
            let bytes = slice
                .get(offset..)
                .ok_or(EnvelopeError::MissingItemHeader)?;
            let (item, item_size) = Self::parse_item(bytes)?;
            offset += item_size;
            items.push(item);
        }

        Ok(items)
    }

    fn parse_item(slice: &[u8]) -> Result<(EnvelopeItem, usize), EnvelopeError> {
        let mut stream = serde_json::Deserializer::from_slice(slice).into_iter();

        let header: EnvelopeItemHeader = match stream.next() {
            None => return Err(EnvelopeError::UnexpectedEof),
            Some(Err(error)) => return Err(EnvelopeError::InvalidItemHeader(error)),
            Some(Ok(header)) => header,
        };

        // Each header is terminated by a UNIX newline.
        let header_end = stream.byte_offset();
        Self::require_termination(slice, header_end)?;

        // The last header does not require a trailing newline, so `payload_start` may point
        // past the end of the buffer.
        let payload_start = std::cmp::min(header_end + 1, slice.len());
        let payload_end = match header.length {
            Some(len) => {
                let payload_end = payload_start + len;
                if slice.len() < payload_end {
                    return Err(EnvelopeError::UnexpectedEof);
                }

                // Each payload is terminated by a UNIX newline.
                Self::require_termination(slice, payload_end)?;
                payload_end
            }
            None => match slice.get(payload_start..) {
                Some(range) => match range.iter().position(|&b| b == b'\n') {
                    Some(relative_end) => payload_start + relative_end,
                    None => slice.len(),
                },
                None => slice.len(),
            },
        };

        let payload = slice.get(payload_start..payload_end).unwrap();

        let item = match header.r#type {
            EnvelopeItemType::Event => serde_json::from_slice(payload).map(EnvelopeItem::Event),
            EnvelopeItemType::Transaction => {
                serde_json::from_slice(payload).map(EnvelopeItem::Transaction)
            }
            EnvelopeItemType::SessionUpdate => {
                serde_json::from_slice(payload).map(EnvelopeItem::SessionUpdate)
            }
            EnvelopeItemType::SessionAggregates => {
                serde_json::from_slice(payload).map(EnvelopeItem::SessionAggregates)
            }
            EnvelopeItemType::Attachment => Ok(EnvelopeItem::Attachment(Attachment {
                buffer: payload.to_owned(),
                filename: header.filename.unwrap_or_default(),
                content_type: header.content_type,
                ty: header.attachment_type,
            })),
            EnvelopeItemType::MonitorCheckIn => {
                serde_json::from_slice(payload).map(EnvelopeItem::MonitorCheckIn)
            }
            #[cfg(feature = "metrics")]
            EnvelopeItemType::Metrics => Ok(EnvelopeItem::Statsd(payload.into())),
        }
        .map_err(EnvelopeError::InvalidItemPayload)?;

        Ok((item, payload_end + 1))
    }

    fn require_termination(slice: &[u8], offset: usize) -> Result<(), EnvelopeError> {
        match slice.get(offset) {
            Some(&b'\n') | None => Ok(()),
            Some(_) => Err(EnvelopeError::MissingNewline),
        }
    }
}

impl<T> From<T> for Envelope
where
    T: Into<EnvelopeItem>,
{
    fn from(item: T) -> Self {
        let mut envelope = Self::default();
        envelope.add_item(item.into());
        envelope
    }
}

#[cfg(test)]
mod test {
    use std::str::FromStr;
    use std::time::{Duration, SystemTime};

    use time::format_description::well_known::Rfc3339;
    use time::OffsetDateTime;

    use super::*;
    use crate::protocol::v7::{
        Level, MonitorCheckInStatus, MonitorConfig, MonitorSchedule, SessionAttributes,
        SessionStatus, Span,
    };

    fn to_str(envelope: Envelope) -> String {
        let mut vec = Vec::new();
        envelope.to_writer(&mut vec).unwrap();
        String::from_utf8_lossy(&vec).to_string()
    }

    fn timestamp(s: &str) -> SystemTime {
        let dt = OffsetDateTime::parse(s, &Rfc3339).unwrap();
        let secs = dt.unix_timestamp() as u64;
        let nanos = dt.nanosecond();
        let duration = Duration::new(secs, nanos);
        SystemTime::UNIX_EPOCH.checked_add(duration).unwrap()
    }

    #[test]
    fn test_empty() {
        assert_eq!(to_str(Envelope::new()), "{}\n");
    }

    #[test]
    fn raw_roundtrip() {
        let buf = r#"{"event_id":"22d00b3f-d1b1-4b5d-8d20-49d138cd8a9c"}
{"type":"event","length":74}
{"event_id":"22d00b3fd1b14b5d8d2049d138cd8a9c","timestamp":1595256674.296}
"#;
        let envelope = Envelope::from_bytes_raw(buf.to_string().into_bytes()).unwrap();
        let serialized = to_str(envelope);
        assert_eq!(&serialized, buf);

        let random_invalid_bytes = b"oh stahp!\0\x01\x02";
        let envelope = Envelope::from_bytes_raw(random_invalid_bytes.to_vec()).unwrap();
        let mut serialized = Vec::new();
        envelope.to_writer(&mut serialized).unwrap();
        assert_eq!(&serialized, random_invalid_bytes);
    }

    #[test]
    fn test_event() {
        let event_id = Uuid::parse_str("22d00b3f-d1b1-4b5d-8d20-49d138cd8a9c").unwrap();
        let timestamp = timestamp("2020-07-20T14:51:14.296Z");
        let event = Event {
            event_id,
            timestamp,
            ..Default::default()
        };
        let envelope: Envelope = event.into();
        assert_eq!(
            to_str(envelope),
            r#"{"event_id":"22d00b3f-d1b1-4b5d-8d20-49d138cd8a9c"}
{"type":"event","length":74}
{"event_id":"22d00b3fd1b14b5d8d2049d138cd8a9c","timestamp":1595256674.296}
"#
        )
    }

    #[test]
    fn test_session() {
        let session_id = Uuid::parse_str("22d00b3f-d1b1-4b5d-8d20-49d138cd8a9c").unwrap();
        let started = timestamp("2020-07-20T14:51:14.296Z");
        let session = SessionUpdate {
            session_id,
            distinct_id: Some("foo@bar.baz".to_owned()),
            sequence: None,
            timestamp: None,
            started,
            init: true,
            duration: Some(1.234),
            status: SessionStatus::Ok,
            errors: 123,
            attributes: SessionAttributes {
                release: "foo-bar@1.2.3".into(),
                environment: Some("production".into()),
                ip_address: None,
                user_agent: None,
            },
        };
        let mut envelope = Envelope::new();
        envelope.add_item(session);
        assert_eq!(
            to_str(envelope),
            r#"{}
{"type":"session","length":222}
{"sid":"22d00b3f-d1b1-4b5d-8d20-49d138cd8a9c","did":"foo@bar.baz","started":"2020-07-20T14:51:14.296Z","init":true,"duration":1.234,"status":"ok","errors":123,"attrs":{"release":"foo-bar@1.2.3","environment":"production"}}
"#
        )
    }

    #[test]
    fn test_transaction() {
        let event_id = Uuid::parse_str("22d00b3f-d1b1-4b5d-8d20-49d138cd8a9c").unwrap();
        let span_id = "d42cee9fc3e74f5c".parse().unwrap();
        let trace_id = "335e53d614474acc9f89e632b776cc28".parse().unwrap();
        let start_timestamp = timestamp("2020-07-20T14:51:14.296Z");
        let spans = vec![Span {
            span_id,
            trace_id,
            start_timestamp,
            ..Default::default()
        }];
        let transaction = Transaction {
            event_id,
            start_timestamp,
            spans,
            ..Default::default()
        };
        let envelope: Envelope = transaction.into();
        assert_eq!(
            to_str(envelope),
            r#"{"event_id":"22d00b3f-d1b1-4b5d-8d20-49d138cd8a9c"}
{"type":"transaction","length":200}
{"event_id":"22d00b3fd1b14b5d8d2049d138cd8a9c","start_timestamp":1595256674.296,"spans":[{"span_id":"d42cee9fc3e74f5c","trace_id":"335e53d614474acc9f89e632b776cc28","start_timestamp":1595256674.296}]}
"#
        )
    }

    #[test]
    fn test_monitor_checkin() {
        let check_in_id = Uuid::parse_str("22d00b3f-d1b1-4b5d-8d20-49d138cd8a9c").unwrap();

        let check_in = MonitorCheckIn {
            check_in_id,
            monitor_slug: "my-monitor".into(),
            status: MonitorCheckInStatus::Ok,
            duration: Some(123.4),
            environment: Some("production".into()),
            monitor_config: Some(MonitorConfig {
                schedule: MonitorSchedule::Crontab {
                    value: "12 0 * * *".into(),
                },
                checkin_margin: Some(5),
                max_runtime: Some(30),
                timezone: Some("UTC".into()),
                failure_issue_threshold: None,
                recovery_threshold: None,
            }),
        };
        let envelope: Envelope = check_in.into();
        assert_eq!(
            to_str(envelope),
            r#"{}
{"type":"check_in","length":259}
{"check_in_id":"22d00b3fd1b14b5d8d2049d138cd8a9c","monitor_slug":"my-monitor","status":"ok","environment":"production","duration":123.4,"monitor_config":{"schedule":{"type":"crontab","value":"12 0 * * *"},"checkin_margin":5,"max_runtime":30,"timezone":"UTC"}}
"#
        )
    }

    #[test]
    fn test_monitor_checkin_with_thresholds() {
        let check_in_id = Uuid::parse_str("22d00b3f-d1b1-4b5d-8d20-49d138cd8a9c").unwrap();

        let check_in = MonitorCheckIn {
            check_in_id,
            monitor_slug: "my-monitor".into(),
            status: MonitorCheckInStatus::Ok,
            duration: Some(123.4),
            environment: Some("production".into()),
            monitor_config: Some(MonitorConfig {
                schedule: MonitorSchedule::Crontab {
                    value: "12 0 * * *".into(),
                },
                checkin_margin: Some(5),
                max_runtime: Some(30),
                timezone: Some("UTC".into()),
                failure_issue_threshold: Some(4),
                recovery_threshold: Some(7),
            }),
        };
        let envelope: Envelope = check_in.into();
        assert_eq!(
            to_str(envelope),
            r#"{}
{"type":"check_in","length":310}
{"check_in_id":"22d00b3fd1b14b5d8d2049d138cd8a9c","monitor_slug":"my-monitor","status":"ok","environment":"production","duration":123.4,"monitor_config":{"schedule":{"type":"crontab","value":"12 0 * * *"},"checkin_margin":5,"max_runtime":30,"timezone":"UTC","failure_issue_threshold":4,"recovery_threshold":7}}
"#
        )
    }

    #[test]
    fn test_event_with_attachment() {
        let event_id = Uuid::parse_str("22d00b3f-d1b1-4b5d-8d20-49d138cd8a9c").unwrap();
        let timestamp = timestamp("2020-07-20T14:51:14.296Z");
        let event = Event {
            event_id,
            timestamp,
            ..Default::default()
        };
        let mut envelope: Envelope = event.into();

        envelope.add_item(Attachment {
            buffer: "some content".as_bytes().to_vec(),
            filename: "file.txt".to_string(),
            ..Default::default()
        });

        assert_eq!(
            to_str(envelope),
            r#"{"event_id":"22d00b3f-d1b1-4b5d-8d20-49d138cd8a9c"}
{"type":"event","length":74}
{"event_id":"22d00b3fd1b14b5d8d2049d138cd8a9c","timestamp":1595256674.296}
{"type":"attachment","length":12,"filename":"file.txt","attachment_type":"event.attachment","content_type":"application/octet-stream"}
some content
"#
        )
    }

    #[test]
    fn test_deserialize_envelope_empty() {
        // Without terminating newline after header
        let bytes = b"{\"event_id\":\"9ec79c33ec9942ab8353589fcb2e04dc\"}";
        let envelope = Envelope::from_slice(bytes).unwrap();

        let event_id = Uuid::from_str("9ec79c33ec9942ab8353589fcb2e04dc").unwrap();
        assert_eq!(envelope.event_id, Some(event_id));
        assert_eq!(envelope.items().count(), 0);
    }

    #[test]
    fn test_deserialize_envelope_empty_newline() {
        // With terminating newline after header
        let bytes = b"{\"event_id\":\"9ec79c33ec9942ab8353589fcb2e04dc\"}\n";
        let envelope = Envelope::from_slice(bytes).unwrap();
        assert_eq!(envelope.items().count(), 0);
    }

    #[test]
    fn test_deserialize_envelope_empty_item_newline() {
        // With terminating newline after item payload
        let bytes = b"\
             {\"event_id\":\"9ec79c33ec9942ab8353589fcb2e04dc\"}\n\
             {\"type\":\"attachment\",\"length\":0}\n\
             \n\
             {\"type\":\"attachment\",\"length\":0}\n\
             ";

        let envelope = Envelope::from_slice(bytes).unwrap();
        assert_eq!(envelope.items().count(), 2);

        let mut items = envelope.items();

        if let EnvelopeItem::Attachment(attachment) = items.next().unwrap() {
            assert_eq!(attachment.buffer.len(), 0);
        } else {
            panic!("invalid item type");
        }

        if let EnvelopeItem::Attachment(attachment) = items.next().unwrap() {
            assert_eq!(attachment.buffer.len(), 0);
        } else {
            panic!("invalid item type");
        }
    }

    #[test]
    fn test_deserialize_envelope_empty_item_eof() {
        // With terminating newline after item payload
        let bytes = b"\
             {\"event_id\":\"9ec79c33ec9942ab8353589fcb2e04dc\"}\n\
             {\"type\":\"attachment\",\"length\":0}\n\
             \n\
             {\"type\":\"attachment\",\"length\":0}\
             ";

        let envelope = Envelope::from_slice(bytes).unwrap();
        assert_eq!(envelope.items().count(), 2);

        let mut items = envelope.items();

        if let EnvelopeItem::Attachment(attachment) = items.next().unwrap() {
            assert_eq!(attachment.buffer.len(), 0);
        } else {
            panic!("invalid item type");
        }

        if let EnvelopeItem::Attachment(attachment) = items.next().unwrap() {
            assert_eq!(attachment.buffer.len(), 0);
        } else {
            panic!("invalid item type");
        }
    }

    #[test]
    fn test_deserialize_envelope_implicit_length() {
        // With terminating newline after item payload
        let bytes = b"\
             {\"event_id\":\"9ec79c33ec9942ab8353589fcb2e04dc\"}\n\
             {\"type\":\"attachment\"}\n\
             helloworld\n\
             ";

        let envelope = Envelope::from_slice(bytes).unwrap();
        assert_eq!(envelope.items().count(), 1);

        let mut items = envelope.items();

        if let EnvelopeItem::Attachment(attachment) = items.next().unwrap() {
            assert_eq!(attachment.buffer.len(), 10);
        } else {
            panic!("invalid item type");
        }
    }

    #[test]
    fn test_deserialize_envelope_implicit_length_eof() {
        // With item ending the envelope
        let bytes = b"\
             {\"event_id\":\"9ec79c33ec9942ab8353589fcb2e04dc\"}\n\
             {\"type\":\"attachment\"}\n\
             helloworld\
             ";

        let envelope = Envelope::from_slice(bytes).unwrap();
        assert_eq!(envelope.items().count(), 1);

        let mut items = envelope.items();

        if let EnvelopeItem::Attachment(attachment) = items.next().unwrap() {
            assert_eq!(attachment.buffer.len(), 10);
        } else {
            panic!("invalid item type");
        }
    }

    #[test]
    fn test_deserialize_envelope_implicit_length_empty_eof() {
        // Empty item with implicit length ending the envelope
        let bytes = b"\
             {\"event_id\":\"9ec79c33ec9942ab8353589fcb2e04dc\"}\n\
             {\"type\":\"attachment\"}\
             ";

        let envelope = Envelope::from_slice(bytes).unwrap();
        assert_eq!(envelope.items().count(), 1);

        let mut items = envelope.items();

        if let EnvelopeItem::Attachment(attachment) = items.next().unwrap() {
            assert_eq!(attachment.buffer.len(), 0);
        } else {
            panic!("invalid item type");
        }
    }

    #[test]
    fn test_deserialize_envelope_multiple_items() {
        // With terminating newline
        let bytes = b"\
            {\"event_id\":\"9ec79c33ec9942ab8353589fcb2e04dc\"}\n\
            {\"type\":\"attachment\",\"length\":10,\"content_type\":\"text/plain\",\"filename\":\"hello.txt\"}\n\
            \xef\xbb\xbfHello\r\n\n\
            {\"type\":\"event\",\"length\":41,\"content_type\":\"application/json\",\"filename\":\"application.log\"}\n\
            {\"message\":\"hello world\",\"level\":\"error\"}\n\
            ";

        let envelope = Envelope::from_slice(bytes).unwrap();
        assert_eq!(envelope.items().count(), 2);

        let mut items = envelope.items();

        if let EnvelopeItem::Attachment(attachment) = items.next().unwrap() {
            assert_eq!(attachment.buffer.len(), 10);
            assert_eq!(attachment.buffer, b"\xef\xbb\xbfHello\r\n");
            assert_eq!(attachment.filename, "hello.txt");
            assert_eq!(attachment.content_type, Some("text/plain".to_string()));
        } else {
            panic!("invalid item type");
        }

        if let EnvelopeItem::Event(event) = items.next().unwrap() {
            assert_eq!(event.message, Some("hello world".to_string()));
            assert_eq!(event.level, Level::Error);
        } else {
            panic!("invalid item type");
        }
    }

    // Test all possible item types in a single envelope
    #[test]
    fn test_deserialize_serialized() {
        // Event
        let event = Event {
            event_id: Uuid::parse_str("22d00b3f-d1b1-4b5d-8d20-49d138cd8a9c").unwrap(),
            timestamp: timestamp("2020-07-20T14:51:14.296Z"),
            ..Default::default()
        };

        // Transaction
        let transaction = Transaction {
            event_id: Uuid::parse_str("22d00b3f-d1b1-4b5d-8d20-49d138cd8a9d").unwrap(),
            start_timestamp: timestamp("2020-07-20T14:51:14.296Z"),
            spans: vec![Span {
                span_id: "d42cee9fc3e74f5c".parse().unwrap(),
                trace_id: "335e53d614474acc9f89e632b776cc28".parse().unwrap(),
                start_timestamp: timestamp("2020-07-20T14:51:14.296Z"),
                ..Default::default()
            }],
            ..Default::default()
        };

        // Session
        let session = SessionUpdate {
            session_id: Uuid::parse_str("22d00b3f-d1b1-4b5d-8d20-49d138cd8a9c").unwrap(),
            distinct_id: Some("foo@bar.baz".to_owned()),
            sequence: None,
            timestamp: None,
            started: timestamp("2020-07-20T14:51:14.296Z"),
            init: true,
            duration: Some(1.234),
            status: SessionStatus::Ok,
            errors: 123,
            attributes: SessionAttributes {
                release: "foo-bar@1.2.3".into(),
                environment: Some("production".into()),
                ip_address: None,
                user_agent: None,
            },
        };

        // Attachment
        let attachment = Attachment {
            buffer: "some content".as_bytes().to_vec(),
            filename: "file.txt".to_string(),
            ..Default::default()
        };

        let mut envelope: Envelope = Envelope::new();

        envelope.add_item(event);
        envelope.add_item(transaction);
        envelope.add_item(session);
        envelope.add_item(attachment);

        let serialized = to_str(envelope);
        let deserialized = Envelope::from_slice(serialized.as_bytes()).unwrap();
        assert_eq!(serialized, to_str(deserialized))
    }
}
