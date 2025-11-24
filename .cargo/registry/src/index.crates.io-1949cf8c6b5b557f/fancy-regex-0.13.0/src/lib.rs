// Copyright 2016 The Fancy Regex Authors.
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
// THE SOFTWARE.

/*!
An implementation of regexes, supporting a relatively rich set of features, including backreferences
and lookaround.

It builds on top of the excellent [regex] crate. If you are not
familiar with it, make sure you read its documentation and maybe you don't even need fancy-regex.

If your regex or parts of it does not use any special features, the matching is delegated to the
regex crate. That means it has linear runtime. But if you use "fancy" features such as
backreferences or look-around, an engine with backtracking needs to be used. In that case, the regex
can be slow and take exponential time to run because of what is called "catastrophic backtracking".
This depends on the regex and the input.

# Usage

The API should feel very similar to the regex crate, and involves compiling a regex and then using
it to find matches in text.

## Example: Matching text

An example with backreferences to check if a text consists of two identical words:

```rust
use fancy_regex::Regex;

let re = Regex::new(r"^(\w+) (\1)$").unwrap();
let result = re.is_match("foo foo");

assert!(result.is_ok());
let did_match = result.unwrap();
assert!(did_match);
```

Note that like in the regex crate, the regex needs anchors like `^` and `$` to match against the
entire input text.

## Example: Finding the position of matches

```rust
use fancy_regex::Regex;

let re = Regex::new(r"(\d)\1").unwrap();
let result = re.find("foo 22");

assert!(result.is_ok(), "execution was successful");
let match_option = result.unwrap();

assert!(match_option.is_some(), "found a match");
let m = match_option.unwrap();

assert_eq!(m.start(), 4);
assert_eq!(m.end(), 6);
assert_eq!(m.as_str(), "22");
```

## Example: Capturing groups

```rust
use fancy_regex::Regex;

let re = Regex::new(r"(?<!AU)\$(\d+)").unwrap();
let result = re.captures("AU$10, $20");

let captures = result.expect("Error running regex").expect("No match found");
let group = captures.get(1).expect("No group");
assert_eq!(group.as_str(), "20");
```

# Syntax

The regex syntax is based on the [regex] crate's, with some additional supported syntax.

Escapes:

`\h`
: hex digit (`[0-9A-Fa-f]`) \
`\H`
: not hex digit (`[^0-9A-Fa-f]`) \
`\e`
: escape control character (`\x1B`) \
`\K`
: keep text matched so far out of the overall match ([docs](https://www.regular-expressions.info/keep.html))\
`\G`
: anchor to where the previous match ended ([docs](https://www.regular-expressions.info/continue.html))

Backreferences:

`\1`
: match the exact string that the first capture group matched \
`\2`
: backref to the second capture group, etc

Named capture groups:

`(?<name>exp)`
: match *exp*, creating capture group named *name* \
`\k<name>`
: match the exact string that the capture group named *name* matched \
`(?P<name>exp)`
: same as `(?<name>exp)` for compatibility with Python, etc. \
`(?P=name)`
: same as `\k<name>` for compatibility with Python, etc.

Look-around assertions for matching without changing the current position:

`(?=exp)`
: look-ahead, succeeds if *exp* matches to the right of the current position \
`(?!exp)`
: negative look-ahead, succeeds if *exp* doesn't match to the right \
`(?<=exp)`
: look-behind, succeeds if *exp* matches to the left of the current position \
`(?<!exp)`
: negative look-behind, succeeds if *exp* doesn't match to the left

Atomic groups using `(?>exp)` to prevent backtracking within `exp`, e.g.:

```
# use fancy_regex::Regex;
let re = Regex::new(r"^a(?>bc|b)c$").unwrap();
assert!(re.is_match("abcc").unwrap());
// Doesn't match because `|b` is never tried because of the atomic group
assert!(!re.is_match("abc").unwrap());
```

Conditionals - if/then/else:

`(?(1))`
: continue only if first capture group matched \
`(?(<name>))`
: continue only if capture group named *name* matched \
`(?(1)true_branch|false_branch)`
: if the first capture group matched then execute the true_branch regex expression, else execute false_branch ([docs](https://www.regular-expressions.info/conditional.html)) \
`(?(condition)true_branch|false_branch)`
: if the condition matches then execute the true_branch regex expression, else execute false_branch from the point just before the condition was evaluated

[regex]: https://crates.io/crates/regex
*/

#![doc(html_root_url = "https://docs.rs/fancy-regex/0.13.0")]
#![deny(missing_docs)]
#![deny(missing_debug_implementations)]
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::borrow::Cow;
use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;

use core::convert::TryFrom;
use core::fmt::{Debug, Formatter};
use core::ops::{Index, Range};
use core::str::FromStr;
use core::{fmt, usize};
use regex_automata::meta::Regex as RaRegex;
use regex_automata::util::captures::Captures as RaCaptures;
use regex_automata::Input as RaInput;

mod analyze;
mod compile;
mod error;
mod expand;
mod parse;
mod replacer;
mod vm;

use crate::analyze::analyze;
use crate::compile::compile;
use crate::parse::{ExprTree, NamedGroups, Parser};
use crate::vm::{Prog, OPTION_SKIPPED_EMPTY_MATCH};

pub use crate::error::{CompileError, Error, ParseError, Result, RuntimeError};
pub use crate::expand::Expander;
pub use crate::replacer::{NoExpand, Replacer, ReplacerRef};

const MAX_RECURSION: usize = 64;

// the public API

/// A builder for a `Regex` to allow configuring options.
#[derive(Debug)]
pub struct RegexBuilder(RegexOptions);

/// A compiled regular expression.
#[derive(Clone)]
pub struct Regex {
    inner: RegexImpl,
    named_groups: Arc<NamedGroups>,
}

// Separate enum because we don't want to expose any of this
#[derive(Clone)]
enum RegexImpl {
    // Do we want to box this? It's pretty big...
    Wrap {
        inner: RaRegex,
        options: RegexOptions,
    },
    Fancy {
        prog: Prog,
        n_groups: usize,
        options: RegexOptions,
    },
}

/// A single match of a regex or group in an input text
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Match<'t> {
    text: &'t str,
    start: usize,
    end: usize,
}

/// An iterator over all non-overlapping matches for a particular string.
///
/// The iterator yields a `Result<Match>`. The iterator stops when no more
/// matches can be found.
///
/// `'r` is the lifetime of the compiled regular expression and `'t` is the
/// lifetime of the matched string.
#[derive(Debug)]
pub struct Matches<'r, 't> {
    re: &'r Regex,
    text: &'t str,
    last_end: usize,
    last_match: Option<usize>,
}

impl<'r, 't> Matches<'r, 't> {
    /// Return the text being searched.
    pub fn text(&self) -> &'t str {
        self.text
    }

    /// Return the underlying regex.
    pub fn regex(&self) -> &'r Regex {
        &self.re
    }
}

impl<'r, 't> Iterator for Matches<'r, 't> {
    type Item = Result<Match<'t>>;

    /// Adapted from the `regex` crate. Calls `find_from_pos` repeatedly.
    /// Ignores empty matches immediately after a match.
    fn next(&mut self) -> Option<Self::Item> {
        if self.last_end > self.text.len() {
            return None;
        }

        let option_flags = if let Some(last_match) = self.last_match {
            if self.last_end > last_match {
                OPTION_SKIPPED_EMPTY_MATCH
            } else {
                0
            }
        } else {
            0
        };
        let mat =
            match self
                .re
                .find_from_pos_with_option_flags(self.text, self.last_end, option_flags)
            {
                Err(error) => return Some(Err(error)),
                Ok(None) => return None,
                Ok(Some(mat)) => mat,
            };

        if mat.start == mat.end {
            // This is an empty match. To ensure we make progress, start
            // the next search at the smallest possible starting position
            // of the next match following this one.
            self.last_end = next_utf8(self.text, mat.end);
            // Don't accept empty matches immediately following a match.
            // Just move on to the next match.
            if Some(mat.end) == self.last_match {
                return self.next();
            }
        } else {
            self.last_end = mat.end;
        }

        self.last_match = Some(mat.end);

        Some(Ok(mat))
    }
}

/// An iterator that yields all non-overlapping capture groups matching a
/// particular regular expression.
///
/// The iterator stops when no more matches can be found.
///
/// `'r` is the lifetime of the compiled regular expression and `'t` is the
/// lifetime of the matched string.
#[derive(Debug)]
pub struct CaptureMatches<'r, 't>(Matches<'r, 't>);

impl<'r, 't> CaptureMatches<'r, 't> {
    /// Return the text being searched.
    pub fn text(&self) -> &'t str {
        self.0.text
    }

    /// Return the underlying regex.
    pub fn regex(&self) -> &'r Regex {
        &self.0.re
    }
}

impl<'r, 't> Iterator for CaptureMatches<'r, 't> {
    type Item = Result<Captures<'t>>;

    /// Adapted from the `regex` crate. Calls `captures_from_pos` repeatedly.
    /// Ignores empty matches immediately after a match.
    fn next(&mut self) -> Option<Self::Item> {
        if self.0.last_end > self.0.text.len() {
            return None;
        }

        let captures = match self.0.re.captures_from_pos(self.0.text, self.0.last_end) {
            Err(error) => return Some(Err(error)),
            Ok(None) => return None,
            Ok(Some(captures)) => captures,
        };

        let mat = captures
            .get(0)
            .expect("`Captures` is expected to have entire match at 0th position");
        if mat.start == mat.end {
            self.0.last_end = next_utf8(self.0.text, mat.end);
            if Some(mat.end) == self.0.last_match {
                return self.next();
            }
        } else {
            self.0.last_end = mat.end;
        }

        self.0.last_match = Some(mat.end);

        Some(Ok(captures))
    }
}

/// A set of capture groups found for a regex.
#[derive(Debug)]
pub struct Captures<'t> {
    inner: CapturesImpl<'t>,
    named_groups: Arc<NamedGroups>,
}

#[derive(Debug)]
enum CapturesImpl<'t> {
    Wrap {
        text: &'t str,
        locations: RaCaptures,
    },
    Fancy {
        text: &'t str,
        saves: Vec<usize>,
    },
}

/// Iterator for captured groups in order in which they appear in the regex.
#[derive(Debug)]
pub struct SubCaptureMatches<'c, 't> {
    caps: &'c Captures<'t>,
    i: usize,
}

#[derive(Clone, Debug)]
struct RegexOptions {
    pattern: String,
    backtrack_limit: usize,
    delegate_size_limit: Option<usize>,
    delegate_dfa_size_limit: Option<usize>,
}

impl Default for RegexOptions {
    fn default() -> Self {
        RegexOptions {
            pattern: String::new(),
            backtrack_limit: 1_000_000,
            delegate_size_limit: None,
            delegate_dfa_size_limit: None,
        }
    }
}

impl RegexBuilder {
    /// Create a new regex builder with a regex pattern.
    ///
    /// If the pattern is invalid, the call to `build` will fail later.
    pub fn new(pattern: &str) -> Self {
        let mut builder = RegexBuilder(RegexOptions::default());
        builder.0.pattern = pattern.to_string();
        builder
    }

    /// Build the `Regex`.
    ///
    /// Returns an [`Error`](enum.Error.html) if the pattern could not be parsed.
    pub fn build(&self) -> Result<Regex> {
        Regex::new_options(self.0.clone())
    }

    /// Limit for how many times backtracking should be attempted for fancy regexes (where
    /// backtracking is used). If this limit is exceeded, execution returns an error with
    /// [`Error::BacktrackLimitExceeded`](enum.Error.html#variant.BacktrackLimitExceeded).
    /// This is for preventing a regex with catastrophic backtracking to run for too long.
    ///
    /// Default is `1_000_000` (1 million).
    pub fn backtrack_limit(&mut self, limit: usize) -> &mut Self {
        self.0.backtrack_limit = limit;
        self
    }

    /// Set the approximate size limit of the compiled regular expression.
    ///
    /// This option is forwarded from the wrapped `regex` crate. Note that depending on the used
    /// regex features there may be multiple delegated sub-regexes fed to the `regex` crate. As
    /// such the actual limit is closer to `<number of delegated regexes> * delegate_size_limit`.
    pub fn delegate_size_limit(&mut self, limit: usize) -> &mut Self {
        self.0.delegate_size_limit = Some(limit);
        self
    }

    /// Set the approximate size of the cache used by the DFA.
    ///
    /// This option is forwarded from the wrapped `regex` crate. Note that depending on the used
    /// regex features there may be multiple delegated sub-regexes fed to the `regex` crate. As
    /// such the actual limit is closer to `<number of delegated regexes> *
    /// delegate_dfa_size_limit`.
    pub fn delegate_dfa_size_limit(&mut self, limit: usize) -> &mut Self {
        self.0.delegate_dfa_size_limit = Some(limit);
        self
    }
}

impl fmt::Debug for Regex {
    /// Shows the original regular expression.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl fmt::Display for Regex {
    /// Shows the original regular expression
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for Regex {
    type Err = Error;

    /// Attempts to parse a string into a regular expression
    fn from_str(s: &str) -> Result<Regex> {
        Regex::new(s)
    }
}

impl Regex {
    /// Parse and compile a regex with default options, see `RegexBuilder`.
    ///
    /// Returns an [`Error`](enum.Error.html) if the pattern could not be parsed.
    pub fn new(re: &str) -> Result<Regex> {
        let options = RegexOptions {
            pattern: re.to_string(),
            ..RegexOptions::default()
        };
        Self::new_options(options)
    }

    fn new_options(options: RegexOptions) -> Result<Regex> {
        let raw_tree = Expr::parse_tree(&options.pattern)?;

        // wrapper to search for re at arbitrary start position,
        // and to capture the match bounds
        let tree = ExprTree {
            expr: Expr::Concat(vec![
                Expr::Repeat {
                    child: Box::new(Expr::Any { newline: true }),
                    lo: 0,
                    hi: usize::MAX,
                    greedy: false,
                },
                Expr::Group(Box::new(raw_tree.expr)),
            ]),
            ..raw_tree
        };

        let info = analyze(&tree)?;

        let inner_info = &info.children[1].children[0]; // references inner expr
        if !inner_info.hard {
            // easy case, wrap regex

            // we do our own to_str because escapes are different
            let mut re_cooked = String::new();
            // same as raw_tree.expr above, but it was moved, so traverse to find it
            let raw_e = match tree.expr {
                Expr::Concat(ref v) => match v[1] {
                    Expr::Group(ref child) => child,
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            };
            raw_e.to_str(&mut re_cooked, 0);
            let inner = compile::compile_inner(&re_cooked, &options)?;
            return Ok(Regex {
                inner: RegexImpl::Wrap { inner, options },
                named_groups: Arc::new(tree.named_groups),
            });
        }

        let prog = compile(&info)?;
        Ok(Regex {
            inner: RegexImpl::Fancy {
                prog,
                n_groups: info.end_group,
                options,
            },
            named_groups: Arc::new(tree.named_groups),
        })
    }

    /// Returns the original string of this regex.
    pub fn as_str(&self) -> &str {
        match &self.inner {
            RegexImpl::Wrap { options, .. } => &options.pattern,
            RegexImpl::Fancy { options, .. } => &options.pattern,
        }
    }

    /// Check if the regex matches the input text.
    ///
    /// # Example
    ///
    /// Test if some text contains the same word twice:
    ///
    /// ```rust
    /// # use fancy_regex::Regex;
    ///
    /// let re = Regex::new(r"(\w+) \1").unwrap();
    /// assert!(re.is_match("mirror mirror on the wall").unwrap());
    /// ```
    pub fn is_match(&self, text: &str) -> Result<bool> {
        match &self.inner {
            RegexImpl::Wrap { ref inner, .. } => Ok(inner.is_match(text)),
            RegexImpl::Fancy {
                ref prog, options, ..
            } => {
                let result = vm::run(prog, text, 0, 0, options)?;
                Ok(result.is_some())
            }
        }
    }

    /// Returns an iterator for each successive non-overlapping match in `text`.
    ///
    /// If you have capturing groups in your regex that you want to extract, use the [Regex::captures_iter()]
    /// method.
    ///
    /// # Example
    ///
    /// Find all words followed by an exclamation point:
    ///
    /// ```rust
    /// # use fancy_regex::Regex;
    ///
    /// let re = Regex::new(r"\w+(?=!)").unwrap();
    /// let mut matches = re.find_iter("so fancy! even with! iterators!");
    /// assert_eq!(matches.next().unwrap().unwrap().as_str(), "fancy");
    /// assert_eq!(matches.next().unwrap().unwrap().as_str(), "with");
    /// assert_eq!(matches.next().unwrap().unwrap().as_str(), "iterators");
    /// assert!(matches.next().is_none());
    /// ```
    pub fn find_iter<'r, 't>(&'r self, text: &'t str) -> Matches<'r, 't> {
        Matches {
            re: &self,
            text,
            last_end: 0,
            last_match: None,
        }
    }

    /// Find the first match in the input text.
    ///
    /// If you have capturing groups in your regex that you want to extract, use the [Regex::captures()]
    /// method.
    ///
    /// # Example
    ///
    /// Find a word that is followed by an exclamation point:
    ///
    /// ```rust
    /// # use fancy_regex::Regex;
    ///
    /// let re = Regex::new(r"\w+(?=!)").unwrap();
    /// assert_eq!(re.find("so fancy!").unwrap().unwrap().as_str(), "fancy");
    /// ```
    pub fn find<'t>(&self, text: &'t str) -> Result<Option<Match<'t>>> {
        self.find_from_pos(text, 0)
    }

    /// Returns the first match in `text`, starting from the specified byte position `pos`.
    ///
    /// # Examples
    ///
    /// Finding match starting at a position:
    ///
    /// ```
    /// # use fancy_regex::Regex;
    /// let re = Regex::new(r"(?m:^)(\d+)").unwrap();
    /// let text = "1 test 123\n2 foo";
    /// let mat = re.find_from_pos(text, 7).unwrap().unwrap();
    ///
    /// assert_eq!(mat.start(), 11);
    /// assert_eq!(mat.end(), 12);
    /// ```
    ///
    /// Note that in some cases this is not the same as using the `find`
    /// method and passing a slice of the string, see [Regex::captures_from_pos()] for details.
    pub fn find_from_pos<'t>(&self, text: &'t str, pos: usize) -> Result<Option<Match<'t>>> {
        self.find_from_pos_with_option_flags(text, pos, 0)
    }

    fn find_from_pos_with_option_flags<'t>(
        &self,
        text: &'t str,
        pos: usize,
        option_flags: u32,
    ) -> Result<Option<Match<'t>>> {
        match &self.inner {
            RegexImpl::Wrap { inner, .. } => Ok(inner
                .search(&RaInput::new(text).span(pos..text.len()))
                .map(|m| Match::new(text, m.start(), m.end()))),
            RegexImpl::Fancy { prog, options, .. } => {
                let result = vm::run(prog, text, pos, option_flags, options)?;
                Ok(result.map(|saves| Match::new(text, saves[0], saves[1])))
            }
        }
    }

    /// Returns an iterator over all the non-overlapping capture groups matched in `text`.
    ///
    /// # Examples
    ///
    /// Finding all matches and capturing parts of each:
    ///
    /// ```rust
    /// # use fancy_regex::Regex;
    ///
    /// let re = Regex::new(r"(\d{4})-(\d{2})").unwrap();
    /// let text = "It was between 2018-04 and 2020-01";
    /// let mut all_captures = re.captures_iter(text);
    ///
    /// let first = all_captures.next().unwrap().unwrap();
    /// assert_eq!(first.get(1).unwrap().as_str(), "2018");
    /// assert_eq!(first.get(2).unwrap().as_str(), "04");
    /// assert_eq!(first.get(0).unwrap().as_str(), "2018-04");
    ///
    /// let second = all_captures.next().unwrap().unwrap();
    /// assert_eq!(second.get(1).unwrap().as_str(), "2020");
    /// assert_eq!(second.get(2).unwrap().as_str(), "01");
    /// assert_eq!(second.get(0).unwrap().as_str(), "2020-01");
    ///
    /// assert!(all_captures.next().is_none());
    /// ```
    pub fn captures_iter<'r, 't>(&'r self, text: &'t str) -> CaptureMatches<'r, 't> {
        CaptureMatches(self.find_iter(text))
    }

    /// Returns the capture groups for the first match in `text`.
    ///
    /// If no match is found, then `Ok(None)` is returned.
    ///
    /// # Examples
    ///
    /// Finding matches and capturing parts of the match:
    ///
    /// ```rust
    /// # use fancy_regex::Regex;
    ///
    /// let re = Regex::new(r"(\d{4})-(\d{2})-(\d{2})").unwrap();
    /// let text = "The date was 2018-04-07";
    /// let captures = re.captures(text).unwrap().unwrap();
    ///
    /// assert_eq!(captures.get(1).unwrap().as_str(), "2018");
    /// assert_eq!(captures.get(2).unwrap().as_str(), "04");
    /// assert_eq!(captures.get(3).unwrap().as_str(), "07");
    /// assert_eq!(captures.get(0).unwrap().as_str(), "2018-04-07");
    /// ```
    pub fn captures<'t>(&self, text: &'t str) -> Result<Option<Captures<'t>>> {
        self.captures_from_pos(text, 0)
    }

    /// Returns the capture groups for the first match in `text`, starting from
    /// the specified byte position `pos`.
    ///
    /// # Examples
    ///
    /// Finding captures starting at a position:
    ///
    /// ```
    /// # use fancy_regex::Regex;
    /// let re = Regex::new(r"(?m:^)(\d+)").unwrap();
    /// let text = "1 test 123\n2 foo";
    /// let captures = re.captures_from_pos(text, 7).unwrap().unwrap();
    ///
    /// let group = captures.get(1).unwrap();
    /// assert_eq!(group.as_str(), "2");
    /// assert_eq!(group.start(), 11);
    /// assert_eq!(group.end(), 12);
    /// ```
    ///
    /// Note that in some cases this is not the same as using the `captures`
    /// method and passing a slice of the string, see the capture that we get
    /// when we do this:
    ///
    /// ```
    /// # use fancy_regex::Regex;
    /// let re = Regex::new(r"(?m:^)(\d+)").unwrap();
    /// let text = "1 test 123\n2 foo";
    /// let captures = re.captures(&text[7..]).unwrap().unwrap();
    /// assert_eq!(captures.get(1).unwrap().as_str(), "123");
    /// ```
    ///
    /// This matched the number "123" because it's at the beginning of the text
    /// of the string slice.
    ///
    pub fn captures_from_pos<'t>(&self, text: &'t str, pos: usize) -> Result<Option<Captures<'t>>> {
        let named_groups = self.named_groups.clone();
        match &self.inner {
            RegexImpl::Wrap { inner, .. } => {
                let mut locations = inner.create_captures();
                inner.captures(RaInput::new(text).span(pos..text.len()), &mut locations);
                Ok(locations.is_match().then(|| Captures {
                    inner: CapturesImpl::Wrap { text, locations },
                    named_groups,
                }))
            }
            RegexImpl::Fancy {
                prog,
                n_groups,
                options,
                ..
            } => {
                let result = vm::run(prog, text, pos, 0, options)?;
                Ok(result.map(|mut saves| {
                    saves.truncate(n_groups * 2);
                    Captures {
                        inner: CapturesImpl::Fancy { text, saves },
                        named_groups,
                    }
                }))
            }
        }
    }

    /// Returns the number of captures, including the implicit capture of the entire expression.
    pub fn captures_len(&self) -> usize {
        match &self.inner {
            RegexImpl::Wrap { inner, .. } => inner.captures_len(),
            RegexImpl::Fancy { n_groups, .. } => *n_groups,
        }
    }

    /// Returns an iterator over the capture names.
    pub fn capture_names(&self) -> CaptureNames {
        let mut names = Vec::new();
        names.resize(self.captures_len(), None);
        for (name, &i) in self.named_groups.iter() {
            names[i] = Some(name.as_str());
        }
        CaptureNames(names.into_iter())
    }

    // for debugging only
    #[doc(hidden)]
    pub fn debug_print(&self) {
        match &self.inner {
            #[cfg(feature = "std")]
            RegexImpl::Wrap { inner, .. } => println!("wrapped {:?}", inner),
            #[cfg(not(feature = "std"))]
            RegexImpl::Wrap { .. } => {}
            RegexImpl::Fancy { prog, .. } => prog.debug_print(),
        }
    }

    /// Replaces the leftmost-first match with the replacement provided.
    /// The replacement can be a regular string (where `$N` and `$name` are
    /// expanded to match capture groups) or a function that takes the matches'
    /// `Captures` and returns the replaced string.
    ///
    /// If no match is found, then a copy of the string is returned unchanged.
    ///
    /// # Replacement string syntax
    ///
    /// All instances of `$name` in the replacement text is replaced with the
    /// corresponding capture group `name`.
    ///
    /// `name` may be an integer corresponding to the index of the
    /// capture group (counted by order of opening parenthesis where `0` is the
    /// entire match) or it can be a name (consisting of letters, digits or
    /// underscores) corresponding to a named capture group.
    ///
    /// If `name` isn't a valid capture group (whether the name doesn't exist
    /// or isn't a valid index), then it is replaced with the empty string.
    ///
    /// The longest possible name is used. e.g., `$1a` looks up the capture
    /// group named `1a` and not the capture group at index `1`. To exert more
    /// precise control over the name, use braces, e.g., `${1}a`.
    ///
    /// To write a literal `$` use `$$`.
    ///
    /// # Examples
    ///
    /// Note that this function is polymorphic with respect to the replacement.
    /// In typical usage, this can just be a normal string:
    ///
    /// ```rust
    /// # use fancy_regex::Regex;
    /// let re = Regex::new("[^01]+").unwrap();
    /// assert_eq!(re.replace("1078910", ""), "1010");
    /// ```
    ///
    /// But anything satisfying the `Replacer` trait will work. For example,
    /// a closure of type `|&Captures| -> String` provides direct access to the
    /// captures corresponding to a match. This allows one to access
    /// capturing group matches easily:
    ///
    /// ```rust
    /// # use fancy_regex::{Regex, Captures};
    /// let re = Regex::new(r"([^,\s]+),\s+(\S+)").unwrap();
    /// let result = re.replace("Springsteen, Bruce", |caps: &Captures| {
    ///     format!("{} {}", &caps[2], &caps[1])
    /// });
    /// assert_eq!(result, "Bruce Springsteen");
    /// ```
    ///
    /// But this is a bit cumbersome to use all the time. Instead, a simple
    /// syntax is supported that expands `$name` into the corresponding capture
    /// group. Here's the last example, but using this expansion technique
    /// with named capture groups:
    ///
    /// ```rust
    /// # use fancy_regex::Regex;
    /// let re = Regex::new(r"(?P<last>[^,\s]+),\s+(?P<first>\S+)").unwrap();
    /// let result = re.replace("Springsteen, Bruce", "$first $last");
    /// assert_eq!(result, "Bruce Springsteen");
    /// ```
    ///
    /// Note that using `$2` instead of `$first` or `$1` instead of `$last`
    /// would produce the same result. To write a literal `$` use `$$`.
    ///
    /// Sometimes the replacement string requires use of curly braces to
    /// delineate a capture group replacement and surrounding literal text.
    /// For example, if we wanted to join two words together with an
    /// underscore:
    ///
    /// ```rust
    /// # use fancy_regex::Regex;
    /// let re = Regex::new(r"(?P<first>\w+)\s+(?P<second>\w+)").unwrap();
    /// let result = re.replace("deep fried", "${first}_$second");
    /// assert_eq!(result, "deep_fried");
    /// ```
    ///
    /// Without the curly braces, the capture group name `first_` would be
    /// used, and since it doesn't exist, it would be replaced with the empty
    /// string.
    ///
    /// Finally, sometimes you just want to replace a literal string with no
    /// regard for capturing group expansion. This can be done by wrapping a
    /// byte string with `NoExpand`:
    ///
    /// ```rust
    /// # use fancy_regex::Regex;
    /// use fancy_regex::NoExpand;
    ///
    /// let re = Regex::new(r"(?P<last>[^,\s]+),\s+(\S+)").unwrap();
    /// let result = re.replace("Springsteen, Bruce", NoExpand("$2 $last"));
    /// assert_eq!(result, "$2 $last");
    /// ```
    pub fn replace<'t, R: Replacer>(&self, text: &'t str, rep: R) -> Cow<'t, str> {
        self.replacen(text, 1, rep)
    }

    /// Replaces all non-overlapping matches in `text` with the replacement
    /// provided. This is the same as calling `replacen` with `limit` set to
    /// `0`.
    ///
    /// See the documentation for `replace` for details on how to access
    /// capturing group matches in the replacement string.
    pub fn replace_all<'t, R: Replacer>(&self, text: &'t str, rep: R) -> Cow<'t, str> {
        self.replacen(text, 0, rep)
    }

    /// Replaces at most `limit` non-overlapping matches in `text` with the
    /// replacement provided. If `limit` is 0, then all non-overlapping matches
    /// are replaced.
    ///
    /// Will panic if any errors are encountered. Use `try_replacen`, which this
    /// function unwraps, if you want to handle errors.
    ///
    /// See the documentation for `replace` for details on how to access
    /// capturing group matches in the replacement string.
    ///
    pub fn replacen<'t, R: Replacer>(&self, text: &'t str, limit: usize, rep: R) -> Cow<'t, str> {
        self.try_replacen(text, limit, rep).unwrap()
    }

    /// Replaces at most `limit` non-overlapping matches in `text` with the
    /// replacement provided. If `limit` is 0, then all non-overlapping matches
    /// are replaced.
    ///
    /// Propagates any errors encountered, such as `RuntimeError::BacktrackLimitExceeded`.
    ///
    /// See the documentation for `replace` for details on how to access
    /// capturing group matches in the replacement string.
    pub fn try_replacen<'t, R: Replacer>(
        &self,
        text: &'t str,
        limit: usize,
        mut rep: R,
    ) -> Result<Cow<'t, str>> {
        // If we know that the replacement doesn't have any capture expansions,
        // then we can fast path. The fast path can make a tremendous
        // difference:
        //
        //   1) We use `find_iter` instead of `captures_iter`. Not asking for
        //      captures generally makes the regex engines faster.
        //   2) We don't need to look up all of the capture groups and do
        //      replacements inside the replacement string. We just push it
        //      at each match and be done with it.
        if let Some(rep) = rep.no_expansion() {
            let mut it = self.find_iter(text).enumerate().peekable();
            if it.peek().is_none() {
                return Ok(Cow::Borrowed(text));
            }
            let mut new = String::with_capacity(text.len());
            let mut last_match = 0;
            for (i, m) in it {
                let m = m?;

                if limit > 0 && i >= limit {
                    break;
                }
                new.push_str(&text[last_match..m.start()]);
                new.push_str(&rep);
                last_match = m.end();
            }
            new.push_str(&text[last_match..]);
            return Ok(Cow::Owned(new));
        }

        // The slower path, which we use if the replacement needs access to
        // capture groups.
        let mut it = self.captures_iter(text).enumerate().peekable();
        if it.peek().is_none() {
            return Ok(Cow::Borrowed(text));
        }
        let mut new = String::with_capacity(text.len());
        let mut last_match = 0;
        for (i, cap) in it {
            let cap = cap?;

            if limit > 0 && i >= limit {
                break;
            }
            // unwrap on 0 is OK because captures only reports matches
            let m = cap.get(0).unwrap();
            new.push_str(&text[last_match..m.start()]);
            rep.replace_append(&cap, &mut new);
            last_match = m.end();
        }
        new.push_str(&text[last_match..]);
        Ok(Cow::Owned(new))
    }
}

impl TryFrom<&str> for Regex {
    type Error = Error;

    /// Attempts to parse a string into a regular expression
    fn try_from(s: &str) -> Result<Self> {
        Self::new(s)
    }
}

impl TryFrom<String> for Regex {
    type Error = Error;

    /// Attempts to parse a string into a regular expression
    fn try_from(s: String) -> Result<Self> {
        Self::new(&s)
    }
}

impl<'t> Match<'t> {
    /// Returns the starting byte offset of the match in the text.
    #[inline]
    pub fn start(&self) -> usize {
        self.start
    }

    /// Returns the ending byte offset of the match in the text.
    #[inline]
    pub fn end(&self) -> usize {
        self.end
    }

    /// Returns the range over the starting and ending byte offsets of the match in text.
    #[inline]
    pub fn range(&self) -> Range<usize> {
        self.start..self.end
    }

    /// Returns the matched text.
    #[inline]
    pub fn as_str(&self) -> &'t str {
        &self.text[self.start..self.end]
    }

    /// Creates a new match from the given text and byte offsets.
    fn new(text: &'t str, start: usize, end: usize) -> Match<'t> {
        Match { text, start, end }
    }
}

impl<'t> From<Match<'t>> for &'t str {
    fn from(m: Match<'t>) -> &'t str {
        m.as_str()
    }
}

impl<'t> From<Match<'t>> for Range<usize> {
    fn from(m: Match<'t>) -> Range<usize> {
        m.range()
    }
}

#[allow(clippy::len_without_is_empty)] // follow regex's API
impl<'t> Captures<'t> {
    /// Get the capture group by its index in the regex.
    ///
    /// If there is no match for that group or the index does not correspond to a group, `None` is
    /// returned. The index 0 returns the whole match.
    pub fn get(&self, i: usize) -> Option<Match<'t>> {
        match &self.inner {
            CapturesImpl::Wrap { text, locations } => locations.get_group(i).map(|span| Match {
                text,
                start: span.start,
                end: span.end,
            }),
            CapturesImpl::Fancy { text, ref saves } => {
                let slot = i * 2;
                if slot >= saves.len() {
                    return None;
                }
                let lo = saves[slot];
                if lo == usize::MAX {
                    return None;
                }
                let hi = saves[slot + 1];
                Some(Match {
                    text,
                    start: lo,
                    end: hi,
                })
            }
        }
    }

    /// Returns the match for a named capture group.  Returns `None` the capture
    /// group did not match or if there is no group with the given name.
    pub fn name(&self, name: &str) -> Option<Match<'t>> {
        self.named_groups.get(name).and_then(|i| self.get(*i))
    }

    /// Expands all instances of `$group` in `replacement` to the corresponding
    /// capture group `name`, and writes them to the `dst` buffer given.
    ///
    /// `group` may be an integer corresponding to the index of the
    /// capture group (counted by order of opening parenthesis where `\0` is the
    /// entire match) or it can be a name (consisting of letters, digits or
    /// underscores) corresponding to a named capture group.
    ///
    /// If `group` isn't a valid capture group (whether the name doesn't exist
    /// or isn't a valid index), then it is replaced with the empty string.
    ///
    /// The longest possible name is used. e.g., `$1a` looks up the capture
    /// group named `1a` and not the capture group at index `1`. To exert more
    /// precise control over the name, use braces, e.g., `${1}a`.
    ///
    /// To write a literal `$`, use `$$`.
    ///
    /// For more control over expansion, see [`Expander`].
    ///
    /// [`Expander`]: expand/struct.Expander.html
    pub fn expand(&self, replacement: &str, dst: &mut String) {
        Expander::default().append_expansion(dst, replacement, self);
    }

    /// Iterate over the captured groups in order in which they appeared in the regex. The first
    /// capture corresponds to the whole match.
    pub fn iter<'c>(&'c self) -> SubCaptureMatches<'c, 't> {
        SubCaptureMatches { caps: self, i: 0 }
    }

    /// How many groups were captured. This is always at least 1 because group 0 returns the whole
    /// match.
    pub fn len(&self) -> usize {
        match &self.inner {
            CapturesImpl::Wrap { locations, .. } => locations.group_len(),
            CapturesImpl::Fancy { saves, .. } => saves.len() / 2,
        }
    }
}

/// Get a group by index.
///
/// `'t` is the lifetime of the matched text.
///
/// The text can't outlive the `Captures` object if this method is
/// used, because of how `Index` is defined (normally `a[i]` is part
/// of `a` and can't outlive it); to do that, use `get()` instead.
///
/// # Panics
///
/// If there is no group at the given index.
impl<'t> Index<usize> for Captures<'t> {
    type Output = str;

    fn index(&self, i: usize) -> &str {
        self.get(i)
            .map(|m| m.as_str())
            .unwrap_or_else(|| panic!("no group at index '{}'", i))
    }
}

/// Get a group by name.
///
/// `'t` is the lifetime of the matched text and `'i` is the lifetime
/// of the group name (the index).
///
/// The text can't outlive the `Captures` object if this method is
/// used, because of how `Index` is defined (normally `a[i]` is part
/// of `a` and can't outlive it); to do that, use `name` instead.
///
/// # Panics
///
/// If there is no group named by the given value.
impl<'t, 'i> Index<&'i str> for Captures<'t> {
    type Output = str;

    fn index<'a>(&'a self, name: &'i str) -> &'a str {
        self.name(name)
            .map(|m| m.as_str())
            .unwrap_or_else(|| panic!("no group named '{}'", name))
    }
}

impl<'c, 't> Iterator for SubCaptureMatches<'c, 't> {
    type Item = Option<Match<'t>>;

    fn next(&mut self) -> Option<Option<Match<'t>>> {
        if self.i < self.caps.len() {
            let result = self.caps.get(self.i);
            self.i += 1;
            Some(result)
        } else {
            None
        }
    }
}

// TODO: might be nice to implement ExactSizeIterator etc for SubCaptures

/// Regular expression AST. This is public for now but may change.
#[derive(Debug, PartialEq, Eq)]
pub enum Expr {
    /// An empty expression, e.g. the last branch in `(a|b|)`
    Empty,
    /// Any character, regex `.`
    Any {
        /// Whether it also matches newlines or not
        newline: bool,
    },
    /// An assertion
    Assertion(Assertion),
    /// The string as a literal, e.g. `a`
    Literal {
        /// The string to match
        val: String,
        /// Whether match is case-insensitive or not
        casei: bool,
    },
    /// Concatenation of multiple expressions, must match in order, e.g. `a.` is a concatenation of
    /// the literal `a` and `.` for any character
    Concat(Vec<Expr>),
    /// Alternative of multiple expressions, one of them must match, e.g. `a|b` is an alternative
    /// where either the literal `a` or `b` must match
    Alt(Vec<Expr>),
    /// Capturing group of expression, e.g. `(a.)` matches `a` and any character and "captures"
    /// (remembers) the match
    Group(Box<Expr>),
    /// Look-around (e.g. positive/negative look-ahead or look-behind) with an expression, e.g.
    /// `(?=a)` means the next character must be `a` (but the match is not consumed)
    LookAround(Box<Expr>, LookAround),
    /// Repeat of an expression, e.g. `a*` or `a+` or `a{1,3}`
    Repeat {
        /// The expression that is being repeated
        child: Box<Expr>,
        /// The minimum number of repetitions
        lo: usize,
        /// The maximum number of repetitions (or `usize::MAX`)
        hi: usize,
        /// Greedy means as much as possible is matched, e.g. `.*b` would match all of `abab`.
        /// Non-greedy means as little as possible, e.g. `.*?b` would match only `ab` in `abab`.
        greedy: bool,
    },
    /// Delegate a regex to the regex crate. This is used as a simplification so that we don't have
    /// to represent all the expressions in the AST, e.g. character classes.
    Delegate {
        /// The regex
        inner: String,
        /// How many characters the regex matches
        size: usize, // TODO: move into analysis result
        /// Whether the matching is case-insensitive or not
        casei: bool,
    },
    /// Back reference to a capture group, e.g. `\1` in `(abc|def)\1` references the captured group
    /// and the whole regex matches either `abcabc` or `defdef`.
    Backref(usize),
    /// Atomic non-capturing group, e.g. `(?>ab|a)` in text that contains `ab` will match `ab` and
    /// never backtrack and try `a`, even if matching fails after the atomic group.
    AtomicGroup(Box<Expr>),
    /// Keep matched text so far out of overall match
    KeepOut,
    /// Anchor to match at the position where the previous match ended
    ContinueFromPreviousMatchEnd,
    /// Conditional expression based on whether the numbered capture group matched or not
    BackrefExistsCondition(usize),
    /// If/Then/Else Condition. If there is no Then/Else, these will just be empty expressions.
    Conditional {
        /// The conditional expression to evaluate
        condition: Box<Expr>,
        /// What to execute if the condition is true
        true_branch: Box<Expr>,
        /// What to execute if the condition is false
        false_branch: Box<Expr>,
    },
}

/// Type of look-around assertion as used for a look-around expression.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum LookAround {
    /// Look-ahead assertion, e.g. `(?=a)`
    LookAhead,
    /// Negative look-ahead assertion, e.g. `(?!a)`
    LookAheadNeg,
    /// Look-behind assertion, e.g. `(?<=a)`
    LookBehind,
    /// Negative look-behind assertion, e.g. `(?<!a)`
    LookBehindNeg,
}

/// An iterator over capture names in a [Regex].  The iterator
/// returns the name of each group, or [None] if the group has
/// no name.  Because capture group 0 cannot have a name, the
/// first item returned is always [None].
pub struct CaptureNames<'r>(vec::IntoIter<Option<&'r str>>);

impl Debug for CaptureNames<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("<CaptureNames>")
    }
}

impl<'r> Iterator for CaptureNames<'r> {
    type Item = Option<&'r str>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

// silly to write my own, but this is super-fast for the common 1-digit
// case.
fn push_usize(s: &mut String, x: usize) {
    if x >= 10 {
        push_usize(s, x / 10);
        s.push((b'0' + (x % 10) as u8) as char);
    } else {
        s.push((b'0' + (x as u8)) as char);
    }
}

fn is_special(c: char) -> bool {
    match c {
        '\\' | '.' | '+' | '*' | '?' | '(' | ')' | '|' | '[' | ']' | '{' | '}' | '^' | '$'
        | '#' => true,
        _ => false,
    }
}

fn push_quoted(buf: &mut String, s: &str) {
    for c in s.chars() {
        if is_special(c) {
            buf.push('\\');
        }
        buf.push(c);
    }
}

/// Escapes special characters in `text` with '\\'.  Returns a string which, when interpreted
/// as a regex, matches exactly `text`.
pub fn escape(text: &str) -> Cow<str> {
    // Using bytes() is OK because all special characters are single bytes.
    match text.bytes().filter(|&b| is_special(b as char)).count() {
        0 => Cow::Borrowed(text),
        n => {
            // The capacity calculation is exact because '\\' is a single byte.
            let mut buf = String::with_capacity(text.len() + n);
            push_quoted(&mut buf, text);
            Cow::Owned(buf)
        }
    }
}

/// Type of assertions
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Assertion {
    /// Start of input text
    StartText,
    /// End of input text
    EndText,
    /// Start of a line
    StartLine {
        /// CRLF mode
        crlf: bool,
    },
    /// End of a line
    EndLine {
        /// CRLF mode
        crlf: bool,
    },
    /// Left word boundary
    LeftWordBoundary,
    /// Right word boundary
    RightWordBoundary,
    /// Both word boundaries
    WordBoundary,
    /// Not word boundary
    NotWordBoundary,
}

impl Assertion {
    pub(crate) fn is_hard(&self) -> bool {
        use Assertion::*;
        matches!(
            self,
            // these will make regex-automata use PikeVM
            LeftWordBoundary | RightWordBoundary | WordBoundary | NotWordBoundary
        )
    }
}

impl Expr {
    /// Parse the regex and return an expression (AST) and a bit set with the indexes of groups
    /// that are referenced by backrefs.
    pub fn parse_tree(re: &str) -> Result<ExprTree> {
        Parser::parse(re)
    }

    /// Convert expression to a regex string in the regex crate's syntax.
    ///
    /// # Panics
    ///
    /// Panics for expressions that are hard, i.e. can not be handled by the regex crate.
    pub fn to_str(&self, buf: &mut String, precedence: u8) {
        match *self {
            Expr::Empty => (),
            Expr::Any { newline } => buf.push_str(if newline { "(?s:.)" } else { "." }),
            Expr::Literal { ref val, casei } => {
                if casei {
                    buf.push_str("(?i:");
                }
                push_quoted(buf, val);
                if casei {
                    buf.push_str(")");
                }
            }
            Expr::Assertion(Assertion::StartText) => buf.push('^'),
            Expr::Assertion(Assertion::EndText) => buf.push('$'),
            Expr::Assertion(Assertion::StartLine { crlf: false }) => buf.push_str("(?m:^)"),
            Expr::Assertion(Assertion::EndLine { crlf: false }) => buf.push_str("(?m:$)"),
            Expr::Assertion(Assertion::StartLine { crlf: true }) => buf.push_str("(?Rm:^)"),
            Expr::Assertion(Assertion::EndLine { crlf: true }) => buf.push_str("(?Rm:$)"),
            Expr::Concat(ref children) => {
                if precedence > 1 {
                    buf.push_str("(?:");
                }
                for child in children {
                    child.to_str(buf, 2);
                }
                if precedence > 1 {
                    buf.push(')')
                }
            }
            Expr::Alt(ref children) => {
                if precedence > 0 {
                    buf.push_str("(?:");
                }
                for (i, child) in children.iter().enumerate() {
                    if i != 0 {
                        buf.push('|');
                    }
                    child.to_str(buf, 1);
                }
                if precedence > 0 {
                    buf.push(')');
                }
            }
            Expr::Group(ref child) => {
                buf.push('(');
                child.to_str(buf, 0);
                buf.push(')');
            }
            Expr::Repeat {
                ref child,
                lo,
                hi,
                greedy,
            } => {
                if precedence > 2 {
                    buf.push_str("(?:");
                }
                child.to_str(buf, 3);
                match (lo, hi) {
                    (0, 1) => buf.push('?'),
                    (0, usize::MAX) => buf.push('*'),
                    (1, usize::MAX) => buf.push('+'),
                    (lo, hi) => {
                        buf.push('{');
                        push_usize(buf, lo);
                        if lo != hi {
                            buf.push(',');
                            if hi != usize::MAX {
                                push_usize(buf, hi);
                            }
                        }
                        buf.push('}');
                    }
                }
                if !greedy {
                    buf.push('?');
                }
                if precedence > 2 {
                    buf.push(')');
                }
            }
            Expr::Delegate {
                ref inner, casei, ..
            } => {
                // at the moment, delegate nodes are just atoms
                if casei {
                    buf.push_str("(?i:");
                }
                buf.push_str(inner);
                if casei {
                    buf.push_str(")");
                }
            }
            _ => panic!("attempting to format hard expr"),
        }
    }
}

// precondition: ix > 0
fn prev_codepoint_ix(s: &str, mut ix: usize) -> usize {
    let bytes = s.as_bytes();
    loop {
        ix -= 1;
        // fancy bit magic for ranges 0..0x80 + 0xc0..
        if (bytes[ix] as i8) >= -0x40 {
            break;
        }
    }
    ix
}

fn codepoint_len(b: u8) -> usize {
    match b {
        b if b < 0x80 => 1,
        b if b < 0xe0 => 2,
        b if b < 0xf0 => 3,
        _ => 4,
    }
}

/// Returns the smallest possible index of the next valid UTF-8 sequence
/// starting after `i`.
/// Adapted from a function with the same name in the `regex` crate.
fn next_utf8(text: &str, i: usize) -> usize {
    let b = match text.as_bytes().get(i) {
        None => return i + 1,
        Some(&b) => b,
    };
    i + codepoint_len(b)
}

// If this returns false, then there is no possible backref in the re

// Both potential implementations are turned off, because we currently
// always need to do a deeper analysis because of 1-character
// look-behind. If we could call a find_from_pos method of regex::Regex,
// it would make sense to bring this back.
/*
pub fn detect_possible_backref(re: &str) -> bool {
    let mut last = b'\x00';
    for b in re.as_bytes() {
        if b'0' <= *b && *b <= b'9' && last == b'\\' { return true; }
        last = *b;
    }
    false
}

pub fn detect_possible_backref(re: &str) -> bool {
    let mut bytes = re.as_bytes();
    loop {
        match memchr::memchr(b'\\', &bytes[..bytes.len() - 1]) {
            Some(i) => {
                bytes = &bytes[i + 1..];
                let c = bytes[0];
                if b'0' <= c && c <= b'9' { return true; }
            }
            None => return false
        }
    }
}
*/

/// The internal module only exists so that the toy example can access internals for debugging and
/// experimenting.
#[doc(hidden)]
pub mod internal {
    pub use crate::analyze::analyze;
    pub use crate::compile::compile;
    pub use crate::vm::{run_default, run_trace, Insn, Prog};
}

#[cfg(test)]
mod tests {
    use alloc::borrow::Cow;
    use alloc::boxed::Box;
    use alloc::string::String;
    use alloc::{format, vec};

    use crate::parse::make_literal;
    use crate::{Expr, Regex};

    //use detect_possible_backref;

    // tests for to_str

    fn to_str(e: Expr) -> String {
        let mut s = String::new();
        e.to_str(&mut s, 0);
        s
    }

    #[test]
    fn to_str_concat_alt() {
        let e = Expr::Concat(vec![
            Expr::Alt(vec![make_literal("a"), make_literal("b")]),
            make_literal("c"),
        ]);
        assert_eq!(to_str(e), "(?:a|b)c");
    }

    #[test]
    fn to_str_rep_concat() {
        let e = Expr::Repeat {
            child: Box::new(Expr::Concat(vec![make_literal("a"), make_literal("b")])),
            lo: 2,
            hi: 3,
            greedy: true,
        };
        assert_eq!(to_str(e), "(?:ab){2,3}");
    }

    #[test]
    fn to_str_group_alt() {
        let e = Expr::Group(Box::new(Expr::Alt(vec![
            make_literal("a"),
            make_literal("b"),
        ])));
        assert_eq!(to_str(e), "(a|b)");
    }

    #[test]
    fn as_str_debug() {
        let s = r"(a+)b\1";
        let regex = Regex::new(s).unwrap();
        assert_eq!(s, regex.as_str());
        assert_eq!(s, format!("{:?}", regex));
    }

    #[test]
    fn display() {
        let s = r"(a+)b\1";
        let regex = Regex::new(s).unwrap();
        assert_eq!(s, format!("{}", regex));
    }

    #[test]
    fn from_str() {
        let s = r"(a+)b\1";
        let regex = s.parse::<Regex>().unwrap();
        assert_eq!(regex.as_str(), s);
    }

    #[test]
    fn to_str_repeat() {
        fn repeat(lo: usize, hi: usize, greedy: bool) -> Expr {
            Expr::Repeat {
                child: Box::new(make_literal("a")),
                lo,
                hi,
                greedy,
            }
        }

        assert_eq!(to_str(repeat(2, 2, true)), "a{2}");
        assert_eq!(to_str(repeat(2, 2, false)), "a{2}?");
        assert_eq!(to_str(repeat(2, 3, true)), "a{2,3}");
        assert_eq!(to_str(repeat(2, 3, false)), "a{2,3}?");
        assert_eq!(to_str(repeat(2, usize::MAX, true)), "a{2,}");
        assert_eq!(to_str(repeat(2, usize::MAX, false)), "a{2,}?");
        assert_eq!(to_str(repeat(0, 1, true)), "a?");
        assert_eq!(to_str(repeat(0, 1, false)), "a??");
        assert_eq!(to_str(repeat(0, usize::MAX, true)), "a*");
        assert_eq!(to_str(repeat(0, usize::MAX, false)), "a*?");
        assert_eq!(to_str(repeat(1, usize::MAX, true)), "a+");
        assert_eq!(to_str(repeat(1, usize::MAX, false)), "a+?");
    }

    #[test]
    fn escape() {
        // Check that strings that need no quoting are borrowed, and that non-special punctuation
        // is not quoted.
        match crate::escape("@foo") {
            Cow::Borrowed(s) => assert_eq!(s, "@foo"),
            _ => panic!("Value should be borrowed."),
        }

        // Check typical usage.
        assert_eq!(crate::escape("fo*o").into_owned(), "fo\\*o");

        // Check that multibyte characters are handled correctly.
        assert_eq!(crate::escape("fø*ø").into_owned(), "fø\\*ø");
    }

    /*
    #[test]
    fn detect_backref() {
        assert_eq!(detect_possible_backref("a0a1a2"), false);
        assert_eq!(detect_possible_backref("a0a1\\a2"), false);
        assert_eq!(detect_possible_backref("a0a\\1a2"), true);
        assert_eq!(detect_possible_backref("a0a1a2\\"), false);
    }
    */
}
