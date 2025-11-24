#[cfg(all(feature = "gen-tonic-messages", feature = "zpages"))]
mod tonic {
    use opentelemetry::trace::{Event, Status};
    use opentelemetry_sdk::trace::SpanData;

    use crate::proto::tonic::{
        trace::v1::{span::Event as SpanEvent, Status as SpanStatus},
        tracez::v1::{ErrorData, LatencyData, RunningData},
    };
    use crate::transform::common::{to_nanos, tonic::Attributes};

    impl From<SpanData> for LatencyData {
        fn from(span_data: SpanData) -> Self {
            LatencyData {
                traceid: span_data.span_context.trace_id().to_bytes().to_vec(),
                spanid: span_data.span_context.span_id().to_bytes().to_vec(),
                parentid: span_data.parent_span_id.to_bytes().to_vec(),
                starttime: to_nanos(span_data.start_time),
                endtime: to_nanos(span_data.end_time),
                attributes: Attributes::from(span_data.attributes).0,
                events: span_data.events.iter().cloned().map(Into::into).collect(),
                links: span_data.links.iter().cloned().map(Into::into).collect(),
            }
        }
    }

    impl From<SpanData> for ErrorData {
        fn from(span_data: SpanData) -> Self {
            ErrorData {
                traceid: span_data.span_context.trace_id().to_bytes().to_vec(),
                spanid: span_data.span_context.span_id().to_bytes().to_vec(),
                parentid: span_data.parent_span_id.to_bytes().to_vec(),
                starttime: to_nanos(span_data.start_time),
                attributes: Attributes::from(span_data.attributes).0,
                events: span_data.events.iter().cloned().map(Into::into).collect(),
                links: span_data.links.iter().cloned().map(Into::into).collect(),
                status: match span_data.status {
                    Status::Error { description } => Some(SpanStatus {
                        message: description.to_string(),
                        code: 2,
                    }),
                    _ => None,
                },
            }
        }
    }

    impl From<SpanData> for RunningData {
        fn from(span_data: SpanData) -> Self {
            RunningData {
                traceid: span_data.span_context.trace_id().to_bytes().to_vec(),
                spanid: span_data.span_context.span_id().to_bytes().to_vec(),
                parentid: span_data.parent_span_id.to_bytes().to_vec(),
                starttime: to_nanos(span_data.start_time),
                attributes: Attributes::from(span_data.attributes).0,
                events: span_data.events.iter().cloned().map(Into::into).collect(),
                links: span_data.links.iter().cloned().map(Into::into).collect(),
            }
        }
    }

    impl From<Event> for SpanEvent {
        fn from(event: Event) -> Self {
            SpanEvent {
                time_unix_nano: to_nanos(event.timestamp),
                name: event.name.to_string(),
                attributes: Attributes::from(event.attributes).0,
                dropped_attributes_count: event.dropped_attributes_count,
            }
        }
    }
}
