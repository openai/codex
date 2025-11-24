use crate::{trace::SpanContext, KeyValue};
use std::borrow::Cow;
use std::error::Error;
use std::time::SystemTime;

/// The interface for a single operation within a trace.
///
/// Spans can be nested to form a trace tree. Each trace contains a root span,
/// which typically describes the entire operation and, optionally, one or more
/// sub-spans for its sub-operations.
///
/// The span `name` concisely identifies the work represented by the span, for
/// example, an RPC method name, a function name, or the name of a subtask or
/// stage within a larger computation. The span name should be the most general
/// string that identifies a (statistically) interesting class of spans, rather
/// than individual span instances while still being human-readable. That is,
/// `"get_user"` is a reasonable name, while `"get_user/314159"`, where `"314159"` is
/// a user ID, is not a good name due to its high cardinality. _Generality_
/// should be prioritized over _human-readability_.
///
/// For example, here are potential span names for an endpoint that gets a
/// hypothetical account information:
///
/// | Span Name         | Guidance     |
/// | ----------------- | ------------ |
/// | `get`             | Too general  |
/// | `get_account/42`  | Too specific |
/// | `get_account`     | Good, and account_id=42 would make a nice Span attribute |
/// | `get_account/{accountId}` | Also good (using the "HTTP route") |
///
/// The span's start and end timestamps reflect the elapsed real time of the
/// operation.
///
/// For example, if a span represents a request-response cycle (e.g. HTTP or an
/// RPC), the span should have a start time that corresponds to the start time
/// of the first sub-operation, and an end time of when the final sub-operation
/// is complete. This includes:
///
/// * receiving the data from the request
/// * parsing of the data (e.g. from a binary or json format)
/// * any middleware or additional processing logic
/// * business logic
/// * construction of the response
/// * sending of the response
///
/// Child spans (or in some cases events) may be created to represent
/// sub-operations which require more detailed observability. Child spans should
/// measure the timing of the respective sub-operation, and may add additional
/// attributes.
pub trait Span {
    /// Record an event in the context this span.
    ///
    /// Note that the OpenTelemetry project documents certain "[standard
    /// attributes]" that have prescribed semantic meanings and are available via
    /// the [opentelemetry_semantic_conventions] crate.
    ///
    /// [standard attributes]: https://github.com/open-telemetry/opentelemetry-specification/blob/v1.9.0/specification/trace/semantic_conventions/README.md
    /// [opentelemetry_semantic_conventions]: https://docs.rs/opentelemetry-semantic-conventions
    fn add_event<T>(&mut self, name: T, attributes: Vec<KeyValue>)
    where
        T: Into<Cow<'static, str>>,
    {
        self.add_event_with_timestamp(name, crate::time::now(), attributes)
    }

    /// Record an error as an event for this span.
    ///
    /// An additional call to [Span::set_status] is required if the status of the
    /// span should be set to error, as this method does not change the span status.
    ///
    /// If this span is not being recorded then this method does nothing.
    fn record_error(&mut self, err: &dyn Error) {
        if self.is_recording() {
            let attributes = vec![KeyValue::new("exception.message", err.to_string())];
            self.add_event("exception", attributes);
        }
    }

    /// Record an event with a timestamp in the context this span.
    ///
    /// Note that the OpenTelemetry project documents certain "[standard
    /// attributes]" that have prescribed semantic meanings and are available via
    /// the [opentelemetry_semantic_conventions] crate.
    ///
    /// [standard attributes]: https://github.com/open-telemetry/opentelemetry-specification/blob/v1.9.0/specification/trace/semantic_conventions/README.md
    /// [opentelemetry_semantic_conventions]: https://docs.rs/opentelemetry-semantic-conventions
    fn add_event_with_timestamp<T>(
        &mut self,
        name: T,
        timestamp: SystemTime,
        attributes: Vec<KeyValue>,
    ) where
        T: Into<Cow<'static, str>>;

    /// A reference to the [`SpanContext`] for this span.
    fn span_context(&self) -> &SpanContext;

    /// Returns `true` if this span is recording information.
    ///
    /// Spans will not be recording information after they have ended.
    ///
    /// This flag may be `true` despite the entire trace being sampled out. This
    /// allows recording and processing of information about the individual
    /// spans without sending it to the backend. An example of this scenario may
    /// be recording and processing of all incoming requests for the processing
    /// and building of SLA/SLO latency charts while sending only a subset -
    /// sampled spans - to the backend.
    fn is_recording(&self) -> bool;

    /// Set an attribute of this span.
    ///
    /// Setting an attribute with the same key as an existing attribute
    /// results in both being stored as attribute, without any de-duplication
    /// performed.
    ///
    /// Note that the OpenTelemetry project documents certain "[standard
    /// attributes]" that have prescribed semantic meanings and are available via
    /// the [opentelemetry_semantic_conventions] crate.
    ///
    /// [standard attributes]: https://github.com/open-telemetry/opentelemetry-specification/blob/v1.9.0/specification/trace/semantic_conventions/README.md
    /// [opentelemetry_semantic_conventions]: https://docs.rs/opentelemetry-semantic-conventions
    fn set_attribute(&mut self, attribute: KeyValue);

    /// Set multiple attributes of this span.
    ///
    /// Setting an attribute with the same key as an existing attribute
    /// results in both being stored as attribute, without any de-duplication
    /// performed.
    ///
    /// Note that the OpenTelemetry project documents certain "[standard
    /// attributes]" that have prescribed semantic meanings and are available via
    /// the [opentelemetry_semantic_conventions] crate.
    ///
    /// [standard attributes]: https://github.com/open-telemetry/opentelemetry-specification/blob/v1.9.0/specification/trace/semantic_conventions/README.md
    /// [opentelemetry_semantic_conventions]: https://docs.rs/opentelemetry-semantic-conventions
    fn set_attributes(&mut self, attributes: impl IntoIterator<Item = KeyValue>) {
        if self.is_recording() {
            for attr in attributes.into_iter() {
                self.set_attribute(attr);
            }
        }
    }

    /// Sets the status of this `Span`.
    ///
    /// If used, this will override the default span status, which is [`Status::Unset`].
    fn set_status(&mut self, status: Status);

    /// Updates the span's name.
    ///
    /// After this update, any sampling behavior based on the name will depend on
    /// the implementation.
    fn update_name<T>(&mut self, new_name: T)
    where
        T: Into<Cow<'static, str>>;

    /// Adds [`Link`] to another [`SpanContext`].
    ///
    /// This method allows linking the current span to another span, identified by its `SpanContext`. Links can be used
    /// to connect spans from different traces or within the same trace. Attributes can be attached to the link to
    /// provide additional context or metadata.
    ///
    /// # Arguments
    ///
    /// * `span_context` - The `SpanContext` of the span to link to. This represents the target span's unique identifiers
    ///   and trace information.
    /// * `attributes` - A vector of `KeyValue` pairs that describe additional attributes of the link. These attributes
    ///   can include any contextual information relevant to the link between the spans.
    ///
    /// [`Link`]: crate::trace::Link
    fn add_link(&mut self, span_context: SpanContext, attributes: Vec<KeyValue>);

    /// Signals that the operation described by this span has now ended.
    fn end(&mut self) {
        self.end_with_timestamp(crate::time::now());
    }

    /// Signals that the operation described by this span ended at the given time.
    fn end_with_timestamp(&mut self, timestamp: SystemTime);
}

/// `SpanKind` describes the relationship between the [`Span`], its parents, and
/// its children in a trace.
///
/// `SpanKind` describes two independent properties that benefit tracing systems
/// during analysis:
///
/// The first property described by `SpanKind` reflects whether the span is a
/// "logical" remote child or parent. By "logical", we mean that the span is
/// logically a remote child or parent, from the point of view of the library
/// that is being instrumented. Spans with a remote parent are interesting
/// because they are sources of external load. Spans with a remote child are
/// interesting because they reflect a non-local system dependency.
///
/// The second property described by `SpanKind` reflects whether a child span
/// represents a synchronous call.  When a child span is synchronous, the parent
/// is expected to wait for it to complete under ordinary circumstances. It can
/// be useful for tracing systems to know this property, since synchronous spans
/// may contribute to the overall trace latency. Asynchronous scenarios can be
/// remote or local.
///
/// In order for `SpanKind` to be meaningful, callers should arrange that a
/// single span does not serve more than one purpose. For example, a server-side
/// span should not be used directly as the parent of another remote span. As a
/// simple guideline, instrumentation should create a new span prior to
/// extracting and serializing the SpanContext for a remote call.
///
/// Note: there are complex scenarios where a `SpanKind::Client` span may have a
/// child that is also logically a `SpanKind::Client` span, or a
/// `SpanKind::Producer` span might have a local child that is a
/// `SpanKind::Client` span, depending on how the various libraries that are
/// providing the functionality are built and instrumented. These scenarios,
/// when they occur, should be detailed in the semantic conventions appropriate
/// to the relevant libraries.
///
/// To summarize the interpretation of these kinds:
///
/// | `SpanKind` | Synchronous | Asynchronous | Remote Incoming | Remote Outgoing |
/// |---|---|---|---|---|
/// | `Client` | yes | | | yes |
/// | `Server` | yes | | yes | |
/// | `Producer` | | yes | | maybe |
/// | `Consumer` | | yes | maybe | |
/// | `Internal` | | | | |
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpanKind {
    /// Indicates that the span describes a request to some remote service. This
    /// span is usually the parent of a remote `SpanKind::Server` span and does
    /// not end until the response is received.
    Client,

    /// Indicates that the span covers server-side handling of a synchronous RPC
    /// or other remote request. This span is often the child of a remote
    /// `SpanKind::Client` span that was expected to wait for a response.
    Server,

    /// Indicates that the span describes the initiators of an asynchronous
    /// request. This parent span will often end before the corresponding child
    /// `SpanKind::Consumer` span, possibly even before the child span starts.
    ///
    /// In messaging scenarios with batching, tracing individual messages
    /// requires a new `SpanKind::Producer` span per message to be created.
    Producer,

    /// Indicates that the span describes a child of an asynchronous
    /// `SpanKind::Producer` request.
    Consumer,

    /// Default value.
    ///
    /// Indicates that the span represents an internal operation within an
    /// application, as opposed to an operations with remote parents or
    /// children.
    Internal,
}

/// The status of a [`Span`].
///
/// These values form a total order: Ok > Error > Unset. This means that setting
/// `Status::Ok` will override any prior or future attempts to set a status with
/// `Status::Error` or `Status::Unset`.
///
/// The status should remain unset, except for the following circumstances:
///
/// Generally, instrumentation libraries should not set the code to
/// `Status::Ok`, unless explicitly configured to do so. Instrumentation
/// libraries should leave the status code as unset unless there is an error.
///
/// Application developers and operators may set the status code to
/// `Status::Ok`.
///
/// When span status is set to `Status::Ok` it should be considered final and
/// any further attempts to change it should be ignored.
///
/// Analysis tools should respond to a `Status::Ok` status by suppressing any
/// errors they would otherwise generate. For example, to suppress noisy errors
/// such as 404s.
///
/// Only the value of the last call will be recorded, and implementations are
/// free to ignore previous calls.
#[derive(Default, Debug, Clone, PartialEq, Eq, PartialOrd)]
pub enum Status {
    /// The default status.
    #[default]
    Unset,

    /// The operation contains an error.
    Error {
        /// The description of the error
        description: Cow<'static, str>,
    },

    /// The operation has been validated by an application developer or operator to
    /// have completed successfully.
    Ok,
}

impl Status {
    /// Create a new error status with a given description.
    ///
    /// # Examples
    ///
    /// ```
    /// use opentelemetry::trace::Status;
    ///
    /// // record error with `str` description
    /// let error_status = Status::error("something went wrong");
    ///
    /// // or with `String` description
    /// let error_status = Status::error(format!("too many foos: {}", 42));
    /// # drop(error_status);
    /// ```
    pub fn error(description: impl Into<Cow<'static, str>>) -> Self {
        Status::Error {
            description: description.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_order() {
        assert!(Status::Ok > Status::error(""));
        assert!(Status::error("") > Status::Unset);
    }
}
