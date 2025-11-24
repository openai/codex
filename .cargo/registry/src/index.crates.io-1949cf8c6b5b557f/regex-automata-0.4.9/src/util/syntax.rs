/*!
Utilities for dealing with the syntax of a regular expression.

This module currently only exposes a [`Config`] type that
itself represents a wrapper around the configuration for a
[`regex-syntax::ParserBuilder`](regex_syntax::ParserBuilder). The purpose of
this wrapper is to make configuring syntax options very similar to how other
configuration is done throughout this crate. Namely, instead of duplicating
syntax options across every builder (of which there are many), we instead
create small config objects like this one that can be passed around and
composed.
*/

use alloc::{vec, vec::Vec};

use regex_syntax::{
    ast,
    hir::{self, Hir},
    Error, ParserBuilder,
};

/// A convenience routine for parsing a pattern into an HIR value with the
/// default configuration.
///
/// # Example
///
/// This shows how to parse a pattern into an HIR value:
///
/// ```
/// use regex_automata::util::syntax;
///
/// let hir = syntax::parse(r"([a-z]+)|([0-9]+)")?;
/// assert_eq!(Some(1), hir.properties().static_explicit_captures_len());
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn parse(pattern: &str) -> Result<Hir, Error> {
    parse_with(pattern, &Config::default())
}

/// A convenience routine for parsing many patterns into HIR value with the
/// default configuration.
///
/// # Example
///
/// This shows how to parse many patterns into an corresponding HIR values:
///
/// ```
/// use {
///     regex_automata::util::syntax,
///     regex_syntax::hir::Properties,
/// };
///
/// let hirs = syntax::parse_many(&[
///     r"([a-z]+)|([0-9]+)",
///     r"foo(A-Z]+)bar",
/// ])?;
/// let props = Properties::union(hirs.iter().map(|h| h.properties()));
/// assert_eq!(Some(1), props.static_explicit_captures_len());
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn parse_many<P: AsRef<str>>(patterns: &[P]) -> Result<Vec<Hir>, Error> {
    parse_many_with(patterns, &Config::default())
}

/// A convenience routine for parsing a pattern into an HIR value using a
/// `Config`.
///
/// # Example
///
/// This shows how to parse a pattern into an HIR value with a non-default
/// configuration:
///
/// ```
/// use regex_automata::util::syntax;
///
/// let hir = syntax::parse_with(
///     r"^[a-z]+$",
///     &syntax::Config::new().multi_line(true).crlf(true),
/// )?;
/// assert!(hir.properties().look_set().contains_anchor_crlf());
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn parse_with(pattern: &str, config: &Config) -> Result<Hir, Error> {
    let mut builder = ParserBuilder::new();
    config.apply(&mut builder);
    builder.build().parse(pattern)
}

/// A convenience routine for parsing many patterns into HIR values using a
/// `Config`.
///
/// # Example
///
/// This shows how to parse many patterns into an corresponding HIR values
/// with a non-default configuration:
///
/// ```
/// use {
///     regex_automata::util::syntax,
///     regex_syntax::hir::Properties,
/// };
///
/// let patterns = &[
///     r"([a-z]+)|([0-9]+)",
///     r"\W",
///     r"foo(A-Z]+)bar",
/// ];
/// let config = syntax::Config::new().unicode(false).utf8(false);
/// let hirs = syntax::parse_many_with(patterns, &config)?;
/// let props = Properties::union(hirs.iter().map(|h| h.properties()));
/// assert!(!props.is_utf8());
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn parse_many_with<P: AsRef<str>>(
    patterns: &[P],
    config: &Config,
) -> Result<Vec<Hir>, Error> {
    let mut builder = ParserBuilder::new();
    config.apply(&mut builder);
    let mut hirs = vec![];
    for p in patterns.iter() {
        hirs.push(builder.build().parse(p.as_ref())?);
    }
    Ok(hirs)
}

/// A common set of configuration options that apply to the syntax of a regex.
///
/// This represents a group of configuration options that specifically apply
/// to how the concrete syntax of a regular expression is interpreted. In
/// particular, they are generally forwarded to the
/// [`ParserBuilder`](https://docs.rs/regex-syntax/*/regex_syntax/struct.ParserBuilder.html)
/// in the
/// [`regex-syntax`](https://docs.rs/regex-syntax)
/// crate when building a regex from its concrete syntax directly.
///
/// These options are defined as a group since they apply to every regex engine
/// in this crate. Instead of re-defining them on every engine's builder, they
/// are instead provided here as one cohesive unit.
#[derive(Clone, Copy, Debug)]
pub struct Config {
    case_insensitive: bool,
    multi_line: bool,
    dot_matches_new_line: bool,
    crlf: bool,
    line_terminator: u8,
    swap_greed: bool,
    ignore_whitespace: bool,
    unicode: bool,
    utf8: bool,
    nest_limit: u32,
    octal: bool,
}

impl Config {
    /// Return a new default syntax configuration.
    pub fn new() -> Config {
        // These defaults match the ones used in regex-syntax.
        Config {
            case_insensitive: false,
            multi_line: false,
            dot_matches_new_line: false,
            crlf: false,
            line_terminator: b'\n',
            swap_greed: false,
            ignore_whitespace: false,
            unicode: true,
            utf8: true,
            nest_limit: 250,
            octal: false,
        }
    }

    /// Enable or disable the case insensitive flag by default.
    ///
    /// When Unicode mode is enabled, case insensitivity is Unicode-aware.
    /// Specifically, it will apply the "simple" case folding rules as
    /// specified by Unicode.
    ///
    /// By default this is disabled. It may alternatively be selectively
    /// enabled in the regular expression itself via the `i` flag.
    pub fn case_insensitive(mut self, yes: bool) -> Config {
        self.case_insensitive = yes;
        self
    }

    /// Enable or disable the multi-line matching flag by default.
    ///
    /// When this is enabled, the `^` and `$` look-around assertions will
    /// match immediately after and immediately before a new line character,
    /// respectively. Note that the `\A` and `\z` look-around assertions are
    /// unaffected by this setting and always correspond to matching at the
    /// beginning and end of the input.
    ///
    /// By default this is disabled. It may alternatively be selectively
    /// enabled in the regular expression itself via the `m` flag.
    pub fn multi_line(mut self, yes: bool) -> Config {
        self.multi_line = yes;
        self
    }

    /// Enable or disable the "dot matches any character" flag by default.
    ///
    /// When this is enabled, `.` will match any character. When it's disabled,
    /// then `.` will match any character except for a new line character.
    ///
    /// Note that `.` is impacted by whether the "unicode" setting is enabled
    /// or not. When Unicode is enabled (the default), `.` will match any UTF-8
    /// encoding of any Unicode scalar value (sans a new line, depending on
    /// whether this "dot matches new line" option is enabled). When Unicode
    /// mode is disabled, `.` will match any byte instead. Because of this,
    /// when Unicode mode is disabled, `.` can only be used when the "allow
    /// invalid UTF-8" option is enabled, since `.` could otherwise match
    /// invalid UTF-8.
    ///
    /// By default this is disabled. It may alternatively be selectively
    /// enabled in the regular expression itself via the `s` flag.
    pub fn dot_matches_new_line(mut self, yes: bool) -> Config {
        self.dot_matches_new_line = yes;
        self
    }

    /// Enable or disable the "CRLF mode" flag by default.
    ///
    /// By default this is disabled. It may alternatively be selectively
    /// enabled in the regular expression itself via the `R` flag.
    ///
    /// When CRLF mode is enabled, the following happens:
    ///
    /// * Unless `dot_matches_new_line` is enabled, `.` will match any character
    /// except for `\r` and `\n`.
    /// * When `multi_line` mode is enabled, `^` and `$` will treat `\r\n`,
    /// `\r` and `\n` as line terminators. And in particular, neither will
    /// match between a `\r` and a `\n`.
    pub fn crlf(mut self, yes: bool) -> Config {
        self.crlf = yes;
        self
    }

    /// Sets the line terminator for use with `(?u-s:.)` and `(?-us:.)`.
    ///
    /// Namely, instead of `.` (by default) matching everything except for `\n`,
    /// this will cause `.` to match everything except for the byte given.
    ///
    /// If `.` is used in a context where Unicode mode is enabled and this byte
    /// isn't ASCII, then an error will be returned. When Unicode mode is
    /// disabled, then any byte is permitted, but will return an error if UTF-8
    /// mode is enabled and it is a non-ASCII byte.
    ///
    /// In short, any ASCII value for a line terminator is always okay. But a
    /// non-ASCII byte might result in an error depending on whether Unicode
    /// mode or UTF-8 mode are enabled.
    ///
    /// Note that if `R` mode is enabled then it always takes precedence and
    /// the line terminator will be treated as `\r` and `\n` simultaneously.
    ///
    /// Note also that this *doesn't* impact the look-around assertions
    /// `(?m:^)` and `(?m:$)`. That's usually controlled by additional
    /// configuration in the regex engine itself.
    pub fn line_terminator(mut self, byte: u8) -> Config {
        self.line_terminator = byte;
        self
    }

    /// Enable or disable the "swap greed" flag by default.
    ///
    /// When this is enabled, `.*` (for example) will become ungreedy and `.*?`
    /// will become greedy.
    ///
    /// By default this is disabled. It may alternatively be selectively
    /// enabled in the regular expression itself via the `U` flag.
    pub fn swap_greed(mut self, yes: bool) -> Config {
        self.swap_greed = yes;
        self
    }

    /// Enable verbose mode in the regular expression.
    ///
    /// When enabled, verbose mode permits insigificant whitespace in many
    /// places in the regular expression, as well as comments. Comments are
    /// started using `#` and continue until the end of the line.
    ///
    /// By default, this is disabled. It may be selectively enabled in the
    /// regular expression by using the `x` flag regardless of this setting.
    pub fn ignore_whitespace(mut self, yes: bool) -> Config {
        self.ignore_whitespace = yes;
        self
    }

    /// Enable or disable the Unicode flag (`u`) by default.
    ///
    /// By default this is **enabled**. It may alternatively be selectively
    /// disabled in the regular expression itself via the `u` flag.
    ///
    /// Note that unless "allow invalid UTF-8" is enabled (it's disabled by
    /// default), a regular expression will fail to parse if Unicode mode is
    /// disabled and a sub-expression could possibly match invalid UTF-8.
    ///
    /// **WARNING**: Unicode mode can greatly increase the size of the compiled
    /// DFA, which can noticeably impact both memory usage and compilation
    /// time. This is especially noticeable if your regex contains character
    /// classes like `\w` that are impacted by whether Unicode is enabled or
    /// not. If Unicode is not necessary, you are encouraged to disable it.
    pub fn unicode(mut self, yes: bool) -> Config {
        self.unicode = yes;
        self
    }

    /// When disabled, the builder will permit the construction of a regular
    /// expression that may match invalid UTF-8.
    ///
    /// For example, when [`Config::unicode`] is disabled, then
    /// expressions like `[^a]` may match invalid UTF-8 since they can match
    /// any single byte that is not `a`. By default, these sub-expressions
    /// are disallowed to avoid returning offsets that split a UTF-8
    /// encoded codepoint. However, in cases where matching at arbitrary
    /// locations is desired, this option can be disabled to permit all such
    /// sub-expressions.
    ///
    /// When enabled (the default), the builder is guaranteed to produce a
    /// regex that will only ever match valid UTF-8 (otherwise, the builder
    /// will return an error).
    pub fn utf8(mut self, yes: bool) -> Config {
        self.utf8 = yes;
        self
    }

    /// Set the nesting limit used for the regular expression parser.
    ///
    /// The nesting limit controls how deep the abstract syntax tree is allowed
    /// to be. If the AST exceeds the given limit (e.g., with too many nested
    /// groups), then an error is returned by the parser.
    ///
    /// The purpose of this limit is to act as a heuristic to prevent stack
    /// overflow when building a finite automaton from a regular expression's
    /// abstract syntax tree. In particular, construction currently uses
    /// recursion. In the future, the implementation may stop using recursion
    /// and this option will no longer be necessary.
    ///
    /// This limit is not checked until the entire AST is parsed. Therefore,
    /// if callers want to put a limit on the amount of heap space used, then
    /// they should impose a limit on the length, in bytes, of the concrete
    /// pattern string. In particular, this is viable since the parser will
    /// limit itself to heap space proportional to the length of the pattern
    /// string.
    ///
    /// Note that a nest limit of `0` will return a nest limit error for most
    /// patterns but not all. For example, a nest limit of `0` permits `a` but
    /// not `ab`, since `ab` requires a concatenation AST item, which results
    /// in a nest depth of `1`. In general, a nest limit is not something that
    /// manifests in an obvious way in the concrete syntax, therefore, it
    /// should not be used in a granular way.
    pub fn nest_limit(mut self, limit: u32) -> Config {
        self.nest_limit = limit;
        self
    }

    /// Whether to support octal syntax or not.
    ///
    /// Octal syntax is a little-known way of uttering Unicode codepoints in
    /// a regular expression. For example, `a`, `\x61`, `\u0061` and
    /// `\141` are all equivalent regular expressions, where the last example
    /// shows octal syntax.
    ///
    /// While supporting octal syntax isn't in and of itself a problem, it does
    /// make good error messages harder. That is, in PCRE based regex engines,
    /// syntax like `\1` invokes a backreference, which is explicitly
    /// unsupported in Rust's regex engine. However, many users expect it to
    /// be supported. Therefore, when octal support is disabled, the error
    /// message will explicitly mention that backreferences aren't supported.
    ///
    /// Octal syntax is disabled by default.
    pub fn octal(mut self, yes: bool) -> Config {
        self.octal = yes;
        self
    }

    /// Returns whether "unicode" mode is enabled.
    pub fn get_unicode(&self) -> bool {
        self.unicode
    }

    /// Returns whether "case insensitive" mode is enabled.
    pub fn get_case_insensitive(&self) -> bool {
        self.case_insensitive
    }

    /// Returns whether "multi line" mode is enabled.
    pub fn get_multi_line(&self) -> bool {
        self.multi_line
    }

    /// Returns whether "dot matches new line" mode is enabled.
    pub fn get_dot_matches_new_line(&self) -> bool {
        self.dot_matches_new_line
    }

    /// Returns whether "CRLF" mode is enabled.
    pub fn get_crlf(&self) -> bool {
        self.crlf
    }

    /// Returns the line terminator in this syntax configuration.
    pub fn get_line_terminator(&self) -> u8 {
        self.line_terminator
    }

    /// Returns whether "swap greed" mode is enabled.
    pub fn get_swap_greed(&self) -> bool {
        self.swap_greed
    }

    /// Returns whether "ignore whitespace" mode is enabled.
    pub fn get_ignore_whitespace(&self) -> bool {
        self.ignore_whitespace
    }

    /// Returns whether UTF-8 mode is enabled.
    pub fn get_utf8(&self) -> bool {
        self.utf8
    }

    /// Returns the "nest limit" setting.
    pub fn get_nest_limit(&self) -> u32 {
        self.nest_limit
    }

    /// Returns whether "octal" mode is enabled.
    pub fn get_octal(&self) -> bool {
        self.octal
    }

    /// Applies this configuration to the given parser.
    pub(crate) fn apply(&self, builder: &mut ParserBuilder) {
        builder
            .unicode(self.unicode)
            .case_insensitive(self.case_insensitive)
            .multi_line(self.multi_line)
            .dot_matches_new_line(self.dot_matches_new_line)
            .crlf(self.crlf)
            .line_terminator(self.line_terminator)
            .swap_greed(self.swap_greed)
            .ignore_whitespace(self.ignore_whitespace)
            .utf8(self.utf8)
            .nest_limit(self.nest_limit)
            .octal(self.octal);
    }

    /// Applies this configuration to the given AST parser.
    pub(crate) fn apply_ast(&self, builder: &mut ast::parse::ParserBuilder) {
        builder
            .ignore_whitespace(self.ignore_whitespace)
            .nest_limit(self.nest_limit)
            .octal(self.octal);
    }

    /// Applies this configuration to the given AST-to-HIR translator.
    pub(crate) fn apply_hir(
        &self,
        builder: &mut hir::translate::TranslatorBuilder,
    ) {
        builder
            .unicode(self.unicode)
            .case_insensitive(self.case_insensitive)
            .multi_line(self.multi_line)
            .crlf(self.crlf)
            .dot_matches_new_line(self.dot_matches_new_line)
            .line_terminator(self.line_terminator)
            .swap_greed(self.swap_greed)
            .utf8(self.utf8);
    }
}

impl Default for Config {
    fn default() -> Config {
        Config::new()
    }
}
