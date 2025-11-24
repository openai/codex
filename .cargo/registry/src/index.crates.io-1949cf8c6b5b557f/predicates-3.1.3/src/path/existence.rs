// Copyright (c) 2018 The predicates-rs Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::fmt;
use std::path;

use crate::reflection;
use crate::utils;
use crate::Predicate;

/// Predicate that checks if a file is present
///
/// This is created by the `predicate::path::exists` and `predicate::path::missing`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExistencePredicate {
    exists: bool,
}

impl Predicate<path::Path> for ExistencePredicate {
    fn eval(&self, path: &path::Path) -> bool {
        path.exists() == self.exists
    }

    fn find_case<'a>(
        &'a self,
        expected: bool,
        variable: &path::Path,
    ) -> Option<reflection::Case<'a>> {
        utils::default_find_case(self, expected, variable).map(|case| {
            case.add_product(reflection::Product::new(
                "var",
                variable.display().to_string(),
            ))
        })
    }
}

impl reflection::PredicateReflection for ExistencePredicate {}

impl fmt::Display for ExistencePredicate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let palette = crate::Palette::new(f.alternate());
        write!(
            f,
            "{}({})",
            palette.description(if self.exists { "exists" } else { "missing" }),
            palette.var("var")
        )
    }
}

/// Creates a new `Predicate` that ensures the path exists.
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// use predicates::prelude::*;
///
/// let predicate_fn = predicate::path::exists();
/// assert_eq!(true, predicate_fn.eval(Path::new("Cargo.toml")));
/// ```
pub fn exists() -> ExistencePredicate {
    ExistencePredicate { exists: true }
}

/// Creates a new `Predicate` that ensures the path doesn't exist.
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// use predicates::prelude::*;
///
/// let predicate_fn = predicate::path::missing();
/// assert_eq!(true, predicate_fn.eval(Path::new("non-existent-file.foo")));
/// ```
pub fn missing() -> ExistencePredicate {
    ExistencePredicate { exists: false }
}
