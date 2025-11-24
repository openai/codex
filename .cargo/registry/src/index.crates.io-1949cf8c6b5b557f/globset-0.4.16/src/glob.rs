use std::fmt::Write;
use std::path::{is_separator, Path};

use regex_automata::meta::Regex;

use crate::{new_regex, Candidate, Error, ErrorKind};

/// Describes a matching strategy for a particular pattern.
///
/// This provides a way to more quickly determine whether a pattern matches
/// a particular file path in a way that scales with a large number of
/// patterns. For example, if many patterns are of the form `*.ext`, then it's
/// possible to test whether any of those patterns matches by looking up a
/// file path's extension in a hash table.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MatchStrategy {
    /// A pattern matches if and only if the entire file path matches this
    /// literal string.
    Literal(String),
    /// A pattern matches if and only if the file path's basename matches this
    /// literal string.
    BasenameLiteral(String),
    /// A pattern matches if and only if the file path's extension matches this
    /// literal string.
    Extension(String),
    /// A pattern matches if and only if this prefix literal is a prefix of the
    /// candidate file path.
    Prefix(String),
    /// A pattern matches if and only if this prefix literal is a prefix of the
    /// candidate file path.
    ///
    /// An exception: if `component` is true, then `suffix` must appear at the
    /// beginning of a file path or immediately following a `/`.
    Suffix {
        /// The actual suffix.
        suffix: String,
        /// Whether this must start at the beginning of a path component.
        component: bool,
    },
    /// A pattern matches only if the given extension matches the file path's
    /// extension. Note that this is a necessary but NOT sufficient criterion.
    /// Namely, if the extension matches, then a full regex search is still
    /// required.
    RequiredExtension(String),
    /// A regex needs to be used for matching.
    Regex,
}

impl MatchStrategy {
    /// Returns a matching strategy for the given pattern.
    pub(crate) fn new(pat: &Glob) -> MatchStrategy {
        if let Some(lit) = pat.basename_literal() {
            MatchStrategy::BasenameLiteral(lit)
        } else if let Some(lit) = pat.literal() {
            MatchStrategy::Literal(lit)
        } else if let Some(ext) = pat.ext() {
            MatchStrategy::Extension(ext)
        } else if let Some(prefix) = pat.prefix() {
            MatchStrategy::Prefix(prefix)
        } else if let Some((suffix, component)) = pat.suffix() {
            MatchStrategy::Suffix { suffix, component }
        } else if let Some(ext) = pat.required_ext() {
            MatchStrategy::RequiredExtension(ext)
        } else {
            MatchStrategy::Regex
        }
    }
}

/// Glob represents a successfully parsed shell glob pattern.
///
/// It cannot be used directly to match file paths, but it can be converted
/// to a regular expression string or a matcher.
#[derive(Clone, Debug, Eq)]
pub struct Glob {
    glob: String,
    re: String,
    opts: GlobOptions,
    tokens: Tokens,
}

impl PartialEq for Glob {
    fn eq(&self, other: &Glob) -> bool {
        self.glob == other.glob && self.opts == other.opts
    }
}

impl std::hash::Hash for Glob {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.glob.hash(state);
        self.opts.hash(state);
    }
}

impl std::fmt::Display for Glob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.glob.fmt(f)
    }
}

impl std::str::FromStr for Glob {
    type Err = Error;

    fn from_str(glob: &str) -> Result<Self, Self::Err> {
        Self::new(glob)
    }
}

/// A matcher for a single pattern.
#[derive(Clone, Debug)]
pub struct GlobMatcher {
    /// The underlying pattern.
    pat: Glob,
    /// The pattern, as a compiled regex.
    re: Regex,
}

impl GlobMatcher {
    /// Tests whether the given path matches this pattern or not.
    pub fn is_match<P: AsRef<Path>>(&self, path: P) -> bool {
        self.is_match_candidate(&Candidate::new(path.as_ref()))
    }

    /// Tests whether the given path matches this pattern or not.
    pub fn is_match_candidate(&self, path: &Candidate<'_>) -> bool {
        self.re.is_match(&path.path)
    }

    /// Returns the `Glob` used to compile this matcher.
    pub fn glob(&self) -> &Glob {
        &self.pat
    }
}

/// A strategic matcher for a single pattern.
#[cfg(test)]
#[derive(Clone, Debug)]
struct GlobStrategic {
    /// The match strategy to use.
    strategy: MatchStrategy,
    /// The pattern, as a compiled regex.
    re: Regex,
}

#[cfg(test)]
impl GlobStrategic {
    /// Tests whether the given path matches this pattern or not.
    fn is_match<P: AsRef<Path>>(&self, path: P) -> bool {
        self.is_match_candidate(&Candidate::new(path.as_ref()))
    }

    /// Tests whether the given path matches this pattern or not.
    fn is_match_candidate(&self, candidate: &Candidate<'_>) -> bool {
        let byte_path = &*candidate.path;

        match self.strategy {
            MatchStrategy::Literal(ref lit) => lit.as_bytes() == byte_path,
            MatchStrategy::BasenameLiteral(ref lit) => {
                lit.as_bytes() == &*candidate.basename
            }
            MatchStrategy::Extension(ref ext) => {
                ext.as_bytes() == &*candidate.ext
            }
            MatchStrategy::Prefix(ref pre) => {
                starts_with(pre.as_bytes(), byte_path)
            }
            MatchStrategy::Suffix { ref suffix, component } => {
                if component && byte_path == &suffix.as_bytes()[1..] {
                    return true;
                }
                ends_with(suffix.as_bytes(), byte_path)
            }
            MatchStrategy::RequiredExtension(ref ext) => {
                let ext = ext.as_bytes();
                &*candidate.ext == ext && self.re.is_match(byte_path)
            }
            MatchStrategy::Regex => self.re.is_match(byte_path),
        }
    }
}

/// A builder for a pattern.
///
/// This builder enables configuring the match semantics of a pattern. For
/// example, one can make matching case insensitive.
///
/// The lifetime `'a` refers to the lifetime of the pattern string.
#[derive(Clone, Debug)]
pub struct GlobBuilder<'a> {
    /// The glob pattern to compile.
    glob: &'a str,
    /// Options for the pattern.
    opts: GlobOptions,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct GlobOptions {
    /// Whether to match case insensitively.
    case_insensitive: bool,
    /// Whether to require a literal separator to match a separator in a file
    /// path. e.g., when enabled, `*` won't match `/`.
    literal_separator: bool,
    /// Whether or not to use `\` to escape special characters.
    /// e.g., when enabled, `\*` will match a literal `*`.
    backslash_escape: bool,
    /// Whether or not an empty case in an alternate will be removed.
    /// e.g., when enabled, `{,a}` will match "" and "a".
    empty_alternates: bool,
}

impl GlobOptions {
    fn default() -> GlobOptions {
        GlobOptions {
            case_insensitive: false,
            literal_separator: false,
            backslash_escape: !is_separator('\\'),
            empty_alternates: false,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Tokens(Vec<Token>);

impl std::ops::Deref for Tokens {
    type Target = Vec<Token>;
    fn deref(&self) -> &Vec<Token> {
        &self.0
    }
}

impl std::ops::DerefMut for Tokens {
    fn deref_mut(&mut self) -> &mut Vec<Token> {
        &mut self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Token {
    Literal(char),
    Any,
    ZeroOrMore,
    RecursivePrefix,
    RecursiveSuffix,
    RecursiveZeroOrMore,
    Class { negated: bool, ranges: Vec<(char, char)> },
    Alternates(Vec<Tokens>),
}

impl Glob {
    /// Builds a new pattern with default options.
    pub fn new(glob: &str) -> Result<Glob, Error> {
        GlobBuilder::new(glob).build()
    }

    /// Returns a matcher for this pattern.
    pub fn compile_matcher(&self) -> GlobMatcher {
        let re =
            new_regex(&self.re).expect("regex compilation shouldn't fail");
        GlobMatcher { pat: self.clone(), re }
    }

    /// Returns a strategic matcher.
    ///
    /// This isn't exposed because it's not clear whether it's actually
    /// faster than just running a regex for a *single* pattern. If it
    /// is faster, then GlobMatcher should do it automatically.
    #[cfg(test)]
    fn compile_strategic_matcher(&self) -> GlobStrategic {
        let strategy = MatchStrategy::new(self);
        let re =
            new_regex(&self.re).expect("regex compilation shouldn't fail");
        GlobStrategic { strategy, re }
    }

    /// Returns the original glob pattern used to build this pattern.
    pub fn glob(&self) -> &str {
        &self.glob
    }

    /// Returns the regular expression string for this glob.
    ///
    /// Note that regular expressions for globs are intended to be matched on
    /// arbitrary bytes (`&[u8]`) instead of Unicode strings (`&str`). In
    /// particular, globs are frequently used on file paths, where there is no
    /// general guarantee that file paths are themselves valid UTF-8. As a
    /// result, callers will need to ensure that they are using a regex API
    /// that can match on arbitrary bytes. For example, the
    /// [`regex`](https://crates.io/regex)
    /// crate's
    /// [`Regex`](https://docs.rs/regex/*/regex/struct.Regex.html)
    /// API is not suitable for this since it matches on `&str`, but its
    /// [`bytes::Regex`](https://docs.rs/regex/*/regex/bytes/struct.Regex.html)
    /// API is suitable for this.
    pub fn regex(&self) -> &str {
        &self.re
    }

    /// Returns the pattern as a literal if and only if the pattern must match
    /// an entire path exactly.
    ///
    /// The basic format of these patterns is `{literal}`.
    fn literal(&self) -> Option<String> {
        if self.opts.case_insensitive {
            return None;
        }
        let mut lit = String::new();
        for t in &*self.tokens {
            let Token::Literal(c) = *t else { return None };
            lit.push(c);
        }
        if lit.is_empty() {
            None
        } else {
            Some(lit)
        }
    }

    /// Returns an extension if this pattern matches a file path if and only
    /// if the file path has the extension returned.
    ///
    /// Note that this extension returned differs from the extension that
    /// std::path::Path::extension returns. Namely, this extension includes
    /// the '.'. Also, paths like `.rs` are considered to have an extension
    /// of `.rs`.
    fn ext(&self) -> Option<String> {
        if self.opts.case_insensitive {
            return None;
        }
        let start = match *self.tokens.get(0)? {
            Token::RecursivePrefix => 1,
            _ => 0,
        };
        match *self.tokens.get(start)? {
            Token::ZeroOrMore => {
                // If there was no recursive prefix, then we only permit
                // `*` if `*` can match a `/`. For example, if `*` can't
                // match `/`, then `*.c` doesn't match `foo/bar.c`.
                if start == 0 && self.opts.literal_separator {
                    return None;
                }
            }
            _ => return None,
        }
        match *self.tokens.get(start + 1)? {
            Token::Literal('.') => {}
            _ => return None,
        }
        let mut lit = ".".to_string();
        for t in self.tokens[start + 2..].iter() {
            match *t {
                Token::Literal('.') | Token::Literal('/') => return None,
                Token::Literal(c) => lit.push(c),
                _ => return None,
            }
        }
        if lit.is_empty() {
            None
        } else {
            Some(lit)
        }
    }

    /// This is like `ext`, but returns an extension even if it isn't sufficient
    /// to imply a match. Namely, if an extension is returned, then it is
    /// necessary but not sufficient for a match.
    fn required_ext(&self) -> Option<String> {
        if self.opts.case_insensitive {
            return None;
        }
        // We don't care at all about the beginning of this pattern. All we
        // need to check for is if it ends with a literal of the form `.ext`.
        let mut ext: Vec<char> = vec![]; // built in reverse
        for t in self.tokens.iter().rev() {
            match *t {
                Token::Literal('/') => return None,
                Token::Literal(c) => {
                    ext.push(c);
                    if c == '.' {
                        break;
                    }
                }
                _ => return None,
            }
        }
        if ext.last() != Some(&'.') {
            None
        } else {
            ext.reverse();
            Some(ext.into_iter().collect())
        }
    }

    /// Returns a literal prefix of this pattern if the entire pattern matches
    /// if the literal prefix matches.
    fn prefix(&self) -> Option<String> {
        if self.opts.case_insensitive {
            return None;
        }
        let (end, need_sep) = match *self.tokens.last()? {
            Token::ZeroOrMore => {
                if self.opts.literal_separator {
                    // If a trailing `*` can't match a `/`, then we can't
                    // assume a match of the prefix corresponds to a match
                    // of the overall pattern. e.g., `foo/*` with
                    // `literal_separator` enabled matches `foo/bar` but not
                    // `foo/bar/baz`, even though `foo/bar/baz` has a `foo/`
                    // literal prefix.
                    return None;
                }
                (self.tokens.len() - 1, false)
            }
            Token::RecursiveSuffix => (self.tokens.len() - 1, true),
            _ => (self.tokens.len(), false),
        };
        let mut lit = String::new();
        for t in &self.tokens[0..end] {
            let Token::Literal(c) = *t else { return None };
            lit.push(c);
        }
        if need_sep {
            lit.push('/');
        }
        if lit.is_empty() {
            None
        } else {
            Some(lit)
        }
    }

    /// Returns a literal suffix of this pattern if the entire pattern matches
    /// if the literal suffix matches.
    ///
    /// If a literal suffix is returned and it must match either the entire
    /// file path or be preceded by a `/`, then also return true. This happens
    /// with a pattern like `**/foo/bar`. Namely, this pattern matches
    /// `foo/bar` and `baz/foo/bar`, but not `foofoo/bar`. In this case, the
    /// suffix returned is `/foo/bar` (but should match the entire path
    /// `foo/bar`).
    ///
    /// When this returns true, the suffix literal is guaranteed to start with
    /// a `/`.
    fn suffix(&self) -> Option<(String, bool)> {
        if self.opts.case_insensitive {
            return None;
        }
        let mut lit = String::new();
        let (start, entire) = match *self.tokens.get(0)? {
            Token::RecursivePrefix => {
                // We only care if this follows a path component if the next
                // token is a literal.
                if let Some(&Token::Literal(_)) = self.tokens.get(1) {
                    lit.push('/');
                    (1, true)
                } else {
                    (1, false)
                }
            }
            _ => (0, false),
        };
        let start = match *self.tokens.get(start)? {
            Token::ZeroOrMore => {
                // If literal_separator is enabled, then a `*` can't
                // necessarily match everything, so reporting a suffix match
                // as a match of the pattern would be a false positive.
                if self.opts.literal_separator {
                    return None;
                }
                start + 1
            }
            _ => start,
        };
        for t in &self.tokens[start..] {
            let Token::Literal(c) = *t else { return None };
            lit.push(c);
        }
        if lit.is_empty() || lit == "/" {
            None
        } else {
            Some((lit, entire))
        }
    }

    /// If this pattern only needs to inspect the basename of a file path,
    /// then the tokens corresponding to only the basename match are returned.
    ///
    /// For example, given a pattern of `**/*.foo`, only the tokens
    /// corresponding to `*.foo` are returned.
    ///
    /// Note that this will return None if any match of the basename tokens
    /// doesn't correspond to a match of the entire pattern. For example, the
    /// glob `foo` only matches when a file path has a basename of `foo`, but
    /// doesn't *always* match when a file path has a basename of `foo`. e.g.,
    /// `foo` doesn't match `abc/foo`.
    fn basename_tokens(&self) -> Option<&[Token]> {
        if self.opts.case_insensitive {
            return None;
        }
        let start = match *self.tokens.get(0)? {
            Token::RecursivePrefix => 1,
            _ => {
                // With nothing to gobble up the parent portion of a path,
                // we can't assume that matching on only the basename is
                // correct.
                return None;
            }
        };
        if self.tokens[start..].is_empty() {
            return None;
        }
        for t in self.tokens[start..].iter() {
            match *t {
                Token::Literal('/') => return None,
                Token::Literal(_) => {} // OK
                Token::Any | Token::ZeroOrMore => {
                    if !self.opts.literal_separator {
                        // In this case, `*` and `?` can match a path
                        // separator, which means this could reach outside
                        // the basename.
                        return None;
                    }
                }
                Token::RecursivePrefix
                | Token::RecursiveSuffix
                | Token::RecursiveZeroOrMore => {
                    return None;
                }
                Token::Class { .. } | Token::Alternates(..) => {
                    // We *could* be a little smarter here, but either one
                    // of these is going to prevent our literal optimizations
                    // anyway, so give up.
                    return None;
                }
            }
        }
        Some(&self.tokens[start..])
    }

    /// Returns the pattern as a literal if and only if the pattern exclusively
    /// matches the basename of a file path *and* is a literal.
    ///
    /// The basic format of these patterns is `**/{literal}`, where `{literal}`
    /// does not contain a path separator.
    fn basename_literal(&self) -> Option<String> {
        let tokens = self.basename_tokens()?;
        let mut lit = String::new();
        for t in tokens {
            let Token::Literal(c) = *t else { return None };
            lit.push(c);
        }
        Some(lit)
    }
}

impl<'a> GlobBuilder<'a> {
    /// Create a new builder for the pattern given.
    ///
    /// The pattern is not compiled until `build` is called.
    pub fn new(glob: &'a str) -> GlobBuilder<'a> {
        GlobBuilder { glob, opts: GlobOptions::default() }
    }

    /// Parses and builds the pattern.
    pub fn build(&self) -> Result<Glob, Error> {
        let mut p = Parser {
            glob: &self.glob,
            stack: vec![Tokens::default()],
            chars: self.glob.chars().peekable(),
            prev: None,
            cur: None,
            opts: &self.opts,
        };
        p.parse()?;
        if p.stack.is_empty() {
            Err(Error {
                glob: Some(self.glob.to_string()),
                kind: ErrorKind::UnopenedAlternates,
            })
        } else if p.stack.len() > 1 {
            Err(Error {
                glob: Some(self.glob.to_string()),
                kind: ErrorKind::UnclosedAlternates,
            })
        } else {
            let tokens = p.stack.pop().unwrap();
            Ok(Glob {
                glob: self.glob.to_string(),
                re: tokens.to_regex_with(&self.opts),
                opts: self.opts,
                tokens,
            })
        }
    }

    /// Toggle whether the pattern matches case insensitively or not.
    ///
    /// This is disabled by default.
    pub fn case_insensitive(&mut self, yes: bool) -> &mut GlobBuilder<'a> {
        self.opts.case_insensitive = yes;
        self
    }

    /// Toggle whether a literal `/` is required to match a path separator.
    ///
    /// By default this is false: `*` and `?` will match `/`.
    pub fn literal_separator(&mut self, yes: bool) -> &mut GlobBuilder<'a> {
        self.opts.literal_separator = yes;
        self
    }

    /// When enabled, a back slash (`\`) may be used to escape
    /// special characters in a glob pattern. Additionally, this will
    /// prevent `\` from being interpreted as a path separator on all
    /// platforms.
    ///
    /// This is enabled by default on platforms where `\` is not a
    /// path separator and disabled by default on platforms where `\`
    /// is a path separator.
    pub fn backslash_escape(&mut self, yes: bool) -> &mut GlobBuilder<'a> {
        self.opts.backslash_escape = yes;
        self
    }

    /// Toggle whether an empty pattern in a list of alternates is accepted.
    ///
    /// For example, if this is set then the glob `foo{,.txt}` will match both
    /// `foo` and `foo.txt`.
    ///
    /// By default this is false.
    pub fn empty_alternates(&mut self, yes: bool) -> &mut GlobBuilder<'a> {
        self.opts.empty_alternates = yes;
        self
    }
}

impl Tokens {
    /// Convert this pattern to a string that is guaranteed to be a valid
    /// regular expression and will represent the matching semantics of this
    /// glob pattern and the options given.
    fn to_regex_with(&self, options: &GlobOptions) -> String {
        let mut re = String::new();
        re.push_str("(?-u)");
        if options.case_insensitive {
            re.push_str("(?i)");
        }
        re.push('^');
        // Special case. If the entire glob is just `**`, then it should match
        // everything.
        if self.len() == 1 && self[0] == Token::RecursivePrefix {
            re.push_str(".*");
            re.push('$');
            return re;
        }
        self.tokens_to_regex(options, &self, &mut re);
        re.push('$');
        re
    }

    fn tokens_to_regex(
        &self,
        options: &GlobOptions,
        tokens: &[Token],
        re: &mut String,
    ) {
        for tok in tokens.iter() {
            match *tok {
                Token::Literal(c) => {
                    re.push_str(&char_to_escaped_literal(c));
                }
                Token::Any => {
                    if options.literal_separator {
                        re.push_str("[^/]");
                    } else {
                        re.push_str(".");
                    }
                }
                Token::ZeroOrMore => {
                    if options.literal_separator {
                        re.push_str("[^/]*");
                    } else {
                        re.push_str(".*");
                    }
                }
                Token::RecursivePrefix => {
                    re.push_str("(?:/?|.*/)");
                }
                Token::RecursiveSuffix => {
                    re.push_str("/.*");
                }
                Token::RecursiveZeroOrMore => {
                    re.push_str("(?:/|/.*/)");
                }
                Token::Class { negated, ref ranges } => {
                    re.push('[');
                    if negated {
                        re.push('^');
                    }
                    for r in ranges {
                        if r.0 == r.1 {
                            // Not strictly necessary, but nicer to look at.
                            re.push_str(&char_to_escaped_literal(r.0));
                        } else {
                            re.push_str(&char_to_escaped_literal(r.0));
                            re.push('-');
                            re.push_str(&char_to_escaped_literal(r.1));
                        }
                    }
                    re.push(']');
                }
                Token::Alternates(ref patterns) => {
                    let mut parts = vec![];
                    for pat in patterns {
                        let mut altre = String::new();
                        self.tokens_to_regex(options, &pat, &mut altre);
                        if !altre.is_empty() || options.empty_alternates {
                            parts.push(altre);
                        }
                    }

                    // It is possible to have an empty set in which case the
                    // resulting alternation '()' would be an error.
                    if !parts.is_empty() {
                        re.push_str("(?:");
                        re.push_str(&parts.join("|"));
                        re.push(')');
                    }
                }
            }
        }
    }
}

/// Convert a Unicode scalar value to an escaped string suitable for use as
/// a literal in a non-Unicode regex.
fn char_to_escaped_literal(c: char) -> String {
    let mut buf = [0; 4];
    let bytes = c.encode_utf8(&mut buf).as_bytes();
    bytes_to_escaped_literal(bytes)
}

/// Converts an arbitrary sequence of bytes to a UTF-8 string. All non-ASCII
/// code units are converted to their escaped form.
fn bytes_to_escaped_literal(bs: &[u8]) -> String {
    let mut s = String::with_capacity(bs.len());
    for &b in bs {
        if b <= 0x7F {
            regex_syntax::escape_into(
                char::from(b).encode_utf8(&mut [0; 4]),
                &mut s,
            );
        } else {
            write!(&mut s, "\\x{:02x}", b).unwrap();
        }
    }
    s
}

struct Parser<'a> {
    glob: &'a str,
    stack: Vec<Tokens>,
    chars: std::iter::Peekable<std::str::Chars<'a>>,
    prev: Option<char>,
    cur: Option<char>,
    opts: &'a GlobOptions,
}

impl<'a> Parser<'a> {
    fn error(&self, kind: ErrorKind) -> Error {
        Error { glob: Some(self.glob.to_string()), kind }
    }

    fn parse(&mut self) -> Result<(), Error> {
        while let Some(c) = self.bump() {
            match c {
                '?' => self.push_token(Token::Any)?,
                '*' => self.parse_star()?,
                '[' => self.parse_class()?,
                '{' => self.push_alternate()?,
                '}' => self.pop_alternate()?,
                ',' => self.parse_comma()?,
                '\\' => self.parse_backslash()?,
                c => self.push_token(Token::Literal(c))?,
            }
        }
        Ok(())
    }

    fn push_alternate(&mut self) -> Result<(), Error> {
        if self.stack.len() > 1 {
            return Err(self.error(ErrorKind::NestedAlternates));
        }
        Ok(self.stack.push(Tokens::default()))
    }

    fn pop_alternate(&mut self) -> Result<(), Error> {
        let mut alts = vec![];
        while self.stack.len() >= 2 {
            alts.push(self.stack.pop().unwrap());
        }
        self.push_token(Token::Alternates(alts))
    }

    fn push_token(&mut self, tok: Token) -> Result<(), Error> {
        if let Some(ref mut pat) = self.stack.last_mut() {
            return Ok(pat.push(tok));
        }
        Err(self.error(ErrorKind::UnopenedAlternates))
    }

    fn pop_token(&mut self) -> Result<Token, Error> {
        if let Some(ref mut pat) = self.stack.last_mut() {
            return Ok(pat.pop().unwrap());
        }
        Err(self.error(ErrorKind::UnopenedAlternates))
    }

    fn have_tokens(&self) -> Result<bool, Error> {
        match self.stack.last() {
            None => Err(self.error(ErrorKind::UnopenedAlternates)),
            Some(ref pat) => Ok(!pat.is_empty()),
        }
    }

    fn parse_comma(&mut self) -> Result<(), Error> {
        // If we aren't inside a group alternation, then don't
        // treat commas specially. Otherwise, we need to start
        // a new alternate.
        if self.stack.len() <= 1 {
            self.push_token(Token::Literal(','))
        } else {
            Ok(self.stack.push(Tokens::default()))
        }
    }

    fn parse_backslash(&mut self) -> Result<(), Error> {
        if self.opts.backslash_escape {
            match self.bump() {
                None => Err(self.error(ErrorKind::DanglingEscape)),
                Some(c) => self.push_token(Token::Literal(c)),
            }
        } else if is_separator('\\') {
            // Normalize all patterns to use / as a separator.
            self.push_token(Token::Literal('/'))
        } else {
            self.push_token(Token::Literal('\\'))
        }
    }

    fn parse_star(&mut self) -> Result<(), Error> {
        let prev = self.prev;
        if self.peek() != Some('*') {
            self.push_token(Token::ZeroOrMore)?;
            return Ok(());
        }
        assert!(self.bump() == Some('*'));
        if !self.have_tokens()? {
            if !self.peek().map_or(true, is_separator) {
                self.push_token(Token::ZeroOrMore)?;
                self.push_token(Token::ZeroOrMore)?;
            } else {
                self.push_token(Token::RecursivePrefix)?;
                assert!(self.bump().map_or(true, is_separator));
            }
            return Ok(());
        }

        if !prev.map(is_separator).unwrap_or(false) {
            if self.stack.len() <= 1
                || (prev != Some(',') && prev != Some('{'))
            {
                self.push_token(Token::ZeroOrMore)?;
                self.push_token(Token::ZeroOrMore)?;
                return Ok(());
            }
        }
        let is_suffix = match self.peek() {
            None => {
                assert!(self.bump().is_none());
                true
            }
            Some(',') | Some('}') if self.stack.len() >= 2 => true,
            Some(c) if is_separator(c) => {
                assert!(self.bump().map(is_separator).unwrap_or(false));
                false
            }
            _ => {
                self.push_token(Token::ZeroOrMore)?;
                self.push_token(Token::ZeroOrMore)?;
                return Ok(());
            }
        };
        match self.pop_token()? {
            Token::RecursivePrefix => {
                self.push_token(Token::RecursivePrefix)?;
            }
            Token::RecursiveSuffix => {
                self.push_token(Token::RecursiveSuffix)?;
            }
            _ => {
                if is_suffix {
                    self.push_token(Token::RecursiveSuffix)?;
                } else {
                    self.push_token(Token::RecursiveZeroOrMore)?;
                }
            }
        }
        Ok(())
    }

    fn parse_class(&mut self) -> Result<(), Error> {
        fn add_to_last_range(
            glob: &str,
            r: &mut (char, char),
            add: char,
        ) -> Result<(), Error> {
            r.1 = add;
            if r.1 < r.0 {
                Err(Error {
                    glob: Some(glob.to_string()),
                    kind: ErrorKind::InvalidRange(r.0, r.1),
                })
            } else {
                Ok(())
            }
        }
        let mut ranges = vec![];
        let negated = match self.chars.peek() {
            Some(&'!') | Some(&'^') => {
                let bump = self.bump();
                assert!(bump == Some('!') || bump == Some('^'));
                true
            }
            _ => false,
        };
        let mut first = true;
        let mut in_range = false;
        loop {
            let c = match self.bump() {
                Some(c) => c,
                // The only way to successfully break this loop is to observe
                // a ']'.
                None => return Err(self.error(ErrorKind::UnclosedClass)),
            };
            match c {
                ']' => {
                    if first {
                        ranges.push((']', ']'));
                    } else {
                        break;
                    }
                }
                '-' => {
                    if first {
                        ranges.push(('-', '-'));
                    } else if in_range {
                        // invariant: in_range is only set when there is
                        // already at least one character seen.
                        let r = ranges.last_mut().unwrap();
                        add_to_last_range(&self.glob, r, '-')?;
                        in_range = false;
                    } else {
                        assert!(!ranges.is_empty());
                        in_range = true;
                    }
                }
                c => {
                    if in_range {
                        // invariant: in_range is only set when there is
                        // already at least one character seen.
                        add_to_last_range(
                            &self.glob,
                            ranges.last_mut().unwrap(),
                            c,
                        )?;
                    } else {
                        ranges.push((c, c));
                    }
                    in_range = false;
                }
            }
            first = false;
        }
        if in_range {
            // Means that the last character in the class was a '-', so add
            // it as a literal.
            ranges.push(('-', '-'));
        }
        self.push_token(Token::Class { negated, ranges })
    }

    fn bump(&mut self) -> Option<char> {
        self.prev = self.cur;
        self.cur = self.chars.next();
        self.cur
    }

    fn peek(&mut self) -> Option<char> {
        self.chars.peek().map(|&ch| ch)
    }
}

#[cfg(test)]
fn starts_with(needle: &[u8], haystack: &[u8]) -> bool {
    needle.len() <= haystack.len() && needle == &haystack[..needle.len()]
}

#[cfg(test)]
fn ends_with(needle: &[u8], haystack: &[u8]) -> bool {
    if needle.len() > haystack.len() {
        return false;
    }
    needle == &haystack[haystack.len() - needle.len()..]
}

#[cfg(test)]
mod tests {
    use super::Token::*;
    use super::{Glob, GlobBuilder, Token};
    use crate::{ErrorKind, GlobSetBuilder};

    #[derive(Clone, Copy, Debug, Default)]
    struct Options {
        casei: Option<bool>,
        litsep: Option<bool>,
        bsesc: Option<bool>,
        ealtre: Option<bool>,
    }

    macro_rules! syntax {
        ($name:ident, $pat:expr, $tokens:expr) => {
            #[test]
            fn $name() {
                let pat = Glob::new($pat).unwrap();
                assert_eq!($tokens, pat.tokens.0);
            }
        };
    }

    macro_rules! syntaxerr {
        ($name:ident, $pat:expr, $err:expr) => {
            #[test]
            fn $name() {
                let err = Glob::new($pat).unwrap_err();
                assert_eq!(&$err, err.kind());
            }
        };
    }

    macro_rules! toregex {
        ($name:ident, $pat:expr, $re:expr) => {
            toregex!($name, $pat, $re, Options::default());
        };
        ($name:ident, $pat:expr, $re:expr, $options:expr) => {
            #[test]
            fn $name() {
                let mut builder = GlobBuilder::new($pat);
                if let Some(casei) = $options.casei {
                    builder.case_insensitive(casei);
                }
                if let Some(litsep) = $options.litsep {
                    builder.literal_separator(litsep);
                }
                if let Some(bsesc) = $options.bsesc {
                    builder.backslash_escape(bsesc);
                }
                if let Some(ealtre) = $options.ealtre {
                    builder.empty_alternates(ealtre);
                }
                let pat = builder.build().unwrap();
                assert_eq!(format!("(?-u){}", $re), pat.regex());
            }
        };
    }

    macro_rules! matches {
        ($name:ident, $pat:expr, $path:expr) => {
            matches!($name, $pat, $path, Options::default());
        };
        ($name:ident, $pat:expr, $path:expr, $options:expr) => {
            #[test]
            fn $name() {
                let mut builder = GlobBuilder::new($pat);
                if let Some(casei) = $options.casei {
                    builder.case_insensitive(casei);
                }
                if let Some(litsep) = $options.litsep {
                    builder.literal_separator(litsep);
                }
                if let Some(bsesc) = $options.bsesc {
                    builder.backslash_escape(bsesc);
                }
                if let Some(ealtre) = $options.ealtre {
                    builder.empty_alternates(ealtre);
                }
                let pat = builder.build().unwrap();
                let matcher = pat.compile_matcher();
                let strategic = pat.compile_strategic_matcher();
                let set = GlobSetBuilder::new().add(pat).build().unwrap();
                assert!(matcher.is_match($path));
                assert!(strategic.is_match($path));
                assert!(set.is_match($path));
            }
        };
    }

    macro_rules! nmatches {
        ($name:ident, $pat:expr, $path:expr) => {
            nmatches!($name, $pat, $path, Options::default());
        };
        ($name:ident, $pat:expr, $path:expr, $options:expr) => {
            #[test]
            fn $name() {
                let mut builder = GlobBuilder::new($pat);
                if let Some(casei) = $options.casei {
                    builder.case_insensitive(casei);
                }
                if let Some(litsep) = $options.litsep {
                    builder.literal_separator(litsep);
                }
                if let Some(bsesc) = $options.bsesc {
                    builder.backslash_escape(bsesc);
                }
                if let Some(ealtre) = $options.ealtre {
                    builder.empty_alternates(ealtre);
                }
                let pat = builder.build().unwrap();
                let matcher = pat.compile_matcher();
                let strategic = pat.compile_strategic_matcher();
                let set = GlobSetBuilder::new().add(pat).build().unwrap();
                assert!(!matcher.is_match($path));
                assert!(!strategic.is_match($path));
                assert!(!set.is_match($path));
            }
        };
    }

    fn s(string: &str) -> String {
        string.to_string()
    }

    fn class(s: char, e: char) -> Token {
        Class { negated: false, ranges: vec![(s, e)] }
    }

    fn classn(s: char, e: char) -> Token {
        Class { negated: true, ranges: vec![(s, e)] }
    }

    fn rclass(ranges: &[(char, char)]) -> Token {
        Class { negated: false, ranges: ranges.to_vec() }
    }

    fn rclassn(ranges: &[(char, char)]) -> Token {
        Class { negated: true, ranges: ranges.to_vec() }
    }

    syntax!(literal1, "a", vec![Literal('a')]);
    syntax!(literal2, "ab", vec![Literal('a'), Literal('b')]);
    syntax!(any1, "?", vec![Any]);
    syntax!(any2, "a?b", vec![Literal('a'), Any, Literal('b')]);
    syntax!(seq1, "*", vec![ZeroOrMore]);
    syntax!(seq2, "a*b", vec![Literal('a'), ZeroOrMore, Literal('b')]);
    syntax!(
        seq3,
        "*a*b*",
        vec![ZeroOrMore, Literal('a'), ZeroOrMore, Literal('b'), ZeroOrMore,]
    );
    syntax!(rseq1, "**", vec![RecursivePrefix]);
    syntax!(rseq2, "**/", vec![RecursivePrefix]);
    syntax!(rseq3, "/**", vec![RecursiveSuffix]);
    syntax!(rseq4, "/**/", vec![RecursiveZeroOrMore]);
    syntax!(
        rseq5,
        "a/**/b",
        vec![Literal('a'), RecursiveZeroOrMore, Literal('b'),]
    );
    syntax!(cls1, "[a]", vec![class('a', 'a')]);
    syntax!(cls2, "[!a]", vec![classn('a', 'a')]);
    syntax!(cls3, "[a-z]", vec![class('a', 'z')]);
    syntax!(cls4, "[!a-z]", vec![classn('a', 'z')]);
    syntax!(cls5, "[-]", vec![class('-', '-')]);
    syntax!(cls6, "[]]", vec![class(']', ']')]);
    syntax!(cls7, "[*]", vec![class('*', '*')]);
    syntax!(cls8, "[!!]", vec![classn('!', '!')]);
    syntax!(cls9, "[a-]", vec![rclass(&[('a', 'a'), ('-', '-')])]);
    syntax!(cls10, "[-a-z]", vec![rclass(&[('-', '-'), ('a', 'z')])]);
    syntax!(cls11, "[a-z-]", vec![rclass(&[('a', 'z'), ('-', '-')])]);
    syntax!(
        cls12,
        "[-a-z-]",
        vec![rclass(&[('-', '-'), ('a', 'z'), ('-', '-')]),]
    );
    syntax!(cls13, "[]-z]", vec![class(']', 'z')]);
    syntax!(cls14, "[--z]", vec![class('-', 'z')]);
    syntax!(cls15, "[ --]", vec![class(' ', '-')]);
    syntax!(cls16, "[0-9a-z]", vec![rclass(&[('0', '9'), ('a', 'z')])]);
    syntax!(cls17, "[a-z0-9]", vec![rclass(&[('a', 'z'), ('0', '9')])]);
    syntax!(cls18, "[!0-9a-z]", vec![rclassn(&[('0', '9'), ('a', 'z')])]);
    syntax!(cls19, "[!a-z0-9]", vec![rclassn(&[('a', 'z'), ('0', '9')])]);
    syntax!(cls20, "[^a]", vec![classn('a', 'a')]);
    syntax!(cls21, "[^a-z]", vec![classn('a', 'z')]);

    syntaxerr!(err_unclosed1, "[", ErrorKind::UnclosedClass);
    syntaxerr!(err_unclosed2, "[]", ErrorKind::UnclosedClass);
    syntaxerr!(err_unclosed3, "[!", ErrorKind::UnclosedClass);
    syntaxerr!(err_unclosed4, "[!]", ErrorKind::UnclosedClass);
    syntaxerr!(err_range1, "[z-a]", ErrorKind::InvalidRange('z', 'a'));
    syntaxerr!(err_range2, "[z--]", ErrorKind::InvalidRange('z', '-'));

    const CASEI: Options =
        Options { casei: Some(true), litsep: None, bsesc: None, ealtre: None };
    const SLASHLIT: Options =
        Options { casei: None, litsep: Some(true), bsesc: None, ealtre: None };
    const NOBSESC: Options = Options {
        casei: None,
        litsep: None,
        bsesc: Some(false),
        ealtre: None,
    };
    const BSESC: Options =
        Options { casei: None, litsep: None, bsesc: Some(true), ealtre: None };
    const EALTRE: Options = Options {
        casei: None,
        litsep: None,
        bsesc: Some(true),
        ealtre: Some(true),
    };

    toregex!(re_casei, "a", "(?i)^a$", &CASEI);

    toregex!(re_slash1, "?", r"^[^/]$", SLASHLIT);
    toregex!(re_slash2, "*", r"^[^/]*$", SLASHLIT);

    toregex!(re1, "a", "^a$");
    toregex!(re2, "?", "^.$");
    toregex!(re3, "*", "^.*$");
    toregex!(re4, "a?", "^a.$");
    toregex!(re5, "?a", "^.a$");
    toregex!(re6, "a*", "^a.*$");
    toregex!(re7, "*a", "^.*a$");
    toregex!(re8, "[*]", r"^[\*]$");
    toregex!(re9, "[+]", r"^[\+]$");
    toregex!(re10, "+", r"^\+$");
    toregex!(re11, "☃", r"^\xe2\x98\x83$");
    toregex!(re12, "**", r"^.*$");
    toregex!(re13, "**/", r"^.*$");
    toregex!(re14, "**/*", r"^(?:/?|.*/).*$");
    toregex!(re15, "**/**", r"^.*$");
    toregex!(re16, "**/**/*", r"^(?:/?|.*/).*$");
    toregex!(re17, "**/**/**", r"^.*$");
    toregex!(re18, "**/**/**/*", r"^(?:/?|.*/).*$");
    toregex!(re19, "a/**", r"^a/.*$");
    toregex!(re20, "a/**/**", r"^a/.*$");
    toregex!(re21, "a/**/**/**", r"^a/.*$");
    toregex!(re22, "a/**/b", r"^a(?:/|/.*/)b$");
    toregex!(re23, "a/**/**/b", r"^a(?:/|/.*/)b$");
    toregex!(re24, "a/**/**/**/b", r"^a(?:/|/.*/)b$");
    toregex!(re25, "**/b", r"^(?:/?|.*/)b$");
    toregex!(re26, "**/**/b", r"^(?:/?|.*/)b$");
    toregex!(re27, "**/**/**/b", r"^(?:/?|.*/)b$");
    toregex!(re28, "a**", r"^a.*.*$");
    toregex!(re29, "**a", r"^.*.*a$");
    toregex!(re30, "a**b", r"^a.*.*b$");
    toregex!(re31, "***", r"^.*.*.*$");
    toregex!(re32, "/a**", r"^/a.*.*$");
    toregex!(re33, "/**a", r"^/.*.*a$");
    toregex!(re34, "/a**b", r"^/a.*.*b$");
    toregex!(re35, "{a,b}", r"^(?:b|a)$");

    matches!(match1, "a", "a");
    matches!(match2, "a*b", "a_b");
    matches!(match3, "a*b*c", "abc");
    matches!(match4, "a*b*c", "a_b_c");
    matches!(match5, "a*b*c", "a___b___c");
    matches!(match6, "abc*abc*abc", "abcabcabcabcabcabcabc");
    matches!(match7, "a*a*a*a*a*a*a*a*a", "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    matches!(match8, "a*b[xyz]c*d", "abxcdbxcddd");
    matches!(match9, "*.rs", ".rs");
    matches!(match10, "☃", "☃");

    matches!(matchrec1, "some/**/needle.txt", "some/needle.txt");
    matches!(matchrec2, "some/**/needle.txt", "some/one/needle.txt");
    matches!(matchrec3, "some/**/needle.txt", "some/one/two/needle.txt");
    matches!(matchrec4, "some/**/needle.txt", "some/other/needle.txt");
    matches!(matchrec5, "**", "abcde");
    matches!(matchrec6, "**", "");
    matches!(matchrec7, "**", ".asdf");
    matches!(matchrec8, "**", "/x/.asdf");
    matches!(matchrec9, "some/**/**/needle.txt", "some/needle.txt");
    matches!(matchrec10, "some/**/**/needle.txt", "some/one/needle.txt");
    matches!(matchrec11, "some/**/**/needle.txt", "some/one/two/needle.txt");
    matches!(matchrec12, "some/**/**/needle.txt", "some/other/needle.txt");
    matches!(matchrec13, "**/test", "one/two/test");
    matches!(matchrec14, "**/test", "one/test");
    matches!(matchrec15, "**/test", "test");
    matches!(matchrec16, "/**/test", "/one/two/test");
    matches!(matchrec17, "/**/test", "/one/test");
    matches!(matchrec18, "/**/test", "/test");
    matches!(matchrec19, "**/.*", ".abc");
    matches!(matchrec20, "**/.*", "abc/.abc");
    matches!(matchrec21, "**/foo/bar", "foo/bar");
    matches!(matchrec22, ".*/**", ".abc/abc");
    matches!(matchrec23, "test/**", "test/");
    matches!(matchrec24, "test/**", "test/one");
    matches!(matchrec25, "test/**", "test/one/two");
    matches!(matchrec26, "some/*/needle.txt", "some/one/needle.txt");

    matches!(matchrange1, "a[0-9]b", "a0b");
    matches!(matchrange2, "a[0-9]b", "a9b");
    matches!(matchrange3, "a[!0-9]b", "a_b");
    matches!(matchrange4, "[a-z123]", "1");
    matches!(matchrange5, "[1a-z23]", "1");
    matches!(matchrange6, "[123a-z]", "1");
    matches!(matchrange7, "[abc-]", "-");
    matches!(matchrange8, "[-abc]", "-");
    matches!(matchrange9, "[-a-c]", "b");
    matches!(matchrange10, "[a-c-]", "b");
    matches!(matchrange11, "[-]", "-");
    matches!(matchrange12, "a[^0-9]b", "a_b");

    matches!(matchpat1, "*hello.txt", "hello.txt");
    matches!(matchpat2, "*hello.txt", "gareth_says_hello.txt");
    matches!(matchpat3, "*hello.txt", "some/path/to/hello.txt");
    matches!(matchpat4, "*hello.txt", "some\\path\\to\\hello.txt");
    matches!(matchpat5, "*hello.txt", "/an/absolute/path/to/hello.txt");
    matches!(matchpat6, "*some/path/to/hello.txt", "some/path/to/hello.txt");
    matches!(
        matchpat7,
        "*some/path/to/hello.txt",
        "a/bigger/some/path/to/hello.txt"
    );

    matches!(matchescape, "_[[]_[]]_[?]_[*]_!_", "_[_]_?_*_!_");

    matches!(matchcasei1, "aBcDeFg", "aBcDeFg", CASEI);
    matches!(matchcasei2, "aBcDeFg", "abcdefg", CASEI);
    matches!(matchcasei3, "aBcDeFg", "ABCDEFG", CASEI);
    matches!(matchcasei4, "aBcDeFg", "AbCdEfG", CASEI);

    matches!(matchalt1, "a,b", "a,b");
    matches!(matchalt2, ",", ",");
    matches!(matchalt3, "{a,b}", "a");
    matches!(matchalt4, "{a,b}", "b");
    matches!(matchalt5, "{**/src/**,foo}", "abc/src/bar");
    matches!(matchalt6, "{**/src/**,foo}", "foo");
    matches!(matchalt7, "{[}],foo}", "}");
    matches!(matchalt8, "{foo}", "foo");
    matches!(matchalt9, "{}", "");
    matches!(matchalt10, "{,}", "");
    matches!(matchalt11, "{*.foo,*.bar,*.wat}", "test.foo");
    matches!(matchalt12, "{*.foo,*.bar,*.wat}", "test.bar");
    matches!(matchalt13, "{*.foo,*.bar,*.wat}", "test.wat");
    matches!(matchalt14, "foo{,.txt}", "foo.txt");
    nmatches!(matchalt15, "foo{,.txt}", "foo");
    matches!(matchalt16, "foo{,.txt}", "foo", EALTRE);

    matches!(matchslash1, "abc/def", "abc/def", SLASHLIT);
    #[cfg(unix)]
    nmatches!(matchslash2, "abc?def", "abc/def", SLASHLIT);
    #[cfg(not(unix))]
    nmatches!(matchslash2, "abc?def", "abc\\def", SLASHLIT);
    nmatches!(matchslash3, "abc*def", "abc/def", SLASHLIT);
    matches!(matchslash4, "abc[/]def", "abc/def", SLASHLIT); // differs
    #[cfg(unix)]
    nmatches!(matchslash5, "abc\\def", "abc/def", SLASHLIT);
    #[cfg(not(unix))]
    matches!(matchslash5, "abc\\def", "abc/def", SLASHLIT);

    matches!(matchbackslash1, "\\[", "[", BSESC);
    matches!(matchbackslash2, "\\?", "?", BSESC);
    matches!(matchbackslash3, "\\*", "*", BSESC);
    matches!(matchbackslash4, "\\[a-z]", "\\a", NOBSESC);
    matches!(matchbackslash5, "\\?", "\\a", NOBSESC);
    matches!(matchbackslash6, "\\*", "\\\\", NOBSESC);
    #[cfg(unix)]
    matches!(matchbackslash7, "\\a", "a");
    #[cfg(not(unix))]
    matches!(matchbackslash8, "\\a", "/a");

    nmatches!(matchnot1, "a*b*c", "abcd");
    nmatches!(matchnot2, "abc*abc*abc", "abcabcabcabcabcabcabca");
    nmatches!(matchnot3, "some/**/needle.txt", "some/other/notthis.txt");
    nmatches!(matchnot4, "some/**/**/needle.txt", "some/other/notthis.txt");
    nmatches!(matchnot5, "/**/test", "test");
    nmatches!(matchnot6, "/**/test", "/one/notthis");
    nmatches!(matchnot7, "/**/test", "/notthis");
    nmatches!(matchnot8, "**/.*", "ab.c");
    nmatches!(matchnot9, "**/.*", "abc/ab.c");
    nmatches!(matchnot10, ".*/**", "a.bc");
    nmatches!(matchnot11, ".*/**", "abc/a.bc");
    nmatches!(matchnot12, "a[0-9]b", "a_b");
    nmatches!(matchnot13, "a[!0-9]b", "a0b");
    nmatches!(matchnot14, "a[!0-9]b", "a9b");
    nmatches!(matchnot15, "[!-]", "-");
    nmatches!(matchnot16, "*hello.txt", "hello.txt-and-then-some");
    nmatches!(matchnot17, "*hello.txt", "goodbye.txt");
    nmatches!(
        matchnot18,
        "*some/path/to/hello.txt",
        "some/path/to/hello.txt-and-then-some"
    );
    nmatches!(
        matchnot19,
        "*some/path/to/hello.txt",
        "some/other/path/to/hello.txt"
    );
    nmatches!(matchnot20, "a", "foo/a");
    nmatches!(matchnot21, "./foo", "foo");
    nmatches!(matchnot22, "**/foo", "foofoo");
    nmatches!(matchnot23, "**/foo/bar", "foofoo/bar");
    nmatches!(matchnot24, "/*.c", "mozilla-sha1/sha1.c");
    nmatches!(matchnot25, "*.c", "mozilla-sha1/sha1.c", SLASHLIT);
    nmatches!(
        matchnot26,
        "**/m4/ltoptions.m4",
        "csharp/src/packages/repositories.config",
        SLASHLIT
    );
    nmatches!(matchnot27, "a[^0-9]b", "a0b");
    nmatches!(matchnot28, "a[^0-9]b", "a9b");
    nmatches!(matchnot29, "[^-]", "-");
    nmatches!(matchnot30, "some/*/needle.txt", "some/needle.txt");
    nmatches!(
        matchrec31,
        "some/*/needle.txt",
        "some/one/two/needle.txt",
        SLASHLIT
    );
    nmatches!(
        matchrec32,
        "some/*/needle.txt",
        "some/one/two/three/needle.txt",
        SLASHLIT
    );
    nmatches!(matchrec33, ".*/**", ".abc");
    nmatches!(matchrec34, "foo/**", "foo");

    macro_rules! extract {
        ($which:ident, $name:ident, $pat:expr, $expect:expr) => {
            extract!($which, $name, $pat, $expect, Options::default());
        };
        ($which:ident, $name:ident, $pat:expr, $expect:expr, $options:expr) => {
            #[test]
            fn $name() {
                let mut builder = GlobBuilder::new($pat);
                if let Some(casei) = $options.casei {
                    builder.case_insensitive(casei);
                }
                if let Some(litsep) = $options.litsep {
                    builder.literal_separator(litsep);
                }
                if let Some(bsesc) = $options.bsesc {
                    builder.backslash_escape(bsesc);
                }
                if let Some(ealtre) = $options.ealtre {
                    builder.empty_alternates(ealtre);
                }
                let pat = builder.build().unwrap();
                assert_eq!($expect, pat.$which());
            }
        };
    }

    macro_rules! literal {
        ($($tt:tt)*) => { extract!(literal, $($tt)*); }
    }

    macro_rules! basetokens {
        ($($tt:tt)*) => { extract!(basename_tokens, $($tt)*); }
    }

    macro_rules! ext {
        ($($tt:tt)*) => { extract!(ext, $($tt)*); }
    }

    macro_rules! required_ext {
        ($($tt:tt)*) => { extract!(required_ext, $($tt)*); }
    }

    macro_rules! prefix {
        ($($tt:tt)*) => { extract!(prefix, $($tt)*); }
    }

    macro_rules! suffix {
        ($($tt:tt)*) => { extract!(suffix, $($tt)*); }
    }

    macro_rules! baseliteral {
        ($($tt:tt)*) => { extract!(basename_literal, $($tt)*); }
    }

    literal!(extract_lit1, "foo", Some(s("foo")));
    literal!(extract_lit2, "foo", None, CASEI);
    literal!(extract_lit3, "/foo", Some(s("/foo")));
    literal!(extract_lit4, "/foo/", Some(s("/foo/")));
    literal!(extract_lit5, "/foo/bar", Some(s("/foo/bar")));
    literal!(extract_lit6, "*.foo", None);
    literal!(extract_lit7, "foo/bar", Some(s("foo/bar")));
    literal!(extract_lit8, "**/foo/bar", None);

    basetokens!(
        extract_basetoks1,
        "**/foo",
        Some(&*vec![Literal('f'), Literal('o'), Literal('o'),])
    );
    basetokens!(extract_basetoks2, "**/foo", None, CASEI);
    basetokens!(
        extract_basetoks3,
        "**/foo",
        Some(&*vec![Literal('f'), Literal('o'), Literal('o'),]),
        SLASHLIT
    );
    basetokens!(extract_basetoks4, "*foo", None, SLASHLIT);
    basetokens!(extract_basetoks5, "*foo", None);
    basetokens!(extract_basetoks6, "**/fo*o", None);
    basetokens!(
        extract_basetoks7,
        "**/fo*o",
        Some(&*vec![Literal('f'), Literal('o'), ZeroOrMore, Literal('o'),]),
        SLASHLIT
    );

    ext!(extract_ext1, "**/*.rs", Some(s(".rs")));
    ext!(extract_ext2, "**/*.rs.bak", None);
    ext!(extract_ext3, "*.rs", Some(s(".rs")));
    ext!(extract_ext4, "a*.rs", None);
    ext!(extract_ext5, "/*.c", None);
    ext!(extract_ext6, "*.c", None, SLASHLIT);
    ext!(extract_ext7, "*.c", Some(s(".c")));

    required_ext!(extract_req_ext1, "*.rs", Some(s(".rs")));
    required_ext!(extract_req_ext2, "/foo/bar/*.rs", Some(s(".rs")));
    required_ext!(extract_req_ext3, "/foo/bar/*.rs", Some(s(".rs")));
    required_ext!(extract_req_ext4, "/foo/bar/.rs", Some(s(".rs")));
    required_ext!(extract_req_ext5, ".rs", Some(s(".rs")));
    required_ext!(extract_req_ext6, "./rs", None);
    required_ext!(extract_req_ext7, "foo", None);
    required_ext!(extract_req_ext8, ".foo/", None);
    required_ext!(extract_req_ext9, "foo/", None);

    prefix!(extract_prefix1, "/foo", Some(s("/foo")));
    prefix!(extract_prefix2, "/foo/*", Some(s("/foo/")));
    prefix!(extract_prefix3, "**/foo", None);
    prefix!(extract_prefix4, "foo/**", Some(s("foo/")));

    suffix!(extract_suffix1, "**/foo/bar", Some((s("/foo/bar"), true)));
    suffix!(extract_suffix2, "*/foo/bar", Some((s("/foo/bar"), false)));
    suffix!(extract_suffix3, "*/foo/bar", None, SLASHLIT);
    suffix!(extract_suffix4, "foo/bar", Some((s("foo/bar"), false)));
    suffix!(extract_suffix5, "*.foo", Some((s(".foo"), false)));
    suffix!(extract_suffix6, "*.foo", None, SLASHLIT);
    suffix!(extract_suffix7, "**/*_test", Some((s("_test"), false)));

    baseliteral!(extract_baselit1, "**/foo", Some(s("foo")));
    baseliteral!(extract_baselit2, "foo", None);
    baseliteral!(extract_baselit3, "*foo", None);
    baseliteral!(extract_baselit4, "*/foo", None);
}
