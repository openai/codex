//! [![github]](https://github.com/dtolnay/path-to-error)&ensp;[![crates-io]](https://crates.io/crates/serde_path_to_error)&ensp;[![docs-rs]](https://docs.rs/serde_path_to_error)
//!
//! [github]: https://img.shields.io/badge/github-8da0cb?style=for-the-badge&labelColor=555555&logo=github
//! [crates-io]: https://img.shields.io/badge/crates.io-fc8d62?style=for-the-badge&labelColor=555555&logo=rust
//! [docs-rs]: https://img.shields.io/badge/docs.rs-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs
//!
//! <br>
//!
//! Find out the path at which a deserialization error occurred. This crate
//! provides a wrapper that works with any existing Serde `Deserializer` and
//! exposes the chain of field names leading to the error.
//!
//! # Example
//!
//! ```
//! # use serde_derive::Deserialize;
//! #
//! use serde::Deserialize;
//! use std::collections::BTreeMap as Map;
//!
//! #[derive(Deserialize)]
//! struct Package {
//!     name: String,
//!     dependencies: Map<String, Dependency>,
//! }
//!
//! #[derive(Deserialize)]
//! struct Dependency {
//!     version: String,
//! }
//!
//! fn main() {
//!     let j = r#"{
//!         "name": "demo",
//!         "dependencies": {
//!             "serde": {
//!                 "version": 1
//!             }
//!         }
//!     }"#;
//!
//!     // Some Deserializer.
//!     let jd = &mut serde_json::Deserializer::from_str(j);
//!
//!     let result: Result<Package, _> = serde_path_to_error::deserialize(jd);
//!     match result {
//!         Ok(_) => panic!("expected a type error"),
//!         Err(err) => {
//!             let path = err.path().to_string();
//!             assert_eq!(path, "dependencies.serde.version");
//!         }
//!     }
//! }
//! ```

#![doc(html_root_url = "https://docs.rs/serde_path_to_error/0.1.20")]
#![no_std]
#![allow(
    clippy::doc_link_with_quotes, // https://github.com/rust-lang/rust-clippy/issues/8961
    clippy::elidable_lifetime_names,
    clippy::iter_not_returning_iterator, // https://github.com/rust-lang/rust-clippy/issues/8285
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::needless_lifetimes,
    clippy::new_without_default,
    clippy::uninlined_format_args
)]
#![allow(unknown_lints, mismatched_lifetime_syntaxes)]

extern crate alloc;
extern crate serde_core as serde;

mod de;
mod path;
mod ser;
mod wrap;

use alloc::string::String;
use core::cell::Cell;
use core::fmt::{self, Display};
use serde::ser::StdError;

pub use crate::de::{deserialize, Deserializer};
pub use crate::path::{Path, Segment, Segments};
pub use crate::ser::{serialize, Serializer};

/// Original deserializer error together with the path at which it occurred.
#[derive(Clone, Debug)]
pub struct Error<E> {
    path: Path,
    original: E,
}

impl<E> Error<E> {
    pub fn new(path: Path, inner: E) -> Self {
        Error {
            path,
            original: inner,
        }
    }

    /// Element path at which this deserialization error occurred.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The Deserializer's underlying error that occurred.
    pub fn into_inner(self) -> E {
        self.original
    }

    /// Reference to the Deserializer's underlying error that occurred.
    pub fn inner(&self) -> &E {
        &self.original
    }
}

impl<E: Display> Display for Error<E> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if !self.path.is_only_unknown() {
            write!(f, "{}: ", self.path)?;
        }
        write!(f, "{}", self.original)
    }
}

impl<E: StdError> StdError for Error<E> {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.original.source()
    }
}

/// State for bookkeeping across nested deserializer calls.
///
/// You don't need this if you are using `serde_path_to_error::deserializer`. If
/// you are managing your own `Deserializer`, see the usage example on
/// [`Deserializer`].
pub struct Track {
    path: Cell<Option<Path>>,
}

impl Track {
    /// Empty state with no error having happened yet.
    pub const fn new() -> Self {
        Track {
            path: Cell::new(None),
        }
    }

    /// Gets path at which the error occurred. Only meaningful after we know
    /// that an error has occurred. Returns an empty path otherwise.
    pub fn path(self) -> Path {
        self.path.into_inner().unwrap_or_else(Path::empty)
    }

    #[inline]
    fn trigger<E>(&self, chain: &Chain, err: E) -> E {
        self.trigger_impl(chain);
        err
    }

    fn trigger_impl(&self, chain: &Chain) {
        self.path.set(Some(match self.path.take() {
            Some(already_set) => already_set,
            None => Path::from_chain(chain),
        }));
    }
}

#[derive(Clone)]
enum Chain<'a> {
    Root,
    Seq {
        parent: &'a Chain<'a>,
        index: usize,
    },
    Map {
        parent: &'a Chain<'a>,
        key: String,
    },
    Struct {
        parent: &'a Chain<'a>,
        key: &'static str,
    },
    Enum {
        parent: &'a Chain<'a>,
        variant: String,
    },
    Some {
        parent: &'a Chain<'a>,
    },
    NewtypeStruct {
        parent: &'a Chain<'a>,
    },
    NewtypeVariant {
        parent: &'a Chain<'a>,
    },
    NonStringKey {
        parent: &'a Chain<'a>,
    },
}
