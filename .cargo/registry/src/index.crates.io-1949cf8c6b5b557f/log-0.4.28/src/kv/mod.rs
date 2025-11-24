//! Structured logging.
//!
//! Add the `kv` feature to your `Cargo.toml` to enable
//! this module:
//!
//! ```toml
//! [dependencies.log]
//! features = ["kv"]
//! ```
//!
//! # Structured logging in `log`
//!
//! Structured logging enhances traditional text-based log records with user-defined
//! attributes. Structured logs can be analyzed using a variety of data processing
//! techniques, without needing to find and parse attributes from unstructured text first.
//!
//! In `log`, user-defined attributes are part of a [`Source`] on the log record.
//! Each attribute is a key-value; a pair of [`Key`] and [`Value`]. Keys are strings
//! and values are a datum of any type that can be formatted or serialized. Simple types
//! like strings, booleans, and numbers are supported, as well as arbitrarily complex
//! structures involving nested objects and sequences.
//!
//! ## Adding key-values to log records
//!
//! Key-values appear before the message format in the `log!` macros:
//!
//! ```
//! # use log::info;
//! info!(a = 1; "Something of interest");
//! ```
//!
//! Key-values support the same shorthand identifier syntax as `format_args`:
//!
//! ```
//! # use log::info;
//! let a = 1;
//!
//! info!(a; "Something of interest");
//! ```
//!
//! Values are capturing using the [`ToValue`] trait by default. To capture a value
//! using a different trait implementation, use a modifier after its key. Here's how
//! the same example can capture `a` using its `Debug` implementation instead:
//!
//! ```
//! # use log::info;
//! info!(a:? = 1; "Something of interest");
//! ```
//!
//! The following capturing modifiers are supported:
//!
//! - `:?` will capture the value using `Debug`.
//! - `:debug` will capture the value using `Debug`.
//! - `:%` will capture the value using `Display`.
//! - `:display` will capture the value using `Display`.
//! - `:err` will capture the value using `std::error::Error` (requires the `kv_std` feature).
//! - `:sval` will capture the value using `sval::Value` (requires the `kv_sval` feature).
//! - `:serde` will capture the value using `serde::Serialize` (requires the `kv_serde` feature).
//!
//! ## Working with key-values on log records
//!
//! Use the [`Record::key_values`](../struct.Record.html#method.key_values) method to access key-values.
//!
//! Individual values can be pulled from the source by their key:
//!
//! ```
//! # fn main() -> Result<(), log::kv::Error> {
//! use log::kv::{Source, Key, Value};
//! # let record = log::Record::builder().key_values(&[("a", 1)]).build();
//!
//! // info!(a = 1; "Something of interest");
//!
//! let a: Value = record.key_values().get(Key::from("a")).unwrap();
//! assert_eq!(1, a.to_i64().unwrap());
//! # Ok(())
//! # }
//! ```
//!
//! All key-values can also be enumerated using a [`VisitSource`]:
//!
//! ```
//! # fn main() -> Result<(), log::kv::Error> {
//! use std::collections::BTreeMap;
//!
//! use log::kv::{self, Source, Key, Value, VisitSource};
//!
//! struct Collect<'kvs>(BTreeMap<Key<'kvs>, Value<'kvs>>);
//!
//! impl<'kvs> VisitSource<'kvs> for Collect<'kvs> {
//!     fn visit_pair(&mut self, key: Key<'kvs>, value: Value<'kvs>) -> Result<(), kv::Error> {
//!         self.0.insert(key, value);
//!
//!         Ok(())
//!     }
//! }
//!
//! let mut visitor = Collect(BTreeMap::new());
//!
//! # let record = log::Record::builder().key_values(&[("a", 1), ("b", 2), ("c", 3)]).build();
//! // info!(a = 1, b = 2, c = 3; "Something of interest");
//!
//! record.key_values().visit(&mut visitor)?;
//!
//! let collected = visitor.0;
//!
//! assert_eq!(
//!     vec!["a", "b", "c"],
//!     collected
//!         .keys()
//!         .map(|k| k.as_str())
//!         .collect::<Vec<_>>(),
//! );
//! # Ok(())
//! # }
//! ```
//!
//! [`Value`]s have methods for conversions to common types:
//!
//! ```
//! # fn main() -> Result<(), log::kv::Error> {
//! use log::kv::{Source, Key};
//! # let record = log::Record::builder().key_values(&[("a", 1)]).build();
//!
//! // info!(a = 1; "Something of interest");
//!
//! let a = record.key_values().get(Key::from("a")).unwrap();
//!
//! assert_eq!(1, a.to_i64().unwrap());
//! # Ok(())
//! # }
//! ```
//!
//! Values also have their own [`VisitValue`] type. Value visitors are a lightweight
//! API for working with primitives types:
//!
//! ```
//! # fn main() -> Result<(), log::kv::Error> {
//! use log::kv::{self, Source, Key, VisitValue};
//! # let record = log::Record::builder().key_values(&[("a", 1)]).build();
//!
//! struct IsNumeric(bool);
//!
//! impl<'kvs> VisitValue<'kvs> for IsNumeric {
//!     fn visit_any(&mut self, _value: kv::Value) -> Result<(), kv::Error> {
//!         self.0 = false;
//!         Ok(())
//!     }
//!
//!     fn visit_u64(&mut self, _value: u64) -> Result<(), kv::Error> {
//!         self.0 = true;
//!         Ok(())
//!     }
//!
//!     fn visit_i64(&mut self, _value: i64) -> Result<(), kv::Error> {
//!         self.0 = true;
//!         Ok(())
//!     }
//!
//!     fn visit_u128(&mut self, _value: u128) -> Result<(), kv::Error> {
//!         self.0 = true;
//!         Ok(())
//!     }
//!
//!     fn visit_i128(&mut self, _value: i128) -> Result<(), kv::Error> {
//!         self.0 = true;
//!         Ok(())
//!     }
//!
//!     fn visit_f64(&mut self, _value: f64) -> Result<(), kv::Error> {
//!         self.0 = true;
//!         Ok(())
//!     }
//! }
//!
//! // info!(a = 1; "Something of interest");
//!
//! let a = record.key_values().get(Key::from("a")).unwrap();
//!
//! let mut visitor = IsNumeric(false);
//!
//! a.visit(&mut visitor)?;
//!
//! let is_numeric = visitor.0;
//!
//! assert!(is_numeric);
//! # Ok(())
//! # }
//! ```
//!
//! To serialize a value to a format like JSON, you can also use either `serde` or `sval`:
//!
//! ```
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # #[cfg(feature = "serde")]
//! # {
//! # use log::kv::Key;
//! #[derive(serde::Serialize)]
//! struct Data {
//!     a: i32, b: bool,
//!     c: &'static str,
//! }
//!
//! let data = Data { a: 1, b: true, c: "Some data" };
//!
//! # let source = [("a", log::kv::Value::from_serde(&data))];
//! # let record = log::Record::builder().key_values(&source).build();
//! // info!(a = data; "Something of interest");
//!
//! let a = record.key_values().get(Key::from("a")).unwrap();
//!
//! assert_eq!("{\"a\":1,\"b\":true,\"c\":\"Some data\"}", serde_json::to_string(&a)?);
//! # }
//! # Ok(())
//! # }
//! ```
//!
//! The choice of serialization framework depends on the needs of the consumer.
//! If you're in a no-std environment, you can use `sval`. In other cases, you can use `serde`.
//! Log producers and log consumers don't need to agree on the serialization framework.
//! A value can be captured using its `serde::Serialize` implementation and still be serialized
//! through `sval` without losing any structure or data.
//!
//! Values can also always be formatted using the standard `Debug` and `Display`
//! traits:
//!
//! ```
//! # use log::kv::Key;
//! #[derive(Debug)]
//! struct Data {
//!     a: i32,
//!     b: bool,
//!     c: &'static str,
//! }
//!
//! let data = Data { a: 1, b: true, c: "Some data" };
//!
//! # let source = [("a", log::kv::Value::from_debug(&data))];
//! # let record = log::Record::builder().key_values(&source).build();
//! // info!(a = data; "Something of interest");
//!
//! let a = record.key_values().get(Key::from("a")).unwrap();
//!
//! assert_eq!("Data { a: 1, b: true, c: \"Some data\" }", format!("{a:?}"));
//! ```

mod error;
mod key;

#[cfg(not(feature = "kv_unstable"))]
mod source;
#[cfg(not(feature = "kv_unstable"))]
mod value;

pub use self::error::Error;
pub use self::key::{Key, ToKey};
pub use self::source::{Source, VisitSource};
pub use self::value::{ToValue, Value, VisitValue};

#[cfg(feature = "kv_unstable")]
pub mod source;
#[cfg(feature = "kv_unstable")]
pub mod value;

#[cfg(feature = "kv_unstable")]
pub use self::source::Visitor;
