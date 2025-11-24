use opentelemetry::{
    baggage::{BaggageExt, KeyValueMetadata},
    otel_warn,
    propagation::{text_map_propagator::FieldIter, Extractor, Injector, TextMapPropagator},
    Context,
};
use percent_encoding::{percent_decode_str, utf8_percent_encode, AsciiSet, CONTROLS};
use std::iter;
use std::sync::OnceLock;

static BAGGAGE_HEADER: &str = "baggage";
const FRAGMENT: &AsciiSet = &CONTROLS.add(b' ').add(b'"').add(b';').add(b',').add(b'=');

// TODO Replace this with LazyLock once it is stable.
static BAGGAGE_FIELDS: OnceLock<[String; 1]> = OnceLock::new();
#[inline]
fn baggage_fields() -> &'static [String; 1] {
    BAGGAGE_FIELDS.get_or_init(|| [BAGGAGE_HEADER.to_owned()])
}

/// Propagates name-value pairs in [W3C Baggage] format.
///
/// Baggage is used to annotate telemetry, adding context and
/// information to metrics, traces, and logs. It is an abstract data type
/// represented by a set of name-value pairs describing user-defined properties.
/// Each name in a [`Baggage`] is associated with exactly one value.
/// `Baggage`s are serialized according to the editor's draft of
/// the [W3C Baggage] specification.
///
/// # Examples
///
/// ```
/// use opentelemetry::{baggage::{Baggage, BaggageExt}, propagation::TextMapPropagator};
/// use opentelemetry_sdk::propagation::BaggagePropagator;
/// use std::collections::HashMap;
///
/// // Example baggage value passed in externally via http headers
/// let mut headers = HashMap::new();
/// headers.insert("baggage".to_string(), "user_id=1".to_string());
///
/// let propagator = BaggagePropagator::new();
/// // can extract from any type that impls `Extractor`, usually an HTTP header map
/// let cx = propagator.extract(&headers);
///
/// // Iterate over extracted name-value pairs
/// for (name, value) in cx.baggage() {
///     // ...
/// }
///
/// // Add new baggage
/// let mut baggage = Baggage::new();
/// let _ = baggage.insert("server_id", "42");
///
/// let cx_with_additions = cx.with_baggage(baggage);
///
/// // Inject baggage into http request
/// propagator.inject_context(&cx_with_additions, &mut headers);
///
/// let header_value = headers.get("baggage").expect("header is injected");
/// assert!(!header_value.contains("user_id=1"), "still contains previous name-value");
/// assert!(header_value.contains("server_id=42"), "does not contain new name-value pair");
/// ```
///
/// [W3C Baggage]: https://w3c.github.io/baggage
/// [`Baggage`]: opentelemetry::baggage::Baggage
#[derive(Debug, Default)]
pub struct BaggagePropagator {
    _private: (),
}

impl BaggagePropagator {
    /// Construct a new baggage propagator.
    pub fn new() -> Self {
        BaggagePropagator { _private: () }
    }
}

impl TextMapPropagator for BaggagePropagator {
    /// Encodes the values of the `Context` and injects them into the provided `Injector`.
    fn inject_context(&self, cx: &Context, injector: &mut dyn Injector) {
        let baggage = cx.baggage();
        if !baggage.is_empty() {
            let header_value = baggage
                .iter()
                .map(|(name, (value, metadata))| {
                    let metadata_str = metadata.as_str().trim();
                    let metadata_prefix = if metadata_str.is_empty() { "" } else { ";" };
                    utf8_percent_encode(name.as_str().trim(), FRAGMENT)
                        .chain(iter::once("="))
                        .chain(utf8_percent_encode(value.as_str().trim(), FRAGMENT))
                        .chain(iter::once(metadata_prefix))
                        .chain(iter::once(metadata_str))
                        .collect()
                })
                .collect::<Vec<String>>()
                .join(",");
            injector.set(BAGGAGE_HEADER, header_value);
        }
    }

    /// Extracts a `Context` with baggage values from a `Extractor`.
    fn extract_with_context(&self, cx: &Context, extractor: &dyn Extractor) -> Context {
        if let Some(header_value) = extractor.get(BAGGAGE_HEADER) {
            let baggage = header_value.split(',').filter_map(|context_value| {
                if let Some((name_and_value, props)) = context_value
                    .split(';')
                    .collect::<Vec<&str>>()
                    .split_first()
                {
                    let mut iter = name_and_value.split('=');
                    if let (Some(name), Some(value)) = (iter.next(), iter.next()) {
                        let decode_name = percent_decode_str(name).decode_utf8();
                        let decode_value = percent_decode_str(value).decode_utf8();

                        if let (Ok(name), Ok(value)) = (decode_name, decode_value) {
                            // Here we don't store the first ; into baggage since it should be treated
                            // as separator rather part of metadata
                            let decoded_props = props
                                .iter()
                                .flat_map(|prop| percent_decode_str(prop).decode_utf8())
                                .map(|prop| prop.trim().to_string())
                                .collect::<Vec<String>>()
                                .join(";"); // join with ; because we deleted all ; when calling split above

                            Some(KeyValueMetadata::new(
                                name.trim().to_owned(),
                                value.trim().to_string(),
                                decoded_props.as_str(),
                            ))
                        } else {
                            otel_warn!(
                                name: "BaggagePropagator.Extract.InvalidUTF8",
                                message = "Invalid UTF8 string in key values",
                                baggage_header = header_value,
                            );
                            None
                        }
                    } else {
                        otel_warn!(
                            name: "BaggagePropagator.Extract.InvalidKeyValueFormat",
                            message = "Invalid baggage key-value format",
                            baggage_header = header_value,
                        );
                        None
                    }
                } else {
                    otel_warn!(
                        name: "BaggagePropagator.Extract.InvalidFormat",
                        message = "Invalid baggage format",
                        baggage_header = header_value);
                    None
                }
            });
            cx.with_baggage(baggage)
        } else {
            cx.clone()
        }
    }

    fn fields(&self) -> FieldIter<'_> {
        FieldIter::new(baggage_fields())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentelemetry::{baggage::BaggageMetadata, Key, KeyValue, StringValue, Value};
    use std::collections::HashMap;

    #[rustfmt::skip]
    fn valid_extract_data() -> Vec<(&'static str, HashMap<Key, StringValue>)> {
        vec![
            // "valid w3cHeader"
            ("key1=val1,key2=val2", vec![(Key::new("key1"), StringValue::from("val1")), (Key::new("key2"), StringValue::from("val2"))].into_iter().collect()),
            // "valid w3cHeader with spaces"
            ("key1 =   val1,  key2 =val2   ", vec![(Key::new("key1"), StringValue::from("val1")), (Key::new("key2"), StringValue::from("val2"))].into_iter().collect()),
            // "valid header with url-escaped comma"
            ("key1=val1,key2=val2%2Cval3", vec![(Key::new("key1"), StringValue::from("val1")), (Key::new("key2"), StringValue::from("val2,val3"))].into_iter().collect()),
            // "valid header with an invalid header"
            ("key1=val1,key2=val2,a,val3", vec![(Key::new("key1"), StringValue::from("val1")), (Key::new("key2"), StringValue::from("val2"))].into_iter().collect()),
            // "valid header with no value"
            ("key1=,key2=val2", vec![(Key::new("key1"), StringValue::from("")), (Key::new("key2"), StringValue::from("val2"))].into_iter().collect()),
        ]
    }

    #[rustfmt::skip]
    #[allow(clippy::type_complexity)]
    fn valid_extract_data_with_metadata() -> Vec<(&'static str, HashMap<Key, (StringValue, BaggageMetadata)>)> {
        vec![
            // "valid w3cHeader with properties"
            ("key1=val1,key2=val2;prop=1", vec![(Key::new("key1"), (StringValue::from("val1"), BaggageMetadata::default())), (Key::new("key2"), (StringValue::from("val2"), BaggageMetadata::from("prop=1")))].into_iter().collect()),
            // prop can don't need to be key value pair
            ("key1=val1,key2=val2;prop1", vec![(Key::new("key1"), (StringValue::from("val1"), BaggageMetadata::default())), (Key::new("key2"), (StringValue::from("val2"), BaggageMetadata::from("prop1")))].into_iter().collect()),
            ("key1=value1;property1;property2, key2 = value2, key3=value3; propertyKey=propertyValue",
             vec![
                 (Key::new("key1"), (StringValue::from("value1"), BaggageMetadata::from("property1;property2"))),
                 (Key::new("key2"), (StringValue::from("value2"), BaggageMetadata::default())),
                 (Key::new("key3"), (StringValue::from("value3"), BaggageMetadata::from("propertyKey=propertyValue"))),
             ].into_iter().collect()),
        ]
    }

    #[rustfmt::skip]
    fn valid_inject_data() -> Vec<(Vec<KeyValue>, Vec<&'static str>)> {
        vec![
            // "two simple values"
            (vec![KeyValue::new("key1", "val1"), KeyValue::new("key2", "val2")], vec!["key1=val1", "key2=val2"]),
            // "two values with escaped chars"
            (vec![KeyValue::new("key1", "val1,val2"), KeyValue::new("key2", "val3=4")], vec!["key1=val1%2Cval2", "key2=val3%3D4"]),
            // "values of non-string non-array types"
            (
                vec![
                    KeyValue::new("key1", true),
                    KeyValue::new("key2", Value::I64(123)),
                    KeyValue::new("key3", Value::F64(123.567)),
                ],
                vec![
                    "key1=true",
                    "key2=123",
                    "key3=123.567",
                ],
            ),
            // "values of array types"
            (
                vec![
                    KeyValue::new("key1", Value::Array(vec![true, false].into())),
                    KeyValue::new("key2", Value::Array(vec![123, 456].into())),
                    KeyValue::new("key3", Value::Array(vec![StringValue::from("val1"), StringValue::from("val2")].into())),
                ],
                vec![
                    "key1=[true%2Cfalse]",
                    "key2=[123%2C456]",
                    "key3=[%22val1%22%2C%22val2%22]",
                ],
            ),
        ]
    }

    #[rustfmt::skip]
    fn valid_inject_data_metadata() -> Vec<(Vec<KeyValueMetadata>, Vec<&'static str>)> {
        vec![
            (
                vec![
                    KeyValueMetadata::new("key1", "val1", "prop1"),
                    KeyValue::new("key2", "val2").into(),
                    KeyValueMetadata::new("key3", "val3", "anykey=anyvalue"),
                ],
                vec![
                    "key1=val1;prop1",
                    "key2=val2",
                    "key3=val3;anykey=anyvalue",
                ],
            )
        ]
    }

    #[test]
    fn extract_baggage() {
        let propagator = BaggagePropagator::new();

        for (header_value, kvs) in valid_extract_data() {
            let mut extractor: HashMap<String, String> = HashMap::new();
            extractor.insert(BAGGAGE_HEADER.to_string(), header_value.to_string());
            let context = propagator.extract(&extractor);
            let baggage = context.baggage();

            assert_eq!(kvs.len(), baggage.len());
            for (key, (value, _metadata)) in baggage {
                assert_eq!(Some(value), kvs.get(key))
            }
        }
    }

    #[test]
    fn inject_baggage() {
        let propagator = BaggagePropagator::new();

        for (kvm, header_parts) in valid_inject_data() {
            let mut injector = HashMap::new();
            let cx = Context::current_with_baggage(kvm);
            propagator.inject_context(&cx, &mut injector);
            let header_value = injector.get(BAGGAGE_HEADER).unwrap();
            assert_eq!(header_parts.join(",").len(), header_value.len(),);
            for header_part in &header_parts {
                assert!(header_value.contains(header_part),)
            }
        }
    }

    #[test]
    fn extract_baggage_with_metadata() {
        let propagator = BaggagePropagator::new();
        for (header_value, kvm) in valid_extract_data_with_metadata() {
            let mut extractor: HashMap<String, String> = HashMap::new();
            extractor.insert(BAGGAGE_HEADER.to_string(), header_value.to_string());
            let context = propagator.extract(&extractor);
            let baggage = context.baggage();

            assert_eq!(kvm.len(), baggage.len());
            for (key, value_and_prop) in baggage {
                assert_eq!(Some(value_and_prop), kvm.get(key))
            }
        }
    }

    #[test]
    fn inject_baggage_with_metadata() {
        let propagator = BaggagePropagator::new();

        for (kvm, header_parts) in valid_inject_data_metadata() {
            let mut injector = HashMap::new();
            let cx = Context::current_with_baggage(kvm);
            propagator.inject_context(&cx, &mut injector);
            let header_value = injector.get(BAGGAGE_HEADER).unwrap();

            assert_eq!(header_parts.join(",").len(), header_value.len());
            for header_part in &header_parts {
                assert!(header_value.contains(header_part),)
            }
        }
    }
}
