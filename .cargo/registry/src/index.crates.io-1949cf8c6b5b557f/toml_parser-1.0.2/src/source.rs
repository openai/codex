use crate::decoder::Encoding;
use crate::decoder::StringBuilder;
use crate::lexer::Lexer;
use crate::ErrorSink;
use crate::Expected;

/// Data encoded as TOML
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Source<'i> {
    input: &'i str,
}

impl<'i> Source<'i> {
    pub fn new(input: &'i str) -> Self {
        Self { input }
    }

    /// Start lexing the TOML encoded data
    pub fn lex(&self) -> Lexer<'i> {
        Lexer::new(self.input)
    }

    /// Access the TOML encoded `&str`
    pub fn input(&self) -> &'i str {
        self.input
    }

    /// Return a subslice of the input
    pub fn get(&self, span: impl SourceIndex) -> Option<Raw<'i>> {
        span.get(self)
    }

    /// Return an unchecked subslice of the input
    ///
    /// ## Safety
    ///
    /// Callers of this function are responsible that these preconditions are satisfied:
    /// - The starting index must not exceed the ending index;
    /// - Indexes must be within bounds of the original slice;
    /// - Indexes must lie on UTF-8 sequence boundaries.
    ///
    /// Or one of:
    /// - `span` came from [`Source::lex`]
    ///
    /// Failing any of those, the returned string slice may reference invalid memory or violate the invariants communicated by `str` type.
    #[cfg(feature = "unsafe")]
    pub unsafe fn get_unchecked(&self, span: impl SourceIndex) -> Raw<'i> {
        // SAFETY: Same safety guarantees are required
        unsafe { span.get_unchecked(self) }
    }

    /// Return a subslice of the input
    fn get_raw_str(&self, span: Span) -> Option<&'i str> {
        let index = span.start()..span.end();
        self.input.get(index)
    }

    /// Return an unchecked subslice of the input
    ///
    /// ## Safety
    ///
    /// Callers of this function are responsible that these preconditions are satisfied:
    /// - The starting index must not exceed the ending index;
    /// - Indexes must be within bounds of the original slice;
    /// - Indexes must lie on UTF-8 sequence boundaries.
    ///
    /// Or one of:
    /// - `span` came from [`Source::lex`]
    ///
    /// Failing any of those, the returned string slice may reference invalid memory or violate the invariants communicated by `str` type.
    #[cfg(feature = "unsafe")]
    unsafe fn get_raw_str_unchecked(&self, span: Span) -> &'i str {
        let index = span.start()..span.end();
        // SAFETY: Same safety guarantees are required
        unsafe { self.input.get_unchecked(index) }
    }
}

/// A slice of [`Source`]
#[derive(Copy, Clone, Debug)]
pub struct Raw<'i> {
    raw: &'i str,
    encoding: Option<Encoding>,
    span: Span,
}

impl<'i> Raw<'i> {
    pub fn new_unchecked(raw: &'i str, encoding: Option<Encoding>, span: Span) -> Self {
        Self {
            raw,
            encoding,
            span,
        }
    }

    pub fn decode_key(&self, output: &mut dyn StringBuilder<'i>, error: &mut dyn ErrorSink) {
        let mut error = |err: crate::ParseError| {
            error.report_error(err.rebase_spans(self.span.start));
        };
        match self.encoding {
            Some(Encoding::LiteralString) => {
                crate::decoder::string::decode_literal_string(*self, output, &mut error);
            }
            Some(Encoding::BasicString) => {
                crate::decoder::string::decode_basic_string(*self, output, &mut error);
            }
            Some(Encoding::MlLiteralString) => {
                error.report_error(
                    crate::ParseError::new("keys cannot be multi-line literal strings")
                        .with_expected(&[
                            Expected::Description("basic string"),
                            Expected::Description("literal string"),
                        ])
                        .with_unexpected(Span::new_unchecked(0, self.len())),
                );
                crate::decoder::string::decode_ml_literal_string(*self, output, &mut error);
            }
            Some(Encoding::MlBasicString) => {
                error.report_error(
                    crate::ParseError::new("keys cannot be multi-line basic strings")
                        .with_expected(&[
                            Expected::Description("basic string"),
                            Expected::Description("literal string"),
                        ])
                        .with_unexpected(Span::new_unchecked(0, self.len())),
                );
                crate::decoder::string::decode_ml_basic_string(*self, output, &mut error);
            }
            None => crate::decoder::string::decode_unquoted_key(*self, output, &mut error),
        }
    }

    #[must_use]
    pub fn decode_scalar(
        &self,
        output: &mut dyn StringBuilder<'i>,
        error: &mut dyn ErrorSink,
    ) -> crate::decoder::scalar::ScalarKind {
        let mut error = |err: crate::ParseError| {
            error.report_error(err.rebase_spans(self.span.start));
        };
        match self.encoding {
            Some(Encoding::LiteralString) => {
                crate::decoder::string::decode_literal_string(*self, output, &mut error);
                crate::decoder::scalar::ScalarKind::String
            }
            Some(Encoding::BasicString) => {
                crate::decoder::string::decode_basic_string(*self, output, &mut error);
                crate::decoder::scalar::ScalarKind::String
            }
            Some(Encoding::MlLiteralString) => {
                crate::decoder::string::decode_ml_literal_string(*self, output, &mut error);
                crate::decoder::scalar::ScalarKind::String
            }
            Some(Encoding::MlBasicString) => {
                crate::decoder::string::decode_ml_basic_string(*self, output, &mut error);
                crate::decoder::scalar::ScalarKind::String
            }
            None => crate::decoder::scalar::decode_unquoted_scalar(*self, output, &mut error),
        }
    }

    pub fn decode_whitespace(&self, _error: &mut dyn ErrorSink) {
        // whitespace is always valid
    }

    pub fn decode_comment(&self, error: &mut dyn ErrorSink) {
        let mut error = |err: crate::ParseError| {
            error.report_error(err.rebase_spans(self.span.start));
        };
        crate::decoder::ws::decode_comment(*self, &mut error);
    }

    pub fn decode_newline(&self, error: &mut dyn ErrorSink) {
        let mut error = |err: crate::ParseError| {
            error.report_error(err.rebase_spans(self.span.start));
        };
        crate::decoder::ws::decode_newline(*self, &mut error);
    }

    pub fn as_str(&self) -> &'i str {
        self.raw
    }

    pub fn as_bytes(&self) -> &'i [u8] {
        self.raw.as_bytes()
    }

    pub fn len(&self) -> usize {
        self.raw.len()
    }

    pub fn is_empty(&self) -> bool {
        self.raw.is_empty()
    }
}

/// Location within the [`Source`]
#[derive(Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Span {
    start: usize,
    end: usize,
}

impl Span {
    pub fn new_unchecked(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn is_empty(&self) -> bool {
        self.end <= self.start
    }

    pub fn len(&self) -> usize {
        self.end - self.start
    }

    pub fn start(&self) -> usize {
        self.start
    }

    pub fn end(&self) -> usize {
        self.end
    }

    pub fn before(&self) -> Self {
        Self::new_unchecked(self.start, self.start)
    }

    pub fn after(&self) -> Self {
        Self::new_unchecked(self.end, self.end)
    }

    /// Extend this `Raw` to the end of `after`
    #[must_use]
    pub fn append(&self, after: Self) -> Self {
        Self::new_unchecked(self.start, after.end)
    }
}

impl core::fmt::Debug for Span {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        (self.start..self.end).fmt(f)
    }
}

impl core::ops::Add<usize> for Span {
    type Output = Self;

    fn add(self, offset: usize) -> Self::Output {
        Self::Output {
            start: self.start + offset,
            end: self.end + offset,
        }
    }
}

impl core::ops::Add<Span> for usize {
    type Output = Span;

    fn add(self, span: Span) -> Self::Output {
        Self::Output {
            start: span.start + self,
            end: span.end + self,
        }
    }
}

impl core::ops::AddAssign<usize> for Span {
    fn add_assign(&mut self, rhs: usize) {
        self.start += rhs;
        self.end += rhs;
    }
}

/// A helper trait used for indexing operations on [`Source`]
pub trait SourceIndex: sealed::Sealed {
    /// Return a subslice of the input
    fn get<'i>(self, source: &Source<'i>) -> Option<Raw<'i>>;

    /// Return an unchecked subslice of the input
    ///
    /// ## Safety
    ///
    /// Callers of this function are responsible that these preconditions are satisfied:
    /// - The starting index must not exceed the ending index;
    /// - Indexes must be within bounds of the original slice;
    /// - Indexes must lie on UTF-8 sequence boundaries.
    ///
    /// Or one of:
    /// - `span` came from [`Source::lex`]
    ///
    /// Failing any of those, the returned string slice may reference invalid memory or violate the invariants communicated by `str` type.
    #[cfg(feature = "unsafe")]
    unsafe fn get_unchecked<'i>(self, source: &Source<'i>) -> Raw<'i>;
}

impl SourceIndex for Span {
    fn get<'i>(self, source: &Source<'i>) -> Option<Raw<'i>> {
        (&self).get(source)
    }

    #[cfg(feature = "unsafe")]
    unsafe fn get_unchecked<'i>(self, source: &Source<'i>) -> Raw<'i> {
        // SAFETY: Same safety guarantees are required
        unsafe { (&self).get_unchecked(source) }
    }
}

impl SourceIndex for &Span {
    fn get<'i>(self, source: &Source<'i>) -> Option<Raw<'i>> {
        let encoding = None;
        source
            .get_raw_str(*self)
            .map(|s| Raw::new_unchecked(s, encoding, *self))
    }

    #[cfg(feature = "unsafe")]
    unsafe fn get_unchecked<'i>(self, source: &Source<'i>) -> Raw<'i> {
        let encoding = None;
        // SAFETY: Same safety guarantees are required
        let raw = unsafe { source.get_raw_str_unchecked(*self) };
        Raw::new_unchecked(raw, encoding, *self)
    }
}

impl SourceIndex for crate::lexer::Token {
    fn get<'i>(self, source: &Source<'i>) -> Option<Raw<'i>> {
        (&self).get(source)
    }

    #[cfg(feature = "unsafe")]
    unsafe fn get_unchecked<'i>(self, source: &Source<'i>) -> Raw<'i> {
        // SAFETY: Same safety guarantees are required
        unsafe { (&self).get_unchecked(source) }
    }
}

impl SourceIndex for &crate::lexer::Token {
    fn get<'i>(self, source: &Source<'i>) -> Option<Raw<'i>> {
        let encoding = self.kind().encoding();
        source
            .get_raw_str(self.span())
            .map(|s| Raw::new_unchecked(s, encoding, self.span()))
    }

    #[cfg(feature = "unsafe")]
    unsafe fn get_unchecked<'i>(self, source: &Source<'i>) -> Raw<'i> {
        let encoding = self.kind().encoding();
        // SAFETY: Same safety guarantees are required
        let raw = unsafe { source.get_raw_str_unchecked(self.span()) };
        Raw::new_unchecked(raw, encoding, self.span())
    }
}

impl SourceIndex for crate::parser::Event {
    fn get<'i>(self, source: &Source<'i>) -> Option<Raw<'i>> {
        (&self).get(source)
    }

    #[cfg(feature = "unsafe")]
    unsafe fn get_unchecked<'i>(self, source: &Source<'i>) -> Raw<'i> {
        // SAFETY: Same safety guarantees are required
        unsafe { (&self).get_unchecked(source) }
    }
}

impl SourceIndex for &crate::parser::Event {
    fn get<'i>(self, source: &Source<'i>) -> Option<Raw<'i>> {
        let encoding = self.encoding();
        source
            .get_raw_str(self.span())
            .map(|s| Raw::new_unchecked(s, encoding, self.span()))
    }

    #[cfg(feature = "unsafe")]
    unsafe fn get_unchecked<'i>(self, source: &Source<'i>) -> Raw<'i> {
        let encoding = self.encoding();
        // SAFETY: Same safety guarantees are required
        let raw = unsafe { source.get_raw_str_unchecked(self.span()) };
        Raw::new_unchecked(raw, encoding, self.span())
    }
}

mod sealed {
    pub trait Sealed {}

    impl Sealed for crate::Span {}
    impl Sealed for &crate::Span {}
    impl Sealed for crate::lexer::Token {}
    impl Sealed for &crate::lexer::Token {}
    impl Sealed for crate::parser::Event {}
    impl Sealed for &crate::parser::Event {}
}
