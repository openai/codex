//! # Lexing and Parsing
//!
//! ## Parse to AST
//!
//! The simplest way to write a parser is to parse directly to the AST.
//!
//! Example:
//! ```rust
#![doc = include_str!("../../examples/arithmetic/parser_ast.rs")]
//! ```
//!
//! ## Lexing
//!
//! However, there are times when you may want to separate lexing from parsing.
//! Winnow provides [`TokenSlice`] to simplify this.
//!
//! Example:
//! ```rust
#![doc = include_str!("../../examples/arithmetic/parser_lexer.rs")]
//! ```

#![allow(unused_imports)]
use crate::stream::TokenSlice;
