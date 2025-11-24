//! # Special Topics
//!
//! These are short recipes for accomplishing common tasks.
//!
//! - [Why `winnow`?][why]
//! - [For `nom` users][nom]
//! - Formats:
//!   - [Elements of Programming Languages][language]
//!   - [Arithmetic][arithmetic]
//!   - [s-expression][s_expression]
//!   - [json]
//!   - [INI][ini]
//!   - [HTTP][http]
//! - Special Topics:
//!   - [Implementing `FromStr`][fromstr]
//!   - [Performance][performance]
//!   - [Parsing Partial Input][partial]
//!   - [Lexing and Parsing][lexing]
//!   - [Custom stream or token][stream]
//!   - [Custom errors][error]
//!   - [Debugging][crate::_tutorial::chapter_8]
//!
//! See also parsers written with `winnow`:
//!
//! - [`toml_edit`](https://crates.io/crates/toml_edit)
//! - [`hcl-edit`](https://crates.io/crates/hcl-edit)
#![allow(clippy::std_instead_of_core)]

pub mod arithmetic;
pub mod error;
pub mod fromstr;
pub mod http;
pub mod ini;
pub mod json;
pub mod language;
pub mod lexing;
pub mod nom;
pub mod partial;
pub mod performance;
pub mod s_expression;
pub mod stream;
pub mod why;
