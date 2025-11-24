//! This crate implements diffing utilities.  It attempts to provide an abstraction
//! interface over different types of diffing algorithms.  The design of the
//! library is inspired by pijul's diff library by Pierre-Étienne Meunier and
//! also inherits the patience diff algorithm from there.
//!
//! The API of the crate is split into high and low level functionality.  Most
//! of what you probably want to use is available top level.  Additionally the
//! following sub modules exist:
//!
//! * [`algorithms`]: This implements the different types of diffing algorithms.
//!   It provides both low level access to the algorithms with the minimal
//!   trait bounds necessary, as well as a generic interface.
//! * [`udiff`]: Unified diff functionality.
//! * [`utils`]: utilities for common diff related operations.  This module
//!   provides additional diffing functions for working with text diffs.
//!
//! # Sequence Diffing
//!
//! If you want to diff sequences generally indexable things you can use the
//! [`capture_diff`] and [`capture_diff_slices`] functions.  They will directly
//! diff an indexable object or slice and return a vector of [`DiffOp`] objects.
//!
//! ```rust
//! use similar::{Algorithm, capture_diff_slices};
//!
//! let a = vec![1, 2, 3, 4, 5];
//! let b = vec![1, 2, 3, 4, 7];
//! let ops = capture_diff_slices(Algorithm::Myers, &a, &b);
//! ```
//!
//! # Text Diffing
//!
//! Similar provides helpful utilities for text (and more specifically line) diff
//! operations.  The main type you want to work with is [`TextDiff`] which
//! uses the underlying diff algorithms to expose a convenient API to work with
//! texts:
//!
//! ```rust
//! # #[cfg(feature = "text")] {
//! use similar::{ChangeTag, TextDiff};
//!
//! let diff = TextDiff::from_lines(
//!     "Hello World\nThis is the second line.\nThis is the third.",
//!     "Hallo Welt\nThis is the second line.\nThis is life.\nMoar and more",
//! );
//!
//! for change in diff.iter_all_changes() {
//!     let sign = match change.tag() {
//!         ChangeTag::Delete => "-",
//!         ChangeTag::Insert => "+",
//!         ChangeTag::Equal => " ",
//!     };
//!     print!("{}{}", sign, change);
//! }
//! # }
//! ```
//!
//! ## Trailing Newlines
//!
//! When working with line diffs (and unified diffs in general) there are two
//! "philosophies" to look at lines.  One is to diff lines without their newline
//! character, the other is to diff with the newline character.  Typically the
//! latter is done because text files do not _have_ to end in a newline character.
//! As a result there is a difference between `foo\n` and `foo` as far as diffs
//! are concerned.
//!
//! In similar this is handled on the [`Change`] or [`InlineChange`] level.  If
//! a diff was created via [`TextDiff::from_lines`] the text diffing system is
//! instructed to check if there are missing newlines encountered
//! ([`TextDiff::newline_terminated`] returns true).
//!
//! In any case the [`Change`] object has a convenience method called
//! [`Change::missing_newline`] which returns `true` if the change is missing
//! a trailing newline.  Armed with that information the caller knows to handle
//! this by either rendering a virtual newline at that position or to indicate
//! it in different ways.  For instance the unified diff code will render the
//! special `\ No newline at end of file` marker.
//!
//! ## Bytes vs Unicode
//!
//! Similar module concerns itself with a looser definition of "text" than you would
//! normally see in Rust.  While by default it can only operate on [`str`] types,
//! by enabling the `bytes` feature it gains support for byte slices with some
//! caveats.
//!
//! A lot of text diff functionality assumes that what is being diffed constitutes
//! text, but in the real world it can often be challenging to ensure that this is
//! all valid utf-8.  Because of this the crate is built so that most functionality
//! also still works with bytes for as long as they are roughly ASCII compatible.
//!
//! This means you will be successful in creating a unified diff from latin1
//! encoded bytes but if you try to do the same with EBCDIC encoded bytes you
//! will only get garbage.
//!
//! # Ops vs Changes
//!
//! Because very commonly two compared sequences will largely match this module
//! splits its functionality into two layers:
//!
//! Changes are encoded as [diff operations](crate::DiffOp).  These are
//! ranges of the differences by index in the source sequence.  Because this
//! can be cumbersome to work with, a separate method [`DiffOp::iter_changes`]
//! (and [`TextDiff::iter_changes`] when working with text diffs) is provided
//! which expands all the changes on an item by item level encoded in an operation.
//!
//! As the [`TextDiff::grouped_ops`] method can isolate clusters of changes
//! this even works for very long files if paired with this method.
//!
//! # Deadlines and Performance
//!
//! For large and very distinct inputs the algorithms as implemented can take
//! a very, very long time to execute.  Too long to make sense in practice.
//! To work around this issue all diffing algorithms also provide a version
//! that accepts a deadline which is the point in time as defined by an
//! [`Instant`] after which the algorithm should give up.  What giving up means
//! depends on the algorithm.  For instance due to the recursive, divide and
//! conquer nature of Myer's diff you will still get a pretty decent diff in
//! many cases when a deadline is reached.  Whereas on the other hand the LCS
//! diff is unlikely to give any decent results in such a situation.
//!
//! The [`TextDiff`] type also lets you configure a deadline and/or timeout
//! when performing a text diff.
//!
//! Note that on wasm targets calling [`Instant::now`] will result in a panic
//! unless you enable the `wasm32_web_time` feataure.  By default similar will
//! silently disable the deadline checks internally unless that feature is
//! enabled.
//!
//! # Feature Flags
//!
//! The crate by default does not have any dependencies however for some use
//! cases it's useful to pull in extra functionality.  Likewise you can turn
//! off some functionality.
//!
//! * `text`: this feature is enabled by default and enables the text based
//!   diffing types such as [`TextDiff`].
//!   If the crate is used without default features it's removed.
//! * `unicode`: when this feature is enabled the text diffing functionality
//!   gains the ability to diff on a grapheme instead of character level.  This
//!   is particularly useful when working with text containing emojis.  This
//!   pulls in some relatively complex dependencies for working with the unicode
//!   database.
//! * `bytes`: this feature adds support for working with byte slices in text
//!   APIs in addition to unicode strings.  This pulls in the
//!   [`bstr`] dependency.
//! * `inline`: this feature gives access to additional functionality of the
//!   text diffing to provide inline information about which values changed
//!   in a line diff.  This currently also enables the `unicode` feature.
//! * `serde`: this feature enables serialization to some types in this
//!   crate.  For enums without payload deserialization is then also supported.
//! * `wasm32_web_time`: this feature swaps out the use of [`std::time`] for
//!   the `web_time` crate.  Because this is a change to the public interface,
//!   this feature must be used with care.  The instant type for this crate is
//!   then re-exported top-level module.
#![warn(missing_docs)]
pub mod algorithms;
pub mod iter;
#[cfg(feature = "text")]
pub mod udiff;
#[cfg(feature = "text")]
pub mod utils;

mod common;
mod deadline_support;
#[cfg(feature = "text")]
mod text;
mod types;

pub use self::common::*;
#[cfg(feature = "text")]
pub use self::text::*;
pub use self::types::*;

// re-export the type for web-time feature
#[cfg(feature = "wasm32_web_time")]
pub use deadline_support::Instant;
