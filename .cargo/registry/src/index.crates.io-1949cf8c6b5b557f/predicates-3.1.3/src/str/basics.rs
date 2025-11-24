// Copyright (c) 2018 The predicates-rs Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::fmt;

use crate::reflection;
use crate::utils;
use crate::Predicate;

/// Predicate that checks for empty strings.
///
/// This is created by `predicates::str::is_empty`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IsEmptyPredicate {}

impl Predicate<str> for IsEmptyPredicate {
    fn eval(&self, variable: &str) -> bool {
        variable.is_empty()
    }

    fn find_case<'a>(&'a self, expected: bool, variable: &str) -> Option<reflection::Case<'a>> {
        utils::default_find_case(self, expected, variable)
            .map(|case| case.add_product(reflection::Product::new("var", variable.to_owned())))
    }
}

impl reflection::PredicateReflection for IsEmptyPredicate {}

impl fmt::Display for IsEmptyPredicate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let palette = crate::Palette::new(f.alternate());
        write!(
            f,
            "{}.{}()",
            palette.var("var"),
            palette.description("is_empty"),
        )
    }
}

/// Creates a new `Predicate` that ensures a str is empty
///
/// # Examples
///
/// ```
/// use predicates::prelude::*;
///
/// let predicate_fn = predicate::str::is_empty();
/// assert_eq!(true, predicate_fn.eval(""));
/// assert_eq!(false, predicate_fn.eval("Food World"));
/// ```
pub fn is_empty() -> IsEmptyPredicate {
    IsEmptyPredicate {}
}

/// Predicate checks start of str
///
/// This is created by `predicates::str::starts_with`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartsWithPredicate {
    pattern: String,
}

impl Predicate<str> for StartsWithPredicate {
    fn eval(&self, variable: &str) -> bool {
        variable.starts_with(&self.pattern)
    }

    fn find_case<'a>(&'a self, expected: bool, variable: &str) -> Option<reflection::Case<'a>> {
        utils::default_find_case(self, expected, variable)
            .map(|case| case.add_product(reflection::Product::new("var", variable.to_owned())))
    }
}

impl reflection::PredicateReflection for StartsWithPredicate {}

impl fmt::Display for StartsWithPredicate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let palette = crate::Palette::new(f.alternate());
        write!(
            f,
            "{}.{}({:?})",
            palette.var("var"),
            palette.description("starts_with"),
            self.pattern
        )
    }
}

/// Creates a new `Predicate` that ensures a str starts with `pattern`
///
/// # Examples
///
/// ```
/// use predicates::prelude::*;
///
/// let predicate_fn = predicate::str::starts_with("Hello");
/// assert_eq!(true, predicate_fn.eval("Hello World"));
/// assert_eq!(false, predicate_fn.eval("Goodbye World"));
/// ```
pub fn starts_with<P>(pattern: P) -> StartsWithPredicate
where
    P: Into<String>,
{
    StartsWithPredicate {
        pattern: pattern.into(),
    }
}

/// Predicate checks end of str
///
/// This is created by `predicates::str::ends_with`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndsWithPredicate {
    pattern: String,
}

impl Predicate<str> for EndsWithPredicate {
    fn eval(&self, variable: &str) -> bool {
        variable.ends_with(&self.pattern)
    }

    fn find_case<'a>(&'a self, expected: bool, variable: &str) -> Option<reflection::Case<'a>> {
        utils::default_find_case(self, expected, variable)
            .map(|case| case.add_product(reflection::Product::new("var", variable.to_owned())))
    }
}

impl reflection::PredicateReflection for EndsWithPredicate {}

impl fmt::Display for EndsWithPredicate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let palette = crate::Palette::new(f.alternate());
        write!(
            f,
            "{}.{}({:?})",
            palette.var("var"),
            palette.description("ends_with"),
            self.pattern
        )
    }
}

/// Creates a new `Predicate` that ensures a str ends with `pattern`
///
/// # Examples
///
/// ```
/// use predicates::prelude::*;
///
/// let predicate_fn = predicate::str::ends_with("World");
/// assert_eq!(true, predicate_fn.eval("Hello World"));
/// assert_eq!(false, predicate_fn.eval("Hello Moon"));
/// ```
pub fn ends_with<P>(pattern: P) -> EndsWithPredicate
where
    P: Into<String>,
{
    EndsWithPredicate {
        pattern: pattern.into(),
    }
}

/// Predicate that checks for patterns.
///
/// This is created by `predicates::str:contains`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainsPredicate {
    pattern: String,
}

impl ContainsPredicate {
    /// Require a specific count of matches.
    ///
    /// # Examples
    ///
    /// ```
    /// use predicates::prelude::*;
    ///
    /// let predicate_fn = predicate::str::contains("Two").count(2);
    /// assert_eq!(true, predicate_fn.eval("One Two Three Two One"));
    /// assert_eq!(false, predicate_fn.eval("One Two Three"));
    /// ```
    pub fn count(self, count: usize) -> MatchesPredicate {
        MatchesPredicate {
            pattern: self.pattern,
            count,
        }
    }
}

impl Predicate<str> for ContainsPredicate {
    fn eval(&self, variable: &str) -> bool {
        variable.contains(&self.pattern)
    }

    fn find_case<'a>(&'a self, expected: bool, variable: &str) -> Option<reflection::Case<'a>> {
        utils::default_find_case(self, expected, variable)
            .map(|case| case.add_product(reflection::Product::new("var", variable.to_owned())))
    }
}

impl reflection::PredicateReflection for ContainsPredicate {}

impl fmt::Display for ContainsPredicate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let palette = crate::Palette::new(f.alternate());
        write!(
            f,
            "{}.{}({})",
            palette.var("var"),
            palette.description("contains"),
            palette.expected(&self.pattern),
        )
    }
}

/// Predicate that checks for repeated patterns.
///
/// This is created by `predicates::str::contains(...).count`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchesPredicate {
    pattern: String,
    count: usize,
}

impl Predicate<str> for MatchesPredicate {
    fn eval(&self, variable: &str) -> bool {
        variable.matches(&self.pattern).count() == self.count
    }

    fn find_case<'a>(&'a self, expected: bool, variable: &str) -> Option<reflection::Case<'a>> {
        let actual_count = variable.matches(&self.pattern).count();
        let result = self.count == actual_count;
        if result == expected {
            Some(
                reflection::Case::new(Some(self), result)
                    .add_product(reflection::Product::new("var", variable.to_owned()))
                    .add_product(reflection::Product::new("actual count", actual_count)),
            )
        } else {
            None
        }
    }
}

impl reflection::PredicateReflection for MatchesPredicate {
    fn parameters<'a>(&'a self) -> Box<dyn Iterator<Item = reflection::Parameter<'a>> + 'a> {
        let params = vec![reflection::Parameter::new("count", &self.count)];
        Box::new(params.into_iter())
    }
}

impl fmt::Display for MatchesPredicate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let palette = crate::Palette::new(f.alternate());
        write!(
            f,
            "{}.{}({})",
            palette.var("var"),
            palette.description("contains"),
            palette.expected(&self.pattern),
        )
    }
}

/// Creates a new `Predicate` that ensures a str contains `pattern`
///
/// # Examples
///
/// ```
/// use predicates::prelude::*;
///
/// let predicate_fn = predicate::str::contains("Two");
/// assert_eq!(true, predicate_fn.eval("One Two Three"));
/// assert_eq!(false, predicate_fn.eval("Four Five Six"));
/// ```
pub fn contains<P>(pattern: P) -> ContainsPredicate
where
    P: Into<String>,
{
    ContainsPredicate {
        pattern: pattern.into(),
    }
}
