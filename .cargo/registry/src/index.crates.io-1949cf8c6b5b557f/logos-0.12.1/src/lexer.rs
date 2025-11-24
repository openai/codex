use super::internal::LexerInternal;
use super::Logos;
use crate::source::{self, Source};

use core::fmt::{self, Debug};
use core::mem::ManuallyDrop;

/// Byte range in the source.
pub type Span = core::ops::Range<usize>;

/// `Lexer` is the main struct of the crate that allows you to read through a
/// `Source` and produce tokens for enums implementing the `Logos` trait.
pub struct Lexer<'source, Token: Logos<'source>> {
    source: &'source Token::Source,
    token: ManuallyDrop<Option<Token>>,
    token_start: usize,
    token_end: usize,

    /// Extras associated with the `Token`.
    pub extras: Token::Extras,
}

impl<'source, Token> Debug for Lexer<'source, Token>
where
    Token: Logos<'source>,
    Token::Source: Debug,
    Token::Extras: Debug,
{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_map()
            .entry(&"source", &self.source)
            .entry(&"extras", &self.extras)
            .finish()
    }
}

impl<'source, Token: Logos<'source>> Lexer<'source, Token> {
    /// Create a new `Lexer`.
    ///
    /// Due to type inference, it might be more ergonomic to construct
    /// it by calling [`Token::lexer`](./trait.Logos.html#method.lexer) on any `Token` with derived `Logos`.
    pub fn new(source: &'source Token::Source) -> Self
    where
        Token::Extras: Default,
    {
        Self::with_extras(source, Default::default())
    }

    /// Create a new `Lexer` with the provided `Extras`.
    ///
    /// Due to type inference, it might be more ergonomic to construct
    /// it by calling [`Token::lexer_with_extras`](./trait.Logos.html#method.lexer_with_extras) on any `Token` with derived `Logos`.
    pub fn with_extras(source: &'source Token::Source, extras: Token::Extras) -> Self {
        Lexer {
            source,
            token: ManuallyDrop::new(None),
            extras,
            token_start: 0,
            token_end: 0,
        }
    }

    /// Source from which this Lexer is reading tokens.
    #[inline]
    pub fn source(&self) -> &'source Token::Source {
        self.source
    }

    /// Wrap the `Lexer` in an [`Iterator`](https://doc.rust-lang.org/std/iter/trait.Iterator.html)
    /// that produces tuples of `(Token, `[`Span`](./type.Span.html)`)`.
    ///
    /// # Example
    ///
    /// ```
    /// use logos::Logos;
    ///
    /// #[derive(Logos, Debug, PartialEq)]
    /// enum Example {
    ///     #[regex(r"[ \n\t\f]+", logos::skip)]
    ///     #[error]
    ///     Error,
    ///
    ///     #[regex("-?[0-9]+", |lex| lex.slice().parse())]
    ///     Integer(i64),
    ///
    ///     #[regex("-?[0-9]+\\.[0-9]+", |lex| lex.slice().parse())]
    ///     Float(f64),
    /// }
    ///
    /// let tokens: Vec<_> = Example::lexer("42 3.14 -5 f").spanned().collect();
    ///
    /// assert_eq!(
    ///     tokens,
    ///     &[
    ///         (Example::Integer(42), 0..2),
    ///         (Example::Float(3.14), 3..7),
    ///         (Example::Integer(-5), 8..10),
    ///         (Example::Error, 11..12), // 'f' is not a recognized token
    ///     ],
    /// );
    /// ```
    #[inline]
    pub fn spanned(self) -> SpannedIter<'source, Token> {
        SpannedIter { lexer: self }
    }

    #[inline]
    #[doc(hidden)]
    #[deprecated(since = "0.11.0", note = "please use `span` instead")]
    pub fn range(&self) -> Span {
        self.span()
    }

    /// Get the range for the current token in `Source`.
    #[inline]
    pub fn span(&self) -> Span {
        self.token_start..self.token_end
    }

    /// Get a string slice of the current token.
    #[inline]
    pub fn slice(&self) -> &'source <Token::Source as Source>::Slice {
        unsafe { self.source.slice_unchecked(self.span()) }
    }

    /// Get a slice of remaining source, starting at the end of current token.
    #[inline]
    pub fn remainder(&self) -> &'source <Token::Source as Source>::Slice {
        unsafe {
            self.source
                .slice_unchecked(self.token_end..self.source.len())
        }
    }

    /// Turn this lexer into a lexer for a new token type.
    ///
    /// The new lexer continues to point at the same span as the current lexer,
    /// and the current token becomes the error token of the new token type.
    pub fn morph<Token2>(self) -> Lexer<'source, Token2>
    where
        Token2: Logos<'source, Source = Token::Source>,
        Token::Extras: Into<Token2::Extras>,
    {
        Lexer {
            source: self.source,
            token: ManuallyDrop::new(None),
            extras: self.extras.into(),
            token_start: self.token_start,
            token_end: self.token_end,
        }
    }

    /// Bumps the end of currently lexed token by `n` bytes.
    ///
    /// # Panics
    ///
    /// Panics if adding `n` to current offset would place the `Lexer` beyond the last byte,
    /// or in the middle of an UTF-8 code point (does not apply when lexing raw `&[u8]`).
    pub fn bump(&mut self, n: usize) {
        self.token_end += n;

        assert!(
            self.source.is_boundary(self.token_end),
            "Invalid Lexer bump",
        )
    }
}

impl<'source, Token> Clone for Lexer<'source, Token>
where
    Token: Logos<'source> + Clone,
    Token::Extras: Clone,
{
    fn clone(&self) -> Self {
        Lexer {
            extras: self.extras.clone(),
            token: self.token.clone(),
            ..*self
        }
    }
}

impl<'source, Token> Iterator for Lexer<'source, Token>
where
    Token: Logos<'source>,
{
    type Item = Token;

    #[inline]
    fn next(&mut self) -> Option<Token> {
        self.token_start = self.token_end;

        Token::lex(self);

        // This basically treats self.token as a temporary field.
        // Since we always immediately return a newly set token here,
        // we don't have to replace it with `None` or manually drop
        // it later.
        unsafe { ManuallyDrop::take(&mut self.token) }
    }
}

/// Iterator that pairs tokens with their position in the source.
///
/// Look at [`Lexer::spanned`](./struct.Lexer.html#method.spanned) for documentation.
pub struct SpannedIter<'source, Token: Logos<'source>> {
    lexer: Lexer<'source, Token>,
}

impl<'source, Token> Iterator for SpannedIter<'source, Token>
where
    Token: Logos<'source>,
{
    type Item = (Token, Span);

    fn next(&mut self) -> Option<Self::Item> {
        self.lexer.next().map(|token| (token, self.lexer.span()))
    }
}

#[doc(hidden)]
/// # WARNING!
///
/// **This trait, and it's methods, are not meant to be used outside of the
/// code produced by `#[derive(Logos)]` macro.**
impl<'source, Token> LexerInternal<'source> for Lexer<'source, Token>
where
    Token: Logos<'source>,
{
    type Token = Token;

    /// Read a `Chunk` at current position of the `Lexer`. If end
    /// of the `Source` has been reached, this will return `0`.
    #[inline]
    fn read<Chunk>(&self) -> Option<Chunk>
    where
        Chunk: source::Chunk<'source>,
    {
        self.source.read(self.token_end)
    }

    /// Read a `Chunk` at a position offset by `n`.
    #[inline]
    fn read_at<Chunk>(&self, n: usize) -> Option<Chunk>
    where
        Chunk: source::Chunk<'source>,
    {
        self.source.read(self.token_end + n)
    }

    #[inline]
    unsafe fn read_unchecked<Chunk>(&self, n: usize) -> Chunk
    where
        Chunk: source::Chunk<'source>,
    {
        self.source.read_unchecked(self.token_end + n)
    }

    /// Test a chunk at current position with a closure.
    #[inline]
    fn test<T, F>(&self, test: F) -> bool
    where
        T: source::Chunk<'source>,
        F: FnOnce(T) -> bool,
    {
        match self.source.read::<T>(self.token_end) {
            Some(chunk) => test(chunk),
            None => false,
        }
    }

    /// Test a chunk at current position offset by `n` with a closure.
    #[inline]
    fn test_at<T, F>(&self, n: usize, test: F) -> bool
    where
        T: source::Chunk<'source>,
        F: FnOnce(T) -> bool,
    {
        match self.source.read::<T>(self.token_end + n) {
            Some(chunk) => test(chunk),
            None => false,
        }
    }

    /// Bump the position `Lexer` is reading from by `size`.
    #[inline]
    fn bump_unchecked(&mut self, size: usize) {
        debug_assert!(
            self.token_end + size <= self.source.len(),
            "Bumping out of bounds!"
        );

        self.token_end += size;
    }

    /// Reset `token_start` to `token_end`.
    #[inline]
    fn trivia(&mut self) {
        self.token_start = self.token_end;
    }

    /// Set the current token to appropriate `#[error]` variant.
    /// Guarantee that `token_end` is at char boundary for `&str`.
    #[inline]
    fn error(&mut self) {
        self.token_end = self.source.find_boundary(self.token_end);
        self.token = ManuallyDrop::new(Some(Token::ERROR));
    }

    #[inline]
    fn end(&mut self) {
        self.token = ManuallyDrop::new(None);
    }

    #[inline]
    fn set(&mut self, token: Token) {
        self.token = ManuallyDrop::new(Some(token));
    }
}
