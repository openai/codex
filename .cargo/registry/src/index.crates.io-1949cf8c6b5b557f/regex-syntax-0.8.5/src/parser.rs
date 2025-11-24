use crate::{ast, hir, Error};

/// A convenience routine for parsing a regex using default options.
///
/// This is equivalent to `Parser::new().parse(pattern)`.
///
/// If you need to set non-default options, then use a [`ParserBuilder`].
///
/// This routine returns an [`Hir`](hir::Hir) value. Namely, it automatically
/// parses the pattern as an [`Ast`](ast::Ast) and then invokes the translator
/// to convert the `Ast` into an `Hir`. If you need access to the `Ast`, then
/// you should use a [`ast::parse::Parser`].
pub fn parse(pattern: &str) -> Result<hir::Hir, Error> {
    Parser::new().parse(pattern)
}

/// A builder for a regular expression parser.
///
/// This builder permits modifying configuration options for the parser.
///
/// This type combines the builder options for both the [AST
/// `ParserBuilder`](ast::parse::ParserBuilder) and the [HIR
/// `TranslatorBuilder`](hir::translate::TranslatorBuilder).
#[derive(Clone, Debug, Default)]
pub struct ParserBuilder {
    ast: ast::parse::ParserBuilder,
    hir: hir::translate::TranslatorBuilder,
}

impl ParserBuilder {
    /// Create a new parser builder with a default configuration.
    pub fn new() -> ParserBuilder {
        ParserBuilder::default()
    }

    /// Build a parser from this configuration with the given pattern.
    pub fn build(&self) -> Parser {
        Parser { ast: self.ast.build(), hir: self.hir.build() }
    }

    /// Set the nesting limit for this parser.
    ///
    /// The nesting limit controls how deep the abstract syntax tree is allowed
    /// to be. If the AST exceeds the given limit (e.g., with too many nested
    /// groups), then an error is returned by the parser.
    ///
    /// The purpose of this limit is to act as a heuristic to prevent stack
    /// overflow for consumers that do structural induction on an `Ast` using
    /// explicit recursion. While this crate never does this (instead using
    /// constant stack space and moving the call stack to the heap), other
    /// crates may.
    ///
    /// This limit is not checked until the entire Ast is parsed. Therefore,
    /// if callers want to put a limit on the amount of heap space used, then
    /// they should impose a limit on the length, in bytes, of the concrete
    /// pattern string. In particular, this is viable since this parser
    /// implementation will limit itself to heap space proportional to the
    /// length of the pattern string.
    ///
    /// Note that a nest limit of `0` will return a nest limit error for most
    /// patterns but not all. For example, a nest limit of `0` permits `a` but
    /// not `ab`, since `ab` requires a concatenation, which results in a nest
    /// depth of `1`. In general, a nest limit is not something that manifests
    /// in an obvious way in the concrete syntax, therefore, it should not be
    /// used in a granular way.
    pub fn nest_limit(&mut self, limit: u32) -> &mut ParserBuilder {
        self.ast.nest_limit(limit);
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
    /// syntax like `\0` invokes a backreference, which is explicitly
    /// unsupported in Rust's regex engine. However, many users expect it to
    /// be supported. Therefore, when octal support is disabled, the error
    /// message will explicitly mention that backreferences aren't supported.
    ///
    /// Octal syntax is disabled by default.
    pub fn octal(&mut self, yes: bool) -> &mut ParserBuilder {
        self.ast.octal(yes);
        self
    }

    /// When disabled, translation will permit the construction of a regular
    /// expression that may match invalid UTF-8.
    ///
    /// When enabled (the default), the translator is guaranteed to produce an
    /// expression that, for non-empty matches, will only ever produce spans
    /// that are entirely valid UTF-8 (otherwise, the translator will return an
    /// error).
    ///
    /// Perhaps surprisingly, when UTF-8 is enabled, an empty regex or even
    /// a negated ASCII word boundary (uttered as `(?-u:\B)` in the concrete
    /// syntax) will be allowed even though they can produce matches that split
    /// a UTF-8 encoded codepoint. This only applies to zero-width or "empty"
    /// matches, and it is expected that the regex engine itself must handle
    /// these cases if necessary (perhaps by suppressing any zero-width matches
    /// that split a codepoint).
    pub fn utf8(&mut self, yes: bool) -> &mut ParserBuilder {
        self.hir.utf8(yes);
        self
    }

    /// Enable verbose mode in the regular expression.
    ///
    /// When enabled, verbose mode permits insignificant whitespace in many
    /// places in the regular expression, as well as comments. Comments are
    /// started using `#` and continue until the end of the line.
    ///
    /// By default, this is disabled. It may be selectively enabled in the
    /// regular expression by using the `x` flag regardless of this setting.
    pub fn ignore_whitespace(&mut self, yes: bool) -> &mut ParserBuilder {
        self.ast.ignore_whitespace(yes);
        self
    }

    /// Enable or disable the case insensitive flag by default.
    ///
    /// By default this is disabled. It may alternatively be selectively
    /// enabled in the regular expression itself via the `i` flag.
    pub fn case_insensitive(&mut self, yes: bool) -> &mut ParserBuilder {
        self.hir.case_insensitive(yes);
        self
    }

    /// Enable or disable the multi-line matching flag by default.
    ///
    /// By default this is disabled. It may alternatively be selectively
    /// enabled in the regular expression itself via the `m` flag.
    pub fn multi_line(&mut self, yes: bool) -> &mut ParserBuilder {
        self.hir.multi_line(yes);
        self
    }

    /// Enable or disable the "dot matches any character" flag by default.
    ///
    /// By default this is disabled. It may alternatively be selectively
    /// enabled in the regular expression itself via the `s` flag.
    pub fn dot_matches_new_line(&mut self, yes: bool) -> &mut ParserBuilder {
        self.hir.dot_matches_new_line(yes);
        self
    }

    /// Enable or disable the CRLF mode flag by default.
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
    pub fn crlf(&mut self, yes: bool) -> &mut ParserBuilder {
        self.hir.crlf(yes);
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
    pub fn line_terminator(&mut self, byte: u8) -> &mut ParserBuilder {
        self.hir.line_terminator(byte);
        self
    }

    /// Enable or disable the "swap greed" flag by default.
    ///
    /// By default this is disabled. It may alternatively be selectively
    /// enabled in the regular expression itself via the `U` flag.
    pub fn swap_greed(&mut self, yes: bool) -> &mut ParserBuilder {
        self.hir.swap_greed(yes);
        self
    }

    /// Enable or disable the Unicode flag (`u`) by default.
    ///
    /// By default this is **enabled**. It may alternatively be selectively
    /// disabled in the regular expression itself via the `u` flag.
    ///
    /// Note that unless `utf8` is disabled (it's enabled by default), a
    /// regular expression will fail to parse if Unicode mode is disabled and a
    /// sub-expression could possibly match invalid UTF-8.
    pub fn unicode(&mut self, yes: bool) -> &mut ParserBuilder {
        self.hir.unicode(yes);
        self
    }
}

/// A convenience parser for regular expressions.
///
/// This parser takes as input a regular expression pattern string (the
/// "concrete syntax") and returns a high-level intermediate representation
/// (the HIR) suitable for most types of analysis. In particular, this parser
/// hides the intermediate state of producing an AST (the "abstract syntax").
/// The AST is itself far more complex than the HIR, so this parser serves as a
/// convenience for never having to deal with it at all.
///
/// If callers have more fine grained use cases that need an AST, then please
/// see the [`ast::parse`] module.
///
/// A `Parser` can be configured in more detail via a [`ParserBuilder`].
#[derive(Clone, Debug)]
pub struct Parser {
    ast: ast::parse::Parser,
    hir: hir::translate::Translator,
}

impl Parser {
    /// Create a new parser with a default configuration.
    ///
    /// The parser can be run with `parse` method. The parse method returns
    /// a high level intermediate representation of the given regular
    /// expression.
    ///
    /// To set configuration options on the parser, use [`ParserBuilder`].
    pub fn new() -> Parser {
        ParserBuilder::new().build()
    }

    /// Parse the regular expression into a high level intermediate
    /// representation.
    pub fn parse(&mut self, pattern: &str) -> Result<hir::Hir, Error> {
        let ast = self.ast.parse(pattern)?;
        let hir = self.hir.translate(pattern, &ast)?;
        Ok(hir)
    }
}
