#![deny(rust_2018_idioms)]

//! A library for formatting of text or programming code snippets.
//!
//! It's primary purpose is to build an ASCII-graphical representation of the snippet
//! with annotations.
//!
//! # Example
//!
//! ```text
//! error[E0308]: mismatched types
//!   --> src/format.rs:52:1
//!    |
//! 51 |   ) -> Option<String> {
//!    |        -------------- expected `Option<String>` because of return type
//! 52 | /     for ann in annotations {
//! 53 | |         match (ann.range.0, ann.range.1) {
//! 54 | |             (None, None) => continue,
//! 55 | |             (Some(start), Some(end)) if start > end_index => continue,
//! ...  |
//! 71 | |         }
//! 72 | |     }
//!    | |_____^ expected enum `std::option::Option`, found ()
//! ```
//!
//! The crate uses a three stage process with two conversions between states:
//!
//! ```text
//! Snippet --> DisplayList --> String
//! ```
//!
//! The input type - [Snippet](self::snippet) is a structure designed
//! to align with likely output from any parser whose code snippet is to be
//! annotated.
//!
//! The middle structure - [DisplayList](self::display_list) is a
//! structure designed to store the snippet data converted into a vector
//! of lines containing semantic information about each line.
//! This structure is the easiest to manipulate and organize.
//!
//! Finally, `impl Display` into a final `String` output.
//!
//! A user of the crate may choose to provide their own equivalent of the input
//! structure with an `Into<DisplayList>` trait.
//!
//! A user of the crate may also choose to provide their own formatter logic,
//! to convert a `DisplayList` into a `String`, or just a `Stylesheet` to
//! use the crate's formatting logic, but with a custom stylesheet.
// TODO: check documentation

pub mod display_list;
pub mod formatter;
pub mod snippet;
pub mod stylesheets;
