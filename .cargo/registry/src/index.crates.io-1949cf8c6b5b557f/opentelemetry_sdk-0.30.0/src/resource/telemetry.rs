use crate::resource::ResourceDetector;
use crate::Resource;
use opentelemetry::KeyValue;

/// Detect the telemetry SDK information used to capture data recorded by the instrumentation libraries.
///
/// It provides:
/// - The name of the telemetry SDK(`telemetry.sdk.name`). It will be `opentelemetry` for SDK provided by opentelemetry project.
/// - The language of the telemetry SDK(`telemetry.sdk.language`). It will be `rust` for this SDK.
/// - The version of the telemetry SDK(`telemetry.sdk.version`). It will be current `opentelemetry_sdk` crate version.
///
///
/// See [semantic conventions](https://github.com/open-telemetry/semantic-conventions/blob/main/docs/resource/README.md#telemetry-sdk) for details.
#[derive(Debug)]
pub struct TelemetryResourceDetector;

impl ResourceDetector for TelemetryResourceDetector {
    fn detect(&self) -> Resource {
        Resource::builder_empty()
            .with_attributes([
                KeyValue::new(super::TELEMETRY_SDK_NAME, "opentelemetry"),
                KeyValue::new(super::TELEMETRY_SDK_LANGUAGE, "rust"),
                KeyValue::new(super::TELEMETRY_SDK_VERSION, env!("CARGO_PKG_VERSION")),
            ])
            .build()
    }
}
