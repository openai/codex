#[cfg(feature = "gen-tonic-messages")]
pub mod tonic {
    use crate::proto::tonic::resource::v1::Resource;
    use crate::proto::tonic::trace::v1::{span, status, ResourceSpans, ScopeSpans, Span, Status};
    use crate::transform::common::{
        to_nanos,
        tonic::{Attributes, ResourceAttributesWithSchema},
    };
    use opentelemetry::trace;
    use opentelemetry::trace::{Link, SpanId, SpanKind};
    use opentelemetry_sdk::trace::SpanData;
    use std::collections::HashMap;

    impl From<SpanKind> for span::SpanKind {
        fn from(span_kind: SpanKind) -> Self {
            match span_kind {
                SpanKind::Client => span::SpanKind::Client,
                SpanKind::Consumer => span::SpanKind::Consumer,
                SpanKind::Internal => span::SpanKind::Internal,
                SpanKind::Producer => span::SpanKind::Producer,
                SpanKind::Server => span::SpanKind::Server,
            }
        }
    }

    impl From<&trace::Status> for status::StatusCode {
        fn from(status: &trace::Status) -> Self {
            match status {
                trace::Status::Ok => status::StatusCode::Ok,
                trace::Status::Unset => status::StatusCode::Unset,
                trace::Status::Error { .. } => status::StatusCode::Error,
            }
        }
    }

    impl From<Link> for span::Link {
        fn from(link: Link) -> Self {
            span::Link {
                trace_id: link.span_context.trace_id().to_bytes().to_vec(),
                span_id: link.span_context.span_id().to_bytes().to_vec(),
                trace_state: link.span_context.trace_state().header(),
                attributes: Attributes::from(link.attributes).0,
                dropped_attributes_count: link.dropped_attributes_count,
                flags: link.span_context.trace_flags().to_u8() as u32,
            }
        }
    }
    impl From<opentelemetry_sdk::trace::SpanData> for Span {
        fn from(source_span: opentelemetry_sdk::trace::SpanData) -> Self {
            let span_kind: span::SpanKind = source_span.span_kind.into();
            Span {
                trace_id: source_span.span_context.trace_id().to_bytes().to_vec(),
                span_id: source_span.span_context.span_id().to_bytes().to_vec(),
                trace_state: source_span.span_context.trace_state().header(),
                parent_span_id: {
                    if source_span.parent_span_id != SpanId::INVALID {
                        source_span.parent_span_id.to_bytes().to_vec()
                    } else {
                        vec![]
                    }
                },
                flags: source_span.span_context.trace_flags().to_u8() as u32,
                name: source_span.name.into_owned(),
                kind: span_kind as i32,
                start_time_unix_nano: to_nanos(source_span.start_time),
                end_time_unix_nano: to_nanos(source_span.end_time),
                dropped_attributes_count: source_span.dropped_attributes_count,
                attributes: Attributes::from(source_span.attributes).0,
                dropped_events_count: source_span.events.dropped_count,
                events: source_span
                    .events
                    .into_iter()
                    .map(|event| span::Event {
                        time_unix_nano: to_nanos(event.timestamp),
                        name: event.name.into(),
                        attributes: Attributes::from(event.attributes).0,
                        dropped_attributes_count: event.dropped_attributes_count,
                    })
                    .collect(),
                dropped_links_count: source_span.links.dropped_count,
                links: source_span.links.into_iter().map(Into::into).collect(),
                status: Some(Status {
                    code: status::StatusCode::from(&source_span.status).into(),
                    message: match source_span.status {
                        trace::Status::Error { description } => description.to_string(),
                        _ => Default::default(),
                    },
                }),
            }
        }
    }

    impl ResourceSpans {
        pub fn new(source_span: SpanData, resource: &ResourceAttributesWithSchema) -> Self {
            let span_kind: span::SpanKind = source_span.span_kind.into();
            ResourceSpans {
                resource: Some(Resource {
                    attributes: resource.attributes.0.clone(),
                    dropped_attributes_count: 0,
                    entity_refs: vec![],
                }),
                schema_url: resource.schema_url.clone().unwrap_or_default(),
                scope_spans: vec![ScopeSpans {
                    schema_url: source_span
                        .instrumentation_scope
                        .schema_url()
                        .map(ToOwned::to_owned)
                        .unwrap_or_default(),
                    scope: Some((source_span.instrumentation_scope, None).into()),
                    spans: vec![Span {
                        trace_id: source_span.span_context.trace_id().to_bytes().to_vec(),
                        span_id: source_span.span_context.span_id().to_bytes().to_vec(),
                        trace_state: source_span.span_context.trace_state().header(),
                        parent_span_id: {
                            if source_span.parent_span_id != SpanId::INVALID {
                                source_span.parent_span_id.to_bytes().to_vec()
                            } else {
                                vec![]
                            }
                        },
                        flags: source_span.span_context.trace_flags().to_u8() as u32,
                        name: source_span.name.into_owned(),
                        kind: span_kind as i32,
                        start_time_unix_nano: to_nanos(source_span.start_time),
                        end_time_unix_nano: to_nanos(source_span.end_time),
                        dropped_attributes_count: source_span.dropped_attributes_count,
                        attributes: Attributes::from(source_span.attributes).0,
                        dropped_events_count: source_span.events.dropped_count,
                        events: source_span
                            .events
                            .into_iter()
                            .map(|event| span::Event {
                                time_unix_nano: to_nanos(event.timestamp),
                                name: event.name.into(),
                                attributes: Attributes::from(event.attributes).0,
                                dropped_attributes_count: event.dropped_attributes_count,
                            })
                            .collect(),
                        dropped_links_count: source_span.links.dropped_count,
                        links: source_span.links.into_iter().map(Into::into).collect(),
                        status: Some(Status {
                            code: status::StatusCode::from(&source_span.status).into(),
                            message: match source_span.status {
                                trace::Status::Error { description } => description.to_string(),
                                _ => Default::default(),
                            },
                        }),
                    }],
                }],
            }
        }
    }

    pub fn group_spans_by_resource_and_scope(
        spans: Vec<SpanData>,
        resource: &ResourceAttributesWithSchema,
    ) -> Vec<ResourceSpans> {
        // Group spans by their instrumentation scope
        let scope_map = spans.iter().fold(
            HashMap::new(),
            |mut scope_map: HashMap<&opentelemetry::InstrumentationScope, Vec<&SpanData>>, span| {
                let instrumentation = &span.instrumentation_scope;
                scope_map.entry(instrumentation).or_default().push(span);
                scope_map
            },
        );

        // Convert the grouped spans into ScopeSpans
        let scope_spans = scope_map
            .into_iter()
            .map(|(instrumentation, span_records)| ScopeSpans {
                scope: Some((instrumentation, None).into()),
                schema_url: resource.schema_url.clone().unwrap_or_default(),
                spans: span_records
                    .into_iter()
                    .map(|span_data| span_data.clone().into())
                    .collect(),
            })
            .collect();

        // Wrap ScopeSpans into a single ResourceSpans
        vec![ResourceSpans {
            resource: Some(Resource {
                attributes: resource.attributes.0.clone(),
                dropped_attributes_count: 0,
                entity_refs: vec![],
            }),
            scope_spans,
            schema_url: resource.schema_url.clone().unwrap_or_default(),
        }]
    }
}

#[cfg(test)]
mod tests {
    use crate::tonic::common::v1::any_value::Value;
    use crate::transform::common::tonic::ResourceAttributesWithSchema;
    use opentelemetry::time::now;
    use opentelemetry::trace::{
        SpanContext, SpanId, SpanKind, Status, TraceFlags, TraceId, TraceState,
    };
    use opentelemetry::InstrumentationScope;
    use opentelemetry::KeyValue;
    use opentelemetry_sdk::resource::Resource;
    use opentelemetry_sdk::trace::SpanData;
    use opentelemetry_sdk::trace::{SpanEvents, SpanLinks};
    use std::borrow::Cow;
    use std::time::Duration;

    fn create_test_span_data(instrumentation_name: &'static str) -> SpanData {
        let span_context = SpanContext::new(
            TraceId::from_u128(123),
            SpanId::from_u64(456),
            TraceFlags::default(),
            false,
            TraceState::default(),
        );

        SpanData {
            span_context,
            parent_span_id: SpanId::from_u64(0),
            span_kind: SpanKind::Internal,
            name: Cow::Borrowed("test_span"),
            start_time: now(),
            end_time: now() + Duration::from_secs(1),
            attributes: vec![KeyValue::new("key", "value")],
            dropped_attributes_count: 0,
            events: SpanEvents::default(),
            links: SpanLinks::default(),
            status: Status::Unset,
            instrumentation_scope: InstrumentationScope::builder(instrumentation_name).build(),
        }
    }

    #[test]
    fn test_group_spans_by_resource_and_scope_single_scope() {
        let resource = Resource::builder_empty()
            .with_attribute(KeyValue::new("resource_key", "resource_value"))
            .build();
        let span_data = create_test_span_data("lib1");

        let spans = vec![span_data.clone()];
        let resource: ResourceAttributesWithSchema = (&resource).into(); // Convert Resource to ResourceAttributesWithSchema

        let grouped_spans =
            crate::transform::trace::tonic::group_spans_by_resource_and_scope(spans, &resource);

        assert_eq!(grouped_spans.len(), 1);

        let resource_spans = &grouped_spans[0];
        assert_eq!(
            resource_spans.resource.as_ref().unwrap().attributes.len(),
            1
        );
        assert_eq!(
            resource_spans.resource.as_ref().unwrap().attributes[0].key,
            "resource_key"
        );
        assert_eq!(
            resource_spans.resource.as_ref().unwrap().attributes[0]
                .value
                .clone()
                .unwrap()
                .value
                .unwrap(),
            Value::StringValue("resource_value".to_string())
        );

        let scope_spans = &resource_spans.scope_spans;
        assert_eq!(scope_spans.len(), 1);

        let scope_span = &scope_spans[0];
        assert_eq!(scope_span.scope.as_ref().unwrap().name, "lib1");
        assert_eq!(scope_span.spans.len(), 1);

        assert_eq!(
            scope_span.spans[0].trace_id,
            span_data.span_context.trace_id().to_bytes().to_vec()
        );
    }

    #[test]
    fn test_group_spans_by_resource_and_scope_multiple_scopes() {
        let resource = Resource::builder_empty()
            .with_attribute(KeyValue::new("resource_key", "resource_value"))
            .build();
        let span_data1 = create_test_span_data("lib1");
        let span_data2 = create_test_span_data("lib1");
        let span_data3 = create_test_span_data("lib2");

        let spans = vec![span_data1.clone(), span_data2.clone(), span_data3.clone()];
        let resource: ResourceAttributesWithSchema = (&resource).into(); // Convert Resource to ResourceAttributesWithSchema

        let grouped_spans =
            crate::transform::trace::tonic::group_spans_by_resource_and_scope(spans, &resource);

        assert_eq!(grouped_spans.len(), 1);

        let resource_spans = &grouped_spans[0];
        assert_eq!(
            resource_spans.resource.as_ref().unwrap().attributes.len(),
            1
        );
        assert_eq!(
            resource_spans.resource.as_ref().unwrap().attributes[0].key,
            "resource_key"
        );
        assert_eq!(
            resource_spans.resource.as_ref().unwrap().attributes[0]
                .value
                .clone()
                .unwrap()
                .value
                .unwrap(),
            Value::StringValue("resource_value".to_string())
        );

        let scope_spans = &resource_spans.scope_spans;
        assert_eq!(scope_spans.len(), 2);

        // Check the scope spans for both lib1 and lib2
        let mut lib1_scope_span = None;
        let mut lib2_scope_span = None;

        for scope_span in scope_spans {
            match scope_span.scope.as_ref().unwrap().name.as_str() {
                "lib1" => lib1_scope_span = Some(scope_span),
                "lib2" => lib2_scope_span = Some(scope_span),
                _ => {}
            }
        }

        let lib1_scope_span = lib1_scope_span.expect("lib1 scope span not found");
        let lib2_scope_span = lib2_scope_span.expect("lib2 scope span not found");

        assert_eq!(lib1_scope_span.scope.as_ref().unwrap().name, "lib1");
        assert_eq!(lib2_scope_span.scope.as_ref().unwrap().name, "lib2");

        assert_eq!(lib1_scope_span.spans.len(), 2);
        assert_eq!(lib2_scope_span.spans.len(), 1);

        assert_eq!(
            lib1_scope_span.spans[0].trace_id,
            span_data1.span_context.trace_id().to_bytes().to_vec()
        );
        assert_eq!(
            lib1_scope_span.spans[1].trace_id,
            span_data2.span_context.trace_id().to_bytes().to_vec()
        );
        assert_eq!(
            lib2_scope_span.spans[0].trace_id,
            span_data3.span_context.trace_id().to_bytes().to_vec()
        );
    }
}
