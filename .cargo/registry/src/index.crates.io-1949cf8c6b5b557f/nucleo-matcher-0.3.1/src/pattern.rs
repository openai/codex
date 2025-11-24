//! This module provides a slightly higher level API for matching strings.

use std::cmp::Reverse;

use crate::{chars, Matcher, Utf32Str};

#[cfg(test)]
mod tests;

use crate::Utf32String;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[non_exhaustive]
/// How to treat a case mismatch between two characters.
pub enum CaseMatching {
    /// Characters never match their case folded version (`a != A`).
    Respect,
    /// Characters always match their case folded version (`a == A`).
    #[cfg(feature = "unicode-casefold")]
    Ignore,
    /// Acts like [`Ignore`](CaseMatching::Ignore) if all characters in a pattern atom are
    /// lowercase and like [`Respect`](CaseMatching::Respect) otherwise.
    #[default]
    #[cfg(feature = "unicode-casefold")]
    Smart,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[non_exhaustive]
/// How to handle unicode normalization,
pub enum Normalization {
    /// Characters never match their normalized version (`a != ä`).
    Never,
    /// Acts like [`Never`](Normalization::Never) if any character in a pattern atom
    /// would need to be normalized. Otherwise normalization occurs (`a == ä` but `ä != a`).
    #[default]
    #[cfg(feature = "unicode-normalization")]
    Smart,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[non_exhaustive]
/// The kind of matching algorithm to run for an atom.
pub enum AtomKind {
    /// Fuzzy matching where the needle must match any haystack characters
    /// (match can contain gaps). This atom kind is used by default if no
    /// special syntax is used. There is no negated fuzzy matching (too
    /// many false positives).
    ///
    /// See also [`Matcher::fuzzy_match`](crate::Matcher::fuzzy_match).
    Fuzzy,
    /// The needle must match a contiguous sequence of haystack characters
    /// without gaps.  This atom kind is parsed from the following syntax:
    /// `'foo` and `!foo` (negated).
    ///
    /// See also [`Matcher::substring_match`](crate::Matcher::substring_match).
    Substring,
    /// The needle must match all leading haystack characters without gaps or
    /// prefix. This atom kind is parsed from the following syntax: `^foo` and
    /// `!^foo` (negated).
    ///
    /// See also [`Matcher::prefix_match`](crate::Matcher::prefix_match).
    Prefix,
    /// The needle must match all trailing haystack characters without gaps or
    /// postfix. This atom kind is parsed from the following syntax: `foo$` and
    /// `!foo$` (negated).
    ///
    /// See also [`Matcher::postfix_match`](crate::Matcher::postfix_match).
    Postfix,
    /// The needle must match all haystack characters without gaps or prefix.
    /// This atom kind is parsed from the following syntax: `^foo$` and `!^foo$`
    /// (negated).
    ///
    /// See also [`Matcher::exact_match`](crate::Matcher::exact_match).
    Exact,
}

/// A single pattern component that is matched with a single [`Matcher`] function
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Atom {
    /// Whether this pattern atom is a negative match.
    /// A negative pattern atom will prevent haystacks matching it from
    /// being matchend. It does not contribute to scoring/indices
    pub negative: bool,
    /// The kind of match that this pattern performs
    pub kind: AtomKind,
    needle: Utf32String,
    ignore_case: bool,
    normalize: bool,
}

impl Atom {
    /// Creates a single [`Atom`] from a string by performing unicode
    /// normalization and case folding (if necessary). Optionally `\ ` can
    /// be escaped to ` `.
    pub fn new(
        needle: &str,
        case: CaseMatching,
        normalize: Normalization,
        kind: AtomKind,
        escape_whitespace: bool,
    ) -> Atom {
        Atom::new_inner(needle, case, normalize, kind, escape_whitespace, false)
    }

    fn new_inner(
        needle: &str,
        case: CaseMatching,
        normalization: Normalization,
        kind: AtomKind,
        escape_whitespace: bool,
        append_dollar: bool,
    ) -> Atom {
        let mut ignore_case;
        let mut normalize;
        #[cfg(feature = "unicode-normalization")]
        {
            normalize = matches!(normalization, Normalization::Smart);
        }
        #[cfg(not(feature = "unicode-normalization"))]
        {
            normalize = false;
        }
        let needle = if needle.is_ascii() {
            let mut needle = if escape_whitespace {
                if let Some((start, rem)) = needle.split_once("\\ ") {
                    let mut needle = start.to_owned();
                    for rem in rem.split("\\ ") {
                        needle.push(' ');
                        needle.push_str(rem);
                    }
                    needle
                } else {
                    needle.to_owned()
                }
            } else {
                needle.to_owned()
            };

            match case {
                #[cfg(feature = "unicode-casefold")]
                CaseMatching::Ignore => {
                    ignore_case = true;
                    needle.make_ascii_lowercase()
                }
                #[cfg(feature = "unicode-casefold")]
                CaseMatching::Smart => {
                    ignore_case = !needle.bytes().any(|b| b.is_ascii_uppercase())
                }
                CaseMatching::Respect => ignore_case = false,
            }
            if append_dollar {
                needle.push('$');
            }
            Utf32String::Ascii(needle.into_boxed_str())
        } else {
            let mut needle_ = Vec::with_capacity(needle.len());
            #[cfg(feature = "unicode-casefold")]
            {
                ignore_case = matches!(case, CaseMatching::Ignore | CaseMatching::Smart);
            }
            #[cfg(not(feature = "unicode-casefold"))]
            {
                ignore_case = false;
            }
            #[cfg(feature = "unicode-normalization")]
            {
                normalize = matches!(normalization, Normalization::Smart);
            }
            if escape_whitespace {
                let mut saw_backslash = false;
                for mut c in chars::graphemes(needle) {
                    if saw_backslash {
                        if c == ' ' {
                            needle_.push(' ');
                            saw_backslash = false;
                            continue;
                        } else {
                            needle_.push('\\');
                        }
                    }
                    saw_backslash = c == '\\';
                    match case {
                        #[cfg(feature = "unicode-casefold")]
                        CaseMatching::Ignore => c = chars::to_lower_case(c),
                        #[cfg(feature = "unicode-casefold")]
                        CaseMatching::Smart => {
                            ignore_case = ignore_case && !chars::is_upper_case(c)
                        }
                        CaseMatching::Respect => (),
                    }
                    match normalization {
                        #[cfg(feature = "unicode-normalization")]
                        Normalization::Smart => {
                            normalize = normalize && chars::normalize(c) == c;
                        }
                        Normalization::Never => (),
                    }
                    needle_.push(c);
                }
            } else {
                let chars = chars::graphemes(needle).map(|mut c| {
                    match case {
                        #[cfg(feature = "unicode-casefold")]
                        CaseMatching::Ignore => c = chars::to_lower_case(c),
                        #[cfg(feature = "unicode-casefold")]
                        CaseMatching::Smart => {
                            ignore_case = ignore_case && !chars::is_upper_case(c);
                        }
                        CaseMatching::Respect => (),
                    }
                    match normalization {
                        #[cfg(feature = "unicode-normalization")]
                        Normalization::Smart => {
                            normalize = normalize && chars::normalize(c) == c;
                        }
                        Normalization::Never => (),
                    }
                    c
                });
                needle_.extend(chars);
            };
            if append_dollar {
                needle_.push('$');
            }
            Utf32String::Unicode(needle_.into_boxed_slice())
        };
        Atom {
            kind,
            needle,
            negative: false,
            ignore_case,
            normalize,
        }
    }

    /// Parse a pattern atom from a string. Some special trailing and leading
    /// characters can be used to control the atom kind. See [`AtomKind`] for
    /// details.
    pub fn parse(raw: &str, case: CaseMatching, normalize: Normalization) -> Atom {
        let mut atom = raw;
        let invert = match atom.as_bytes() {
            [b'!', ..] => {
                atom = &atom[1..];
                true
            }
            [b'\\', b'!', ..] => {
                atom = &atom[1..];
                false
            }
            _ => false,
        };

        let mut kind = match atom.as_bytes() {
            [b'^', ..] => {
                atom = &atom[1..];
                AtomKind::Prefix
            }
            [b'\'', ..] => {
                atom = &atom[1..];
                AtomKind::Substring
            }
            [b'\\', b'^' | b'\'', ..] => {
                atom = &atom[1..];
                AtomKind::Fuzzy
            }
            _ => AtomKind::Fuzzy,
        };

        let mut append_dollar = false;
        match atom.as_bytes() {
            [.., b'\\', b'$'] => {
                append_dollar = true;
                atom = &atom[..atom.len() - 2]
            }
            [.., b'$'] => {
                kind = if kind == AtomKind::Fuzzy {
                    AtomKind::Postfix
                } else {
                    AtomKind::Exact
                };
                atom = &atom[..atom.len() - 1]
            }
            _ => (),
        }

        if invert && kind == AtomKind::Fuzzy {
            kind = AtomKind::Substring
        }

        let mut pattern = Atom::new_inner(atom, case, normalize, kind, true, append_dollar);
        pattern.negative = invert;
        pattern
    }

    /// Matches this pattern against `haystack` (using the allocation and configuration
    /// from `matcher`) and calculates a ranking score. See the [`Matcher`].
    /// Documentation for more details.
    ///
    /// *Note:*  The `ignore_case` setting is overwritten to match the casing of
    /// each pattern atom.
    pub fn score(&self, haystack: Utf32Str<'_>, matcher: &mut Matcher) -> Option<u16> {
        matcher.config.ignore_case = self.ignore_case;
        matcher.config.normalize = self.normalize;
        let pattern_score = match self.kind {
            AtomKind::Exact => matcher.exact_match(haystack, self.needle.slice(..)),
            AtomKind::Fuzzy => matcher.fuzzy_match(haystack, self.needle.slice(..)),
            AtomKind::Substring => matcher.substring_match(haystack, self.needle.slice(..)),
            AtomKind::Prefix => matcher.prefix_match(haystack, self.needle.slice(..)),
            AtomKind::Postfix => matcher.postfix_match(haystack, self.needle.slice(..)),
        };
        if self.negative {
            if pattern_score.is_some() {
                return None;
            }
            Some(0)
        } else {
            pattern_score
        }
    }

    /// Matches this pattern against `haystack` (using the allocation and
    /// configuration from `matcher`), calculates a ranking score and the match
    /// indices. See the [`Matcher`]. Documentation for more
    /// details.
    ///
    /// *Note:*  The `ignore_case` setting is overwritten to match the casing of
    /// each pattern atom.
    ///
    /// *Note:*  The `indices` vector is not cleared by this function.
    pub fn indices(
        &self,
        haystack: Utf32Str<'_>,
        matcher: &mut Matcher,
        indices: &mut Vec<u32>,
    ) -> Option<u16> {
        matcher.config.ignore_case = self.ignore_case;
        matcher.config.normalize = self.normalize;
        if self.negative {
            let pattern_score = match self.kind {
                AtomKind::Exact => matcher.exact_match(haystack, self.needle.slice(..)),
                AtomKind::Fuzzy => matcher.fuzzy_match(haystack, self.needle.slice(..)),
                AtomKind::Substring => matcher.substring_match(haystack, self.needle.slice(..)),
                AtomKind::Prefix => matcher.prefix_match(haystack, self.needle.slice(..)),
                AtomKind::Postfix => matcher.postfix_match(haystack, self.needle.slice(..)),
            };
            pattern_score.is_none().then_some(0)
        } else {
            match self.kind {
                AtomKind::Exact => matcher.exact_indices(haystack, self.needle.slice(..), indices),
                AtomKind::Fuzzy => matcher.fuzzy_indices(haystack, self.needle.slice(..), indices),
                AtomKind::Substring => {
                    matcher.substring_indices(haystack, self.needle.slice(..), indices)
                }
                AtomKind::Prefix => {
                    matcher.prefix_indices(haystack, self.needle.slice(..), indices)
                }
                AtomKind::Postfix => {
                    matcher.postfix_indices(haystack, self.needle.slice(..), indices)
                }
            }
        }
    }

    /// Returns the needle text that is passed to the matcher. All indices
    /// produced by the `indices` functions produce char indices used to index
    /// this text
    pub fn needle_text(&self) -> Utf32Str<'_> {
        self.needle.slice(..)
    }
    /// Convenience function to easily match (and sort) a (relatively small)
    /// list of inputs.
    ///
    /// *Note* This function is not recommended for building a full fuzzy
    /// matching application that can match large numbers of matches (like all
    /// files in a directory) as all matching is done on the current thread,
    /// effectively blocking the UI. For such applications the high level
    /// `nucleo` crate can be used instead.
    pub fn match_list<T: AsRef<str>>(
        &self,
        items: impl IntoIterator<Item = T>,
        matcher: &mut Matcher,
    ) -> Vec<(T, u16)> {
        if self.needle.is_empty() {
            return items.into_iter().map(|item| (item, 0)).collect();
        }
        let mut buf = Vec::new();
        let mut items: Vec<_> = items
            .into_iter()
            .filter_map(|item| {
                self.score(Utf32Str::new(item.as_ref(), &mut buf), matcher)
                    .map(|score| (item, score))
            })
            .collect();
        items.sort_by_key(|(_, score)| Reverse(*score));
        items
    }
}

fn pattern_atoms(pattern: &str) -> impl Iterator<Item = &str> + '_ {
    let mut saw_backslash = false;
    pattern.split(move |c| {
        saw_backslash = match c {
            ' ' if !saw_backslash => return true,
            '\\' => true,
            _ => false,
        };
        false
    })
}

#[derive(Debug, Default)]
/// A text pattern made up of (potentially multiple) [atoms](crate::pattern::Atom).
#[non_exhaustive]
pub struct Pattern {
    /// The individual pattern (words) in this pattern
    pub atoms: Vec<Atom>,
}

impl Pattern {
    /// Creates a pattern where each word is matched individually (whitespaces
    /// can be escaped with `\`). Otherwise no parsing is performed (so $, !, '
    /// and ^ don't receive special treatment). If you want to match the entire
    /// pattern as a single needle use a single [`Atom`] instead.
    pub fn new(
        pattern: &str,
        case_matching: CaseMatching,
        normalize: Normalization,
        kind: AtomKind,
    ) -> Pattern {
        let atoms = pattern_atoms(pattern)
            .filter_map(|pat| {
                let pat = Atom::new(pat, case_matching, normalize, kind, true);
                (!pat.needle.is_empty()).then_some(pat)
            })
            .collect();
        Pattern { atoms }
    }
    /// Creates a pattern where each word is matched individually (whitespaces
    /// can be escaped with `\`). And $, !, ' and ^ at word boundaries will
    /// cause different matching behaviour (see [`AtomKind`]). These can be
    /// escaped with backslash.
    pub fn parse(pattern: &str, case_matching: CaseMatching, normalize: Normalization) -> Pattern {
        let atoms = pattern_atoms(pattern)
            .filter_map(|pat| {
                let pat = Atom::parse(pat, case_matching, normalize);
                (!pat.needle.is_empty()).then_some(pat)
            })
            .collect();
        Pattern { atoms }
    }

    /// Convenience function to easily match (and sort) a (relatively small)
    /// list of inputs.
    ///
    /// *Note* This function is not recommended for building a full fuzzy
    /// matching application that can match large numbers of matches (like all
    /// files in a directory) as all matching is done on the current thread,
    /// effectively blocking the UI. For such applications the high level
    /// `nucleo` crate can be used instead.
    pub fn match_list<T: AsRef<str>>(
        &self,
        items: impl IntoIterator<Item = T>,
        matcher: &mut Matcher,
    ) -> Vec<(T, u32)> {
        if self.atoms.is_empty() {
            return items.into_iter().map(|item| (item, 0)).collect();
        }
        let mut buf = Vec::new();
        let mut items: Vec<_> = items
            .into_iter()
            .filter_map(|item| {
                self.score(Utf32Str::new(item.as_ref(), &mut buf), matcher)
                    .map(|score| (item, score))
            })
            .collect();
        items.sort_by_key(|(_, score)| Reverse(*score));
        items
    }

    /// Matches this pattern against `haystack` (using the allocation and configuration
    /// from `matcher`) and calculates a ranking score. See the [`Matcher`].
    /// Documentation for more details.
    ///
    /// *Note:*  The `ignore_case` setting is overwritten to match the casing of
    /// each pattern atom.
    pub fn score(&self, haystack: Utf32Str<'_>, matcher: &mut Matcher) -> Option<u32> {
        if self.atoms.is_empty() {
            return Some(0);
        }
        let mut score = 0;
        for pattern in &self.atoms {
            score += pattern.score(haystack, matcher)? as u32;
        }
        Some(score)
    }

    /// Matches this pattern against `haystack` (using the allocation and
    /// configuration from `matcher`), calculates a ranking score and the match
    /// indices. See the [`Matcher`]. Documentation for more
    /// details.
    ///
    /// *Note:*  The `ignore_case` setting is overwritten to match the casing of
    /// each pattern atom.
    ///
    /// *Note:*  The indices for each pattern are calculated individually
    /// and simply appended to the `indices` vector and not deduplicated/sorted.
    /// This allows associating the match indices to their source pattern. If
    /// required (like for highlighting) unique/sorted indices can be obtained
    /// as follows:
    ///
    /// ```
    /// # let mut indices: Vec<u32> = Vec::new();
    /// indices.sort_unstable();
    /// indices.dedup();
    /// ```
    pub fn indices(
        &self,
        haystack: Utf32Str<'_>,
        matcher: &mut Matcher,
        indices: &mut Vec<u32>,
    ) -> Option<u32> {
        if self.atoms.is_empty() {
            return Some(0);
        }
        let mut score = 0;
        for pattern in &self.atoms {
            score += pattern.indices(haystack, matcher, indices)? as u32;
        }
        Some(score)
    }

    /// Refreshes this pattern by reparsing it from a string. This is mostly
    /// equivalent to just constructing a new pattern using [`Pattern::parse`]
    /// but is slightly more efficient by reusing some allocations
    pub fn reparse(
        &mut self,
        pattern: &str,
        case_matching: CaseMatching,
        normalize: Normalization,
    ) {
        self.atoms.clear();
        let atoms = pattern_atoms(pattern).filter_map(|atom| {
            let atom = Atom::parse(atom, case_matching, normalize);
            if atom.needle.is_empty() {
                return None;
            }
            Some(atom)
        });
        self.atoms.extend(atoms);
    }
}

impl Clone for Pattern {
    fn clone(&self) -> Self {
        Self {
            atoms: self.atoms.clone(),
        }
    }

    fn clone_from(&mut self, source: &Self) {
        self.atoms.clone_from(&source.atoms);
    }
}
