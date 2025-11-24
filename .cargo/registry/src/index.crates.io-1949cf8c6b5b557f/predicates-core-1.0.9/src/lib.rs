// Copyright (c) 2018 The predicates-rs Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/license/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Composable first-order predicate trait.
//!
//! This library implements an interface to "predicates" - boolean-valued
//! functions of one argument. This allows combinatorial logic to be created and
//! assembled at runtime and then used one or more times for evaluating values.
//! This sort of object is really useful when creating filters and checks that
//! can be changed at runtime with user interaction - it allows a clean
//! separation of concerns where the configuration code can be used to build up
//! a predicate, and then that predicate can be given to the code that does the
//! actual filtering without the filtering code knowing anything about user
//! configuration. See the examples for how this can work.

#![warn(missing_docs, missing_debug_implementations)]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]

mod core;
pub use crate::core::*;
pub mod reflection;
