//! # Custom Errors
//!
//! A lot can be accomplished with the built-in error tools, like:
//! - [`ContextError`]
//! - [`Parser::context`]
//! - [`cut_err`]
//!
//! *(see [tutorial][chapter_7])*
//!
//! Most other needs can likely be met by using a custom context type with [`ContextError`] instead
//! of [`StrContext`].
//! This will require implementing a custom renderer.
//!
//! ## `ParserError` Trait
//!
//! When needed, you can also create your own type that implements [`ParserError`].
//!
//! Optional traits include:
//! - [`AddContext`]
//! - [`FromExternalError`]
//! - [`ErrorConvert`]
//!
//! There are multiple strategies for implementing support for [`AddContext`] and [`FromExternalError`]:
//! - Make your error type generic over the context or external error
//! - Require a trait for the context or external error and `Box` it
//! - Make the context an enum like [`StrContext`]
//! - Implement the trait multiple times, one for each concrete context or external error type,
//!   allowing custom behavior per type
//!
//! Example:
//!```rust
#![doc = include_str!("../../examples/custom_error.rs")]
//!```

#![allow(unused_imports)]
use crate::combinator::cut_err;
use crate::error::ContextError;
use crate::error::ErrorConvert;
use crate::error::StrContext;
use crate::Parser;
use crate::_tutorial::chapter_7;
use crate::error::AddContext;
use crate::error::FromExternalError;
use crate::error::ParserError;
