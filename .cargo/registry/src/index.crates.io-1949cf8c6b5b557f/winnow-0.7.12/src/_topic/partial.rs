//! # Parsing Partial Input
//!
//! Typically, the input being parsed is all in-memory, or is complete. Some data sources are too
//! large to fit into memory, only allowing parsing an incomplete or [`Partial`] subset of the
//! data, requiring incrementally parsing.
//!
//! By wrapping a stream, like `&[u8]`, with [`Partial`], parsers will report when the data is
//! [`Incomplete`] and more input is [`Needed`], allowing the caller to stream-in additional data
//! to be parsed. The data is then parsed a chunk at a time.
//!
//! Chunks are typically defined by either:
//! - A header reporting the number of bytes, like with [`length_and_then`]
//!   - [`Partial`] can explicitly be changed to being complete once the specified bytes are
//!     acquired via [`StreamIsPartial::complete`].
//! - A delimiter, like with [ndjson](https://github.com/ndjson/ndjson-spec/)
//!   - You can parse up-to the delimiter or do a `take_until(0.., delim).and_then(parser)`
//!
//! If the chunks are not homogeneous, a state machine will be needed to track what the expected
//! parser is for the next chunk.
//!
//! Caveats:
//! - `winnow` takes the approach of re-parsing from scratch. Chunks should be relatively small to
//!   prevent the re-parsing overhead from dominating.
//! - Parsers like [`repeat`] do not know when an `eof` is from insufficient data or the end of the
//!   stream, causing them to always report [`Incomplete`].
//!
//! # Example
//!
//! `main.rs`:
//! ```rust,ignore
#![doc = include_str!("../../examples/ndjson/main.rs")]
//! ```
//!
//! `parser.rs`:
//! ```rust,ignore
#![doc = include_str!("../../examples/ndjson/parser.rs")]
//! ```

#![allow(unused_imports)] // Used for intra-doc links

use crate::binary::length_and_then;
use crate::combinator::repeat;
use crate::error::ErrMode::Incomplete;
use crate::error::Needed;
use crate::stream::Partial;
use crate::stream::StreamIsPartial;
