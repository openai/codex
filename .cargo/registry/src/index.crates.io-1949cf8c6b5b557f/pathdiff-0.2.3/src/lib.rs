// Copyright 2012-2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Adapted from rustc's path_relative_from
// https://github.com/rust-lang/rust/blob/e1d0de82cc40b666b88d4a6d2c9dcbc81d7ed27f/src/librustc_back/rpath.rs#L116-L158

#![cfg_attr(docsrs, feature(doc_cfg))]

use std::path::*;

/// Construct a relative path from a provided base directory path to the provided path.
///
/// ```rust
/// use pathdiff::diff_paths;
/// use std::path::*;
///
/// assert_eq!(diff_paths("/foo/bar",      "/foo/bar/baz"),  Some("../".into()));
/// assert_eq!(diff_paths("/foo/bar/baz",  "/foo/bar"),      Some("baz".into()));
/// assert_eq!(diff_paths("/foo/bar/quux", "/foo/bar/baz"),  Some("../quux".into()));
/// assert_eq!(diff_paths("/foo/bar/baz",  "/foo/bar/quux"), Some("../baz".into()));
/// assert_eq!(diff_paths("/foo/bar",      "/foo/bar/quux"), Some("../".into()));
///
/// assert_eq!(diff_paths("/foo/bar",      "baz"),           Some("/foo/bar".into()));
/// assert_eq!(diff_paths("/foo/bar",      "/baz"),          Some("../foo/bar".into()));
/// assert_eq!(diff_paths("foo",           "bar"),           Some("../foo".into()));
///
/// assert_eq!(
///     diff_paths(&"/foo/bar/baz", "/foo/bar".to_string()),
///     Some("baz".into())
/// );
/// assert_eq!(
///     diff_paths(Path::new("/foo/bar/baz"), Path::new("/foo/bar").to_path_buf()),
///     Some("baz".into())
/// );
/// ```
pub fn diff_paths<P, B>(path: P, base: B) -> Option<PathBuf>
where
    P: AsRef<Path>,
    B: AsRef<Path>,
{
    let path = path.as_ref();
    let base = base.as_ref();

    if path.is_absolute() != base.is_absolute() {
        if path.is_absolute() {
            Some(PathBuf::from(path))
        } else {
            None
        }
    } else {
        let mut ita = path.components();
        let mut itb = base.components();
        let mut comps: Vec<Component> = vec![];
        loop {
            match (ita.next(), itb.next()) {
                (None, None) => break,
                (Some(a), None) => {
                    comps.push(a);
                    comps.extend(ita.by_ref());
                    break;
                }
                (None, _) => comps.push(Component::ParentDir),
                (Some(a), Some(b)) if comps.is_empty() && a == b => (),
                (Some(a), Some(b)) if b == Component::CurDir => comps.push(a),
                (Some(_), Some(b)) if b == Component::ParentDir => return None,
                (Some(a), Some(_)) => {
                    comps.push(Component::ParentDir);
                    for _ in itb {
                        comps.push(Component::ParentDir);
                    }
                    comps.push(a);
                    comps.extend(ita.by_ref());
                    break;
                }
            }
        }
        Some(comps.iter().map(|c| c.as_os_str()).collect())
    }
}

#[cfg(feature = "camino")]
mod utf8_paths {
    use camino::{Utf8Component, Utf8Path, Utf8PathBuf};

    /// Construct a relative UTF-8 path from a provided base directory path to the provided path.
    ///
    /// Requires the `camino` feature.
    ///
    /// ```rust
    /// # extern crate camino;
    /// use camino::*;
    /// use pathdiff::diff_utf8_paths;
    ///
    /// assert_eq!(diff_utf8_paths("/foo/bar",      "/foo/bar/baz"),  Some("../".into()));
    /// assert_eq!(diff_utf8_paths("/foo/bar/baz",  "/foo/bar"),      Some("baz".into()));
    /// assert_eq!(diff_utf8_paths("/foo/bar/quux", "/foo/bar/baz"),  Some("../quux".into()));
    /// assert_eq!(diff_utf8_paths("/foo/bar/baz",  "/foo/bar/quux"), Some("../baz".into()));
    /// assert_eq!(diff_utf8_paths("/foo/bar",      "/foo/bar/quux"), Some("../".into()));
    ///
    /// assert_eq!(diff_utf8_paths("/foo/bar",      "baz"),           Some("/foo/bar".into()));
    /// assert_eq!(diff_utf8_paths("/foo/bar",      "/baz"),          Some("../foo/bar".into()));
    /// assert_eq!(diff_utf8_paths("foo",           "bar"),           Some("../foo".into()));
    ///
    /// assert_eq!(
    ///     diff_utf8_paths(&"/foo/bar/baz", "/foo/bar".to_string()),
    ///     Some("baz".into())
    /// );
    /// assert_eq!(
    ///     diff_utf8_paths(Utf8Path::new("/foo/bar/baz"), Utf8Path::new("/foo/bar").to_path_buf()),
    ///     Some("baz".into())
    /// );
    /// ```
    #[cfg_attr(docsrs, doc(cfg(feature = "camino")))]
    pub fn diff_utf8_paths<P, B>(path: P, base: B) -> Option<Utf8PathBuf>
    where
        P: AsRef<Utf8Path>,
        B: AsRef<Utf8Path>,
    {
        let path = path.as_ref();
        let base = base.as_ref();

        if path.is_absolute() != base.is_absolute() {
            if path.is_absolute() {
                Some(Utf8PathBuf::from(path))
            } else {
                None
            }
        } else {
            let mut ita = path.components();
            let mut itb = base.components();
            let mut comps: Vec<Utf8Component> = vec![];
            loop {
                match (ita.next(), itb.next()) {
                    (None, None) => break,
                    (Some(a), None) => {
                        comps.push(a);
                        comps.extend(ita.by_ref());
                        break;
                    }
                    (None, _) => comps.push(Utf8Component::ParentDir),
                    (Some(a), Some(b)) if comps.is_empty() && a == b => (),
                    (Some(a), Some(b)) if b == Utf8Component::CurDir => comps.push(a),
                    (Some(_), Some(b)) if b == Utf8Component::ParentDir => return None,
                    (Some(a), Some(_)) => {
                        comps.push(Utf8Component::ParentDir);
                        for _ in itb {
                            comps.push(Utf8Component::ParentDir);
                        }
                        comps.push(a);
                        comps.extend(ita.by_ref());
                        break;
                    }
                }
            }
            Some(comps.iter().map(|c| c.as_str()).collect())
        }
    }
}

#[cfg(feature = "camino")]
pub use crate::utf8_paths::*;

#[cfg(test)]
mod tests {
    use super::*;
    use cfg_if::cfg_if;

    #[test]
    fn test_absolute() {
        fn abs(path: &str) -> String {
            // Absolute paths look different on Windows vs Unix.
            cfg_if! {
                if #[cfg(windows)] {
                    format!("C:\\{}", path)
                } else {
                    format!("/{}", path)
                }
            }
        }

        assert_diff_paths(&abs("foo"), &abs("bar"), Some("../foo"));
        assert_diff_paths(&abs("foo"), "bar", Some(&abs("foo")));
        assert_diff_paths("foo", &abs("bar"), None);
        assert_diff_paths("foo", "bar", Some("../foo"));
    }

    #[test]
    fn test_identity() {
        assert_diff_paths(".", ".", Some(""));
        assert_diff_paths("../foo", "../foo", Some(""));
        assert_diff_paths("./foo", "./foo", Some(""));
        assert_diff_paths("/foo", "/foo", Some(""));
        assert_diff_paths("foo", "foo", Some(""));

        assert_diff_paths("../foo/bar/baz", "../foo/bar/baz", Some("".into()));
        assert_diff_paths("foo/bar/baz", "foo/bar/baz", Some(""));
    }

    #[test]
    fn test_subset() {
        assert_diff_paths("foo", "fo", Some("../foo"));
        assert_diff_paths("fo", "foo", Some("../fo"));
    }

    #[test]
    fn test_empty() {
        assert_diff_paths("", "", Some(""));
        assert_diff_paths("foo", "", Some("foo"));
        assert_diff_paths("", "foo", Some(".."));
    }

    #[test]
    fn test_relative() {
        assert_diff_paths("../foo", "../bar", Some("../foo"));
        assert_diff_paths("../foo", "../foo/bar/baz", Some("../.."));
        assert_diff_paths("../foo/bar/baz", "../foo", Some("bar/baz"));

        assert_diff_paths("foo/bar/baz", "foo", Some("bar/baz"));
        assert_diff_paths("foo/bar/baz", "foo/bar", Some("baz"));
        assert_diff_paths("foo/bar/baz", "foo/bar/baz", Some(""));
        assert_diff_paths("foo/bar/baz", "foo/bar/baz/", Some(""));

        assert_diff_paths("foo/bar/baz/", "foo", Some("bar/baz"));
        assert_diff_paths("foo/bar/baz/", "foo/bar", Some("baz"));
        assert_diff_paths("foo/bar/baz/", "foo/bar/baz", Some(""));
        assert_diff_paths("foo/bar/baz/", "foo/bar/baz/", Some(""));

        assert_diff_paths("foo/bar/baz", "foo/", Some("bar/baz"));
        assert_diff_paths("foo/bar/baz", "foo/bar/", Some("baz"));
        assert_diff_paths("foo/bar/baz", "foo/bar/baz", Some(""));
    }

    #[test]
    fn test_current_directory() {
        assert_diff_paths(".", "foo", Some("../."));
        assert_diff_paths("foo", ".", Some("foo"));
        assert_diff_paths("/foo", "/.", Some("foo"));
    }

    fn assert_diff_paths(path: &str, base: &str, expected: Option<&str>) {
        assert_eq!(diff_paths(path, base), expected.map(|s| s.into()));
        #[cfg(feature = "camino")]
        assert_eq!(diff_utf8_paths(path, base), expected.map(|s| s.into()));
    }
}
