use std::{borrow::Cow, collections::HashSet, error::Error, sync::Arc};

use opentelemetry::{
    metrics::{AsyncInstrument, SyncInstrument},
    InstrumentationScope, Key, KeyValue,
};

use crate::metrics::{aggregation::Aggregation, internal::Measure};

use super::meter::{
    INSTRUMENT_NAME_EMPTY, INSTRUMENT_NAME_FIRST_ALPHABETIC, INSTRUMENT_NAME_INVALID_CHAR,
    INSTRUMENT_NAME_LENGTH, INSTRUMENT_UNIT_INVALID_CHAR, INSTRUMENT_UNIT_LENGTH,
};

use super::Temporality;

/// The identifier of a group of instruments that all perform the same function.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum InstrumentKind {
    /// Identifies a group of instruments that record increasing values synchronously
    /// with the code path they are measuring.
    Counter,
    /// A group of instruments that record increasing and decreasing values
    /// synchronously with the code path they are measuring.
    UpDownCounter,
    /// A group of instruments that record a distribution of values synchronously with
    /// the code path they are measuring.
    Histogram,
    /// A group of instruments that record increasing values in an asynchronous
    /// callback.
    ObservableCounter,
    /// A group of instruments that record increasing and decreasing values in an
    /// asynchronous callback.
    ObservableUpDownCounter,

    /// a group of instruments that record current value synchronously with
    /// the code path they are measuring.
    Gauge,
    ///
    /// a group of instruments that record current values in an asynchronous callback.
    ObservableGauge,
}

impl InstrumentKind {
    /// Select the [Temporality] preference based on [InstrumentKind]
    ///
    /// [exporter-docs]: https://github.com/open-telemetry/opentelemetry-specification/blob/a1c13d59bb7d0fb086df2b3e1eaec9df9efef6cc/specification/metrics/sdk_exporters/otlp.md#additional-configuration
    pub(crate) fn temporality_preference(&self, temporality: Temporality) -> Temporality {
        match temporality {
            Temporality::Cumulative => Temporality::Cumulative,
            Temporality::Delta => match self {
                Self::Counter
                | Self::Histogram
                | Self::ObservableCounter
                | Self::Gauge
                | Self::ObservableGauge => Temporality::Delta,
                Self::UpDownCounter | InstrumentKind::ObservableUpDownCounter => {
                    Temporality::Cumulative
                }
            },
            Temporality::LowMemory => match self {
                Self::Counter | InstrumentKind::Histogram => Temporality::Delta,
                Self::ObservableCounter
                | Self::Gauge
                | Self::ObservableGauge
                | Self::UpDownCounter
                | Self::ObservableUpDownCounter => Temporality::Cumulative,
            },
        }
    }
}
/// Describes the properties of an instrument at creation, used for filtering in
/// views. This is utilized in the `with_view` methods on `MeterProviderBuilder`
/// to customize metric output.
///
/// Users can use a reference to `Instrument` to select which instrument(s) a
/// [Stream] should be applied to.
///
/// # Example
///
/// ```rust
/// use opentelemetry_sdk::metrics::{Instrument, Stream};
///
/// let my_view_change_cardinality = |i: &Instrument| {
///     if i.name() == "my_second_histogram" {
///         // Note: If Stream is invalid, `build()` will return an error. By
///         // calling `.ok()`, any such error is ignored and treated as if the
///         // view does not match the instrument. If this is not the desired
///         // behavior, consider handling the error explicitly.
///         Stream::builder().with_cardinality_limit(2).build().ok()
///     } else {
///         None
///     }
/// };
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Instrument {
    /// The human-readable identifier of the instrument.
    pub(crate) name: Cow<'static, str>,
    /// describes the purpose of the instrument.
    pub(crate) description: Cow<'static, str>,
    /// The functional group of the instrument.
    pub(crate) kind: InstrumentKind,
    /// Unit is the unit of measurement recorded by the instrument.
    pub(crate) unit: Cow<'static, str>,
    /// The instrumentation that created the instrument.
    pub(crate) scope: InstrumentationScope,
}

impl Instrument {
    /// Instrument name.
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }

    /// Instrument kind.
    pub fn kind(&self) -> InstrumentKind {
        self.kind
    }

    /// Instrument unit.
    pub fn unit(&self) -> &str {
        self.unit.as_ref()
    }

    /// Instrument scope.
    pub fn scope(&self) -> &InstrumentationScope {
        &self.scope
    }
}

/// A builder for creating Stream objects.
///
/// # Example
///
/// ```
/// use opentelemetry_sdk::metrics::{Aggregation, Stream};
/// use opentelemetry::Key;
///
/// let stream = Stream::builder()
///     .with_name("my_stream")
///     .with_aggregation(Aggregation::Sum)
///     .with_cardinality_limit(100)
///     .build()
///     .unwrap();
/// ```
#[derive(Default, Debug)]
pub struct StreamBuilder {
    name: Option<Cow<'static, str>>,
    description: Option<Cow<'static, str>>,
    unit: Option<Cow<'static, str>>,
    aggregation: Option<Aggregation>,
    allowed_attribute_keys: Option<Arc<HashSet<Key>>>,
    cardinality_limit: Option<usize>,
}

impl StreamBuilder {
    /// Create a new stream builder with default values.
    pub(crate) fn new() -> Self {
        StreamBuilder::default()
    }

    /// Set the stream name. If this is not set, name provide while creating the instrument will be used.
    pub fn with_name(mut self, name: impl Into<Cow<'static, str>>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the stream description. If this is not set, description provided while creating the instrument will be used.
    pub fn with_description(mut self, description: impl Into<Cow<'static, str>>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the stream unit. If this is not set, unit provided while creating the instrument will be used.
    pub fn with_unit(mut self, unit: impl Into<Cow<'static, str>>) -> Self {
        self.unit = Some(unit.into());
        self
    }

    #[cfg(feature = "spec_unstable_metrics_views")]
    /// Set the stream aggregation. This is used to customize the aggregation.
    /// If not set, the default aggregation based on the instrument kind will be used.
    pub fn with_aggregation(mut self, aggregation: Aggregation) -> Self {
        self.aggregation = Some(aggregation);
        self
    }

    #[cfg(feature = "spec_unstable_metrics_views")]
    /// Set the stream allowed attribute keys.
    ///
    /// Any attribute recorded for the stream with a key not in this set will be
    /// dropped. If the set is empty, all attributes will be dropped.
    /// If this method is not used, all attributes will be kept.
    pub fn with_allowed_attribute_keys(
        mut self,
        attribute_keys: impl IntoIterator<Item = Key>,
    ) -> Self {
        self.allowed_attribute_keys = Some(Arc::new(attribute_keys.into_iter().collect()));
        self
    }

    /// Set the stream cardinality limit. If this is not set, the default limit of 2000 will be used.
    pub fn with_cardinality_limit(mut self, limit: usize) -> Self {
        self.cardinality_limit = Some(limit);
        self
    }

    /// Build a new Stream instance using the configuration in this builder.
    ///
    /// # Returns
    ///
    /// A Result containing the new Stream instance or an error if the build failed.
    pub fn build(self) -> Result<Stream, Box<dyn Error>> {
        // TODO: Avoid copying the validation logic from meter.rs,
        // and instead move it to a common place and do it once.
        // It is a bug that validations are done in meter.rs
        // as it'll not allow users to fix instrumentation mistakes
        // using views.

        // Validate name if provided
        if let Some(name) = &self.name {
            if name.is_empty() {
                return Err(INSTRUMENT_NAME_EMPTY.into());
            }

            if name.len() > super::meter::INSTRUMENT_NAME_MAX_LENGTH {
                return Err(INSTRUMENT_NAME_LENGTH.into());
            }

            if name.starts_with(|c: char| !c.is_ascii_alphabetic()) {
                return Err(INSTRUMENT_NAME_FIRST_ALPHABETIC.into());
            }

            if name.contains(|c: char| {
                !c.is_ascii_alphanumeric()
                    && !super::meter::INSTRUMENT_NAME_ALLOWED_NON_ALPHANUMERIC_CHARS.contains(&c)
            }) {
                return Err(INSTRUMENT_NAME_INVALID_CHAR.into());
            }
        }

        // Validate unit if provided
        if let Some(unit) = &self.unit {
            if unit.len() > super::meter::INSTRUMENT_UNIT_NAME_MAX_LENGTH {
                return Err(INSTRUMENT_UNIT_LENGTH.into());
            }

            if unit.contains(|c: char| !c.is_ascii()) {
                return Err(INSTRUMENT_UNIT_INVALID_CHAR.into());
            }
        }

        // Validate cardinality limit
        if let Some(limit) = self.cardinality_limit {
            if limit == 0 {
                return Err("Cardinality limit must be greater than 0".into());
            }
        }

        // Validate bucket boundaries if using ExplicitBucketHistogram
        if let Some(Aggregation::ExplicitBucketHistogram { boundaries, .. }) = &self.aggregation {
            validate_bucket_boundaries(boundaries)?;
        }

        Ok(Stream {
            name: self.name,
            description: self.description,
            unit: self.unit,
            aggregation: self.aggregation,
            allowed_attribute_keys: self.allowed_attribute_keys,
            cardinality_limit: self.cardinality_limit,
        })
    }
}

fn validate_bucket_boundaries(boundaries: &[f64]) -> Result<(), String> {
    // Validate boundaries do not contain f64::NAN, f64::INFINITY, or f64::NEG_INFINITY
    for boundary in boundaries {
        if boundary.is_nan() || boundary.is_infinite() {
            return Err(
                "Bucket boundaries must not contain NaN, Infinity, or -Infinity".to_string(),
            );
        }
    }

    // validate that buckets are sorted and non-duplicate
    for i in 1..boundaries.len() {
        if boundaries[i] <= boundaries[i - 1] {
            return Err(
                "Bucket boundaries must be sorted and not contain any duplicates".to_string(),
            );
        }
    }

    Ok(())
}

/// Describes the stream of data an instrument produces. Used in `with_view`
/// methods on `MeterProviderBuilder` to customize the metric output.
#[derive(Default, Debug)]
pub struct Stream {
    /// The human-readable identifier of the stream.
    pub(crate) name: Option<Cow<'static, str>>,
    /// Describes the purpose of the data.
    pub(crate) description: Option<Cow<'static, str>>,
    /// the unit of measurement recorded.
    pub(crate) unit: Option<Cow<'static, str>>,
    /// Aggregation the stream uses for an instrument.
    pub(crate) aggregation: Option<Aggregation>,
    /// An allow-list of attribute keys that will be preserved for the stream.
    ///
    /// Any attribute recorded for the stream with a key not in this set will be
    /// dropped. If the set is empty, all attributes will be dropped, if `None` all
    /// attributes will be kept.
    pub(crate) allowed_attribute_keys: Option<Arc<HashSet<Key>>>,

    /// Cardinality limit for the stream.
    pub(crate) cardinality_limit: Option<usize>,
}

impl Stream {
    /// Create a new stream builder with default values.
    pub fn builder() -> StreamBuilder {
        StreamBuilder::new()
    }
}

/// The identifying properties of an instrument.
#[derive(Debug, PartialEq, Eq, Hash)]
pub(crate) struct InstrumentId {
    /// The human-readable identifier of the instrument.
    pub(crate) name: Cow<'static, str>,
    /// Describes the purpose of the data.
    pub(crate) description: Cow<'static, str>,
    /// Defines the functional group of the instrument.
    pub(crate) kind: InstrumentKind,
    /// The unit of measurement recorded.
    pub(crate) unit: Cow<'static, str>,
    /// Number is the underlying data type of the instrument.
    pub(crate) number: Cow<'static, str>,
}

impl InstrumentId {
    /// Instrument names are considered case-insensitive ASCII.
    ///
    /// Standardize the instrument name to always be lowercase so it can be compared
    /// via hash.
    ///
    /// See [naming syntax] for full requirements.
    ///
    /// [naming syntax]: https://github.com/open-telemetry/opentelemetry-specification/blob/v1.21.0/specification/metrics/api.md#instrument-name-syntax
    pub(crate) fn normalize(&mut self) {
        if self.name.chars().any(|c| c.is_ascii_uppercase()) {
            self.name = self.name.to_ascii_lowercase().into();
        }
    }
}

pub(crate) struct ResolvedMeasures<T> {
    pub(crate) measures: Vec<Arc<dyn Measure<T>>>,
}

impl<T: Copy + 'static> SyncInstrument<T> for ResolvedMeasures<T> {
    fn measure(&self, val: T, attrs: &[KeyValue]) {
        for measure in &self.measures {
            measure.call(val, attrs)
        }
    }
}

#[derive(Clone)]
pub(crate) struct Observable<T> {
    measures: Vec<Arc<dyn Measure<T>>>,
}

impl<T> Observable<T> {
    pub(crate) fn new(measures: Vec<Arc<dyn Measure<T>>>) -> Self {
        Self { measures }
    }
}

impl<T: Copy + Send + Sync + 'static> AsyncInstrument<T> for Observable<T> {
    fn observe(&self, measurement: T, attrs: &[KeyValue]) {
        for measure in &self.measures {
            measure.call(measurement, attrs)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::StreamBuilder;
    use crate::metrics::meter::{
        INSTRUMENT_NAME_EMPTY, INSTRUMENT_NAME_FIRST_ALPHABETIC, INSTRUMENT_NAME_INVALID_CHAR,
        INSTRUMENT_NAME_LENGTH, INSTRUMENT_UNIT_INVALID_CHAR, INSTRUMENT_UNIT_LENGTH,
    };

    #[test]
    fn stream_name_validation() {
        // (name, expected error)
        let stream_name_test_cases = vec![
            ("validateName", ""),
            ("_startWithNoneAlphabet", INSTRUMENT_NAME_FIRST_ALPHABETIC),
            ("utf8char锈", INSTRUMENT_NAME_INVALID_CHAR),
            ("a".repeat(255).leak(), ""),
            ("a".repeat(256).leak(), INSTRUMENT_NAME_LENGTH),
            ("invalid name", INSTRUMENT_NAME_INVALID_CHAR),
            ("allow/slash", ""),
            ("allow_under_score", ""),
            ("allow.dots.ok", ""),
            ("", INSTRUMENT_NAME_EMPTY),
            ("\\allow\\slash /sec", INSTRUMENT_NAME_FIRST_ALPHABETIC),
            ("\\allow\\$$slash /sec", INSTRUMENT_NAME_FIRST_ALPHABETIC),
            ("Total $ Count", INSTRUMENT_NAME_INVALID_CHAR),
            (
                "\\test\\UsagePercent(Total) > 80%",
                INSTRUMENT_NAME_FIRST_ALPHABETIC,
            ),
            ("/not / allowed", INSTRUMENT_NAME_FIRST_ALPHABETIC),
        ];

        for (name, expected_error) in stream_name_test_cases {
            let builder = StreamBuilder::new().with_name(name);
            let result = builder.build();

            if expected_error.is_empty() {
                assert!(
                    result.is_ok(),
                    "Expected successful build for name '{}', but got error: {:?}",
                    name,
                    result.err()
                );
            } else {
                let err = result.err().unwrap();
                let err_str = err.to_string();
                assert!(
                    err_str == expected_error,
                    "For name '{}', expected error '{}', but got '{}'",
                    name,
                    expected_error,
                    err_str
                );
            }
        }
    }

    #[test]
    fn stream_unit_validation() {
        // (unit, expected error)
        let stream_unit_test_cases = vec![
            (
                "0123456789012345678901234567890123456789012345678901234567890123",
                INSTRUMENT_UNIT_LENGTH,
            ),
            ("utf8char锈", INSTRUMENT_UNIT_INVALID_CHAR),
            ("kb", ""),
            ("Kb/sec", ""),
            ("%", ""),
            ("", ""),
        ];

        for (unit, expected_error) in stream_unit_test_cases {
            // Use a valid name to isolate unit validation
            let builder = StreamBuilder::new().with_name("valid_name").with_unit(unit);

            let result = builder.build();

            if expected_error.is_empty() {
                assert!(
                    result.is_ok(),
                    "Expected successful build for unit '{}', but got error: {:?}",
                    unit,
                    result.err()
                );
            } else {
                let err = result.err().unwrap();
                let err_str = err.to_string();
                assert!(
                    err_str == expected_error,
                    "For unit '{}', expected error '{}', but got '{}'",
                    unit,
                    expected_error,
                    err_str
                );
            }
        }
    }

    #[test]
    fn stream_cardinality_limit_validation() {
        // Test zero cardinality limit (invalid)
        let builder = StreamBuilder::new()
            .with_name("valid_name")
            .with_cardinality_limit(0);

        let result = builder.build();
        assert!(result.is_err(), "Expected error for zero cardinality limit");
        assert_eq!(
            result.err().unwrap().to_string(),
            "Cardinality limit must be greater than 0",
            "Expected cardinality limit validation error message"
        );

        // Test valid cardinality limits
        let valid_limits = vec![1, 10, 100, 1000];
        for limit in valid_limits {
            let builder = StreamBuilder::new()
                .with_name("valid_name")
                .with_cardinality_limit(limit);

            let result = builder.build();
            assert!(
                result.is_ok(),
                "Expected successful build for cardinality limit {}, but got error: {:?}",
                limit,
                result.err()
            );
        }
    }

    #[test]
    fn stream_valid_build() {
        // Test with valid configuration
        let stream = StreamBuilder::new()
            .with_name("valid_name")
            .with_description("Valid description")
            .with_unit("ms")
            .with_cardinality_limit(100)
            .build();

        assert!(
            stream.is_ok(),
            "Expected valid Stream to be built successfully"
        );
    }

    #[cfg(feature = "spec_unstable_metrics_views")]
    #[test]
    fn stream_histogram_bucket_validation() {
        use super::Aggregation;

        // Test with valid bucket boundaries
        let valid_boundaries = vec![1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0];
        let builder = StreamBuilder::new()
            .with_name("valid_histogram")
            .with_aggregation(Aggregation::ExplicitBucketHistogram {
                boundaries: valid_boundaries.clone(),
                record_min_max: true,
            });

        let result = builder.build();
        assert!(
            result.is_ok(),
            "Expected successful build with valid bucket boundaries"
        );

        // Test with invalid bucket boundaries (NaN and Infinity)

        // Test with NaN
        let invalid_nan_boundaries = vec![1.0, 2.0, f64::NAN, 10.0];

        let builder = StreamBuilder::new()
            .with_name("invalid_histogram_nan")
            .with_aggregation(Aggregation::ExplicitBucketHistogram {
                boundaries: invalid_nan_boundaries,
                record_min_max: true,
            });

        let result = builder.build();
        assert!(
            result.is_err(),
            "Expected error for NaN in bucket boundaries"
        );
        assert_eq!(
            result.err().unwrap().to_string(),
            "Bucket boundaries must not contain NaN, Infinity, or -Infinity",
            "Expected correct validation error for NaN"
        );

        // Test with infinity
        let invalid_inf_boundaries = vec![1.0, 5.0, f64::INFINITY, 100.0];

        let builder = StreamBuilder::new()
            .with_name("invalid_histogram_inf")
            .with_aggregation(Aggregation::ExplicitBucketHistogram {
                boundaries: invalid_inf_boundaries,
                record_min_max: true,
            });

        let result = builder.build();
        assert!(
            result.is_err(),
            "Expected error for Infinity in bucket boundaries"
        );
        assert_eq!(
            result.err().unwrap().to_string(),
            "Bucket boundaries must not contain NaN, Infinity, or -Infinity",
            "Expected correct validation error for Infinity"
        );

        // Test with negative infinity
        let invalid_neg_inf_boundaries = vec![f64::NEG_INFINITY, 5.0, 10.0, 100.0];

        let builder = StreamBuilder::new()
            .with_name("invalid_histogram_neg_inf")
            .with_aggregation(Aggregation::ExplicitBucketHistogram {
                boundaries: invalid_neg_inf_boundaries,
                record_min_max: true,
            });

        let result = builder.build();
        assert!(
            result.is_err(),
            "Expected error for negative Infinity in bucket boundaries"
        );
        assert_eq!(
            result.err().unwrap().to_string(),
            "Bucket boundaries must not contain NaN, Infinity, or -Infinity",
            "Expected correct validation error for negative Infinity"
        );

        // Test with unsorted bucket boundaries
        let unsorted_boundaries = vec![1.0, 5.0, 2.0, 10.0]; // 2.0 comes after 5.0, which is incorrect

        let builder = StreamBuilder::new()
            .with_name("unsorted_histogram")
            .with_aggregation(Aggregation::ExplicitBucketHistogram {
                boundaries: unsorted_boundaries,
                record_min_max: true,
            });

        let result = builder.build();
        assert!(
            result.is_err(),
            "Expected error for unsorted bucket boundaries"
        );
        assert_eq!(
            result.err().unwrap().to_string(),
            "Bucket boundaries must be sorted and not contain any duplicates",
            "Expected correct validation error for unsorted boundaries"
        );

        // Test with duplicate bucket boundaries
        let duplicate_boundaries = vec![1.0, 2.0, 5.0, 5.0, 10.0]; // 5.0 appears twice

        let builder = StreamBuilder::new()
            .with_name("duplicate_histogram")
            .with_aggregation(Aggregation::ExplicitBucketHistogram {
                boundaries: duplicate_boundaries,
                record_min_max: true,
            });

        let result = builder.build();
        assert!(
            result.is_err(),
            "Expected error for duplicate bucket boundaries"
        );
        assert_eq!(
            result.err().unwrap().to_string(),
            "Bucket boundaries must be sorted and not contain any duplicates",
            "Expected correct validation error for duplicate boundaries"
        );
    }
}
