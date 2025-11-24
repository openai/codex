// Copyright (c) 2018 The predicates-rs Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::fmt;
use std::fs;
use std::io::{self, Read};
use std::path;

use crate::reflection;
use crate::utils;
use crate::Predicate;

fn read_file(path: &path::Path) -> io::Result<Vec<u8>> {
    let mut buffer = Vec::new();
    fs::File::open(path)?.read_to_end(&mut buffer)?;
    Ok(buffer)
}

/// Predicate that compares file matches
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryFilePredicate {
    path: path::PathBuf,
    content: utils::DebugAdapter<Vec<u8>>,
}

impl BinaryFilePredicate {
    fn eval(&self, path: &path::Path) -> io::Result<bool> {
        let content = read_file(path)?;
        Ok(self.content.debug == content)
    }

    /// Creates a new `Predicate` that ensures complete equality
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::Path;
    /// use predicates::prelude::*;
    ///
    /// let predicate_file = predicate::path::eq_file(Path::new("Cargo.toml")).utf8().unwrap();
    /// assert_eq!(true, predicate_file.eval(Path::new("Cargo.toml")));
    /// assert_eq!(false, predicate_file.eval(Path::new("Cargo.lock")));
    /// assert_eq!(false, predicate_file.eval(Path::new("src")));
    ///
    /// assert_eq!(false, predicate_file.eval("Not a real Cargo.toml file content"));
    /// ```
    pub fn utf8(self) -> Option<StrFilePredicate> {
        let path = self.path;
        let content = String::from_utf8(self.content.debug).ok()?;
        Some(StrFilePredicate { path, content })
    }
}

impl Predicate<path::Path> for BinaryFilePredicate {
    fn eval(&self, path: &path::Path) -> bool {
        self.eval(path).unwrap_or(false)
    }

    fn find_case<'a>(
        &'a self,
        expected: bool,
        variable: &path::Path,
    ) -> Option<reflection::Case<'a>> {
        utils::default_find_case(self, expected, variable)
    }
}

impl Predicate<[u8]> for BinaryFilePredicate {
    fn eval(&self, actual: &[u8]) -> bool {
        self.content.debug == actual
    }

    fn find_case<'a>(&'a self, expected: bool, variable: &[u8]) -> Option<reflection::Case<'a>> {
        utils::default_find_case(self, expected, variable)
    }
}

impl reflection::PredicateReflection for BinaryFilePredicate {
    fn parameters<'a>(&'a self) -> Box<dyn Iterator<Item = reflection::Parameter<'a>> + 'a> {
        let params = vec![reflection::Parameter::new("content", &self.content)];
        Box::new(params.into_iter())
    }
}

impl fmt::Display for BinaryFilePredicate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let palette = crate::Palette::new(f.alternate());
        write!(
            f,
            "{} {} {}",
            palette.var("var"),
            palette.description("is"),
            palette.expected(self.path.display())
        )
    }
}

/// Creates a new `Predicate` that ensures complete equality
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// use predicates::prelude::*;
///
/// let predicate_file = predicate::path::eq_file(Path::new("Cargo.toml"));
/// assert_eq!(true, predicate_file.eval(Path::new("Cargo.toml")));
/// assert_eq!(false, predicate_file.eval(Path::new("src")));
/// assert_eq!(false, predicate_file.eval(Path::new("src")));
/// ```
pub fn eq_file<P: Into<path::PathBuf>>(path: P) -> BinaryFilePredicate {
    let path = path.into();
    let content = utils::DebugAdapter::new(read_file(&path).unwrap());
    BinaryFilePredicate { path, content }
}

/// Predicate that compares string content of files
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StrFilePredicate {
    path: path::PathBuf,
    content: String,
}

impl StrFilePredicate {
    fn eval(&self, path: &path::Path) -> Option<bool> {
        let content = read_file(path).ok()?;
        let content = String::from_utf8(content).ok()?;
        Some(self.content == content)
    }
}

impl Predicate<path::Path> for StrFilePredicate {
    fn eval(&self, path: &path::Path) -> bool {
        self.eval(path).unwrap_or(false)
    }

    fn find_case<'a>(
        &'a self,
        expected: bool,
        variable: &path::Path,
    ) -> Option<reflection::Case<'a>> {
        utils::default_find_case(self, expected, variable)
    }
}

impl Predicate<str> for StrFilePredicate {
    fn eval(&self, actual: &str) -> bool {
        self.content == actual
    }

    fn find_case<'a>(&'a self, expected: bool, variable: &str) -> Option<reflection::Case<'a>> {
        utils::default_find_case(self, expected, variable)
    }
}

impl reflection::PredicateReflection for StrFilePredicate {
    fn parameters<'a>(&'a self) -> Box<dyn Iterator<Item = reflection::Parameter<'a>> + 'a> {
        let params = vec![reflection::Parameter::new("content", &self.content)];
        Box::new(params.into_iter())
    }
}

impl fmt::Display for StrFilePredicate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let palette = crate::Palette::new(f.alternate());
        write!(
            f,
            "{} {} {}",
            palette.var("var"),
            palette.description("is"),
            palette.expected(self.path.display())
        )
    }
}
