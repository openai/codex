//! Representations of entities producing telemetry.
//!
//! A [Resource] is an immutable representation of the entity producing
//! telemetry as attributes. For example, a process producing telemetry that is
//! running in a container on Kubernetes has a Pod name, it is in a namespace
//! and possibly is part of a Deployment which also has a name. All three of
//! these attributes can be included in the `Resource`. Note that there are
//! certain ["standard attributes"] that have prescribed meanings.
//!
//! ["standard attributes"]: https://github.com/open-telemetry/opentelemetry-specification/blob/v1.9.0/specification/resource/semantic_conventions/README.md
//!
//! # Resource detectors
//!
//! [`ResourceDetector`]s are used to detect resource from runtime or
//! environmental variables. The following are provided by default with this
//! SDK.
//!
//! - [`EnvResourceDetector`] - detect resource from environmental variables.
//! - [`TelemetryResourceDetector`] - detect telemetry SDK's information.
//!
//! The OS and Process resource detectors are packaged separately in the
//! [`opentelemetry-resource-detector` crate](https://github.com/open-telemetry/opentelemetry-rust-contrib/tree/main/opentelemetry-resource-detectors).
mod env;
mod telemetry;

mod attributes;
pub(crate) use attributes::*;

pub use env::EnvResourceDetector;
pub use env::SdkProvidedResourceDetector;
pub use telemetry::TelemetryResourceDetector;

use opentelemetry::{Key, KeyValue, Value};
use std::borrow::Cow;
use std::collections::{hash_map, HashMap};
use std::ops::Deref;
use std::sync::Arc;

/// Inner structure of `Resource` holding the actual data.
/// This structure is designed to be shared among `Resource` instances via `Arc`.
#[derive(Debug, Clone, PartialEq)]
struct ResourceInner {
    attrs: HashMap<Key, Value>,
    schema_url: Option<Cow<'static, str>>,
}

/// An immutable representation of the entity producing telemetry as attributes.
/// Utilizes `Arc` for efficient sharing and cloning.
#[derive(Clone, Debug, PartialEq)]
pub struct Resource {
    inner: Arc<ResourceInner>,
}

impl Resource {
    /// Creates a [ResourceBuilder] that allows you to configure multiple aspects of the Resource.
    ///
    /// This [ResourceBuilder] will include the following [ResourceDetector]s:
    /// - [SdkProvidedResourceDetector]
    /// - [TelemetryResourceDetector]
    /// - [EnvResourceDetector]
    ///   If you'd like to start from an empty resource, use [Resource::builder_empty].
    pub fn builder() -> ResourceBuilder {
        ResourceBuilder {
            resource: Self::from_detectors(&[
                Box::new(SdkProvidedResourceDetector),
                Box::new(TelemetryResourceDetector),
                Box::new(EnvResourceDetector::new()),
            ]),
        }
    }

    /// Creates a [ResourceBuilder] that allows you to configure multiple aspects of the Resource.
    ///
    /// This [ResourceBuilder] will not include any attributes or [ResourceDetector]s by default.
    pub fn builder_empty() -> ResourceBuilder {
        ResourceBuilder {
            resource: Resource::empty(),
        }
    }

    /// Creates an empty resource.
    /// This is the basic constructor that initializes a resource with no attributes and no schema URL.
    pub(crate) fn empty() -> Self {
        Resource {
            inner: Arc::new(ResourceInner {
                attrs: HashMap::new(),
                schema_url: None,
            }),
        }
    }

    /// Create a new `Resource` from key value pairs.
    ///
    /// Values are de-duplicated by key, and the last key-value pair will be retained
    pub(crate) fn new<T: IntoIterator<Item = KeyValue>>(kvs: T) -> Self {
        let mut attrs = HashMap::new();
        for kv in kvs {
            attrs.insert(kv.key, kv.value);
        }

        Resource {
            inner: Arc::new(ResourceInner {
                attrs,
                schema_url: None,
            }),
        }
    }

    /// Create a new `Resource` from a key value pairs and [schema url].
    ///
    /// Values are de-duplicated by key, and the first key-value pair with a non-empty string value
    /// will be retained.
    ///
    /// schema_url must be a valid URL using HTTP or HTTPS protocol.
    ///
    /// [schema url]: https://github.com/open-telemetry/opentelemetry-specification/blob/v1.9.0/specification/schemas/overview.md#schema-url
    fn from_schema_url<KV, S>(kvs: KV, schema_url: S) -> Self
    where
        KV: IntoIterator<Item = KeyValue>,
        S: Into<Cow<'static, str>>,
    {
        let schema_url_str = schema_url.into();
        let normalized_schema_url = if schema_url_str.is_empty() {
            None
        } else {
            Some(schema_url_str)
        };
        let mut attrs = HashMap::new();
        for kv in kvs {
            attrs.insert(kv.key, kv.value);
        }
        Resource {
            inner: Arc::new(ResourceInner {
                attrs,
                schema_url: normalized_schema_url,
            }),
        }
    }

    /// Create a new `Resource` from resource detectors.
    fn from_detectors(detectors: &[Box<dyn ResourceDetector>]) -> Self {
        let mut resource = Resource::empty();
        for detector in detectors {
            let detected_res = detector.detect();
            // This call ensures that if the Arc is not uniquely owned,
            // the data is cloned before modification, preserving safety.
            // If the Arc is uniquely owned, it simply returns a mutable reference to the data.
            let inner = Arc::make_mut(&mut resource.inner);
            for (key, value) in detected_res.into_iter() {
                inner.attrs.insert(Key::new(key.clone()), value.clone());
            }
        }

        resource
    }

    /// Create a new `Resource` by combining two resources.
    ///
    /// ### Key value pairs
    /// Keys from the `other` resource have priority over keys from this resource, even if the
    /// updated value is empty.
    ///
    /// ### [Schema url]
    /// If both of the resource are not empty. Schema url is determined by the following rules, in order:
    /// 1. If this resource has a schema url, it will be used.
    /// 2. If this resource does not have a schema url, and the other resource has a schema url, it will be used.
    /// 3. If both resources have a schema url and it's the same, it will be used.
    /// 4. If both resources have a schema url and it's different, the schema url will be empty.
    /// 5. If both resources do not have a schema url, the schema url will be empty.
    ///
    /// [Schema url]: https://github.com/open-telemetry/opentelemetry-specification/blob/v1.9.0/specification/schemas/overview.md#schema-url
    pub(crate) fn merge<T: Deref<Target = Self>>(&self, other: T) -> Self {
        if self.is_empty() && self.schema_url().is_none() {
            return other.clone();
        }
        if other.is_empty() && other.schema_url().is_none() {
            return self.clone();
        }
        let mut combined_attrs = self.inner.attrs.clone();
        for (k, v) in other.inner.attrs.iter() {
            combined_attrs.insert(k.clone(), v.clone());
        }

        // Resolve the schema URL according to the precedence rules
        let combined_schema_url = match (&self.inner.schema_url, &other.inner.schema_url) {
            // If both resources have a schema URL and it's the same, use it
            (Some(url1), Some(url2)) if url1 == url2 => Some(url1.clone()),
            // If both resources have a schema URL but they are not the same, the schema URL will be empty
            (Some(_), Some(_)) => None,
            // If this resource does not have a schema URL, and the other resource has a schema URL, it will be used
            (None, Some(url)) => Some(url.clone()),
            // If this resource has a schema URL, it will be used (covers case 1 and any other cases where `self` has a schema URL)
            (Some(url), _) => Some(url.clone()),
            // If both resources do not have a schema URL, the schema URL will be empty
            (None, None) => None,
        };
        Resource {
            inner: Arc::new(ResourceInner {
                attrs: combined_attrs,
                schema_url: combined_schema_url,
            }),
        }
    }

    /// Return the [schema url] of the resource. If the resource does not have a schema url, return `None`.
    ///
    /// [schema url]: https://github.com/open-telemetry/opentelemetry-specification/blob/v1.9.0/specification/schemas/overview.md#schema-url
    pub fn schema_url(&self) -> Option<&str> {
        self.inner.schema_url.as_ref().map(|s| s.as_ref())
    }

    /// Returns the number of attributes for this resource
    pub fn len(&self) -> usize {
        self.inner.attrs.len()
    }

    /// Returns `true` if the resource contains no attributes.
    pub fn is_empty(&self) -> bool {
        self.inner.attrs.is_empty()
    }

    /// Gets an iterator over the attributes of this resource.
    pub fn iter(&self) -> Iter<'_> {
        Iter(self.inner.attrs.iter())
    }

    /// Retrieve the value from resource associate with given key.
    pub fn get(&self, key: &Key) -> Option<Value> {
        self.inner.attrs.get(key).cloned()
    }
}

/// An iterator over the entries of a `Resource`.
#[derive(Debug)]
pub struct Iter<'a>(hash_map::Iter<'a, Key, Value>);

impl<'a> Iterator for Iter<'a> {
    type Item = (&'a Key, &'a Value);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl<'a> IntoIterator for &'a Resource {
    type Item = (&'a Key, &'a Value);
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        Iter(self.inner.attrs.iter())
    }
}

/// ResourceDetector detects OpenTelemetry resource information
///
/// Implementations of this trait can be passed to
/// the [`ResourceBuilder::with_detectors`] function to generate a Resource from the merged information.
pub trait ResourceDetector {
    /// detect returns an initialized Resource based on gathered information.
    ///
    /// If source information to construct a Resource is inaccessible, an empty Resource should be returned
    ///
    /// If source information to construct a Resource is invalid, for example,
    /// missing required values. an empty Resource should be returned.
    fn detect(&self) -> Resource;
}

/// Builder for [Resource]
#[derive(Debug)]
pub struct ResourceBuilder {
    resource: Resource,
}

impl ResourceBuilder {
    /// Add a single [ResourceDetector] to your resource.
    pub fn with_detector(self, detector: Box<dyn ResourceDetector>) -> Self {
        self.with_detectors(&[detector])
    }

    /// Add multiple [ResourceDetector]s to your resource.
    pub fn with_detectors(mut self, detectors: &[Box<dyn ResourceDetector>]) -> Self {
        self.resource = self.resource.merge(&Resource::from_detectors(detectors));
        self
    }

    /// Add a [KeyValue] to the resource.
    pub fn with_attribute(self, kv: KeyValue) -> Self {
        self.with_attributes([kv])
    }

    /// Add multiple [KeyValue]s to the resource.
    pub fn with_attributes<T: IntoIterator<Item = KeyValue>>(mut self, kvs: T) -> Self {
        self.resource = self.resource.merge(&Resource::new(kvs));
        self
    }

    /// Add `service.name` resource attribute.
    pub fn with_service_name(self, name: impl Into<Value>) -> Self {
        self.with_attribute(KeyValue::new(SERVICE_NAME, name.into()))
    }

    /// This will merge the provided `schema_url` with the current state of the Resource being built. It
    /// will use the following rules to determine which `schema_url` should be used.
    ///
    /// ### [Schema url]
    /// Schema url is determined by the following rules, in order:
    /// 1. If the current builder resource doesn't have a `schema_url`, the provided `schema_url` will be used.
    /// 2. If the current builder resource has a `schema_url`, and the provided `schema_url` is different from the builder resource, `schema_url` will be empty.
    /// 3. If the provided `schema_url` is the same as the current builder resource, it will be used.
    ///
    /// [Schema url]: https://github.com/open-telemetry/opentelemetry-specification/blob/v1.9.0/specification/schemas/overview.md#schema-url
    pub fn with_schema_url<KV, S>(mut self, attributes: KV, schema_url: S) -> Self
    where
        KV: IntoIterator<Item = KeyValue>,
        S: Into<Cow<'static, str>>,
    {
        self.resource = Resource::from_schema_url(attributes, schema_url).merge(&self.resource);
        self
    }

    /// Create a [Resource] with the options provided to the [ResourceBuilder].
    pub fn build(self) -> Resource {
        self.resource
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case([KeyValue::new("a", ""), KeyValue::new("a", "final")], [(Key::new("a"), Value::from("final"))])]
    #[case([KeyValue::new("a", "final"), KeyValue::new("a", "")], [(Key::new("a"), Value::from(""))])]
    fn new_resource(
        #[case] given_attributes: [KeyValue; 2],
        #[case] expected_attrs: [(Key, Value); 1],
    ) {
        // Arrange
        let expected = HashMap::from_iter(expected_attrs.into_iter());

        // Act
        let resource = Resource::builder_empty()
            .with_attributes(given_attributes)
            .build();
        let resource_inner = Arc::try_unwrap(resource.inner).expect("Failed to unwrap Arc");

        // Assert
        assert_eq!(resource_inner.attrs, expected);
        assert_eq!(resource_inner.schema_url, None);
    }

    #[test]
    fn merge_resource_key_value_pairs() {
        let resource_a = Resource::builder_empty()
            .with_attributes([
                KeyValue::new("a", ""),
                KeyValue::new("b", "b-value"),
                KeyValue::new("d", "d-value"),
            ])
            .build();

        let resource_b = Resource::builder_empty()
            .with_attributes([
                KeyValue::new("a", "a-value"),
                KeyValue::new("c", "c-value"),
                KeyValue::new("d", ""),
            ])
            .build();

        let mut expected_attrs = HashMap::new();
        expected_attrs.insert(Key::new("a"), Value::from("a-value"));
        expected_attrs.insert(Key::new("b"), Value::from("b-value"));
        expected_attrs.insert(Key::new("c"), Value::from("c-value"));
        expected_attrs.insert(Key::new("d"), Value::from(""));

        let expected_resource = Resource {
            inner: Arc::new(ResourceInner {
                attrs: expected_attrs,
                schema_url: None, // Assuming schema_url handling if needed
            }),
        };

        assert_eq!(resource_a.merge(&resource_b), expected_resource);
    }

    #[rstest]
    #[case(Some("http://schema/a"), None, Some("http://schema/a"))]
    #[case(Some("http://schema/a"), Some("http://schema/b"), None)]
    #[case(None, Some("http://schema/b"), Some("http://schema/b"))]
    #[case(
        Some("http://schema/a"),
        Some("http://schema/a"),
        Some("http://schema/a")
    )]
    #[case(None, None, None)]
    fn merge_resource_schema_url(
        #[case] schema_url_a: Option<&'static str>,
        #[case] schema_url_b: Option<&'static str>,
        #[case] expected_schema_url: Option<&'static str>,
    ) {
        let resource_a =
            Resource::from_schema_url([KeyValue::new("key", "")], schema_url_a.unwrap_or(""));
        let resource_b =
            Resource::from_schema_url([KeyValue::new("key", "")], schema_url_b.unwrap_or(""));

        let merged_resource = resource_a.merge(&resource_b);
        let result_schema_url = merged_resource.schema_url();

        assert_eq!(
            result_schema_url.map(|s| s as &str),
            expected_schema_url,
            "Merging schema_url_a {:?} with schema_url_b {:?} did not yield expected result {:?}",
            schema_url_a,
            schema_url_b,
            expected_schema_url
        );
    }

    #[rstest]
    #[case(vec![], vec![KeyValue::new("key", "b")], Some("http://schema/a"), None, Some("http://schema/a"))]
    #[case(vec![KeyValue::new("key", "a")], vec![KeyValue::new("key", "b")], Some("http://schema/a"), None, Some("http://schema/a"))]
    #[case(vec![KeyValue::new("key", "a")], vec![KeyValue::new("key", "b")], Some("http://schema/a"), None, Some("http://schema/a"))]
    #[case(vec![KeyValue::new("key", "a")], vec![KeyValue::new("key", "b")], Some("http://schema/a"), Some("http://schema/b"), None)]
    #[case(vec![KeyValue::new("key", "a")], vec![KeyValue::new("key", "b")], None, Some("http://schema/b"), Some("http://schema/b"))]
    fn merge_resource_with_missing_attributes(
        #[case] key_values_a: Vec<KeyValue>,
        #[case] key_values_b: Vec<KeyValue>,
        #[case] schema_url_a: Option<&'static str>,
        #[case] schema_url_b: Option<&'static str>,
        #[case] expected_schema_url: Option<&'static str>,
    ) {
        let resource = match schema_url_a {
            Some(schema) => Resource::from_schema_url(key_values_a, schema),
            None => Resource::new(key_values_a),
        };

        let other_resource = match schema_url_b {
            Some(schema) => Resource::builder_empty()
                .with_schema_url(key_values_b, schema)
                .build(),
            None => Resource::new(key_values_b),
        };

        assert_eq!(
            resource.merge(&other_resource).schema_url(),
            expected_schema_url
        );
    }

    #[test]
    fn detect_resource() {
        temp_env::with_vars(
            [
                (
                    "OTEL_RESOURCE_ATTRIBUTES",
                    Some("key=value, k = v , a= x, a=z"),
                ),
                ("IRRELEVANT", Some("20200810")),
            ],
            || {
                let detector = EnvResourceDetector::new();
                let resource = Resource::from_detectors(&[Box::new(detector)]);
                assert_eq!(
                    resource,
                    Resource::builder_empty()
                        .with_attributes([
                            KeyValue::new("key", "value"),
                            KeyValue::new("k", "v"),
                            KeyValue::new("a", "x"),
                            KeyValue::new("a", "z"),
                        ])
                        .build()
                )
            },
        )
    }

    #[rstest]
    #[case(Some("http://schema/a"), Some("http://schema/b"), None)]
    #[case(None, Some("http://schema/b"), Some("http://schema/b"))]
    #[case(
        Some("http://schema/a"),
        Some("http://schema/a"),
        Some("http://schema/a")
    )]
    fn builder_with_schema_url(
        #[case] schema_url_a: Option<&'static str>,
        #[case] schema_url_b: Option<&'static str>,
        #[case] expected_schema_url: Option<&'static str>,
    ) {
        let base_builder = if let Some(url) = schema_url_a {
            ResourceBuilder {
                resource: Resource::from_schema_url(vec![KeyValue::new("key", "")], url),
            }
        } else {
            ResourceBuilder {
                resource: Resource::empty(),
            }
        };

        let resource = base_builder
            .with_schema_url(
                vec![KeyValue::new("key", "")],
                schema_url_b.expect("should always be Some for this test"),
            )
            .build();

        assert_eq!(
            resource.schema_url().map(|s| s as &str),
            expected_schema_url,
            "Merging schema_url_a {:?} with schema_url_b {:?} did not yield expected result {:?}",
            schema_url_a,
            schema_url_b,
            expected_schema_url
        );
    }

    #[test]
    fn builder_detect_resource() {
        temp_env::with_vars(
            [
                (
                    "OTEL_RESOURCE_ATTRIBUTES",
                    Some("key=value, k = v , a= x, a=z"),
                ),
                ("IRRELEVANT", Some("20200810")),
            ],
            || {
                let resource = Resource::builder_empty()
                    .with_detector(Box::new(EnvResourceDetector::new()))
                    .with_service_name("testing_service")
                    .with_attribute(KeyValue::new("test1", "test_value"))
                    .with_attributes([
                        KeyValue::new("test1", "test_value1"),
                        KeyValue::new("test2", "test_value2"),
                    ])
                    .build();

                assert_eq!(
                    resource,
                    Resource::builder_empty()
                        .with_attributes([
                            KeyValue::new("key", "value"),
                            KeyValue::new("test1", "test_value1"),
                            KeyValue::new("test2", "test_value2"),
                            KeyValue::new(SERVICE_NAME, "testing_service"),
                            KeyValue::new("k", "v"),
                            KeyValue::new("a", "x"),
                            KeyValue::new("a", "z"),
                        ])
                        .build()
                )
            },
        )
    }
}
