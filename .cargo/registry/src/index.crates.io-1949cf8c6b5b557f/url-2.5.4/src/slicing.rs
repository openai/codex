// Copyright 2016 The rust-url developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use core::ops::{Index, Range, RangeFrom, RangeFull, RangeTo};

use crate::Url;

impl Index<RangeFull> for Url {
    type Output = str;
    fn index(&self, _: RangeFull) -> &str {
        &self.serialization
    }
}

impl Index<RangeFrom<Position>> for Url {
    type Output = str;
    fn index(&self, range: RangeFrom<Position>) -> &str {
        &self.serialization[self.index(range.start)..]
    }
}

impl Index<RangeTo<Position>> for Url {
    type Output = str;
    fn index(&self, range: RangeTo<Position>) -> &str {
        &self.serialization[..self.index(range.end)]
    }
}

impl Index<Range<Position>> for Url {
    type Output = str;
    fn index(&self, range: Range<Position>) -> &str {
        &self.serialization[self.index(range.start)..self.index(range.end)]
    }
}

// Counts how many base-10 digits are required to represent n in the given base
fn count_digits(n: u16) -> usize {
    match n {
        0..=9 => 1,
        10..=99 => 2,
        100..=999 => 3,
        1000..=9999 => 4,
        10000..=65535 => 5,
    }
}

#[test]
fn test_count_digits() {
    assert_eq!(count_digits(0), 1);
    assert_eq!(count_digits(1), 1);
    assert_eq!(count_digits(9), 1);
    assert_eq!(count_digits(10), 2);
    assert_eq!(count_digits(99), 2);
    assert_eq!(count_digits(100), 3);
    assert_eq!(count_digits(9999), 4);
    assert_eq!(count_digits(65535), 5);
}

/// Indicates a position within a URL based on its components.
///
/// A range of positions can be used for slicing `Url`:
///
/// ```rust
/// # use url::{Url, Position};
/// # fn something(some_url: Url) {
/// let serialization: &str = &some_url[..];
/// let serialization_without_fragment: &str = &some_url[..Position::AfterQuery];
/// let authority: &str = &some_url[Position::BeforeUsername..Position::AfterPort];
/// let data_url_payload: &str = &some_url[Position::BeforePath..Position::AfterQuery];
/// let scheme_relative: &str = &some_url[Position::BeforeUsername..];
/// # }
/// ```
///
/// In a pseudo-grammar (where `[`…`]?` makes a sub-sequence optional),
/// URL components and delimiters that separate them are:
///
/// ```notrust
/// url =
///     scheme ":"
///     [ "//" [ username [ ":" password ]? "@" ]? host [ ":" port ]? ]?
///     path [ "?" query ]? [ "#" fragment ]?
/// ```
///
/// When a given component is not present,
/// its "before" and "after" position are the same
/// (so that `&some_url[BeforeFoo..AfterFoo]` is the empty string)
/// and component ordering is preserved
/// (so that a missing query "is between" a path and a fragment).
///
/// The end of a component and the start of the next are either the same or separate
/// by a delimiter.
/// (Note that the initial `/` of a path is considered part of the path here, not a delimiter.)
/// For example, `&url[..BeforeFragment]` would include a `#` delimiter (if present in `url`),
/// so `&url[..AfterQuery]` might be desired instead.
///
/// `BeforeScheme` and `AfterFragment` are always the start and end of the entire URL,
/// so `&url[BeforeScheme..X]` is the same as `&url[..X]`
/// and `&url[X..AfterFragment]` is the same as `&url[X..]`.
#[derive(Copy, Clone, Debug)]
pub enum Position {
    BeforeScheme,
    AfterScheme,
    BeforeUsername,
    AfterUsername,
    BeforePassword,
    AfterPassword,
    BeforeHost,
    AfterHost,
    BeforePort,
    AfterPort,
    BeforePath,
    AfterPath,
    BeforeQuery,
    AfterQuery,
    BeforeFragment,
    AfterFragment,
}

impl Url {
    #[inline]
    fn index(&self, position: Position) -> usize {
        match position {
            Position::BeforeScheme => 0,

            Position::AfterScheme => self.scheme_end as usize,

            Position::BeforeUsername => {
                if self.has_authority() {
                    self.scheme_end as usize + "://".len()
                } else {
                    debug_assert!(self.byte_at(self.scheme_end) == b':');
                    debug_assert!(self.scheme_end + ":".len() as u32 == self.username_end);
                    self.scheme_end as usize + ":".len()
                }
            }

            Position::AfterUsername => self.username_end as usize,

            Position::BeforePassword => {
                if self.has_authority() && self.byte_at(self.username_end) == b':' {
                    self.username_end as usize + ":".len()
                } else {
                    debug_assert!(self.username_end == self.host_start);
                    self.username_end as usize
                }
            }

            Position::AfterPassword => {
                if self.has_authority() && self.byte_at(self.username_end) == b':' {
                    debug_assert!(self.byte_at(self.host_start - "@".len() as u32) == b'@');
                    self.host_start as usize - "@".len()
                } else {
                    debug_assert!(self.username_end == self.host_start);
                    self.host_start as usize
                }
            }

            Position::BeforeHost => self.host_start as usize,

            Position::AfterHost => self.host_end as usize,

            Position::BeforePort => {
                if self.port.is_some() {
                    debug_assert!(self.byte_at(self.host_end) == b':');
                    self.host_end as usize + ":".len()
                } else {
                    self.host_end as usize
                }
            }

            Position::AfterPort => {
                if let Some(port) = self.port {
                    debug_assert!(self.byte_at(self.host_end) == b':');
                    self.host_end as usize + ":".len() + count_digits(port)
                } else {
                    self.host_end as usize
                }
            }

            Position::BeforePath => self.path_start as usize,

            Position::AfterPath => match (self.query_start, self.fragment_start) {
                (Some(q), _) => q as usize,
                (None, Some(f)) => f as usize,
                (None, None) => self.serialization.len(),
            },

            Position::BeforeQuery => match (self.query_start, self.fragment_start) {
                (Some(q), _) => {
                    debug_assert!(self.byte_at(q) == b'?');
                    q as usize + "?".len()
                }
                (None, Some(f)) => f as usize,
                (None, None) => self.serialization.len(),
            },

            Position::AfterQuery => match self.fragment_start {
                None => self.serialization.len(),
                Some(f) => f as usize,
            },

            Position::BeforeFragment => match self.fragment_start {
                Some(f) => {
                    debug_assert!(self.byte_at(f) == b'#');
                    f as usize + "#".len()
                }
                None => self.serialization.len(),
            },

            Position::AfterFragment => self.serialization.len(),
        }
    }
}
