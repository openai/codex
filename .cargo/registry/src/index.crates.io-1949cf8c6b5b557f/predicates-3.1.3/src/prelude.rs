// Copyright (c) 2018 The predicates-rs Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Module that contains the essentials for working with predicates.

pub use crate::boolean::PredicateBooleanExt;
pub use crate::boxed::PredicateBoxExt;
pub use crate::name::PredicateNameExt;
pub use crate::path::PredicateFileContentExt;
pub use crate::str::PredicateStrExt;
pub use crate::Predicate;

/// Predicate factories
pub mod predicate {
    // primitive `Predicate` types
    pub use crate::constant::{always, never};
    pub use crate::function::function;
    pub use crate::iter::{in_hash, in_iter};
    pub use crate::ord::{eq, ge, gt, le, lt, ne};

    /// `str` Predicate factories
    ///
    /// This module contains predicates specific to string handling.
    pub mod str {
        pub use crate::str::is_empty;
        pub use crate::str::{contains, ends_with, starts_with};

        #[cfg(feature = "diff")]
        pub use crate::str::diff;

        #[cfg(feature = "regex")]
        pub use crate::str::is_match;
    }

    /// `Path` Predicate factories
    ///
    /// This module contains predicates specific to path handling.
    pub mod path {
        pub use crate::path::eq_file;
        pub use crate::path::{exists, missing};
        pub use crate::path::{is_dir, is_file, is_symlink};
    }

    /// `f64` Predicate factories
    ///
    /// This module contains predicates specific to float handling.
    pub mod float {
        #[cfg(feature = "float-cmp")]
        pub use crate::float::is_close;
    }
}
