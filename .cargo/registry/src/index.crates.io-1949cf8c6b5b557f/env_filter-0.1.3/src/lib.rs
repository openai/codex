//! Filtering for log records.
//!
//! You can use the [`Filter`] type in your own logger implementation to use the same
//! filter parsing and matching as `env_logger`.
//!
//! ## Using `env_filter` in your own logger
//!
//! You can use `env_filter`'s filtering functionality with your own logger.
//! Call [`Builder::parse`] to parse directives from a string when constructing
//! your logger. Call [`Filter::matches`] to check whether a record should be
//! logged based on the parsed filters when log records are received.
//!
//! ```
//! use env_filter::Filter;
//! use log::{Log, Metadata, Record};
//!
//! struct PrintLogger;
//!
//! impl Log for PrintLogger {
//!     fn enabled(&self, metadata: &Metadata) -> bool {
//!         true
//!     }
//!
//!     fn log(&self, record: &Record) {
//!         println!("{:?}", record);
//!     }
//!
//!     fn flush(&self) {}
//! }
//!
//! let mut builder = env_filter::Builder::new();
//! // Parse a directives string from an environment variable
//! if let Ok(ref filter) = std::env::var("MY_LOG_LEVEL") {
//!     builder.parse(filter);
//! }
//!
//! let logger = env_filter::FilteredLog::new(PrintLogger, builder.build());
//! ```

#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![warn(missing_docs)]
#![warn(clippy::print_stderr)]
#![warn(clippy::print_stdout)]

mod directive;
mod filter;
mod filtered_log;
mod op;
mod parser;

use directive::enabled;
use directive::Directive;
use op::FilterOp;
use parser::parse_spec;

pub use filter::Builder;
pub use filter::Filter;
pub use filtered_log::FilteredLog;
pub use parser::ParseError;
