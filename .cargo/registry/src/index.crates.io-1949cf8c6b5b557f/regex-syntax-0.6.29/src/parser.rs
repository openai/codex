use crate::ast;
use crate::hir;

use crate::Result;

/// A builder for a regular expression parser.
///
/// This builder permits modifying configuration options for the parser.
///
/// This type combines the builder options for both the
/// [AST `ParserBuilder`](ast/parse/struct.ParserBuilder.html)
/// and the
/// [HIR `TranslatorBuilder`](hir/translate/struct.TranslatorBuilder.html).
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

    /// When enabled, the parser will permit the construction of a regular
    /// expression that may match invalid UTF-8.
    ///
    /// When disabled (the default), the parser is guaranteed to produce
    /// an expression that will only ever match valid UTF-8 (otherwise, the
    /// parser will return an error).
    ///
    /// Perhaps surprisingly, when invalid UTF-8 isn't allowed, a negated ASCII
    /// word boundary (uttered as `(?-u:\B)` in the concrete syntax) will cause
    /// the parser to return an error. Namely, a negated ASCII word boundary
    /// can result in matching positions that aren't valid UTF-8 boundaries.
    pub fn allow_invalid_utf8(&mut self, yes: bool) -> &mut ParserBuilder {
        self.hir.allow_invalid_utf8(yes);
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
    /// Note that unless `allow_invalid_utf8` is enabled (it's disabled by
    /// default), a regular expression will fail to parse if Unicode mode is
    /// disabled and a sub-expression could possibly match invalid UTF-8.
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
/// see the [`ast::parse`](ast/parse/index.html) module.
///
/// A `Parser` can be configured in more detail via a
/// [`ParserBuilder`](struct.ParserBuilder.html).
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
    /// To set configuration options on the parser, use
    /// [`ParserBuilder`](struct.ParserBuilder.html).
    pub fn new() -> Parser {
        ParserBuilder::new().build()
    }

    /// Parse the regular expression into a high level intermediate
    /// representation.
    pub fn parse(&mut self, pattern: &str) -> Result<hir::Hir> {
        let ast = self.ast.parse(pattern)?;
        let hir = self.hir.translate(pattern, &ast)?;
        Ok(hir)
    }
}
