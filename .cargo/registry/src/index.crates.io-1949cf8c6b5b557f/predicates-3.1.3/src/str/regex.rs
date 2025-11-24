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

/// An error that occurred during parsing or compiling a regular expression.
pub type RegexError = regex::Error;

/// Predicate that uses regex matching
///
/// This is created by the `predicate::str::is_match`.
#[derive(Debug, Clone)]
pub struct RegexPredicate {
    re: regex::Regex,
}

impl RegexPredicate {
    /// Require a specific count of matches.
    ///
    /// # Examples
    ///
    /// ```
    /// use predicates::prelude::*;
    ///
    /// let predicate_fn = predicate::str::is_match("T[a-z]*").unwrap().count(3);
    /// assert_eq!(true, predicate_fn.eval("One Two Three Two One"));
    /// assert_eq!(false, predicate_fn.eval("One Two Three"));
    /// ```
    pub fn count(self, count: usize) -> RegexMatchesPredicate {
        RegexMatchesPredicate { re: self.re, count }
    }
}

impl Predicate<str> for RegexPredicate {
    fn eval(&self, variable: &str) -> bool {
        self.re.is_match(variable)
    }

    fn find_case<'a>(&'a self, expected: bool, variable: &str) -> Option<reflection::Case<'a>> {
        utils::default_find_case(self, expected, variable)
            .map(|case| case.add_product(reflection::Product::new("var", variable.to_owned())))
    }
}

impl reflection::PredicateReflection for RegexPredicate {}

impl fmt::Display for RegexPredicate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let palette = crate::Palette::new(f.alternate());
        write!(
            f,
            "{}.{}({})",
            palette.var("var"),
            palette.description("is_match"),
            palette.expected(&self.re),
        )
    }
}

/// Predicate that checks for repeated patterns.
///
/// This is created by `predicates::str::is_match(...).count`.
#[derive(Debug, Clone)]
pub struct RegexMatchesPredicate {
    re: regex::Regex,
    count: usize,
}

impl Predicate<str> for RegexMatchesPredicate {
    fn eval(&self, variable: &str) -> bool {
        self.re.find_iter(variable).count() == self.count
    }

    fn find_case<'a>(&'a self, expected: bool, variable: &str) -> Option<reflection::Case<'a>> {
        let actual_count = self.re.find_iter(variable).count();
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

impl reflection::PredicateReflection for RegexMatchesPredicate {
    fn parameters<'a>(&'a self) -> Box<dyn Iterator<Item = reflection::Parameter<'a>> + 'a> {
        let params = vec![reflection::Parameter::new("count", &self.count)];
        Box::new(params.into_iter())
    }
}

impl fmt::Display for RegexMatchesPredicate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let palette = crate::Palette::new(f.alternate());
        write!(
            f,
            "{}.{}({})",
            palette.var("var"),
            palette.description("is_match"),
            palette.expected(&self.re),
        )
    }
}

/// Creates a new `Predicate` that uses a regular expression to match the string.
///
/// # Examples
///
/// ```
/// use predicates::prelude::*;
///
/// let predicate_fn = predicate::str::is_match("^Hello.*$").unwrap();
/// assert_eq!(true, predicate_fn.eval("Hello World"));
/// assert_eq!(false, predicate_fn.eval("Food World"));
/// ```
pub fn is_match<S>(pattern: S) -> Result<RegexPredicate, RegexError>
where
    S: AsRef<str>,
{
    regex::Regex::new(pattern.as_ref()).map(|re| RegexPredicate { re })
}
