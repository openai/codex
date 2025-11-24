use crate::error::Needed;
use crate::lib::std::iter::Enumerate;
use crate::lib::std::slice::Iter;
use crate::stream::Checkpoint;
use crate::stream::Compare;
use crate::stream::CompareResult;
use crate::stream::Location;
use crate::stream::Offset;
#[cfg(feature = "unstable-recover")]
#[cfg(feature = "std")]
use crate::stream::Recover;
use crate::stream::SliceLen;
use crate::stream::Stream;
use crate::stream::StreamIsPartial;
use crate::stream::UpdateSlice;

/// Specialized input for parsing lexed tokens
///
/// Helpful impls
/// - Any `PartialEq` type (e.g. a `TokenKind` or `&str`) can be used with
///   [`literal`][crate::token::literal]
/// - A `PartialEq` for `&str` allows for using `&str` as a parser for tokens
/// - [`ContainsToken`][crate::stream::ContainsToken] for `T` to for parsing with token sets
/// - [`Location`] for `T` to extract spans from tokens
///
/// See also [Lexing and Parsing][crate::_topic::lexing].
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct TokenSlice<'t, T> {
    initial: &'t [T],
    input: &'t [T],
}

impl<'t, T> TokenSlice<'t, T>
where
    T: crate::lib::std::fmt::Debug + Clone,
{
    /// Make a stream to parse tokens
    #[inline]
    pub fn new(input: &'t [T]) -> Self {
        Self {
            initial: input,
            input,
        }
    }

    /// Reset the stream to the start
    ///
    /// This is useful for formats that encode a graph with addresses relative to the start of the
    /// input.
    #[doc(alias = "fseek")]
    #[inline]
    pub fn reset_to_start(&mut self) {
        let start = self.initial.checkpoint();
        self.input.reset(&start);
    }

    /// Iterate over consumed tokens starting with the last emitted
    ///
    /// This is intended to help build up appropriate context when reporting errors.
    #[inline]
    pub fn previous_tokens(&self) -> impl Iterator<Item = &'t T> {
        let offset = self.input.offset_from(&self.initial);
        self.initial[0..offset].iter().rev()
    }
}

/// Track locations by implementing [`Location`] on the Token.
impl<T> TokenSlice<'_, T>
where
    T: Location,
{
    #[inline(always)]
    fn previous_token_end(&self) -> Option<usize> {
        let index = self.input.offset_from(&self.initial);
        index
            .checked_sub(1)
            .map(|i| self.initial[i].previous_token_end())
    }

    #[inline(always)]
    fn current_token_start(&self) -> Option<usize> {
        self.input.first().map(|t| t.current_token_start())
    }
}

impl<T> Default for TokenSlice<'_, T>
where
    T: crate::lib::std::fmt::Debug + Clone,
{
    fn default() -> Self {
        Self::new(&[])
    }
}

impl<T> crate::lib::std::ops::Deref for TokenSlice<'_, T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.input
    }
}

impl<T: core::fmt::Debug> core::fmt::Debug for TokenSlice<'_, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.input.fmt(f)
    }
}

impl<T> SliceLen for TokenSlice<'_, T> {
    #[inline(always)]
    fn slice_len(&self) -> usize {
        self.input.slice_len()
    }
}

impl<'t, T> Stream for TokenSlice<'t, T>
where
    T: crate::lib::std::fmt::Debug + Clone,
{
    type Token = &'t T;
    type Slice = &'t [T];

    type IterOffsets = Enumerate<Iter<'t, T>>;

    type Checkpoint = Checkpoint<&'t [T], Self>;

    #[inline(always)]
    fn iter_offsets(&self) -> Self::IterOffsets {
        self.input.iter().enumerate()
    }
    #[inline(always)]
    fn eof_offset(&self) -> usize {
        self.input.eof_offset()
    }

    #[inline(always)]
    fn next_token(&mut self) -> Option<Self::Token> {
        let (token, next) = self.input.split_first()?;
        self.input = next;
        Some(token)
    }

    #[inline(always)]
    fn peek_token(&self) -> Option<Self::Token> {
        self.input.first()
    }

    #[inline(always)]
    fn offset_for<P>(&self, predicate: P) -> Option<usize>
    where
        P: Fn(Self::Token) -> bool,
    {
        self.input.iter().position(predicate)
    }
    #[inline(always)]
    fn offset_at(&self, tokens: usize) -> Result<usize, Needed> {
        self.input.offset_at(tokens)
    }
    #[inline(always)]
    fn next_slice(&mut self, offset: usize) -> Self::Slice {
        self.input.next_slice(offset)
    }
    #[inline(always)]
    unsafe fn next_slice_unchecked(&mut self, offset: usize) -> Self::Slice {
        // SAFETY: Passing up invariants
        unsafe { self.input.next_slice_unchecked(offset) }
    }
    #[inline(always)]
    fn peek_slice(&self, offset: usize) -> Self::Slice {
        self.input.peek_slice(offset)
    }
    #[inline(always)]
    unsafe fn peek_slice_unchecked(&self, offset: usize) -> Self::Slice {
        // SAFETY: Passing up invariants
        unsafe { self.input.peek_slice_unchecked(offset) }
    }

    #[inline(always)]
    fn checkpoint(&self) -> Self::Checkpoint {
        Checkpoint::<_, Self>::new(self.input)
    }
    #[inline(always)]
    fn reset(&mut self, checkpoint: &Self::Checkpoint) {
        self.input = checkpoint.inner;
    }

    #[inline(always)]
    fn raw(&self) -> &dyn crate::lib::std::fmt::Debug {
        #![allow(deprecated)]
        self.input.raw()
    }

    fn trace(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.input.trace(f)
    }
}

impl<T> Location for TokenSlice<'_, T>
where
    T: Location,
{
    #[inline(always)]
    fn previous_token_end(&self) -> usize {
        self.previous_token_end()
            .or_else(|| self.current_token_start())
            .unwrap_or(0)
    }
    #[inline(always)]
    fn current_token_start(&self) -> usize {
        self.current_token_start()
            .or_else(|| self.previous_token_end())
            .unwrap_or(0)
    }
}

#[cfg(feature = "unstable-recover")]
#[cfg(feature = "std")]
impl<T, E> Recover<E> for TokenSlice<'_, T>
where
    T: crate::lib::std::fmt::Debug + Clone,
{
    #[inline(always)]
    fn record_err(
        &mut self,
        _token_start: &Self::Checkpoint,
        _err_start: &Self::Checkpoint,
        err: E,
    ) -> Result<(), E> {
        Err(err)
    }

    /// Report whether the [`Stream`] can save off errors for recovery
    #[inline(always)]
    fn is_recovery_supported() -> bool {
        false
    }
}

impl<'t, T> StreamIsPartial for TokenSlice<'t, T>
where
    T: crate::lib::std::fmt::Debug + Clone,
{
    type PartialState = <&'t [T] as StreamIsPartial>::PartialState;

    #[inline]
    fn complete(&mut self) -> Self::PartialState {
        #![allow(clippy::semicolon_if_nothing_returned)]
        self.input.complete()
    }

    #[inline]
    fn restore_partial(&mut self, state: Self::PartialState) {
        self.input.restore_partial(state);
    }

    #[inline(always)]
    fn is_partial_supported() -> bool {
        <&[T] as StreamIsPartial>::is_partial_supported()
    }

    #[inline(always)]
    fn is_partial(&self) -> bool {
        self.input.is_partial()
    }
}

impl<T> Offset for TokenSlice<'_, T>
where
    T: crate::lib::std::fmt::Debug + Clone,
{
    #[inline(always)]
    fn offset_from(&self, other: &Self) -> usize {
        self.offset_from(&other.checkpoint())
    }
}

impl<T> Offset<<TokenSlice<'_, T> as Stream>::Checkpoint> for TokenSlice<'_, T>
where
    T: crate::lib::std::fmt::Debug + Clone,
{
    #[inline(always)]
    fn offset_from(&self, other: &<TokenSlice<'_, T> as Stream>::Checkpoint) -> usize {
        self.checkpoint().offset_from(other)
    }
}

impl<T, O> Compare<O> for TokenSlice<'_, T>
where
    T: PartialEq<O> + Eq,
{
    #[inline]
    fn compare(&self, t: O) -> CompareResult {
        if let Some(token) = self.first() {
            if *token == t {
                CompareResult::Ok(1)
            } else {
                CompareResult::Error
            }
        } else {
            CompareResult::Incomplete
        }
    }
}

impl<T> UpdateSlice for TokenSlice<'_, T>
where
    T: crate::lib::std::fmt::Debug + Clone,
{
    #[inline(always)]
    fn update_slice(mut self, inner: Self::Slice) -> Self {
        self.input = <&[T] as UpdateSlice>::update_slice(self.input, inner);
        self
    }
}
