//! # Serde
//!
//! Serde is a framework for ***ser***ializing and ***de***serializing Rust data
//! structures efficiently and generically.
//!
//! The Serde ecosystem consists of data structures that know how to serialize
//! and deserialize themselves along with data formats that know how to
//! serialize and deserialize other things. Serde provides the layer by which
//! these two groups interact with each other, allowing any supported data
//! structure to be serialized and deserialized using any supported data format.
//!
//! See the Serde website <https://serde.rs> for additional documentation and
//! usage examples.
//!
//! ## Design
//!
//! Where many other languages rely on runtime reflection for serializing data,
//! Serde is instead built on Rust's powerful trait system. A data structure
//! that knows how to serialize and deserialize itself is one that implements
//! Serde's `Serialize` and `Deserialize` traits (or uses Serde's derive
//! attribute to automatically generate implementations at compile time). This
//! avoids any overhead of reflection or runtime type information. In fact in
//! many situations the interaction between data structure and data format can
//! be completely optimized away by the Rust compiler, leaving Serde
//! serialization to perform the same speed as a handwritten serializer for the
//! specific selection of data structure and data format.
//!
//! ## Data formats
//!
//! The following is a partial list of data formats that have been implemented
//! for Serde by the community.
//!
//! - [JSON], the ubiquitous JavaScript Object Notation used by many HTTP APIs.
//! - [Postcard], a no\_std and embedded-systems friendly compact binary format.
//! - [CBOR], a Concise Binary Object Representation designed for small message
//!   size without the need for version negotiation.
//! - [YAML], a self-proclaimed human-friendly configuration language that ain't
//!   markup language.
//! - [MessagePack], an efficient binary format that resembles a compact JSON.
//! - [TOML], a minimal configuration format used by [Cargo].
//! - [Pickle], a format common in the Python world.
//! - [RON], a Rusty Object Notation.
//! - [BSON], the data storage and network transfer format used by MongoDB.
//! - [Avro], a binary format used within Apache Hadoop, with support for schema
//!   definition.
//! - [JSON5], a superset of JSON including some productions from ES5.
//! - [URL] query strings, in the x-www-form-urlencoded format.
//! - [Starlark], the format used for describing build targets by the Bazel and
//!   Buck build systems. *(serialization only)*
//! - [Envy], a way to deserialize environment variables into Rust structs.
//!   *(deserialization only)*
//! - [Envy Store], a way to deserialize [AWS Parameter Store] parameters into
//!   Rust structs. *(deserialization only)*
//! - [S-expressions], the textual representation of code and data used by the
//!   Lisp language family.
//! - [D-Bus]'s binary wire format.
//! - [FlexBuffers], the schemaless cousin of Google's FlatBuffers zero-copy
//!   serialization format.
//! - [Bencode], a simple binary format used in the BitTorrent protocol.
//! - [Token streams], for processing Rust procedural macro input.
//!   *(deserialization only)*
//! - [DynamoDB Items], the format used by [rusoto_dynamodb] to transfer data to
//!   and from DynamoDB.
//! - [Hjson], a syntax extension to JSON designed around human reading and
//!   editing. *(deserialization only)*
//! - [CSV], Comma-separated values is a tabular text file format.
//!
//! [JSON]: https://github.com/serde-rs/json
//! [Postcard]: https://github.com/jamesmunns/postcard
//! [CBOR]: https://github.com/enarx/ciborium
//! [YAML]: https://github.com/dtolnay/serde-yaml
//! [MessagePack]: https://github.com/3Hren/msgpack-rust
//! [TOML]: https://docs.rs/toml
//! [Pickle]: https://github.com/birkenfeld/serde-pickle
//! [RON]: https://github.com/ron-rs/ron
//! [BSON]: https://github.com/mongodb/bson-rust
//! [Avro]: https://docs.rs/apache-avro
//! [JSON5]: https://github.com/callum-oakley/json5-rs
//! [URL]: https://docs.rs/serde_qs
//! [Starlark]: https://github.com/dtolnay/serde-starlark
//! [Envy]: https://github.com/softprops/envy
//! [Envy Store]: https://github.com/softprops/envy-store
//! [Cargo]: https://doc.rust-lang.org/cargo/reference/manifest.html
//! [AWS Parameter Store]: https://docs.aws.amazon.com/systems-manager/latest/userguide/systems-manager-parameter-store.html
//! [S-expressions]: https://github.com/rotty/lexpr-rs
//! [D-Bus]: https://docs.rs/zvariant
//! [FlexBuffers]: https://github.com/google/flatbuffers/tree/master/rust/flexbuffers
//! [Bencode]: https://github.com/P3KI/bendy
//! [Token streams]: https://github.com/oxidecomputer/serde_tokenstream
//! [DynamoDB Items]: https://docs.rs/serde_dynamo
//! [rusoto_dynamodb]: https://docs.rs/rusoto_dynamodb
//! [Hjson]: https://github.com/Canop/deser-hjson
//! [CSV]: https://docs.rs/csv

////////////////////////////////////////////////////////////////////////////////

// Serde types in rustdoc of other crates get linked to here.
#![doc(html_root_url = "https://docs.rs/serde/1.0.228")]
// Support using Serde without the standard library!
#![cfg_attr(not(feature = "std"), no_std)]
// Show which crate feature enables conditionally compiled APIs in documentation.
#![cfg_attr(docsrs, feature(doc_cfg, rustdoc_internals))]
#![cfg_attr(docsrs, allow(internal_features))]
// Unstable functionality only if the user asks for it. For tracking and
// discussion of these features please refer to this issue:
//
//    https://github.com/serde-rs/serde/issues/812
#![cfg_attr(feature = "unstable", feature(never_type))]
#![allow(
    unknown_lints,
    bare_trait_objects,
    deprecated,
    mismatched_lifetime_syntaxes
)]
// Ignored clippy and clippy_pedantic lints
#![allow(
    // clippy bug: https://github.com/rust-lang/rust-clippy/issues/5704
    clippy::unnested_or_patterns,
    // clippy bug: https://github.com/rust-lang/rust-clippy/issues/7768
    clippy::semicolon_if_nothing_returned,
    // not available in our oldest supported compiler
    clippy::empty_enum,
    clippy::type_repetition_in_bounds, // https://github.com/rust-lang/rust-clippy/issues/8772
    // integer and float ser/de requires these sorts of casts
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    // things are often more readable this way
    clippy::cast_lossless,
    clippy::module_name_repetitions,
    clippy::single_match_else,
    clippy::type_complexity,
    clippy::use_self,
    clippy::zero_prefixed_literal,
    // correctly used
    clippy::derive_partial_eq_without_eq,
    clippy::enum_glob_use,
    clippy::explicit_auto_deref,
    clippy::incompatible_msrv,
    clippy::let_underscore_untyped,
    clippy::map_err_ignore,
    clippy::new_without_default,
    clippy::result_unit_err,
    clippy::wildcard_imports,
    // not practical
    clippy::needless_pass_by_value,
    clippy::similar_names,
    clippy::too_many_lines,
    // preference
    clippy::doc_markdown,
    clippy::elidable_lifetime_names,
    clippy::needless_lifetimes,
    clippy::unseparated_literal_suffix,
    // false positive
    clippy::needless_doctest_main,
    // noisy
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
)]
// Restrictions
#![deny(clippy::question_mark_used)]
// Rustc lints.
#![deny(missing_docs, unused_imports)]

////////////////////////////////////////////////////////////////////////////////

#[cfg(feature = "alloc")]
extern crate alloc;

// Rustdoc has a lot of shortcomings related to cross-crate re-exports that make
// the rendered documentation of serde_core traits in serde more challenging to
// understand than the equivalent documentation of the same items in serde_core.
// https://github.com/rust-lang/rust/labels/A-cross-crate-reexports
// So, just for the purpose of docs.rs documentation, we inline the contents of
// serde_core into serde. This sidesteps all the cross-crate rustdoc bugs.
#[cfg(docsrs)]
#[macro_use]
#[path = "core/crate_root.rs"]
mod crate_root;

#[cfg(docsrs)]
#[macro_use]
#[path = "core/macros.rs"]
mod macros;

#[cfg(not(docsrs))]
macro_rules! crate_root {
    () => {
        /// A facade around all the types we need from the `std`, `core`, and `alloc`
        /// crates. This avoids elaborate import wrangling having to happen in every
        /// module.
        mod lib {
            mod core {
                #[cfg(not(feature = "std"))]
                pub use core::*;
                #[cfg(feature = "std")]
                pub use std::*;
            }

            pub use self::core::{f32, f64};
            pub use self::core::{ptr, str};

            #[cfg(any(feature = "std", feature = "alloc"))]
            pub use self::core::slice;

            pub use self::core::clone;
            pub use self::core::convert;
            pub use self::core::default;
            pub use self::core::fmt::{self, Debug, Display, Write as FmtWrite};
            pub use self::core::marker::{self, PhantomData};
            pub use self::core::option;
            pub use self::core::result;

            #[cfg(all(feature = "alloc", not(feature = "std")))]
            pub use alloc::borrow::{Cow, ToOwned};
            #[cfg(feature = "std")]
            pub use std::borrow::{Cow, ToOwned};

            #[cfg(all(feature = "alloc", not(feature = "std")))]
            pub use alloc::string::{String, ToString};
            #[cfg(feature = "std")]
            pub use std::string::{String, ToString};

            #[cfg(all(feature = "alloc", not(feature = "std")))]
            pub use alloc::vec::Vec;
            #[cfg(feature = "std")]
            pub use std::vec::Vec;

            #[cfg(all(feature = "alloc", not(feature = "std")))]
            pub use alloc::boxed::Box;
            #[cfg(feature = "std")]
            pub use std::boxed::Box;
        }

        // None of this crate's error handling needs the `From::from` error conversion
        // performed implicitly by the `?` operator or the standard library's `try!`
        // macro. This simplified macro gives a 5.5% improvement in compile time
        // compared to standard `try!`, and 9% improvement compared to `?`.
        #[cfg(not(no_serde_derive))]
        macro_rules! tri {
            ($expr:expr) => {
                match $expr {
                    Ok(val) => val,
                    Err(err) => return Err(err),
                }
            };
        }

        ////////////////////////////////////////////////////////////////////////////////

        pub use serde_core::{
            de, forward_to_deserialize_any, ser, Deserialize, Deserializer, Serialize, Serializer,
        };

        // Used by generated code and doc tests. Not public API.
        #[doc(hidden)]
        mod private;

        include!(concat!(env!("OUT_DIR"), "/private.rs"));
    };
}

crate_root!();

mod integer128;

// Re-export #[derive(Serialize, Deserialize)].
//
// The reason re-exporting is not enabled by default is that disabling it would
// be annoying for crates that provide handwritten impls or data formats. They
// would need to disable default features and then explicitly re-enable std.
#[cfg(feature = "serde_derive")]
extern crate serde_derive;

/// Derive macro available if serde is built with `features = ["derive"]`.
#[cfg(feature = "serde_derive")]
#[cfg_attr(docsrs, doc(cfg(feature = "derive")))]
pub use serde_derive::{Deserialize, Serialize};

#[macro_export]
#[doc(hidden)]
macro_rules! __require_serde_not_serde_core {
    () => {};
}
