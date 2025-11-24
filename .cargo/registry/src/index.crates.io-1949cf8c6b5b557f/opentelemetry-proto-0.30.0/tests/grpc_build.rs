use std::collections::HashMap;
use std::path::Path;
use tempfile::TempDir;

const TONIC_OUT_DIR: &str = "src/proto/tonic";
const TONIC_PROTO_FILES: &[&str] = &[
    "src/proto/opentelemetry-proto/opentelemetry/proto/common/v1/common.proto",
    "src/proto/opentelemetry-proto/opentelemetry/proto/resource/v1/resource.proto",
    "src/proto/opentelemetry-proto/opentelemetry/proto/trace/v1/trace.proto",
    "src/proto/opentelemetry-proto/opentelemetry/proto/collector/trace/v1/trace_service.proto",
    "src/proto/opentelemetry-proto/opentelemetry/proto/metrics/v1/metrics.proto",
    "src/proto/opentelemetry-proto/opentelemetry/proto/collector/metrics/v1/metrics_service.proto",
    "src/proto/opentelemetry-proto/opentelemetry/proto/logs/v1/logs.proto",
    "src/proto/opentelemetry-proto/opentelemetry/proto/collector/logs/v1/logs_service.proto",
    "src/proto/opentelemetry-proto/opentelemetry/proto/profiles/v1development/profiles.proto",
    "src/proto/tracez.proto",
];
const TONIC_INCLUDES: &[&str] = &["src/proto/opentelemetry-proto", "src/proto"];

#[test]
fn build_tonic() {
    let before_build = build_content_map(TONIC_OUT_DIR, true);

    let out_dir = TempDir::new().expect("failed to create temp dir to store the generated files");

    // build the generated files into OUT_DIR for now so we don't have to touch the src unless we have to
    let mut builder = tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .server_mod_attribute(".", "#[cfg(feature = \"gen-tonic\")]")
        .client_mod_attribute(".", "#[cfg(feature = \"gen-tonic\")]")
        .type_attribute(
            ".",
            "#[cfg_attr(feature = \"with-schemars\", derive(schemars::JsonSchema))]",
        )
        .type_attribute(
            ".",
            "#[cfg_attr(feature = \"with-serde\", derive(serde::Serialize, serde::Deserialize))]",
        )
        .type_attribute(
            ".",
            "#[cfg_attr(feature = \"with-serde\", serde(rename_all = \"camelCase\"))]",
        );

    // Optional numeric, string and array fields need to default to their default value otherwise
    // JSON files without those field cannot deserialize
    // we cannot add serde(default) to all generated types because enums cannot be annotated with serde(default)
    for path in [
        "trace.v1.Span",
        "trace.v1.Span.Link",
        "trace.v1.ScopeSpans",
        "trace.v1.ResourceSpans",
        "common.v1.InstrumentationScope",
        "resource.v1.Resource",
        "trace.v1.Span.Event",
        "trace.v1.Status",
        "logs.v1.LogRecord",
        "logs.v1.ScopeLogs",
        "logs.v1.ResourceLogs",
        "metrics.v1.Metric",
        "metrics.v1.ResourceMetrics",
        "metrics.v1.ScopeMetrics",
        "metrics.v1.Gauge",
        "metrics.v1.Sum",
        "metrics.v1.Histogram",
        "metrics.v1.ExponentialHistogram",
        "metrics.v1.Summary",
        "metrics.v1.NumberDataPoint",
        "metrics.v1.HistogramDataPoint",
        "profiles.v1development.Function",
    ] {
        builder = builder.type_attribute(
            path,
            "#[cfg_attr(feature = \"with-serde\", serde(default))]",
        )
    }

    // special serializer and deserializer for traceId and spanId
    // OTLP/JSON format uses hex string for traceId and spanId
    // the proto file uses bytes for traceId and spanId
    // Thus, special serializer and deserializer are needed
    for path in [
        "trace.v1.Span.trace_id",
        "trace.v1.Span.span_id",
        "trace.v1.Span.parent_span_id",
        "trace.v1.Span.Link.trace_id",
        "trace.v1.Span.Link.span_id",
        "logs.v1.LogRecord.span_id",
        "logs.v1.LogRecord.trace_id",
        "metrics.v1.Exemplar.span_id",
        "metrics.v1.Exemplar.trace_id",
        "profiles.v1development.Profile.profile_id",
    ] {
        builder = builder
            .field_attribute(path, "#[cfg_attr(feature = \"with-serde\", serde(serialize_with = \"crate::proto::serializers::serialize_to_hex_string\", deserialize_with = \"crate::proto::serializers::deserialize_from_hex_string\"))]")
    }

    // special serializer and deserializer for timestamp
    // OTLP/JSON format may uses string for timestamp
    // the proto file uses u64 for timestamp
    // Thus, special serializer and deserializer are needed
    for path in [
        "trace.v1.Span.start_time_unix_nano",
        "trace.v1.Span.end_time_unix_nano",
        "trace.v1.Span.Event.time_unix_nano",
        "logs.v1.LogRecord.time_unix_nano",
        "logs.v1.LogRecord.observed_time_unix_nano",
        "metrics.v1.HistogramDataPoint.start_time_unix_nano",
        "metrics.v1.HistogramDataPoint.time_unix_nano",
        "metrics.v1.NumberDataPoint.start_time_unix_nano",
        "metrics.v1.NumberDataPoint.time_unix_nano",
    ] {
        builder = builder
            .field_attribute(path, "#[cfg_attr(feature = \"with-serde\", serde(serialize_with = \"crate::proto::serializers::serialize_u64_to_string\", deserialize_with = \"crate::proto::serializers::deserialize_string_to_u64\"))]")
    }
    for path in ["profiles.v1development.Profile.time_nanos"] {
        builder = builder
            .field_attribute(path, "#[cfg_attr(feature = \"with-serde\", serde(serialize_with = \"crate::proto::serializers::serialize_i64_to_string\", deserialize_with = \"crate::proto::serializers::deserialize_string_to_i64\"))]")
    }
    for path in ["profiles.v1development.Sample.timestamps_unix_nano"] {
        builder = builder
            .field_attribute(path, "#[cfg_attr(feature = \"with-serde\", serde(serialize_with = \"crate::proto::serializers::serialize_vec_u64_to_string\", deserialize_with = \"crate::proto::serializers::deserialize_vec_string_to_vec_u64\"))]")
    }

    // special serializer and deserializer for value
    // The Value::value field must be hidden
    builder = builder
        .field_attribute("common.v1.AnyValue.value", "#[cfg_attr(feature =\"with-serde\", serde(flatten, serialize_with = \"crate::proto::serializers::serialize_to_value\", deserialize_with = \"crate::proto::serializers::deserialize_from_value\"))]");

    // flatten
    for path in ["metrics.v1.Metric.data", "metrics.v1.NumberDataPoint.value"] {
        builder =
            builder.field_attribute(path, "#[cfg_attr(feature =\"with-serde\", serde(flatten))]");
    }

    builder
        .out_dir(out_dir.path())
        .compile_protos(TONIC_PROTO_FILES, TONIC_INCLUDES)
        .expect("cannot compile protobuf using tonic");

    let after_build = build_content_map(out_dir.path(), true);
    ensure_files_are_same(before_build, after_build, TONIC_OUT_DIR);
}

fn build_content_map(path: impl AsRef<Path>, normalize_line_feed: bool) -> HashMap<String, String> {
    std::fs::read_dir(path)
        .expect("cannot open dictionary of generated files")
        .flatten()
        .map(|entry| {
            let path = entry.path();
            let file_name = path
                .file_name()
                .expect("file name should always exist for generated files");

            let mut file_contents = std::fs::read_to_string(path.clone())
                .expect("cannot read from existing generated file");

            if normalize_line_feed {
                file_contents = get_platform_specific_string(file_contents);
            }

            (file_name.to_string_lossy().to_string(), file_contents)
        })
        .collect()
}

///  Returns a String which uses the platform specific new line feed character.
fn get_platform_specific_string(input: String) -> String {
    if cfg!(windows) && !input.ends_with("\r\n") && input.ends_with('\n') {
        return input.replace('\n', "\r\n");
    }
    input
}

fn ensure_files_are_same(
    before_build: HashMap<String, String>,
    after_build: HashMap<String, String>,
    target_dir: &'static str,
) {
    if after_build == before_build {
        return;
    }

    if std::env::var("CI").is_ok() {
        panic!("generated file has changed but it's a CI environment, please rerun this test locally and commit the changes");
    }

    // if there is at least one changes we will just copy the whole directory over
    for (file_name, content) in after_build {
        std::fs::write(Path::new(target_dir).join(file_name), content)
            .expect("cannot write to the proto generate file. If it's happening in CI env, please return the test locally and commit the change");
    }

    panic!("generated file has changed, please commit the change file and rerun the test");
}
