#[cfg(all(feature = "with-serde", feature = "gen-tonic-messages"))]
mod json_serde {
    #[cfg(feature = "logs")]
    use opentelemetry_proto::tonic::collector::logs::v1::ExportLogsServiceRequest;
    #[cfg(feature = "metrics")]
    use opentelemetry_proto::tonic::collector::metrics::v1::ExportMetricsServiceRequest;
    #[cfg(feature = "trace")]
    use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
    use opentelemetry_proto::tonic::common::v1::any_value::Value;
    use opentelemetry_proto::tonic::common::v1::{
        AnyValue, ArrayValue, InstrumentationScope, KeyValue, KeyValueList,
    };
    #[cfg(feature = "logs")]
    use opentelemetry_proto::tonic::logs::v1::{LogRecord, ResourceLogs, ScopeLogs};
    #[cfg(feature = "metrics")]
    use opentelemetry_proto::tonic::metrics::v1::{
        metric::Data, number_data_point::Value as MetricValue, Gauge, Histogram,
        HistogramDataPoint, Metric, NumberDataPoint, ResourceMetrics, ScopeMetrics, Sum,
    };
    use opentelemetry_proto::tonic::resource::v1::Resource;
    #[cfg(feature = "trace")]
    use opentelemetry_proto::tonic::trace::v1::{
        span::{Event, Link},
        ResourceSpans, ScopeSpans, Span, Status,
    };

    #[cfg(feature = "trace")]
    mod export_trace_service_request {
        use super::*;

        // `ExportTraceServiceRequest` from the OpenTelemetry proto examples
        // see <https://github.com/open-telemetry/opentelemetry-proto/blob/v1.3.2/examples/trace.json>
        mod example {
            use super::*;

            fn value() -> ExportTraceServiceRequest {
                ExportTraceServiceRequest {
                    resource_spans: vec![ResourceSpans {
                        resource: Some(Resource {
                            attributes: vec![KeyValue {
                                key: String::from("service.name"),
                                value: Some(AnyValue {
                                    value: Some(Value::StringValue(String::from("my.service"))),
                                }),
                            }],
                            dropped_attributes_count: 0,
                            entity_refs: vec![],
                        }),
                        scope_spans: vec![ScopeSpans {
                            scope: Some(InstrumentationScope {
                                name: String::from("my.library"),
                                version: String::from("1.0.0"),
                                attributes: vec![KeyValue {
                                    key: String::from("my.scope.attribute"),
                                    value: Some(AnyValue {
                                        value: Some(Value::StringValue(String::from(
                                            "some scope attribute",
                                        ))),
                                    }),
                                }],
                                dropped_attributes_count: 0,
                            }),
                            spans: vec![Span {
                                trace_id: hex::decode("5b8efff798038103d269b633813fc60c").unwrap(),
                                span_id: hex::decode("eee19b7ec3c1b174").unwrap(),
                                trace_state: String::new(),
                                parent_span_id: hex::decode("eee19b7ec3c1b173").unwrap(),
                                flags: 0,
                                name: String::from("I'm a server span"),
                                kind: 2,
                                start_time_unix_nano: 1544712660000000000,
                                end_time_unix_nano: 1544712661000000000,
                                attributes: vec![KeyValue {
                                    key: String::from("my.span.attr"),
                                    value: Some(AnyValue {
                                        value: Some(Value::StringValue(String::from("some value"))),
                                    }),
                                }],
                                dropped_attributes_count: 0,
                                events: vec![],
                                dropped_events_count: 0,
                                links: vec![],
                                dropped_links_count: 0,
                                status: None,
                            }],
                            schema_url: String::new(),
                        }],
                        schema_url: String::new(),
                    }],
                }
            }

            // language=json
            const CANONICAL: &str = r#"{
  "resourceSpans": [
    {
      "resource": {
        "attributes": [
          {
            "key": "service.name",
            "value": {
              "stringValue": "my.service"
            }
          }
        ],
        "droppedAttributesCount": 0
      },
      "scopeSpans": [
        {
          "scope": {
            "name": "my.library",
            "version": "1.0.0",
            "attributes": [
              {
                "key": "my.scope.attribute",
                "value": {
                  "stringValue": "some scope attribute"
                }
              }
            ],
            "droppedAttributesCount": 0
          },
          "spans": [
            {
              "traceId": "5b8efff798038103d269b633813fc60c",
              "spanId": "eee19b7ec3c1b174",
              "traceState": "",
              "parentSpanId": "eee19b7ec3c1b173",
              "flags": 0,
              "name": "I'm a server span",
              "kind": 2,
              "startTimeUnixNano": "1544712660000000000",
              "endTimeUnixNano": "1544712661000000000",
              "attributes": [
                {
                  "key": "my.span.attr",
                  "value": {
                    "stringValue": "some value"
                  }
                }
              ],
              "droppedAttributesCount": 0,
              "events": [],
              "droppedEventsCount": 0,
              "links": [],
              "droppedLinksCount": 0,
              "status": null
            }
          ],
          "schemaUrl": ""
        }
      ],
      "schemaUrl": ""
    }
  ]
}"#;

            // copied from the example json file
            // language=json
            const ALTERNATIVE: &str = r#"{
  "resourceSpans": [
    {
      "resource": {
        "attributes": [
          {
            "key": "service.name",
            "value": {
              "stringValue": "my.service"
            }
          }
        ]
      },
      "scopeSpans": [
        {
          "scope": {
            "name": "my.library",
            "version": "1.0.0",
            "attributes": [
              {
                "key": "my.scope.attribute",
                "value": {
                  "stringValue": "some scope attribute"
                }
              }
            ]
          },
          "spans": [
            {
              "traceId": "5B8EFFF798038103D269B633813FC60C",
              "spanId": "EEE19B7EC3C1B174",
              "parentSpanId": "EEE19B7EC3C1B173",
              "name": "I'm a server span",
              "startTimeUnixNano": "1544712660000000000",
              "endTimeUnixNano": "1544712661000000000",
              "kind": 2,
              "attributes": [
                {
                  "key": "my.span.attr",
                  "value": {
                    "stringValue": "some value"
                  }
                }
              ]
            }
          ]
        }
      ]
    }
  ]
}
"#;

            #[test]
            fn serialize() {
                let input: ExportTraceServiceRequest = value();
                let actual =
                    serde_json::to_string_pretty(&input).expect("serialization must succeed");
                assert_eq!(actual, CANONICAL);
            }

            #[test]
            fn deserialize_canonical() {
                let actual: ExportTraceServiceRequest =
                    serde_json::from_str(CANONICAL).expect("deserialization must succeed");
                let expected: ExportTraceServiceRequest = value();
                assert_eq!(actual, expected);
            }

            #[test]
            fn deserialize_alternative() {
                let actual: ExportTraceServiceRequest =
                    serde_json::from_str(ALTERNATIVE).expect("deserialization must succeed");
                let expected: ExportTraceServiceRequest = value();
                assert_eq!(actual, expected);
            }
        }

        // request where all fields are set
        mod complete {
            use super::*;

            fn value() -> ExportTraceServiceRequest {
                ExportTraceServiceRequest {
                    resource_spans: vec![ResourceSpans {
                        resource: Some(Resource {
                            attributes: vec![KeyValue {
                                key: String::from("service.name"),
                                value: Some(AnyValue {
                                    value: Some(Value::StringValue(String::from("my.service"))),
                                }),
                            }],
                            dropped_attributes_count: 1,
                            entity_refs: vec![],
                        }),
                        scope_spans: vec![ScopeSpans {
                            scope: Some(InstrumentationScope {
                                name: String::from("my.library"),
                                version: String::from("1.0.0"),
                                attributes: vec![KeyValue {
                                    key: String::from("my.scope.attribute"),
                                    value: Some(AnyValue {
                                        value: Some(Value::StringValue(String::from(
                                            "some scope attribute",
                                        ))),
                                    }),
                                }],
                                dropped_attributes_count: 1,
                            }),
                            spans: vec![Span {
                                trace_id: hex::decode("5b8efff798038103d269b633813fc60c").unwrap(),
                                span_id: hex::decode("eee19b7ec3c1b174").unwrap(),
                                trace_state: String::from("browser=firefox,os=linux"),
                                parent_span_id: hex::decode("eee19b7ec3c1b173").unwrap(),
                                flags: 1,
                                name: String::from("I'm a server span"),
                                kind: 2,
                                start_time_unix_nano: 1544712660000000000,
                                end_time_unix_nano: 1544712661000000000,
                                attributes: vec![
                                    KeyValue {
                                        key: String::from("my.span.attr"),
                                        value: Some(AnyValue {
                                            value: Some(Value::StringValue(String::from(
                                                "some value",
                                            ))),
                                        }),
                                    },
                                    KeyValue {
                                        key: String::from("my.span.bytes.attr"),
                                        value: Some(AnyValue {
                                            value: Some(Value::BytesValue(vec![0x80, 0x80, 0x80])),
                                        }),
                                    },
                                ],
                                dropped_attributes_count: 1,
                                events: vec![Event {
                                    time_unix_nano: 1544712660500000000,
                                    name: String::from("user.created"),
                                    attributes: vec![KeyValue {
                                        key: String::from("my.event.attr"),
                                        value: Some(AnyValue {
                                            value: Some(Value::StringValue(String::from(
                                                "snowman",
                                            ))),
                                        }),
                                    }],
                                    dropped_attributes_count: 1,
                                }],
                                dropped_events_count: 1,
                                links: vec![Link {
                                    trace_id: hex::decode("5b8efff798038103d269b633813fc60b")
                                        .unwrap(),
                                    span_id: hex::decode("eee19b7ec3c1b172").unwrap(),
                                    trace_state: String::from("food=pizza,color=red"),
                                    attributes: vec![KeyValue {
                                        key: String::from("my.link.attr"),
                                        value: Some(AnyValue {
                                            value: Some(Value::StringValue(String::from("rust"))),
                                        }),
                                    }],
                                    dropped_attributes_count: 1,
                                    flags: 0x0200,
                                }],
                                dropped_links_count: 1,
                                status: Some(Status {
                                    message: String::from("service temporarily unavailable"),
                                    code: 2,
                                }),
                            }],
                            schema_url: String::from("https://opentelemetry.io/schemas/1.24.0"),
                        }],
                        schema_url: String::from("https://opentelemetry.io/schemas/1.24.0"),
                    }],
                }
            }

            // language=json
            const CANONICAL: &str = r#"{
  "resourceSpans": [
    {
      "resource": {
        "attributes": [
          {
            "key": "service.name",
            "value": {
              "stringValue": "my.service"
            }
          }
        ],
        "droppedAttributesCount": 1
      },
      "scopeSpans": [
        {
          "scope": {
            "name": "my.library",
            "version": "1.0.0",
            "attributes": [
              {
                "key": "my.scope.attribute",
                "value": {
                  "stringValue": "some scope attribute"
                }
              }
            ],
            "droppedAttributesCount": 1
          },
          "spans": [
            {
              "traceId": "5b8efff798038103d269b633813fc60c",
              "spanId": "eee19b7ec3c1b174",
              "traceState": "browser=firefox,os=linux",
              "parentSpanId": "eee19b7ec3c1b173",
              "flags": 1,
              "name": "I'm a server span",
              "kind": 2,
              "startTimeUnixNano": "1544712660000000000",
              "endTimeUnixNano": "1544712661000000000",
              "attributes": [
                {
                  "key": "my.span.attr",
                  "value": {
                    "stringValue": "some value"
                  }
                },
                {
                  "key": "my.span.bytes.attr",
                  "value": {
                    "bytesValue": "gICA"
                  }
                }
              ],
              "droppedAttributesCount": 1,
              "events": [
                {
                  "timeUnixNano": "1544712660500000000",
                  "name": "user.created",
                  "attributes": [
                    {
                      "key": "my.event.attr",
                      "value": {
                        "stringValue": "snowman"
                      }
                    }
                  ],
                  "droppedAttributesCount": 1
                }
              ],
              "droppedEventsCount": 1,
              "links": [
                {
                  "traceId": "5b8efff798038103d269b633813fc60b",
                  "spanId": "eee19b7ec3c1b172",
                  "traceState": "food=pizza,color=red",
                  "attributes": [
                    {
                      "key": "my.link.attr",
                      "value": {
                        "stringValue": "rust"
                      }
                    }
                  ],
                  "droppedAttributesCount": 1,
                  "flags": 512
                }
              ],
              "droppedLinksCount": 1,
              "status": {
                "message": "service temporarily unavailable",
                "code": 2
              }
            }
          ],
          "schemaUrl": "https://opentelemetry.io/schemas/1.24.0"
        }
      ],
      "schemaUrl": "https://opentelemetry.io/schemas/1.24.0"
    }
  ]
}"#;

            #[test]
            fn serialize() {
                let input: ExportTraceServiceRequest = value();
                let actual =
                    serde_json::to_string_pretty(&input).expect("serialization must succeed");
                assert_eq!(actual, CANONICAL);
            }

            #[test]
            fn deserialize_canonical() {
                let actual: ExportTraceServiceRequest =
                    serde_json::from_str(CANONICAL).expect("deserialization must succeed");
                let expected: ExportTraceServiceRequest = value();
                assert_eq!(actual, expected);
            }
        }
    }

    mod value {
        use super::*;

        #[test]
        fn string() {
            let value = Value::StringValue(String::from("my.service"));
            // language=json
            let json = r#"{"stringValue":"my.service"}"#;
            assert_eq!(
                serde_json::to_string(&value).expect("serialization succeeds"),
                json
            );
            assert_eq!(
                serde_json::from_str::<Value>(json).expect("deserialization succeeds"),
                value
            );
        }

        #[test]
        fn bool() {
            let value = Value::BoolValue(true);
            // language=json
            let json = r#"{"boolValue":true}"#;
            assert_eq!(
                serde_json::to_string(&value).expect("serialization succeeds"),
                json
            );
            assert_eq!(
                serde_json::from_str::<Value>(json).expect("deserialization succeeds"),
                value
            );
        }

        #[test]
        fn int() {
            let value = Value::IntValue(123);
            // language=json
            let json = r#"{"intValue":123}"#;
            assert_eq!(
                serde_json::to_string(&value).expect("serialization succeeds"),
                json
            );
            assert_eq!(
                serde_json::from_str::<Value>(json).expect("deserialization succeeds"),
                value
            );
        }

        #[test]
        fn array_empty() {
            let value = Value::ArrayValue(ArrayValue { values: vec![] });
            // language=json
            let json = r#"{"arrayValue":{"values":[]}}"#;
            assert_eq!(
                serde_json::to_string(&value).expect("serialization succeeds"),
                json
            );
            assert_eq!(
                serde_json::from_str::<Value>(json).expect("deserialization succeeds"),
                value
            );
        }

        #[test]
        fn array_strings() {
            let value = Value::ArrayValue(ArrayValue {
                values: vec![
                    AnyValue {
                        value: Some(Value::StringValue(String::from("foo"))),
                    },
                    AnyValue {
                        value: Some(Value::StringValue(String::from("bar"))),
                    },
                ],
            });
            // language=json
            let json = r#"{
  "arrayValue": {
    "values": [
      {
        "stringValue": "foo"
      },
      {
        "stringValue": "bar"
      }
    ]
  }
}"#;
            assert_eq!(
                serde_json::to_string_pretty(&value).expect("serialization succeeds"),
                json
            );
            assert_eq!(
                serde_json::from_str::<Value>(json).expect("deserialization succeeds"),
                value
            );
        }

        #[test]
        fn array_mixed() {
            let value = Value::ArrayValue(ArrayValue {
                values: vec![
                    AnyValue {
                        value: Some(Value::StringValue(String::from("foo"))),
                    },
                    AnyValue {
                        value: Some(Value::IntValue(1337)),
                    },
                ],
            });
            // language=json
            let json = r#"{
  "arrayValue": {
    "values": [
      {
        "stringValue": "foo"
      },
      {
        "intValue": "1337"
      }
    ]
  }
}"#;
            assert_eq!(
                serde_json::to_string_pretty(&value).expect("serialization succeeds"),
                json
            );
            assert_eq!(
                serde_json::from_str::<Value>(json).expect("deserialization succeeds"),
                value
            );
        }

        #[test]
        fn map_single_string() {
            let value = Value::KvlistValue(KeyValueList {
                values: vec![KeyValue {
                    key: String::from("some.map.key"),
                    value: Some(AnyValue {
                        value: Some(Value::StringValue(String::from("some value"))),
                    }),
                }],
            });
            // language=json
            let json = r#"{
  "kvlistValue": {
    "values": [
      {
        "key": "some.map.key",
        "value": {
          "stringValue": "some value"
        }
      }
    ]
  }
}"#;
            assert_eq!(
                serde_json::to_string_pretty(&value).expect("serialization succeeds"),
                json
            );
            assert_eq!(
                serde_json::from_str::<Value>(json).expect("deserialization succeeds"),
                value
            );
        }
    }

    mod key_value {
        use super::*;

        mod string {
            use super::*;

            fn value() -> KeyValue {
                KeyValue {
                    key: String::from("service.name"),
                    value: Some(AnyValue {
                        value: Some(Value::StringValue(String::from("my.service"))),
                    }),
                }
            }

            // language=json
            const CANONICAL: &str = r#"{
  "key": "service.name",
  "value": {
    "stringValue": "my.service"
  }
}"#;

            #[test]
            fn serialize() {
                let input: KeyValue = value();
                let actual =
                    serde_json::to_string_pretty(&input).expect("serialization must succeed");
                assert_eq!(actual, CANONICAL);
            }

            #[test]
            fn deserialize_canonical() {
                let actual: KeyValue =
                    serde_json::from_str(CANONICAL).expect("deserialization must succeed");
                let expected: KeyValue = value();
                assert_eq!(actual, expected);
            }
        }

        mod int {
            use super::*;

            fn value() -> KeyValue {
                KeyValue {
                    key: String::from("service.id"),
                    value: Some(AnyValue {
                        value: Some(Value::IntValue(33)),
                    }),
                }
            }

            // language=json
            const CANONICAL: &str = r#"{
  "key": "service.id",
  "value": {
    "intValue": "33"
  }
}"#;

            // unquoted int value
            // language=json
            const ALTERNATIVE: &str = r#"{
  "key": "service.id",
  "value": {
    "intValue": 33
  }
}"#;

            #[test]
            fn serialize() {
                let input: KeyValue = value();
                let actual =
                    serde_json::to_string_pretty(&input).expect("serialization must succeed");
                assert_eq!(actual, CANONICAL);
            }

            #[test]
            fn deserialize_canonical() {
                let actual: KeyValue =
                    serde_json::from_str(CANONICAL).expect("deserialization must succeed");
                let expected: KeyValue = value();
                assert_eq!(actual, expected);
            }

            #[test]
            fn deserialize_alternative() {
                let actual: KeyValue =
                    serde_json::from_str(ALTERNATIVE).expect("deserialization must succeed");
                let expected: KeyValue = value();
                assert_eq!(actual, expected);
            }
        }
    }

    #[cfg(feature = "trace")]
    mod event {
        use super::*;

        mod simple {
            use super::*;

            fn value() -> Event {
                Event {
                    time_unix_nano: 1234567890,
                    name: String::from("my_event"),
                    attributes: vec![],
                    dropped_attributes_count: 0,
                }
            }

            // language=json
            const CANONICAL: &str = r#"{
  "timeUnixNano": "1234567890",
  "name": "my_event",
  "attributes": [],
  "droppedAttributesCount": 0
}"#;

            // language=json
            const ALTERNATIVE: &str = r#"{
  "name": "my_event",
  "timeUnixNano": "1234567890"
}"#;

            #[test]
            fn serialize() {
                let input: Event = value();
                let actual =
                    serde_json::to_string_pretty(&input).expect("serialization must succeed");
                assert_eq!(actual, CANONICAL);
            }

            #[test]
            fn deserialize_canonical() {
                let actual: Event =
                    serde_json::from_str(CANONICAL).expect("deserialization must succeed");
                let expected: Event = value();
                assert_eq!(actual, expected);
            }

            #[test]
            fn deserialize_alternative() {
                let actual: Event =
                    serde_json::from_str(ALTERNATIVE).expect("deserialization must succeed");
                let expected: Event = value();
                assert_eq!(actual, expected);
            }
        }
    }

    #[cfg(feature = "metrics")]
    mod export_metrics_service_request {
        use super::*;

        // `ExportTraceServiceRequest` from the OpenTelemetry proto examples
        // see <https://github.com/open-telemetry/opentelemetry-proto/blob/v1.3.2/examples/metrics.json>
        mod example {
            use super::*;

            fn value() -> ExportMetricsServiceRequest {
                ExportMetricsServiceRequest {
                    resource_metrics: vec![ResourceMetrics {
                        resource: Some(Resource {
                            attributes: vec![KeyValue {
                                key: String::from("service.name"),
                                value: Some(AnyValue {
                                    value: Some(Value::StringValue(String::from("my.service"))),
                                }),
                            }],
                            dropped_attributes_count: 0,
                            entity_refs: vec![],
                        }),
                        scope_metrics: vec![ScopeMetrics {
                            scope: Some(InstrumentationScope {
                                name: String::from("my.library"),
                                version: String::from("1.0.0"),
                                attributes: vec![KeyValue {
                                    key: String::from("my.scope.attribute"),
                                    value: Some(AnyValue {
                                        value: Some(Value::StringValue(String::from(
                                            "some scope attribute",
                                        ))),
                                    }),
                                }],
                                dropped_attributes_count: 0,
                            }),
                            metrics: vec![
                                Metric {
                                    name: String::from("my.counter"),
                                    description: String::from("I am a Counter"),
                                    unit: String::from("1"),
                                    metadata: vec![],
                                    data: Some(Data::Sum(Sum {
                                        data_points: vec![NumberDataPoint {
                                            attributes: vec![KeyValue {
                                                key: String::from("my.counter.attr"),
                                                value: Some(AnyValue {
                                                    value: Some(Value::StringValue(String::from(
                                                        "some value",
                                                    ))),
                                                }),
                                            }],
                                            start_time_unix_nano: 1544712660300000000,
                                            time_unix_nano: 1544712660300000000,
                                            exemplars: vec![],
                                            flags: 0,
                                            value: Some(MetricValue::AsDouble(5.0)),
                                        }],
                                        aggregation_temporality: 1,
                                        is_monotonic: true,
                                    })),
                                },
                                Metric {
                                    name: String::from("my.gauge"),
                                    description: String::from("I am a Gauge"),
                                    unit: String::from("1"),
                                    metadata: vec![],
                                    data: Some(Data::Gauge(Gauge {
                                        data_points: vec![NumberDataPoint {
                                            attributes: vec![KeyValue {
                                                key: String::from("my.gauge.attr"),
                                                value: Some(AnyValue {
                                                    value: Some(Value::StringValue(String::from(
                                                        "some value",
                                                    ))),
                                                }),
                                            }],
                                            start_time_unix_nano: 0,
                                            time_unix_nano: 1544712660300000000,
                                            exemplars: vec![],
                                            flags: 0,
                                            value: Some(MetricValue::AsDouble(10.0)),
                                        }],
                                    })),
                                },
                                Metric {
                                    name: String::from("my.histogram"),
                                    description: String::from("I am a Histogram"),
                                    unit: String::from("1"),
                                    metadata: vec![],
                                    data: Some(Data::Histogram(Histogram {
                                        data_points: vec![HistogramDataPoint {
                                            attributes: vec![KeyValue {
                                                key: String::from("my.histogram.attr"),
                                                value: Some(AnyValue {
                                                    value: Some(Value::StringValue(String::from(
                                                        "some value",
                                                    ))),
                                                }),
                                            }],
                                            start_time_unix_nano: 1544712660300000000,
                                            time_unix_nano: 1544712660300000000,
                                            count: 2,
                                            sum: Some(2.0),
                                            bucket_counts: vec![1, 1],
                                            explicit_bounds: vec![1.0],
                                            exemplars: vec![],
                                            flags: 0,
                                            min: Some(0.0),
                                            max: Some(2.0),
                                        }],
                                        aggregation_temporality: 1,
                                    })),
                                },
                            ],
                            schema_url: String::new(),
                        }],
                        schema_url: String::new(),
                    }],
                }
            }

            // language=json
            const CANONICAL: &str = r#"{
  "resourceMetrics": [
    {
      "resource": {
        "attributes": [
          {
            "key": "service.name",
            "value": {
              "stringValue": "my.service"
            }
          }
        ],
        "droppedAttributesCount": 0
      },
      "scopeMetrics": [
        {
          "scope": {
            "name": "my.library",
            "version": "1.0.0",
            "attributes": [
              {
                "key": "my.scope.attribute",
                "value": {
                  "stringValue": "some scope attribute"
                }
              }
            ],
            "droppedAttributesCount": 0
          },
          "metrics": [
            {
              "name": "my.counter",
              "description": "I am a Counter",
              "unit": "1",
              "metadata": [],
              "sum": {
                "dataPoints": [
                  {
                    "attributes": [
                      {
                        "key": "my.counter.attr",
                        "value": {
                          "stringValue": "some value"
                        }
                      }
                    ],
                    "startTimeUnixNano": "1544712660300000000",
                    "timeUnixNano": "1544712660300000000",
                    "exemplars": [],
                    "flags": 0,
                    "asDouble": 5.0
                  }
                ],
                "aggregationTemporality": 1,
                "isMonotonic": true
              }
            },
            {
              "name": "my.gauge",
              "description": "I am a Gauge",
              "unit": "1",
              "metadata": [],
              "gauge": {
                "dataPoints": [
                  {
                    "attributes": [
                      {
                        "key": "my.gauge.attr",
                        "value": {
                          "stringValue": "some value"
                        }
                      }
                    ],
                    "startTimeUnixNano": "0",
                    "timeUnixNano": "1544712660300000000",
                    "exemplars": [],
                    "flags": 0,
                    "asDouble": 10.0
                  }
                ]
              }
            },
            {
              "name": "my.histogram",
              "description": "I am a Histogram",
              "unit": "1",
              "metadata": [],
              "histogram": {
                "dataPoints": [
                  {
                    "attributes": [
                      {
                        "key": "my.histogram.attr",
                        "value": {
                          "stringValue": "some value"
                        }
                      }
                    ],
                    "startTimeUnixNano": "1544712660300000000",
                    "timeUnixNano": "1544712660300000000",
                    "count": 2,
                    "sum": 2.0,
                    "bucketCounts": [
                      1,
                      1
                    ],
                    "explicitBounds": [
                      1.0
                    ],
                    "exemplars": [],
                    "flags": 0,
                    "min": 0.0,
                    "max": 2.0
                  }
                ],
                "aggregationTemporality": 1
              }
            }
          ],
          "schemaUrl": ""
        }
      ],
      "schemaUrl": ""
    }
  ]
}"#;

            // copied from the example json file
            // language=json
            const ALTERNATIVE: &str = r#"{
  "resourceMetrics": [
    {
      "resource": {
        "attributes": [
          {
            "key": "service.name",
            "value": {
              "stringValue": "my.service"
            }
          }
        ]
      },
      "scopeMetrics": [
        {
          "scope": {
            "name": "my.library",
            "version": "1.0.0",
            "attributes": [
              {
                "key": "my.scope.attribute",
                "value": {
                  "stringValue": "some scope attribute"
                }
              }
            ]
          },
          "metrics": [
            {
              "name": "my.counter",
              "unit": "1",
              "description": "I am a Counter",
              "sum": {
                "aggregationTemporality": 1,
                "isMonotonic": true,
                "dataPoints": [
                  {
                    "asDouble": 5,
                    "startTimeUnixNano": "1544712660300000000",
                    "timeUnixNano": "1544712660300000000",
                    "attributes": [
                      {
                        "key": "my.counter.attr",
                        "value": {
                          "stringValue": "some value"
                        }
                      }
                    ]
                  }
                ]
              }
            },
            {
              "name": "my.gauge",
              "unit": "1",
              "description": "I am a Gauge",
              "gauge": {
                "dataPoints": [
                  {
                    "asDouble": 10,
                    "timeUnixNano": "1544712660300000000",
                    "attributes": [
                      {
                        "key": "my.gauge.attr",
                        "value": {
                          "stringValue": "some value"
                        }
                      }
                    ]
                  }
                ]
              }
            },
            {
              "name": "my.histogram",
              "unit": "1",
              "description": "I am a Histogram",
              "histogram": {
                "aggregationTemporality": 1,
                "dataPoints": [
                  {
                    "startTimeUnixNano": "1544712660300000000",
                    "timeUnixNano": "1544712660300000000",
                    "count": 2,
                    "sum": 2,
                    "bucketCounts": [1,1],
                    "explicitBounds": [1],
                    "min": 0,
                    "max": 2,
                    "attributes": [
                      {
                        "key": "my.histogram.attr",
                        "value": {
                          "stringValue": "some value"
                        }
                      }
                    ]
                  }
                ]
              }
            }
          ]
        }
      ]
    }
  ]
}
"#;

            #[test]
            fn serialize() {
                let input: ExportMetricsServiceRequest = value();
                let actual =
                    serde_json::to_string_pretty(&input).expect("serialization must succeed");
                assert_eq!(actual, CANONICAL);
            }

            #[test]
            fn deserialize_canonical() {
                let actual: ExportMetricsServiceRequest =
                    serde_json::from_str(CANONICAL).expect("deserialization must succeed");
                let expected: ExportMetricsServiceRequest = value();
                assert_eq!(actual, expected);
            }

            #[test]
            fn deserialize_alternative() {
                let actual: ExportMetricsServiceRequest =
                    serde_json::from_str(ALTERNATIVE).expect("deserialization must succeed");
                let expected: ExportMetricsServiceRequest = value();
                assert_eq!(actual, expected);
            }
        }
    }

    #[cfg(feature = "logs")]
    mod export_logs_service_request {
        use super::*;

        // `ExportTraceServiceRequest` from the OpenTelemetry proto examples
        // see <https://github.com/open-telemetry/opentelemetry-proto/blob/v1.3.2/examples/logs.json>
        mod example {
            use super::*;

            fn value() -> ExportLogsServiceRequest {
                ExportLogsServiceRequest {
                    resource_logs: vec![ResourceLogs {
                        resource: Some(Resource {
                            attributes: vec![KeyValue {
                                key: String::from("service.name"),
                                value: Some(AnyValue {
                                    value: Some(Value::StringValue(String::from("my.service"))),
                                }),
                            }],
                            dropped_attributes_count: 0,
                            entity_refs: vec![],
                        }),
                        scope_logs: vec![ScopeLogs {
                            scope: Some(InstrumentationScope {
                                name: String::from("my.library"),
                                version: String::from("1.0.0"),
                                attributes: vec![KeyValue {
                                    key: String::from("my.scope.attribute"),
                                    value: Some(AnyValue {
                                        value: Some(Value::StringValue(String::from(
                                            "some scope attribute",
                                        ))),
                                    }),
                                }],
                                dropped_attributes_count: 0,
                            }),
                            log_records: vec![LogRecord {
                                time_unix_nano: 1544712660300000000,
                                observed_time_unix_nano: 1544712660300000000,
                                severity_number: 10,
                                severity_text: String::from("Information"),
                                body: Some(AnyValue {
                                    value: Some(Value::StringValue(String::from(
                                        "Example log record",
                                    ))),
                                }),
                                event_name: "test_log_event".to_string(),
                                attributes: vec![
                                    KeyValue {
                                        key: String::from("string.attribute"),
                                        value: Some(AnyValue {
                                            value: Some(Value::StringValue(String::from(
                                                "some string",
                                            ))),
                                        }),
                                    },
                                    KeyValue {
                                        key: String::from("boolean.attribute"),
                                        value: Some(AnyValue {
                                            value: Some(Value::BoolValue(true)),
                                        }),
                                    },
                                    KeyValue {
                                        key: String::from("int.attribute"),
                                        value: Some(AnyValue {
                                            value: Some(Value::IntValue(10)),
                                        }),
                                    },
                                    KeyValue {
                                        key: String::from("double.attribute"),
                                        value: Some(AnyValue {
                                            value: Some(Value::DoubleValue(637.704)),
                                        }),
                                    },
                                    KeyValue {
                                        key: String::from("array.attribute"),
                                        value: Some(AnyValue {
                                            value: Some(Value::ArrayValue(ArrayValue {
                                                values: vec![
                                                    AnyValue {
                                                        value: Some(Value::StringValue(
                                                            String::from("many"),
                                                        )),
                                                    },
                                                    AnyValue {
                                                        value: Some(Value::StringValue(
                                                            String::from("values"),
                                                        )),
                                                    },
                                                ],
                                            })),
                                        }),
                                    },
                                    KeyValue {
                                        key: String::from("map.attribute"),
                                        value: Some(AnyValue {
                                            value: Some(Value::KvlistValue(KeyValueList {
                                                values: vec![KeyValue {
                                                    key: String::from("some.map.key"),
                                                    value: Some(AnyValue {
                                                        value: Some(Value::StringValue(
                                                            String::from("some value"),
                                                        )),
                                                    }),
                                                }],
                                            })),
                                        }),
                                    },
                                ],
                                dropped_attributes_count: 0,
                                flags: 0,
                                trace_id: hex::decode("5b8efff798038103d269b633813fc60c").unwrap(),
                                span_id: hex::decode("eee19b7ec3c1b174").unwrap(),
                            }],
                            schema_url: String::new(),
                        }],
                        schema_url: String::new(),
                    }],
                }
            }

            // language=json
            const CANONICAL: &str = r#"{
  "resourceLogs": [
    {
      "resource": {
        "attributes": [
          {
            "key": "service.name",
            "value": {
              "stringValue": "my.service"
            }
          }
        ],
        "droppedAttributesCount": 0
      },
      "scopeLogs": [
        {
          "scope": {
            "name": "my.library",
            "version": "1.0.0",
            "attributes": [
              {
                "key": "my.scope.attribute",
                "value": {
                  "stringValue": "some scope attribute"
                }
              }
            ],
            "droppedAttributesCount": 0
          },
          "logRecords": [
            {
              "timeUnixNano": "1544712660300000000",
              "observedTimeUnixNano": "1544712660300000000",
              "severityNumber": 10,
              "severityText": "Information",
              "body": {
                "stringValue": "Example log record"
              },
              "attributes": [
                {
                  "key": "string.attribute",
                  "value": {
                    "stringValue": "some string"
                  }
                },
                {
                  "key": "boolean.attribute",
                  "value": {
                    "boolValue": true
                  }
                },
                {
                  "key": "int.attribute",
                  "value": {
                    "intValue": "10"
                  }
                },
                {
                  "key": "double.attribute",
                  "value": {
                    "doubleValue": 637.704
                  }
                },
                {
                  "key": "array.attribute",
                  "value": {
                    "arrayValue": {
                      "values": [
                        {
                          "stringValue": "many"
                        },
                        {
                          "stringValue": "values"
                        }
                      ]
                    }
                  }
                },
                {
                  "key": "map.attribute",
                  "value": {
                    "kvlistValue": {
                      "values": [
                        {
                          "key": "some.map.key",
                          "value": {
                            "stringValue": "some value"
                          }
                        }
                      ]
                    }
                  }
                }
              ],
              "droppedAttributesCount": 0,
              "flags": 0,
              "traceId": "5b8efff798038103d269b633813fc60c",
              "spanId": "eee19b7ec3c1b174",
              "eventName": "test_log_event"
            }
          ],
          "schemaUrl": ""
        }
      ],
      "schemaUrl": ""
    }
  ]
}"#;

            // copied from the example json file with fix for <https://github.com/open-telemetry/opentelemetry-proto/issues/579>
            // language=json
            const ALTERNATIVE: &str = r#"{
  "resourceLogs": [
    {
      "resource": {
        "attributes": [
          {
            "key": "service.name",
            "value": {
              "stringValue": "my.service"
            }
          }
        ]
      },
      "scopeLogs": [
        {
          "scope": {
            "name": "my.library",
            "version": "1.0.0",
            "attributes": [
              {
                "key": "my.scope.attribute",
                "value": {
                  "stringValue": "some scope attribute"
                }
              }
            ]
          },
          "logRecords": [
            {
              "timeUnixNano": "1544712660300000000",
              "observedTimeUnixNano": "1544712660300000000",
              "severityNumber": 10,
              "severityText": "Information",
              "traceId": "5B8EFFF798038103D269B633813FC60C",
              "spanId": "EEE19B7EC3C1B174",
              "body": {
                "stringValue": "Example log record"
              },
              "attributes": [
                {
                  "key": "string.attribute",
                  "value": {
                    "stringValue": "some string"
                  }
                },
                {
                  "key": "boolean.attribute",
                  "value": {
                    "boolValue": true
                  }
                },
                {
                  "key": "int.attribute",
                  "value": {
                    "intValue": "10"
                  }
                },
                {
                  "key": "double.attribute",
                  "value": {
                    "doubleValue": 637.704
                  }
                },
                {
                  "key": "array.attribute",
                  "value": {
                    "arrayValue": {
                      "values": [
                        {
                          "stringValue": "many"
                        },
                        {
                          "stringValue": "values"
                        }
                      ]
                    }
                  }
                },
                {
                  "key": "map.attribute",
                  "value": {
                    "kvlistValue": {
                      "values": [
                        {
                          "key": "some.map.key",
                          "value": {
                            "stringValue": "some value"
                          }
                        }
                      ]
                    }
                  }
                }
              ],
              "eventName": "test_log_event"
            }
          ]
        }
      ]
    }
  ]
}
"#;

            #[test]
            fn serialize() {
                let input: ExportLogsServiceRequest = value();
                let actual =
                    serde_json::to_string_pretty(&input).expect("serialization must succeed");
                assert_eq!(actual, CANONICAL);
            }

            #[test]
            fn deserialize_canonical() {
                let actual: ExportLogsServiceRequest =
                    serde_json::from_str(CANONICAL).expect("deserialization must succeed");
                let expected: ExportLogsServiceRequest = value();
                assert_eq!(actual, expected);
            }

            #[test]
            fn deserialize_alternative() {
                let actual: ExportLogsServiceRequest =
                    serde_json::from_str(ALTERNATIVE).expect("deserialization must succeed");
                let expected: ExportLogsServiceRequest = value();
                assert_eq!(actual, expected);
            }
        }
    }
}
