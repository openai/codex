// Copyright (c) 2018 The predicates-rs Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::fmt;
use std::fs;
use std::io;
use std::path;

use crate::reflection;
use crate::Predicate;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FileType {
    File,
    Dir,
    Symlink,
}

impl FileType {
    fn from_path(path: &path::Path, follow: bool) -> io::Result<FileType> {
        let file_type = if follow {
            path.metadata()
        } else {
            path.symlink_metadata()
        }?
        .file_type();
        if file_type.is_dir() {
            return Ok(FileType::Dir);
        }
        if file_type.is_file() {
            return Ok(FileType::File);
        }
        Ok(FileType::Symlink)
    }

    fn eval(self, ft: fs::FileType) -> bool {
        match self {
            FileType::File => ft.is_file(),
            FileType::Dir => ft.is_dir(),
            FileType::Symlink => ft.is_symlink(),
        }
    }
}

impl fmt::Display for FileType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let t = match *self {
            FileType::File => "file",
            FileType::Dir => "dir",
            FileType::Symlink => "symlink",
        };
        write!(f, "{t}")
    }
}

/// Predicate that checks the `std::fs::FileType`.
///
/// This is created by the `predicate::path::is_file`, `predicate::path::is_dir`, and `predicate::path::is_symlink`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileTypePredicate {
    ft: FileType,
    follow: bool,
}

impl FileTypePredicate {
    /// Follow symbolic links.
    ///
    /// When yes is true, symbolic links are followed as if they were normal directories and files.
    ///
    /// Default: disabled.
    pub fn follow_links(mut self, yes: bool) -> Self {
        self.follow = yes;
        self
    }

    /// Allow to create an `FileTypePredicate` from a `path`
    pub fn from_path(path: &path::Path) -> io::Result<FileTypePredicate> {
        Ok(FileTypePredicate {
            ft: FileType::from_path(path, true)?,
            follow: true,
        })
    }
}

impl Predicate<path::Path> for FileTypePredicate {
    fn eval(&self, path: &path::Path) -> bool {
        let metadata = if self.follow {
            path.metadata()
        } else {
            path.symlink_metadata()
        };
        metadata
            .map(|m| self.ft.eval(m.file_type()))
            .unwrap_or(false)
    }

    fn find_case<'a>(
        &'a self,
        expected: bool,
        variable: &path::Path,
    ) -> Option<reflection::Case<'a>> {
        let actual_type = FileType::from_path(variable, self.follow);
        match (expected, actual_type) {
            (_, Ok(actual_type)) => {
                let result = self.ft == actual_type;
                if result == expected {
                    Some(
                        reflection::Case::new(Some(self), result)
                            .add_product(reflection::Product::new("actual filetype", actual_type)),
                    )
                } else {
                    None
                }
            }
            (true, Err(_)) => None,
            (false, Err(err)) => Some(
                reflection::Case::new(Some(self), false)
                    .add_product(reflection::Product::new("error", err)),
            ),
        }
    }
}

impl reflection::PredicateReflection for FileTypePredicate {
    fn parameters<'a>(&'a self) -> Box<dyn Iterator<Item = reflection::Parameter<'a>> + 'a> {
        let params = vec![reflection::Parameter::new("follow", &self.follow)];
        Box::new(params.into_iter())
    }
}

impl fmt::Display for FileTypePredicate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let palette = crate::Palette::new(f.alternate());
        write!(
            f,
            "{} {} {}",
            palette.var("var"),
            palette.description("is"),
            palette.expected(self.ft)
        )
    }
}

/// Creates a new `Predicate` that ensures the path points to a file.
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// use predicates::prelude::*;
///
/// let predicate_fn = predicate::path::is_file();
/// assert_eq!(true, predicate_fn.eval(Path::new("Cargo.toml")));
/// assert_eq!(false, predicate_fn.eval(Path::new("src")));
/// assert_eq!(false, predicate_fn.eval(Path::new("non-existent-file.foo")));
/// ```
pub fn is_file() -> FileTypePredicate {
    FileTypePredicate {
        ft: FileType::File,
        follow: false,
    }
}

/// Creates a new `Predicate` that ensures the path points to a directory.
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// use predicates::prelude::*;
///
/// let predicate_fn = predicate::path::is_dir();
/// assert_eq!(false, predicate_fn.eval(Path::new("Cargo.toml")));
/// assert_eq!(true, predicate_fn.eval(Path::new("src")));
/// assert_eq!(false, predicate_fn.eval(Path::new("non-existent-file.foo")));
/// ```
pub fn is_dir() -> FileTypePredicate {
    FileTypePredicate {
        ft: FileType::Dir,
        follow: false,
    }
}

/// Creates a new `Predicate` that ensures the path points to a symlink.
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// use predicates::prelude::*;
///
/// let predicate_fn = predicate::path::is_symlink();
/// assert_eq!(false, predicate_fn.eval(Path::new("Cargo.toml")));
/// assert_eq!(false, predicate_fn.eval(Path::new("src")));
/// assert_eq!(false, predicate_fn.eval(Path::new("non-existent-file.foo")));
/// ```
pub fn is_symlink() -> FileTypePredicate {
    FileTypePredicate {
        ft: FileType::Symlink,
        follow: false,
    }
}
