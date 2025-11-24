//! [![github]](https://github.com/dtolnay/basic-toml)&ensp;[![crates-io]](https://crates.io/crates/basic-toml)&ensp;[![docs-rs]](https://docs.rs/basic-toml)
//!
//! [github]: https://img.shields.io/badge/github-8da0cb?style=for-the-badge&labelColor=555555&logo=github
//! [crates-io]: https://img.shields.io/badge/crates.io-fc8d62?style=for-the-badge&labelColor=555555&logo=rust
//! [docs-rs]: https://img.shields.io/badge/docs.rs-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs
//!
//! <br>
//!
//! A library for parsing and producing data in [TOML] format using [Serde].
//!
//! TOML is designed to be "a config file format for humans": minimal and easy
//! to read due to obvious semantics.
//!
//! ```toml
//! [package]
//! name = "basic-toml"
#![doc = concat!("version = \"", env!("CARGO_PKG_VERSION_MAJOR"), ".", env!("CARGO_PKG_VERSION_MINOR"), ".", env!("CARGO_PKG_VERSION_PATCH"), "\"")]
//! authors = ["Alex Crichton <alex@alexcrichton.com>"]
//!
//! [dependencies]
//! serde = "1.0"
//! ```
//!
//! The TOML format is widely used throughout the Rust community for
//! configuration, notably being used by [Cargo], Rust's package manager.
//!
//! [TOML]: https://toml.io
//! [Serde]: https://serde.rs
//! [Cargo]: https://crates.io
//!
//! # Deserialization
//!
//! ```
//! use semver::{Version, VersionReq};
//! use serde_derive::Deserialize;
//! use std::collections::BTreeMap as Map;
//!
//! #[derive(Deserialize)]
//! struct Manifest {
//!     package: Package,
//!     #[serde(default)]
//!     dependencies: Map<String, VersionReq>,
//! }
//!
//! #[derive(Deserialize)]
//! struct Package {
//!     name: String,
//!     version: Version,
//!     #[serde(default)]
//!     authors: Vec<String>,
//! }
//!
//! fn main() {
//!     let manifest: Manifest = basic_toml::from_str(r#"
//!         [package]
//!         name = "basic-toml"
#![doc = concat!("        version = \"", env!("CARGO_PKG_VERSION_MAJOR"), ".", env!("CARGO_PKG_VERSION_MINOR"), ".", env!("CARGO_PKG_VERSION_PATCH"), "\"")]
//!         authors = ["Alex Crichton <alex@alexcrichton.com>"]
//!
//!         [dependencies]
//!         serde = "^1.0"
//!     "#).unwrap();
//!
//!     assert_eq!(manifest.package.name, "basic-toml");
#![doc = concat!("    assert_eq!(manifest.package.version, Version::new(", env!("CARGO_PKG_VERSION_MAJOR"), ", ", env!("CARGO_PKG_VERSION_MINOR"), ", ", env!("CARGO_PKG_VERSION_PATCH"), "));")]
//!     assert_eq!(manifest.package.authors, ["Alex Crichton <alex@alexcrichton.com>"]);
//!     assert_eq!(manifest.dependencies["serde"].to_string(), "^1.0");
//! }
//! ```
//!
//! # Serialization
//!
//! ```
//! use semver::{Version, VersionReq};
//! use serde_derive::Serialize;
//! use std::collections::BTreeMap as Map;
//!
//! #[derive(Serialize)]
//! struct Manifest {
//!     package: Package,
//!     dependencies: Map<String, VersionReq>,
//! }
//!
//! #[derive(Serialize)]
//! struct Package {
//!     name: String,
//!     version: Version,
//!     authors: Vec<String>,
//! }
//!
//! fn main() {
//!     let manifest = Manifest {
//!         package: Package {
//!             name: "basic-toml".to_owned(),
#![doc = concat!("            version: Version::new(", env!("CARGO_PKG_VERSION_MAJOR"), ", ", env!("CARGO_PKG_VERSION_MINOR"), ", ", env!("CARGO_PKG_VERSION_PATCH"), "),")]
//!             authors: vec!["Alex Crichton <alex@alexcrichton.com>".to_owned()],
//!         },
//!         dependencies: {
//!             let mut dependencies = Map::new();
//!             dependencies.insert("serde".to_owned(), "^1.0".parse().unwrap());
//!             dependencies
//!         },
//!     };
//!
//!     let toml = basic_toml::to_string(&manifest).unwrap();
//!     print!("{}", toml);
//! }
//! ```
//!
//! # Spec compatibility
//!
//! TOML v0.5.0.
//!
//! TOML's date and time syntax are not supported.

#![doc(html_root_url = "https://docs.rs/basic-toml/0.1.10")]
#![deny(missing_docs)]
#![allow(
    clippy::bool_to_int_with_if,
    clippy::elidable_lifetime_names,
    clippy::let_underscore_untyped,
    clippy::manual_let_else,
    clippy::manual_range_contains,
    clippy::match_like_matches_macro,
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
    clippy::needless_doctest_main,
    clippy::needless_lifetimes,
    clippy::needless_pass_by_value,
    clippy::similar_names,
    clippy::type_complexity,
    clippy::uninlined_format_args,
    clippy::unnecessary_box_returns,
    clippy::unwrap_or_default
)]

mod de;
mod error;
mod ser;
mod tokens;

pub use crate::de::{from_slice, from_str};
pub use crate::error::Error;
pub use crate::ser::to_string;
