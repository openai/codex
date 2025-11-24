//! Environment variables resource detector
//!
//! Implementation of `ResourceDetector` to extract a `Resource` from environment
//! variables.
use crate::resource::{Resource, ResourceDetector};
use opentelemetry::{Key, KeyValue, Value};
use std::env;

const OTEL_RESOURCE_ATTRIBUTES: &str = "OTEL_RESOURCE_ATTRIBUTES";
const OTEL_SERVICE_NAME: &str = "OTEL_SERVICE_NAME";

/// EnvResourceDetector extract resource from environment variable
/// `OTEL_RESOURCE_ATTRIBUTES`. See [OpenTelemetry Resource
/// Spec](https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/resource/sdk.md#specifying-resource-information-via-an-environment-variable)
/// for details.
#[derive(Debug)]
pub struct EnvResourceDetector {
    _private: (),
}

impl ResourceDetector for EnvResourceDetector {
    fn detect(&self) -> Resource {
        match env::var(OTEL_RESOURCE_ATTRIBUTES) {
            Ok(s) if !s.is_empty() => construct_otel_resources(s),
            Ok(_) | Err(_) => Resource::empty(), // return empty resource
        }
    }
}

impl EnvResourceDetector {
    /// Create `EnvResourceDetector` instance.
    pub fn new() -> Self {
        EnvResourceDetector { _private: () }
    }
}

impl Default for EnvResourceDetector {
    fn default() -> Self {
        EnvResourceDetector::new()
    }
}

/// Extract key value pairs and construct a resource from resources string like
/// key1=value1,key2=value2,...
fn construct_otel_resources(s: String) -> Resource {
    Resource::builder_empty()
        .with_attributes(s.split_terminator(',').filter_map(|entry| {
            let parts = match entry.split_once('=') {
                Some(p) => p,
                None => return None,
            };
            let key = parts.0.trim();
            let value = parts.1.trim();

            Some(KeyValue::new(key.to_owned(), value.to_owned()))
        }))
        .build()
}

/// There are attributes which MUST be provided by the SDK as specified in
/// [the Resource SDK specification]. This detector detects those attributes and
/// if the attribute cannot be detected, it uses the default value.
///
/// This detector will first try `OTEL_SERVICE_NAME` env. If it's not available,
/// then it will check the `OTEL_RESOURCE_ATTRIBUTES` env and see if it contains
/// `service.name` resource. If it's also not available, it will use `unknown_service`.
///
/// If users want to set an empty service name, they can provide
/// a resource with empty value and `service.name` key.
///
/// [the Resource SDK specification]:https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/resource/sdk.md#sdk-provided-resource-attributes
#[derive(Debug)]
pub struct SdkProvidedResourceDetector;

impl ResourceDetector for SdkProvidedResourceDetector {
    fn detect(&self) -> Resource {
        Resource::builder_empty()
            .with_attributes([KeyValue::new(
                super::SERVICE_NAME,
                env::var(OTEL_SERVICE_NAME)
                    .ok()
                    .filter(|s| !s.is_empty())
                    .map(Value::from)
                    .or_else(|| {
                        EnvResourceDetector::new()
                            .detect()
                            .get(&Key::new(super::SERVICE_NAME))
                    })
                    .unwrap_or_else(|| "unknown_service".into()),
            )])
            .build()
    }
}

#[cfg(test)]
mod tests {
    use crate::resource::env::{
        SdkProvidedResourceDetector, OTEL_RESOURCE_ATTRIBUTES, OTEL_SERVICE_NAME,
    };
    use crate::resource::{EnvResourceDetector, Resource, ResourceDetector};
    use opentelemetry::{Key, KeyValue, Value};

    #[test]
    fn test_read_from_env() {
        temp_env::with_vars(
            [
                (
                    "OTEL_RESOURCE_ATTRIBUTES",
                    Some("key=value, k = v , a= x, a=z,base64=SGVsbG8sIFdvcmxkIQ=="),
                ),
                ("IRRELEVANT", Some("20200810")),
            ],
            || {
                let detector = EnvResourceDetector::new();
                let resource = detector.detect();
                assert_eq!(
                    resource,
                    Resource::builder_empty()
                        .with_attributes([
                            KeyValue::new("key", "value"),
                            KeyValue::new("k", "v"),
                            KeyValue::new("a", "x"),
                            KeyValue::new("a", "z"),
                            KeyValue::new("base64", "SGVsbG8sIFdvcmxkIQ=="), // base64('Hello, World!')
                        ])
                        .build()
                );
            },
        );

        let detector = EnvResourceDetector::new();
        let resource = detector.detect();
        assert!(resource.is_empty());
    }

    #[test]
    fn test_sdk_provided_resource_detector() {
        // Ensure no env var set
        let no_env = SdkProvidedResourceDetector.detect();
        assert_eq!(
            no_env.get(&Key::from_static_str(crate::resource::SERVICE_NAME)),
            Some(Value::from("unknown_service")),
        );

        temp_env::with_var(OTEL_SERVICE_NAME, Some("test service"), || {
            let with_service = SdkProvidedResourceDetector.detect();
            assert_eq!(
                with_service.get(&Key::from_static_str(crate::resource::SERVICE_NAME)),
                Some(Value::from("test service")),
            )
        });

        temp_env::with_var(
            OTEL_RESOURCE_ATTRIBUTES,
            Some("service.name=test service1"),
            || {
                let with_service = SdkProvidedResourceDetector.detect();
                assert_eq!(
                    with_service.get(&Key::from_static_str(crate::resource::SERVICE_NAME)),
                    Some(Value::from("test service1")),
                )
            },
        );

        // OTEL_SERVICE_NAME takes priority
        temp_env::with_vars(
            [
                (OTEL_SERVICE_NAME, Some("test service")),
                (OTEL_RESOURCE_ATTRIBUTES, Some("service.name=test service3")),
            ],
            || {
                let with_service = SdkProvidedResourceDetector.detect();
                assert_eq!(
                    with_service.get(&Key::from_static_str(crate::resource::SERVICE_NAME)),
                    Some(Value::from("test service"))
                );
            },
        );
    }
}
