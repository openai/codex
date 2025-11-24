// Copyright (c) 2018 The predicates-rs Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Float Predicates
//!
//! This module contains predicates specific to string handling.

#[cfg(feature = "float-cmp")]
mod close;
#[cfg(feature = "float-cmp")]
pub use self::close::{is_close, IsClosePredicate};
