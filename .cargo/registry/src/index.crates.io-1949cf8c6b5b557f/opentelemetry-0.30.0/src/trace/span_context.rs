use crate::{SpanId, TraceFlags, TraceId};
use std::collections::VecDeque;
use std::hash::Hash;
use std::str::FromStr;
use thiserror::Error;

/// TraceState carries system-specific configuration data, represented as a list
/// of key-value pairs. TraceState allows multiple tracing systems to
/// participate in the same trace.
///
/// Please review the [W3C specification] for details on this field.
///
/// [W3C specification]: https://www.w3.org/TR/trace-context/#tracestate-header
#[derive(Clone, Debug, Default, Eq, PartialEq, Hash)]
pub struct TraceState(Option<VecDeque<(String, String)>>);

impl TraceState {
    /// The default `TraceState`, as a constant
    pub const NONE: TraceState = TraceState(None);

    /// Validates that the given `TraceState` list-member key is valid per the [W3 Spec].
    ///
    /// [W3 Spec]: https://www.w3.org/TR/trace-context/#key
    fn valid_key(key: &str) -> bool {
        if key.len() > 256 {
            return false;
        }

        let allowed_special = |b: u8| (b == b'_' || b == b'-' || b == b'*' || b == b'/');
        let mut vendor_start = None;
        for (i, &b) in key.as_bytes().iter().enumerate() {
            if !(b.is_ascii_lowercase() || b.is_ascii_digit() || allowed_special(b) || b == b'@') {
                return false;
            }

            if i == 0 && (!b.is_ascii_lowercase() && !b.is_ascii_digit()) {
                return false;
            } else if b == b'@' {
                if vendor_start.is_some() || i + 14 < key.len() {
                    return false;
                }
                vendor_start = Some(i);
            } else if let Some(start) = vendor_start {
                if i == start + 1 && !(b.is_ascii_lowercase() || b.is_ascii_digit()) {
                    return false;
                }
            }
        }

        true
    }

    /// Validates that the given `TraceState` list-member value is valid per the [W3 Spec].
    ///
    /// [W3 Spec]: https://www.w3.org/TR/trace-context/#value
    fn valid_value(value: &str) -> bool {
        if value.len() > 256 {
            return false;
        }

        !(value.contains(',') || value.contains('='))
    }

    /// Creates a new `TraceState` from the given key-value collection.
    ///
    /// # Examples
    ///
    /// ```
    /// use opentelemetry::trace::TraceState;
    ///
    /// let kvs = vec![("foo", "bar"), ("apple", "banana")];
    /// let trace_state = TraceState::from_key_value(kvs);
    ///
    /// assert!(trace_state.is_ok());
    /// assert_eq!(trace_state.unwrap().header(), String::from("foo=bar,apple=banana"))
    /// ```
    pub fn from_key_value<T, K, V>(trace_state: T) -> TraceStateResult<Self>
    where
        T: IntoIterator<Item = (K, V)>,
        K: ToString,
        V: ToString,
    {
        let ordered_data = trace_state
            .into_iter()
            .map(|(key, value)| {
                let (key, value) = (key.to_string(), value.to_string());
                if !TraceState::valid_key(key.as_str()) {
                    return Err(TraceStateError::Key(key));
                }
                if !TraceState::valid_value(value.as_str()) {
                    return Err(TraceStateError::Value(value));
                }

                Ok((key, value))
            })
            .collect::<Result<VecDeque<_>, TraceStateError>>()?;

        if ordered_data.is_empty() {
            Ok(TraceState(None))
        } else {
            Ok(TraceState(Some(ordered_data)))
        }
    }

    /// Retrieves a value for a given key from the `TraceState` if it exists.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.as_ref().and_then(|kvs| {
            kvs.iter().find_map(|item| {
                if item.0.as_str() == key {
                    Some(item.1.as_str())
                } else {
                    None
                }
            })
        })
    }

    /// Inserts the given key-value pair into the `TraceState`. If a value already exists for the
    /// given key, this updates the value and updates the value's position. If the key or value are
    /// invalid per the [W3 Spec] an `Err` is returned, else a new `TraceState` with the
    /// updated key/value is returned.
    ///
    /// [W3 Spec]: https://www.w3.org/TR/trace-context/#mutating-the-tracestate-field
    pub fn insert<K, V>(&self, key: K, value: V) -> TraceStateResult<TraceState>
    where
        K: Into<String>,
        V: Into<String>,
    {
        let (key, value) = (key.into(), value.into());
        if !TraceState::valid_key(key.as_str()) {
            return Err(TraceStateError::Key(key));
        }
        if !TraceState::valid_value(value.as_str()) {
            return Err(TraceStateError::Value(value));
        }

        let mut trace_state = self.delete_from_deque(&key);
        let kvs = trace_state.0.get_or_insert(VecDeque::with_capacity(1));

        kvs.push_front((key, value));

        Ok(trace_state)
    }

    /// Removes the given key-value pair from the `TraceState`. If the key is invalid per the
    /// [W3 Spec] an `Err` is returned. Else, a new `TraceState`
    /// with the removed entry is returned.
    ///
    /// If the key is not in `TraceState`. The original `TraceState` will be cloned and returned.
    ///
    /// [W3 Spec]: https://www.w3.org/TR/trace-context/#mutating-the-tracestate-field
    pub fn delete<K: Into<String>>(&self, key: K) -> TraceStateResult<TraceState> {
        let key = key.into();
        if !TraceState::valid_key(key.as_str()) {
            return Err(TraceStateError::Key(key));
        }

        Ok(self.delete_from_deque(&key))
    }

    /// Delete key from trace state's deque. The key MUST be valid
    fn delete_from_deque(&self, key: &str) -> TraceState {
        let mut owned = self.clone();
        if let Some(kvs) = owned.0.as_mut() {
            if let Some(index) = kvs.iter().position(|x| x.0 == key) {
                kvs.remove(index);
            }
        }
        owned
    }

    /// Creates a new `TraceState` header string, delimiting each key and value with a `=` and each
    /// entry with a `,`.
    pub fn header(&self) -> String {
        self.header_delimited("=", ",")
    }

    /// Creates a new `TraceState` header string, with the given key/value delimiter and entry delimiter.
    pub fn header_delimited(&self, entry_delimiter: &str, list_delimiter: &str) -> String {
        self.0
            .as_ref()
            .map(|kvs| {
                kvs.iter()
                    .map(|(key, value)| format!("{}{}{}", key, entry_delimiter, value))
                    .collect::<Vec<String>>()
                    .join(list_delimiter)
            })
            .unwrap_or_default()
    }
}

impl FromStr for TraceState {
    type Err = TraceStateError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let list_members: Vec<&str> = s.split_terminator(',').collect();
        let mut key_value_pairs: Vec<(String, String)> = Vec::with_capacity(list_members.len());

        for list_member in list_members {
            match list_member.find('=') {
                None => return Err(TraceStateError::List(list_member.to_string())),
                Some(separator_index) => {
                    let (key, value) = list_member.split_at(separator_index);
                    key_value_pairs
                        .push((key.to_string(), value.trim_start_matches('=').to_string()));
                }
            }
        }

        TraceState::from_key_value(key_value_pairs)
    }
}

/// A specialized `Result` type for trace state operations.
type TraceStateResult<T> = Result<T, TraceStateError>;

/// Error returned by `TraceState` operations.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum TraceStateError {
    /// The key is invalid.
    ///
    /// See <https://www.w3.org/TR/trace-context/#key> for requirement for keys.
    #[error("{0} is not a valid key in TraceState, see https://www.w3.org/TR/trace-context/#key for more details")]
    Key(String),

    /// The value is invalid.
    ///
    /// See <https://www.w3.org/TR/trace-context/#value> for requirement for values.
    #[error("{0} is not a valid value in TraceState, see https://www.w3.org/TR/trace-context/#value for more details")]
    Value(String),

    /// The list is invalid.
    ///
    /// See <https://www.w3.org/TR/trace-context/#list> for requirement for list members.
    #[error("{0} is not a valid list member in TraceState, see https://www.w3.org/TR/trace-context/#list for more details")]
    List(String),
}

/// Immutable portion of a [`Span`] which can be serialized and propagated.
///
/// This representation conforms to the [W3C TraceContext specification].
///
/// Spans that do not have the `sampled` flag set in their [`TraceFlags`] will
/// be ignored by most tracing tools.
///
/// [`Span`]: crate::trace::Span
/// [W3C TraceContext specification]: https://www.w3.org/TR/trace-context
#[derive(Clone, Debug, PartialEq, Hash, Eq)]
pub struct SpanContext {
    trace_id: TraceId,
    span_id: SpanId,
    trace_flags: TraceFlags,
    is_remote: bool,
    trace_state: TraceState,
}

impl SpanContext {
    /// An invalid span context
    pub const NONE: SpanContext = SpanContext {
        trace_id: TraceId::INVALID,
        span_id: SpanId::INVALID,
        trace_flags: TraceFlags::NOT_SAMPLED,
        is_remote: false,
        trace_state: TraceState::NONE,
    };

    /// Create an invalid empty span context
    pub fn empty_context() -> Self {
        SpanContext::NONE
    }

    /// Construct a new `SpanContext`
    pub fn new(
        trace_id: TraceId,
        span_id: SpanId,
        trace_flags: TraceFlags,
        is_remote: bool,
        trace_state: TraceState,
    ) -> Self {
        SpanContext {
            trace_id,
            span_id,
            trace_flags,
            is_remote,
            trace_state,
        }
    }

    /// The [`TraceId`] for this span context.
    pub fn trace_id(&self) -> TraceId {
        self.trace_id
    }

    /// The [`SpanId`] for this span context.
    pub fn span_id(&self) -> SpanId {
        self.span_id
    }

    /// Returns details about the trace.
    ///
    /// Unlike `TraceState` values, these are present in all traces. The current
    /// version of the specification only supports a single flag [`TraceFlags::SAMPLED`].
    pub fn trace_flags(&self) -> TraceFlags {
        self.trace_flags
    }

    /// Returns `true` if the span context has a valid (non-zero) `trace_id` and a
    /// valid (non-zero) `span_id`.
    pub fn is_valid(&self) -> bool {
        self.trace_id != TraceId::INVALID && self.span_id != SpanId::INVALID
    }

    /// Returns `true` if the span context was propagated from a remote parent.
    pub fn is_remote(&self) -> bool {
        self.is_remote
    }

    /// Returns `true` if the `sampled` trace flag is set.
    ///
    /// Spans that are not sampled will be ignored by most tracing tools.
    pub fn is_sampled(&self) -> bool {
        self.trace_flags.is_sampled()
    }

    /// A reference to the span context's [`TraceState`].
    pub fn trace_state(&self) -> &TraceState {
        &self.trace_state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{trace::TraceContextExt, Context};

    #[rustfmt::skip]
    fn trace_state_test_data() -> Vec<(TraceState, &'static str, &'static str)> {
        vec![
            (TraceState::from_key_value(vec![("foo", "bar")]).unwrap(), "foo=bar", "foo"),
            (TraceState::from_key_value(vec![("foo", ""), ("apple", "banana")]).unwrap(), "foo=,apple=banana", "apple"),
            (TraceState::from_key_value(vec![("foo", "bar"), ("apple", "banana")]).unwrap(), "foo=bar,apple=banana", "apple"),
        ]
    }

    #[test]
    fn test_trace_state() {
        for test_case in trace_state_test_data() {
            assert_eq!(test_case.0.clone().header(), test_case.1);

            let new_key = format!("{}-{}", test_case.0.get(test_case.2).unwrap(), "test");

            let updated_trace_state = test_case.0.insert(test_case.2, new_key.clone());
            assert!(updated_trace_state.is_ok());
            let updated_trace_state = updated_trace_state.unwrap();

            let updated = format!("{}={}", test_case.2, new_key);

            let index = updated_trace_state.clone().header().find(&updated);

            assert!(index.is_some());
            assert_eq!(index.unwrap(), 0);

            let deleted_trace_state = updated_trace_state.delete(test_case.2.to_string());
            assert!(deleted_trace_state.is_ok());

            let deleted_trace_state = deleted_trace_state.unwrap();

            assert!(deleted_trace_state.get(test_case.2).is_none());
        }
    }

    #[test]
    fn test_trace_state_key() {
        let test_data: Vec<(&'static str, bool)> = vec![
            ("123", true),
            ("bar", true),
            ("foo@bar", true),
            ("foo@0123456789abcdef", false),
            ("foo@012345678", true),
            ("FOO@BAR", false),
            ("你好", false),
        ];

        for (key, expected) in test_data {
            assert_eq!(TraceState::valid_key(key), expected, "test key: {:?}", key);
        }
    }

    #[test]
    fn test_trace_state_insert() {
        let trace_state = TraceState::from_key_value(vec![("foo", "bar")]).unwrap();
        let inserted_trace_state = trace_state.insert("testkey", "testvalue").unwrap();
        assert!(trace_state.get("testkey").is_none()); // The original state doesn't change
        assert_eq!(inserted_trace_state.get("testkey").unwrap(), "testvalue"); //
    }

    #[test]
    fn test_context_span_debug() {
        let cx = Context::current();
        assert_eq!(
            format!("{:?}", cx),
            "Context { span: \"None\", entries count: 0, suppress_telemetry: false }"
        );
        let cx = Context::current().with_remote_span_context(SpanContext::NONE);
        assert_eq!(
            format!("{:?}", cx),
            "Context { \
               span: SpanContext { \
                       trace_id: 00000000000000000000000000000000, \
                       span_id: 0000000000000000, \
                       trace_flags: TraceFlags(0), \
                       is_remote: false, \
                       trace_state: TraceState(None) \
                     }, \
               entries count: 1, suppress_telemetry: false \
             }"
        );
    }
}
