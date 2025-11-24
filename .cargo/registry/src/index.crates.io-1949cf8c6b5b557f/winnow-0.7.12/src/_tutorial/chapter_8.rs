//! # Chapter 8: Debugging
//!
//! When things inevitably go wrong, you can introspect the parsing state by running your test case
//! with `--features winnow/debug`.
//!
//! For example, the trace output of an [escaped string parser][crate::_topic::language#escaped-strings]:
//! ![Trace output from string example](https://raw.githubusercontent.com/winnow-rs/winnow/main/assets/trace.svg "Example output")
//!
//! You can extend your own parsers to show up by wrapping their body with
//! [`trace`][crate::combinator::trace].  Going back to [`do_nothing_parser`][super::chapter_1].
//! ```rust
//! # use winnow::ModalResult;
//! # use winnow::Parser;
//! use winnow::combinator::trace;
//!
//! pub fn do_nothing_parser<'s>(input: &mut &'s str) -> ModalResult<&'s str> {
//!     trace(
//!         "do_nothing_parser",
//!         |i: &mut _| Ok("")
//!     ).parse_next(input)
//! }
//! #
//! # fn main() {
//! #     let mut input = "0x1a2b Hello";
//! #
//! #     let output = do_nothing_parser.parse_next(&mut input).unwrap();
//! #     // Same as:
//! #     // let output = do_nothing_parser(&mut input).unwrap();
//! #
//! #     assert_eq!(input, "0x1a2b Hello");
//! #     assert_eq!(output, "");
//! # }
//! ```

pub use super::chapter_7 as previous;
pub use crate::_topic as next;
pub use crate::_tutorial as table_of_contents;
