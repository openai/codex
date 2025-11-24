//! High performance XML reader/writer.
//!
//! # Description
//!
//! quick-xml contains two modes of operation:
//!
//! A streaming API based on the [StAX] model. This is suited for larger XML documents which
//! cannot completely read into memory at once.
//!
//! The user has to explicitly _ask_ for the next XML event, similar to a database cursor.
//! This is achieved by the following two structs:
//!
//! - [`Reader`]: A low level XML pull-reader where buffer allocation/clearing is left to user.
//! - [`Writer`]: A XML writer. Can be nested with readers if you want to transform XMLs.
//!
//! Especially for nested XML elements, the user must keep track _where_ (how deep)
//! in the XML document the current event is located.
//!
//! quick-xml contains optional support of asynchronous reading and writing using [tokio].
//! To get it enable the [`async-tokio`](#async-tokio) feature.
//!
//! Furthermore, quick-xml also contains optional [Serde] support to directly
//! serialize and deserialize from structs, without having to deal with the XML events.
//! To get it enable the [`serialize`](#serialize) feature. Read more about mapping Rust types
//! to XML in the documentation of [`de`] module. Also check [`serde_helpers`]
//! module.
//!
//! # Examples
//!
//! - For a reading example see [`Reader`]
//! - For a writing example see [`Writer`]
//!
//! # Features
//!
//! `quick-xml` supports the following features:
//!
//! [StAX]: https://en.wikipedia.org/wiki/StAX
//! [tokio]: https://tokio.rs/
//! [Serde]: https://serde.rs/
//! [`de`]: ./de/index.html
#![cfg_attr(
    feature = "document-features",
    cfg_attr(doc, doc = ::document_features::document_features!(
        feature_label = "<a id=\"{feature}\" href=\"#{feature}\"><strong><code>{feature}</code></strong></a>"
    ))
)]
#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![recursion_limit = "1024"]
// Enable feature requirements in the docs from 1.57
// See https://stackoverflow.com/questions/61417452
// docs.rs defines `docsrs` when building documentation
#![cfg_attr(docsrs, feature(doc_auto_cfg))]

#[cfg(feature = "serialize")]
pub mod de;
pub mod encoding;
pub mod errors;
pub mod escape;
pub mod events;
pub mod name;
pub mod parser;
pub mod reader;
#[cfg(feature = "serialize")]
pub mod se;
#[cfg(feature = "serde-types")]
pub mod serde_helpers;
/// Not an official API, public for integration tests
#[doc(hidden)]
pub mod utils;
pub mod writer;

// reexports
pub use crate::encoding::Decoder;
#[cfg(feature = "serialize")]
pub use crate::errors::serialize::{DeError, SeError};
pub use crate::errors::{Error, Result};
pub use crate::reader::{NsReader, Reader};
pub use crate::writer::{ElementWriter, Writer};
