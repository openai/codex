/*!
`nucleo_matcher` is a low level crate that contains the matcher implementation
used by the high level `nucleo` crate.

**NOTE**: If you are building an fzf-like interactive fuzzy finder that is
meant to match a reasonably large number of items (> 100) using the high level
`nucleo` crate is highly recommended. Using `nucleo-matcher` directly in you ui
loop will be very slow. Implementing this logic yourself is very complex.

The matcher is hightly optimized and can significantly outperform `fzf` and
`skim` (the `fuzzy-matcher` crate). However some of these optimizations require
a slightly less convenient API. Be sure to carefully read the documentation of
the [`Matcher`] to avoid unexpected behaviour.
# Examples

For almost all usecases the [`pattern`] API should be used instead of calling
the matcher methods directly. [`Pattern::parse`](pattern::Pattern::parse) will
construct a single Atom (a single match operation) for each word. The pattern
can contain special characters to control what kind of match is performed (see
[`AtomKind`](crate::pattern::AtomKind)).

```
# use nucleo_matcher::{Matcher, Config};
# use nucleo_matcher::pattern::{Pattern, Normalization, CaseMatching};
let paths = ["foo/bar", "bar/foo", "foobar"];
let mut matcher = Matcher::new(Config::DEFAULT.match_paths());
let matches = Pattern::parse("foo bar", CaseMatching::Ignore, Normalization::Smart).match_list(paths, &mut matcher);
assert_eq!(matches, vec![("foo/bar", 168), ("bar/foo", 168), ("foobar", 140)]);
let matches = Pattern::parse("^foo bar", CaseMatching::Ignore, Normalization::Smart).match_list(paths, &mut matcher);
assert_eq!(matches, vec![("foo/bar", 168), ("foobar", 140)]);
```

If the pattern should be matched literally (without this special parsing)
[`Pattern::new`](pattern::Pattern::new) can be used instead.

```
# use nucleo_matcher::{Matcher, Config};
# use nucleo_matcher::pattern::{Pattern, CaseMatching, AtomKind, Normalization};
let paths = ["foo/bar", "bar/foo", "foobar"];
let mut matcher = Matcher::new(Config::DEFAULT.match_paths());
let matches = Pattern::new("foo bar", CaseMatching::Ignore, Normalization::Smart, AtomKind::Fuzzy).match_list(paths, &mut matcher);
assert_eq!(matches, vec![("foo/bar", 168), ("bar/foo", 168), ("foobar", 140)]);
let paths = ["^foo/bar", "bar/^foo", "foobar"];
let matches = Pattern::new("^foo bar", CaseMatching::Ignore, Normalization::Smart, AtomKind::Fuzzy).match_list(paths, &mut matcher);
assert_eq!(matches, vec![("^foo/bar", 188), ("bar/^foo", 188)]);
```

If word segmentation is also not desired, a single `Atom` can be constructed directly.

```
# use nucleo_matcher::{Matcher, Config};
# use nucleo_matcher::pattern::{Pattern, Atom, CaseMatching, Normalization, AtomKind};
let paths = ["foobar", "foo bar"];
let mut matcher = Matcher::new(Config::DEFAULT);
let matches = Atom::new("foo bar", CaseMatching::Ignore, Normalization::Smart, AtomKind::Fuzzy, false).match_list(paths, &mut matcher);
assert_eq!(matches, vec![("foo bar", 192)]);
```


# Status

Nucleo is used in the helix-editor and therefore has a large user base with lots or real world testing. The core matcher implementation is considered complete and is unlikely to see major changes. The `nucleo-matcher` crate is finished and ready for widespread use, breaking changes should be very rare (a 1.0 release should not be far away).

*/

// sadly ranges don't optmimzie well
#![allow(clippy::manual_range_contains)]
#![warn(missing_docs)]

pub mod chars;
mod config;
#[cfg(test)]
mod debug;
mod exact;
mod fuzzy_greedy;
mod fuzzy_optimal;
mod matrix;
pub mod pattern;
mod prefilter;
mod score;
mod utf32_str;

#[cfg(test)]
mod tests;

pub use crate::config::Config;
pub use crate::utf32_str::{Utf32Str, Utf32String};

use crate::chars::{AsciiChar, Char};
use crate::matrix::MatrixSlab;

/// A matcher engine that can execute (fuzzy) matches.
///
/// A matches contains **heap allocated** scratch memory that is reused during
/// matching. This scratch memory allows the matcher to guarantee that it will
/// **never allocate** during matching (with the exception of pushing to the
/// `indices` vector if there isn't enough capacity). However this scratch
/// memory is fairly large (around 135KB) so creating a matcher is expensive.
///
/// All `.._match` functions will not compute the indices  of the matched
/// characters. These should be used to prefilter to filter and rank all
/// matches. All `.._indices` functions will also compute the indices of the
/// matched characters but are slower compared to the `..match` variant. These
/// should be used when rendering the best N matches. Note that the `indices`
/// argument is **never cleared**. This allows running multiple different
/// matches on the same haystack and merging the indices by sorting and
/// deduplicating the vector.
///
/// The `needle` argument for each function must always be normalized by the
/// caller (unicode normalization and case folding). Otherwise, the matcher
/// may fail to produce a match. The [`pattern`] modules provides utilities
/// to preprocess needles and **should usually be preferred over invoking the
/// matcher directly**.  Additionally it's recommend to perform separate matches
/// for each word in the needle. Consider the folloling example:
///
/// If `foo bar` is used as the needle it matches both `foo test baaar` and
/// `foo hello-world bar`. However, `foo test baaar` will receive a higher
/// score than `foo hello-world bar`. `baaar` contains a 2 character gap which
/// will receive a penalty and therefore the user will likely expect it to rank
/// lower. However, if `foo bar` is matched as a single query `hello-world` and
/// `test` are both considered gaps too. As `hello-world` is a much longer gap
/// then `test` the extra penalty for `baaar` is canceled out. If both words
/// are matched individually the interspersed words do not receive a penalty and
/// `foo hello-world bar` ranks higher.
///
/// In general nucleo is a **substring matching tool** (except for the prefix/
/// postfix matching modes) with no penalty assigned to matches that start
/// later within the same pattern (which enables matching words individually
/// as shown above). If patterns show a large variety in length and the syntax
/// described above is not used it may be preferable to give preference to
/// matches closer to the start of a haystack. To accommodate that usecase the
/// [`prefer_prefix`](Config::prefer_prefix) option can be set to true.
///
/// Matching is limited to 2^32-1 codepoints, if the haystack is longer than
/// that the matcher **will panic**. The caller must decide whether it wants to
/// filter out long haystacks or truncate them.
pub struct Matcher {
    #[allow(missing_docs)]
    pub config: Config,
    slab: MatrixSlab,
}

// this is just here for convenience not sure if we should implement this
impl Clone for Matcher {
    fn clone(&self) -> Self {
        Matcher {
            config: self.config.clone(),
            slab: MatrixSlab::new(),
        }
    }
}

impl std::fmt::Debug for Matcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Matcher")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl Default for Matcher {
    fn default() -> Self {
        Matcher {
            config: Config::DEFAULT,
            slab: MatrixSlab::new(),
        }
    }
}

impl Matcher {
    /// Creates a new matcher instance, note that this will eagerly allocate a
    /// fairly large chunk of heap memory (around 135KB currently but subject to
    /// change) so matchers should be reused if called often (like in a loop).
    pub fn new(config: Config) -> Self {
        Self {
            config,
            slab: MatrixSlab::new(),
        }
    }

    /// Find the fuzzy match with the highest score in the `haystack`.
    ///
    /// This functions has `O(mn)` time complexity for short inputs.
    /// To avoid slowdowns it automatically falls back to
    /// [greedy matching](crate::Matcher::fuzzy_match_greedy) for large
    /// needles and haystacks.
    ///
    /// See the [matcher documentation](crate::Matcher) for more details.
    pub fn fuzzy_match(&mut self, haystack: Utf32Str<'_>, needle: Utf32Str<'_>) -> Option<u16> {
        assert!(haystack.len() <= u32::MAX as usize);
        self.fuzzy_matcher_impl::<false>(haystack, needle, &mut Vec::new())
    }

    /// Find the fuzzy match with the higehest score in the `haystack` and
    /// compute its indices.
    ///
    /// This functions has `O(mn)` time complexity for short inputs. To
    /// avoid slowdowns it automatically falls back to [greedy matching]
    /// (crate::Matcher::fuzzy_match_greedy) for large needles and haystacks
    ///
    /// See the [matcher documentation](crate::Matcher) for more details.
    pub fn fuzzy_indices(
        &mut self,
        haystack: Utf32Str<'_>,
        needle: Utf32Str<'_>,
        indices: &mut Vec<u32>,
    ) -> Option<u16> {
        assert!(haystack.len() <= u32::MAX as usize);
        self.fuzzy_matcher_impl::<true>(haystack, needle, indices)
    }

    fn fuzzy_matcher_impl<const INDICES: bool>(
        &mut self,
        haystack_: Utf32Str<'_>,
        needle_: Utf32Str<'_>,
        indices: &mut Vec<u32>,
    ) -> Option<u16> {
        if needle_.len() > haystack_.len() {
            return None;
        }
        if needle_.is_empty() {
            return Some(0);
        }
        if needle_.len() == haystack_.len() {
            return self.exact_match_impl::<INDICES>(
                haystack_,
                needle_,
                0,
                haystack_.len(),
                indices,
            );
        }
        assert!(
            haystack_.len() <= u32::MAX as usize,
            "fuzzy matching is only support for up to 2^32-1 codepoints"
        );
        match (haystack_, needle_) {
            (Utf32Str::Ascii(haystack), Utf32Str::Ascii(needle)) => {
                if let &[needle] = needle {
                    return self.substring_match_1_ascii::<INDICES>(haystack, needle, indices);
                }
                let (start, greedy_end, end) = self.prefilter_ascii(haystack, needle, false)?;
                if needle_.len() == end - start {
                    return Some(self.calculate_score::<INDICES, _, _>(
                        AsciiChar::cast(haystack),
                        AsciiChar::cast(needle),
                        start,
                        greedy_end,
                        indices,
                    ));
                }
                self.fuzzy_match_optimal::<INDICES, AsciiChar, AsciiChar>(
                    AsciiChar::cast(haystack),
                    AsciiChar::cast(needle),
                    start,
                    greedy_end,
                    end,
                    indices,
                )
            }
            (Utf32Str::Ascii(_), Utf32Str::Unicode(_)) => {
                // a purely ascii haystack can never be transformed to match
                // a needle that contains non-ascii chars since we don't allow gaps
                None
            }
            (Utf32Str::Unicode(haystack), Utf32Str::Ascii(needle)) => {
                if let &[needle] = needle {
                    let (start, _) = self.prefilter_non_ascii(haystack, needle_, true)?;
                    let res = self.substring_match_1_non_ascii::<INDICES>(
                        haystack,
                        needle as char,
                        start,
                        indices,
                    );
                    return Some(res);
                }
                let (start, end) = self.prefilter_non_ascii(haystack, needle_, false)?;
                if needle_.len() == end - start {
                    return self
                        .exact_match_impl::<INDICES>(haystack_, needle_, start, end, indices);
                }
                self.fuzzy_match_optimal::<INDICES, char, AsciiChar>(
                    haystack,
                    AsciiChar::cast(needle),
                    start,
                    start + 1,
                    end,
                    indices,
                )
            }
            (Utf32Str::Unicode(haystack), Utf32Str::Unicode(needle)) => {
                if let &[needle] = needle {
                    let (start, _) = self.prefilter_non_ascii(haystack, needle_, true)?;
                    let res = self
                        .substring_match_1_non_ascii::<INDICES>(haystack, needle, start, indices);
                    return Some(res);
                }
                let (start, end) = self.prefilter_non_ascii(haystack, needle_, false)?;
                if needle_.len() == end - start {
                    return self
                        .exact_match_impl::<INDICES>(haystack_, needle_, start, end, indices);
                }
                self.fuzzy_match_optimal::<INDICES, char, char>(
                    haystack,
                    needle,
                    start,
                    start + 1,
                    end,
                    indices,
                )
            }
        }
    }

    /// Greedly find a fuzzy match in the `haystack`.
    ///
    /// This functions has `O(n)` time complexity but may provide unintutive (non-optimal)
    /// indices and scores. Usually [fuzzy_match](crate::Matcher::fuzzy_match) should
    /// be preferred.
    ///
    /// See the [matcher documentation](crate::Matcher) for more details.
    pub fn fuzzy_match_greedy(
        &mut self,
        haystack: Utf32Str<'_>,
        needle: Utf32Str<'_>,
    ) -> Option<u16> {
        assert!(haystack.len() <= u32::MAX as usize);
        self.fuzzy_match_greedy_impl::<false>(haystack, needle, &mut Vec::new())
    }

    /// Greedly find a fuzzy match in the `haystack` and compute its indices.
    ///
    /// This functions has `O(n)` time complexity but may provide unintuitive (non-optimal)
    /// indices and scores. Usually [fuzzy_indices](crate::Matcher::fuzzy_indices) should
    /// be preferred.
    ///
    /// See the [matcher documentation](crate::Matcher) for more details.
    pub fn fuzzy_indices_greedy(
        &mut self,
        haystack: Utf32Str<'_>,
        needle: Utf32Str<'_>,
        indices: &mut Vec<u32>,
    ) -> Option<u16> {
        assert!(haystack.len() <= u32::MAX as usize);
        self.fuzzy_match_greedy_impl::<true>(haystack, needle, indices)
    }

    fn fuzzy_match_greedy_impl<const INDICES: bool>(
        &mut self,
        haystack: Utf32Str<'_>,
        needle_: Utf32Str<'_>,
        indices: &mut Vec<u32>,
    ) -> Option<u16> {
        if needle_.len() > haystack.len() {
            return None;
        }
        if needle_.is_empty() {
            return Some(0);
        }
        if needle_.len() == haystack.len() {
            return self.exact_match_impl::<INDICES>(haystack, needle_, 0, haystack.len(), indices);
        }
        assert!(
            haystack.len() <= u32::MAX as usize,
            "matching is only support for up to 2^32-1 codepoints"
        );
        match (haystack, needle_) {
            (Utf32Str::Ascii(haystack), Utf32Str::Ascii(needle)) => {
                let (start, greedy_end, _) = self.prefilter_ascii(haystack, needle, true)?;
                if needle_.len() == greedy_end - start {
                    return Some(self.calculate_score::<INDICES, _, _>(
                        AsciiChar::cast(haystack),
                        AsciiChar::cast(needle),
                        start,
                        greedy_end,
                        indices,
                    ));
                }
                self.fuzzy_match_greedy_::<INDICES, AsciiChar, AsciiChar>(
                    AsciiChar::cast(haystack),
                    AsciiChar::cast(needle),
                    start,
                    greedy_end,
                    indices,
                )
            }
            (Utf32Str::Ascii(_), Utf32Str::Unicode(_)) => {
                // a purely ascii haystack can never be transformed to match
                // a needle that contains non-ascii chars since we don't allow gaps
                None
            }
            (Utf32Str::Unicode(haystack), Utf32Str::Ascii(needle)) => {
                let (start, _) = self.prefilter_non_ascii(haystack, needle_, true)?;
                self.fuzzy_match_greedy_::<INDICES, char, AsciiChar>(
                    haystack,
                    AsciiChar::cast(needle),
                    start,
                    start + 1,
                    indices,
                )
            }
            (Utf32Str::Unicode(haystack), Utf32Str::Unicode(needle)) => {
                let (start, _) = self.prefilter_non_ascii(haystack, needle_, true)?;
                self.fuzzy_match_greedy_::<INDICES, char, char>(
                    haystack,
                    needle,
                    start,
                    start + 1,
                    indices,
                )
            }
        }
    }

    /// Finds the substring match with the highest score in the `haystack`.
    ///
    /// This functions has `O(nm)` time complexity. However many cases can
    /// be significantly accelerated using prefilters so it's usually very fast
    /// in practice.
    ///
    /// See the [matcher documentation](crate::Matcher) for more details.
    pub fn substring_match(
        &mut self,
        haystack: Utf32Str<'_>,
        needle_: Utf32Str<'_>,
    ) -> Option<u16> {
        self.substring_match_impl::<false>(haystack, needle_, &mut Vec::new())
    }

    /// Finds the substring match with the highest score in the `haystack` and
    /// compute its indices.
    ///
    /// This functions has `O(nm)` time complexity. However many cases can
    /// be significantly accelerated using prefilters so it's usually fast
    /// in practice.
    ///
    /// See the [matcher documentation](crate::Matcher) for more details.
    pub fn substring_indices(
        &mut self,
        haystack: Utf32Str<'_>,
        needle_: Utf32Str<'_>,
        indices: &mut Vec<u32>,
    ) -> Option<u16> {
        self.substring_match_impl::<true>(haystack, needle_, indices)
    }

    fn substring_match_impl<const INDICES: bool>(
        &mut self,
        haystack: Utf32Str<'_>,
        needle_: Utf32Str<'_>,
        indices: &mut Vec<u32>,
    ) -> Option<u16> {
        if needle_.len() > haystack.len() {
            return None;
        }
        if needle_.is_empty() {
            return Some(0);
        }
        if needle_.len() == haystack.len() {
            return self.exact_match_impl::<INDICES>(haystack, needle_, 0, haystack.len(), indices);
        }
        assert!(
            haystack.len() <= u32::MAX as usize,
            "matching is only support for up to 2^32-1 codepoints"
        );
        match (haystack, needle_) {
            (Utf32Str::Ascii(haystack), Utf32Str::Ascii(needle)) => {
                if let &[needle] = needle {
                    return self.substring_match_1_ascii::<INDICES>(haystack, needle, indices);
                }
                self.substring_match_ascii::<INDICES>(haystack, needle, indices)
            }
            (Utf32Str::Ascii(_), Utf32Str::Unicode(_)) => {
                // a purely ascii haystack can never be transformed to match
                // a needle that contains non-ascii chars since we don't allow gaps
                None
            }
            (Utf32Str::Unicode(haystack), Utf32Str::Ascii(needle)) => {
                if let &[needle] = needle {
                    let (start, _) = self.prefilter_non_ascii(haystack, needle_, true)?;
                    let res = self.substring_match_1_non_ascii::<INDICES>(
                        haystack,
                        needle as char,
                        start,
                        indices,
                    );
                    return Some(res);
                }
                let (start, _) = self.prefilter_non_ascii(haystack, needle_, false)?;
                self.substring_match_non_ascii::<INDICES, _>(
                    haystack,
                    AsciiChar::cast(needle),
                    start,
                    indices,
                )
            }
            (Utf32Str::Unicode(haystack), Utf32Str::Unicode(needle)) => {
                if let &[needle] = needle {
                    let (start, _) = self.prefilter_non_ascii(haystack, needle_, true)?;
                    let res = self
                        .substring_match_1_non_ascii::<INDICES>(haystack, needle, start, indices);
                    return Some(res);
                }
                let (start, _) = self.prefilter_non_ascii(haystack, needle_, false)?;
                self.substring_match_non_ascii::<INDICES, _>(haystack, needle, start, indices)
            }
        }
    }

    /// Checks whether needle and haystack match exactly.
    ///
    /// This functions has `O(n)` time complexity.
    ///
    /// See the [matcher documentation](crate::Matcher) for more details.
    pub fn exact_match(&mut self, haystack: Utf32Str<'_>, needle: Utf32Str<'_>) -> Option<u16> {
        if needle.is_empty() {
            return Some(0);
        }
        let mut leading_space = 0;
        let mut trailing_space = 0;
        if !needle.first().is_whitespace() {
            leading_space = haystack.leading_white_space()
        }
        if !needle.last().is_whitespace() {
            trailing_space = haystack.trailing_white_space()
        }
        // avoid wraparound in size check
        if trailing_space == haystack.len() {
            return None;
        }
        self.exact_match_impl::<false>(
            haystack,
            needle,
            leading_space,
            haystack.len() - trailing_space,
            &mut Vec::new(),
        )
    }

    /// Checks whether needle and haystack match exactly and compute the matches indices.
    ///
    /// This functions has `O(n)` time complexity.
    ///
    /// See the [matcher documentation](crate::Matcher) for more details.
    pub fn exact_indices(
        &mut self,
        haystack: Utf32Str<'_>,
        needle: Utf32Str<'_>,
        indices: &mut Vec<u32>,
    ) -> Option<u16> {
        if needle.is_empty() {
            return Some(0);
        }
        let mut leading_space = 0;
        let mut trailing_space = 0;
        if !needle.first().is_whitespace() {
            leading_space = haystack.leading_white_space()
        }
        if !needle.last().is_whitespace() {
            trailing_space = haystack.trailing_white_space()
        }
        // avoid wraparound in size check
        if trailing_space == haystack.len() {
            return None;
        }
        self.exact_match_impl::<true>(
            haystack,
            needle,
            leading_space,
            haystack.len() - trailing_space,
            indices,
        )
    }

    /// Checks whether needle is a prefix of the haystack.
    ///
    /// This functions has `O(n)` time complexity.
    ///
    /// See the [matcher documentation](crate::Matcher) for more details.
    pub fn prefix_match(&mut self, haystack: Utf32Str<'_>, needle: Utf32Str<'_>) -> Option<u16> {
        if needle.is_empty() {
            return Some(0);
        }
        let mut leading_space = 0;
        if !needle.first().is_whitespace() {
            leading_space = haystack.leading_white_space()
        }
        if haystack.len() - leading_space < needle.len() {
            None
        } else {
            self.exact_match_impl::<false>(
                haystack,
                needle,
                leading_space,
                needle.len() + leading_space,
                &mut Vec::new(),
            )
        }
    }

    /// Checks whether needle is a prefix of the haystack and compute the matches indices.
    ///
    /// This functions has `O(n)` time complexity.
    ///
    /// See the [matcher documentation](crate::Matcher) for more details.
    pub fn prefix_indices(
        &mut self,
        haystack: Utf32Str<'_>,
        needle: Utf32Str<'_>,
        indices: &mut Vec<u32>,
    ) -> Option<u16> {
        if needle.is_empty() {
            return Some(0);
        }
        let mut leading_space = 0;
        if !needle.first().is_whitespace() {
            leading_space = haystack.leading_white_space()
        }
        if haystack.len() - leading_space < needle.len() {
            None
        } else {
            self.exact_match_impl::<true>(
                haystack,
                needle,
                leading_space,
                needle.len() + leading_space,
                indices,
            )
        }
    }

    /// Checks whether needle is a postfix of the haystack.
    ///
    /// This functions has `O(n)` time complexity.
    ///
    /// See the [matcher documentation](crate::Matcher) for more details.
    pub fn postfix_match(&mut self, haystack: Utf32Str<'_>, needle: Utf32Str<'_>) -> Option<u16> {
        if needle.is_empty() {
            return Some(0);
        }
        let mut trailing_spaces = 0;
        if !needle.last().is_whitespace() {
            trailing_spaces = haystack.trailing_white_space()
        }
        if haystack.len() - trailing_spaces < needle.len() {
            None
        } else {
            self.exact_match_impl::<false>(
                haystack,
                needle,
                haystack.len() - needle.len() - trailing_spaces,
                haystack.len() - trailing_spaces,
                &mut Vec::new(),
            )
        }
    }

    /// Checks whether needle is a postfix of the haystack and compute the matches indices.
    ///
    /// This functions has `O(n)` time complexity.
    ///
    /// See the [matcher documentation](crate::Matcher) for more details.
    pub fn postfix_indices(
        &mut self,
        haystack: Utf32Str<'_>,
        needle: Utf32Str<'_>,
        indices: &mut Vec<u32>,
    ) -> Option<u16> {
        if needle.is_empty() {
            return Some(0);
        }
        let mut trailing_spaces = 0;
        if !needle.last().is_whitespace() {
            trailing_spaces = haystack.trailing_white_space()
        }
        if haystack.len() - trailing_spaces < needle.len() {
            None
        } else {
            self.exact_match_impl::<true>(
                haystack,
                needle,
                haystack.len() - needle.len() - trailing_spaces,
                haystack.len() - trailing_spaces,
                indices,
            )
        }
    }

    fn exact_match_impl<const INDICES: bool>(
        &mut self,
        haystack: Utf32Str<'_>,
        needle_: Utf32Str<'_>,
        start: usize,
        end: usize,
        indices: &mut Vec<u32>,
    ) -> Option<u16> {
        if needle_.len() != end - start {
            return None;
        }
        assert!(
            haystack.len() <= u32::MAX as usize,
            "matching is only support for up to 2^32-1 codepoints"
        );
        let score = match (haystack, needle_) {
            (Utf32Str::Ascii(haystack), Utf32Str::Ascii(needle)) => {
                let matched = if self.config.ignore_case {
                    AsciiChar::cast(haystack)[start..end]
                        .iter()
                        .map(|c| c.normalize(&self.config))
                        .eq(AsciiChar::cast(needle)
                            .iter()
                            .map(|c| c.normalize(&self.config)))
                } else {
                    haystack == needle
                };
                if !matched {
                    return None;
                }
                self.calculate_score::<INDICES, _, _>(
                    AsciiChar::cast(haystack),
                    AsciiChar::cast(needle),
                    start,
                    end,
                    indices,
                )
            }
            (Utf32Str::Ascii(_), Utf32Str::Unicode(_)) => {
                // a purely ascii haystack can never be transformed to match
                // a needle that contains non-ascii chars since we don't allow gaps
                return None;
            }
            (Utf32Str::Unicode(haystack), Utf32Str::Ascii(needle)) => {
                let matched = haystack[start..end]
                    .iter()
                    .map(|c| c.normalize(&self.config))
                    .eq(AsciiChar::cast(needle)
                        .iter()
                        .map(|c| c.normalize(&self.config)));
                if !matched {
                    return None;
                }

                self.calculate_score::<INDICES, _, _>(
                    haystack,
                    AsciiChar::cast(needle),
                    start,
                    end,
                    indices,
                )
            }
            (Utf32Str::Unicode(haystack), Utf32Str::Unicode(needle)) => {
                let matched = haystack[start..end]
                    .iter()
                    .map(|c| c.normalize(&self.config))
                    .eq(needle.iter().map(|c| c.normalize(&self.config)));
                if !matched {
                    return None;
                }
                self.calculate_score::<INDICES, _, _>(haystack, needle, start, end, indices)
            }
        };
        Some(score)
    }
}
