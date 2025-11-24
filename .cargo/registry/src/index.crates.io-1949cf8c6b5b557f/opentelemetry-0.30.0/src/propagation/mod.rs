//! # OpenTelemetry Propagator interface
//! Cross-cutting concerns send their state to the next process using Propagators, which are defined
//! as objects used to read and write context data to and from messages exchanged by the applications.
//!
//! `Propagator`s leverage the [`Context`] to inject and extract data for each cross-cutting concern,
//! such as `TraceContext` and [`Baggage`].
//!
//! The Propagators API is expected to be leveraged by users writing instrumentation libraries.
//!
//! Currently, the following `Propagator` types are supported:
//! -  [`TextMapPropagator`], inject values into and extracts values from carriers as string key/value pairs
//!
//! A binary Propagator type will be added in
//! the future, See [tracking issues](https://github.com/open-telemetry/opentelemetry-specification/issues/437)).
//!
//! `Propagator`s uses [`Injector`] and [`Extractor`] to read and write context data to and from messages.
//! Each specific Propagator type defines its expected carrier type, such as a string map or a byte array.
//!
//! [`Baggage`]: crate::baggage::Baggage
//! [`Context`]: crate::Context

use std::collections::HashMap;

pub mod composite;
pub mod text_map_propagator;

pub use composite::TextMapCompositePropagator;
pub use text_map_propagator::TextMapPropagator;

/// Injector provides an interface for adding fields from an underlying struct like `HashMap`
pub trait Injector {
    /// Add a key and value to the underlying data.
    fn set(&mut self, key: &str, value: String);
}

/// Extractor provides an interface for removing fields from an underlying struct like `HashMap`
pub trait Extractor {
    /// Get a value from a key from the underlying data.
    fn get(&self, key: &str) -> Option<&str>;

    /// Collect all the keys from the underlying data.
    fn keys(&self) -> Vec<&str>;
}

impl<S: std::hash::BuildHasher> Injector for HashMap<String, String, S> {
    /// Set a key and value in the HashMap.
    fn set(&mut self, key: &str, value: String) {
        self.insert(key.to_lowercase(), value);
    }
}

impl<S: std::hash::BuildHasher> Extractor for HashMap<String, String, S> {
    /// Get a value for a key from the HashMap.
    fn get(&self, key: &str) -> Option<&str> {
        self.get(&key.to_lowercase()).map(|v| v.as_str())
    }

    /// Collect all the keys from the HashMap.
    fn keys(&self) -> Vec<&str> {
        self.keys().map(|k| k.as_str()).collect::<Vec<_>>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_map_get() {
        let mut carrier = HashMap::new();
        carrier.set("headerName", "value".to_string());

        assert_eq!(
            Extractor::get(&carrier, "HEADERNAME"),
            Some("value"),
            "case insensitive extraction"
        );
    }

    #[test]
    fn hash_map_keys() {
        let mut carrier = HashMap::new();
        carrier.set("headerName1", "value1".to_string());
        carrier.set("headerName2", "value2".to_string());

        let got = Extractor::keys(&carrier);
        assert_eq!(got.len(), 2);
        assert!(got.contains(&"headername1"));
        assert!(got.contains(&"headername2"));
    }
}
