// Copyright (c) 2018 The predicates-rs Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::borrow;
use std::fmt;

use crate::reflection;
use crate::Predicate;

/// Predicate that diffs two strings.
///
/// This is created by the `predicate::str::diff`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DifferencePredicate {
    orig: borrow::Cow<'static, str>,
}

impl Predicate<str> for DifferencePredicate {
    fn eval(&self, edit: &str) -> bool {
        edit == self.orig
    }

    fn find_case<'a>(&'a self, expected: bool, variable: &str) -> Option<reflection::Case<'a>> {
        let result = variable != self.orig;
        if result == expected {
            None
        } else {
            let palette = crate::Palette::new(true);
            let orig: Vec<_> = self.orig.lines().map(|l| format!("{l}\n")).collect();
            let variable: Vec<_> = variable.lines().map(|l| format!("{l}\n")).collect();
            let diff = difflib::unified_diff(
                &orig,
                &variable,
                "",
                "",
                &palette.expected("orig").to_string(),
                &palette.var("var").to_string(),
                0,
            );
            let mut diff = colorize_diff(diff, palette);
            diff.insert(0, "\n".to_owned());

            Some(
                reflection::Case::new(Some(self), result)
                    .add_product(reflection::Product::new("diff", diff.join(""))),
            )
        }
    }
}

impl reflection::PredicateReflection for DifferencePredicate {
    fn parameters<'a>(&'a self) -> Box<dyn Iterator<Item = reflection::Parameter<'a>> + 'a> {
        let params = vec![reflection::Parameter::new("original", &self.orig)];
        Box::new(params.into_iter())
    }
}

impl fmt::Display for DifferencePredicate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let palette = crate::Palette::new(f.alternate());
        write!(
            f,
            "{:#} {:#} {:#}",
            palette.description("diff"),
            palette.expected("original"),
            palette.var("var"),
        )
    }
}

/// Creates a new `Predicate` that diffs two strings.
///
/// # Examples
///
/// ```
/// use predicates::prelude::*;
///
/// let predicate_fn = predicate::str::diff("Hello World");
/// assert_eq!(true, predicate_fn.eval("Hello World"));
/// assert!(predicate_fn.find_case(false, "Hello World").is_none());
/// assert_eq!(false, predicate_fn.eval("Goodbye World"));
/// assert!(predicate_fn.find_case(false, "Goodbye World").is_some());
/// ```
pub fn diff<S>(orig: S) -> DifferencePredicate
where
    S: Into<borrow::Cow<'static, str>>,
{
    DifferencePredicate { orig: orig.into() }
}

#[cfg(feature = "color")]
fn colorize_diff(mut lines: Vec<String>, palette: crate::Palette) -> Vec<String> {
    for (i, line) in lines.iter_mut().enumerate() {
        match (i, line.as_bytes().first()) {
            (0, _) => {
                if let Some((prefix, body)) = line.split_once(' ') {
                    *line = format!("{:#} {}", palette.expected(prefix), body);
                }
            }
            (1, _) => {
                if let Some((prefix, body)) = line.split_once(' ') {
                    *line = format!("{:#} {}", palette.var(prefix), body);
                }
            }
            (_, Some(b'-')) => {
                let (prefix, body) = line.split_at(1);
                *line = format!("{:#}{}", palette.expected(prefix), body);
            }
            (_, Some(b'+')) => {
                let (prefix, body) = line.split_at(1);
                *line = format!("{:#}{}", palette.var(prefix), body);
            }
            (_, Some(b'@')) => {
                *line = format!("{:#}", palette.description(&line));
            }
            _ => (),
        }
    }
    lines
}

#[cfg(not(feature = "color"))]
fn colorize_diff(lines: Vec<String>, _palette: crate::Palette) -> Vec<String> {
    lines
}
