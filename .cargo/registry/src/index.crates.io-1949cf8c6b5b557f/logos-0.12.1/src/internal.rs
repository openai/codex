use crate::source::Chunk;
use crate::{Filter, FilterResult, Lexer, Logos, Skip};

/// Trait used by the functions contained in the `Lexicon`.
///
/// # WARNING!
///
/// **This trait, and it's methods, are not meant to be used outside of the
/// code produced by `#[derive(Logos)]` macro.**
pub trait LexerInternal<'source> {
    type Token;

    /// Read a chunk at current position.
    fn read<T: Chunk<'source>>(&self) -> Option<T>;

    /// Read a chunk at current position, offset by `n`.
    fn read_at<T: Chunk<'source>>(&self, n: usize) -> Option<T>;

    /// Unchecked read a chunk at current position, offset by `n`.
    unsafe fn read_unchecked<T: Chunk<'source>>(&self, n: usize) -> T;

    /// Test a chunk at current position with a closure.
    fn test<T: Chunk<'source>, F: FnOnce(T) -> bool>(&self, test: F) -> bool;

    /// Test a chunk at current position offset by `n` with a closure.
    fn test_at<T: Chunk<'source>, F: FnOnce(T) -> bool>(&self, n: usize, test: F) -> bool;

    /// Bump the position by `size`.
    fn bump_unchecked(&mut self, size: usize);

    /// Reset `token_start` to `token_end`.
    fn trivia(&mut self);

    /// Set the current token to appropriate `#[error]` variant.
    /// Guarantee that `token_end` is at char boundary for `&str`.
    fn error(&mut self);

    fn end(&mut self);

    fn set(&mut self, token: Self::Token);
}

pub trait CallbackResult<'s, P, T: Logos<'s>> {
    fn construct<Constructor>(self, c: Constructor, lex: &mut Lexer<'s, T>)
    where
        Constructor: Fn(P) -> T;
}

impl<'s, P, T: Logos<'s>> CallbackResult<'s, P, T> for P {
    #[inline]
    fn construct<Constructor>(self, c: Constructor, lex: &mut Lexer<'s, T>)
    where
        Constructor: Fn(P) -> T,
    {
        lex.set(c(self))
    }
}

impl<'s, T: Logos<'s>> CallbackResult<'s, (), T> for bool {
    #[inline]
    fn construct<Constructor>(self, c: Constructor, lex: &mut Lexer<'s, T>)
    where
        Constructor: Fn(()) -> T,
    {
        match self {
            true => lex.set(c(())),
            false => lex.set(T::ERROR),
        }
    }
}

impl<'s, P, T: Logos<'s>> CallbackResult<'s, P, T> for Option<P> {
    #[inline]
    fn construct<Constructor>(self, c: Constructor, lex: &mut Lexer<'s, T>)
    where
        Constructor: Fn(P) -> T,
    {
        match self {
            Some(product) => lex.set(c(product)),
            None => lex.set(T::ERROR),
        }
    }
}

impl<'s, P, E, T: Logos<'s>> CallbackResult<'s, P, T> for Result<P, E> {
    #[inline]
    fn construct<Constructor>(self, c: Constructor, lex: &mut Lexer<'s, T>)
    where
        Constructor: Fn(P) -> T,
    {
        match self {
            Ok(product) => lex.set(c(product)),
            Err(_) => lex.set(T::ERROR),
        }
    }
}

impl<'s, T: Logos<'s>> CallbackResult<'s, (), T> for Skip {
    #[inline]
    fn construct<Constructor>(self, _: Constructor, lex: &mut Lexer<'s, T>)
    where
        Constructor: Fn(()) -> T,
    {
        lex.trivia();
        T::lex(lex);
    }
}

impl<'s, P, T: Logos<'s>> CallbackResult<'s, P, T> for Filter<P> {
    #[inline]
    fn construct<Constructor>(self, c: Constructor, lex: &mut Lexer<'s, T>)
    where
        Constructor: Fn(P) -> T,
    {
        match self {
            Filter::Emit(product) => lex.set(c(product)),
            Filter::Skip => {
                lex.trivia();
                T::lex(lex);
            }
        }
    }
}

impl<'s, P, T: Logos<'s>> CallbackResult<'s, P, T> for FilterResult<P> {
    fn construct<Constructor>(self, c: Constructor, lex: &mut Lexer<'s, T>)
    where
        Constructor: Fn(P) -> T,
    {
        match self {
            FilterResult::Emit(product) => lex.set(c(product)),
            FilterResult::Skip => {
                lex.trivia();
                T::lex(lex);
            }
            FilterResult::Error => lex.set(T::ERROR),
        }
    }
}
