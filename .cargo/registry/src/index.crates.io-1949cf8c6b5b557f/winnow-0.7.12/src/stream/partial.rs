use crate::error::Needed;
use crate::stream::AsBStr;
use crate::stream::AsBytes;
use crate::stream::Checkpoint;
use crate::stream::Compare;
use crate::stream::CompareResult;
use crate::stream::FindSlice;
use crate::stream::Location;
use crate::stream::Offset;
#[cfg(feature = "unstable-recover")]
#[cfg(feature = "std")]
use crate::stream::Recover;
use crate::stream::SliceLen;
use crate::stream::Stream;
use crate::stream::StreamIsPartial;
use crate::stream::UpdateSlice;

/// Mark the input as a partial buffer for streaming input.
///
/// Complete input means that we already have all of the data. This will be the common case with
/// small files that can be read entirely to memory.
///
/// In contrast, streaming input assumes that we might not have all of the data.
/// This can happen with some network protocol or large file parsers, where the
/// input buffer can be full and need to be resized or refilled.
/// - [`ErrMode::Incomplete`][crate::error::ErrMode::Incomplete] will report how much more data is needed.
/// - [`Parser::complete_err`][crate::Parser::complete_err] transform
///   [`ErrMode::Incomplete`][crate::error::ErrMode::Incomplete] to
///   [`ErrMode::Backtrack`][crate::error::ErrMode::Backtrack]
///
/// See also [`StreamIsPartial`] to tell whether the input supports complete or partial parsing.
///
/// See also [Special Topics: Parsing Partial Input][crate::_topic::partial].
///
/// # Example
///
/// Here is how it works in practice:
///
/// ```rust
/// # use winnow::{Result, error::ErrMode, error::Needed, error::ContextError, token, ascii, stream::Partial};
/// # use winnow::prelude::*;
///
/// fn take_partial<'s>(i: &mut Partial<&'s [u8]>) -> ModalResult<&'s [u8], ContextError> {
///   token::take(4u8).parse_next(i)
/// }
///
/// fn take_complete<'s>(i: &mut &'s [u8]) -> ModalResult<&'s [u8], ContextError> {
///   token::take(4u8).parse_next(i)
/// }
///
/// // both parsers will take 4 bytes as expected
/// assert_eq!(take_partial.parse_peek(Partial::new(&b"abcde"[..])), Ok((Partial::new(&b"e"[..]), &b"abcd"[..])));
/// assert_eq!(take_complete.parse_peek(&b"abcde"[..]), Ok((&b"e"[..], &b"abcd"[..])));
///
/// // if the input is smaller than 4 bytes, the partial parser
/// // will return `Incomplete` to indicate that we need more data
/// assert_eq!(take_partial.parse_peek(Partial::new(&b"abc"[..])), Err(ErrMode::Incomplete(Needed::new(1))));
///
/// // but the complete parser will return an error
/// assert!(take_complete.parse_peek(&b"abc"[..]).is_err());
///
/// // the alpha0 function takes 0 or more alphabetic characters
/// fn alpha0_partial<'s>(i: &mut Partial<&'s str>) -> ModalResult<&'s str, ContextError> {
///   ascii::alpha0.parse_next(i)
/// }
///
/// fn alpha0_complete<'s>(i: &mut &'s str) -> ModalResult<&'s str, ContextError> {
///   ascii::alpha0.parse_next(i)
/// }
///
/// // if there's a clear limit to the taken characters, both parsers work the same way
/// assert_eq!(alpha0_partial.parse_peek(Partial::new("abcd;")), Ok((Partial::new(";"), "abcd")));
/// assert_eq!(alpha0_complete.parse_peek("abcd;"), Ok((";", "abcd")));
///
/// // but when there's no limit, the partial version returns `Incomplete`, because it cannot
/// // know if more input data should be taken. The whole input could be "abcd;", or
/// // "abcde;"
/// assert_eq!(alpha0_partial.parse_peek(Partial::new("abcd")), Err(ErrMode::Incomplete(Needed::new(1))));
///
/// // while the complete version knows that all of the data is there
/// assert_eq!(alpha0_complete.parse_peek("abcd"), Ok(("", "abcd")));
/// ```
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Partial<I> {
    input: I,
    partial: bool,
}

impl<I> Partial<I>
where
    I: StreamIsPartial,
{
    /// Create a partial input
    #[inline]
    pub fn new(input: I) -> Self {
        debug_assert!(
            !I::is_partial_supported(),
            "`Partial` can only wrap complete sources"
        );
        let partial = true;
        Self { input, partial }
    }

    /// Extract the original [`Stream`]
    #[inline(always)]
    pub fn into_inner(self) -> I {
        self.input
    }
}

impl<I> Default for Partial<I>
where
    I: Default + StreamIsPartial,
{
    #[inline]
    fn default() -> Self {
        Self::new(I::default())
    }
}

impl<I> crate::lib::std::ops::Deref for Partial<I> {
    type Target = I;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.input
    }
}

impl<I: crate::lib::std::fmt::Display> crate::lib::std::fmt::Display for Partial<I> {
    fn fmt(&self, f: &mut crate::lib::std::fmt::Formatter<'_>) -> crate::lib::std::fmt::Result {
        self.input.fmt(f)
    }
}

impl<I> SliceLen for Partial<I>
where
    I: SliceLen,
{
    #[inline(always)]
    fn slice_len(&self) -> usize {
        self.input.slice_len()
    }
}

impl<I: Stream> Stream for Partial<I> {
    type Token = <I as Stream>::Token;
    type Slice = <I as Stream>::Slice;

    type IterOffsets = <I as Stream>::IterOffsets;

    type Checkpoint = Checkpoint<I::Checkpoint, Self>;

    #[inline(always)]
    fn iter_offsets(&self) -> Self::IterOffsets {
        self.input.iter_offsets()
    }
    #[inline(always)]
    fn eof_offset(&self) -> usize {
        self.input.eof_offset()
    }

    #[inline(always)]
    fn next_token(&mut self) -> Option<Self::Token> {
        self.input.next_token()
    }

    #[inline(always)]
    fn peek_token(&self) -> Option<Self::Token> {
        self.input.peek_token()
    }

    #[inline(always)]
    fn offset_for<P>(&self, predicate: P) -> Option<usize>
    where
        P: Fn(Self::Token) -> bool,
    {
        self.input.offset_for(predicate)
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
        Checkpoint::<_, Self>::new(self.input.checkpoint())
    }
    #[inline(always)]
    fn reset(&mut self, checkpoint: &Self::Checkpoint) {
        self.input.reset(&checkpoint.inner);
    }

    #[inline(always)]
    fn raw(&self) -> &dyn crate::lib::std::fmt::Debug {
        &self.input
    }
}

impl<I> Location for Partial<I>
where
    I: Location,
{
    #[inline(always)]
    fn previous_token_end(&self) -> usize {
        self.input.previous_token_end()
    }
    #[inline(always)]
    fn current_token_start(&self) -> usize {
        self.input.current_token_start()
    }
}

#[cfg(feature = "unstable-recover")]
#[cfg(feature = "std")]
impl<I, E> Recover<E> for Partial<I>
where
    I: Recover<E>,
    I: Stream,
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

impl<I> StreamIsPartial for Partial<I>
where
    I: StreamIsPartial,
{
    type PartialState = bool;

    #[inline]
    fn complete(&mut self) -> Self::PartialState {
        core::mem::replace(&mut self.partial, false)
    }

    #[inline]
    fn restore_partial(&mut self, state: Self::PartialState) {
        self.partial = state;
    }

    #[inline(always)]
    fn is_partial_supported() -> bool {
        true
    }

    #[inline(always)]
    fn is_partial(&self) -> bool {
        self.partial
    }
}

impl<I> Offset for Partial<I>
where
    I: Stream,
{
    #[inline(always)]
    fn offset_from(&self, start: &Self) -> usize {
        self.offset_from(&start.checkpoint())
    }
}

impl<I> Offset<<Partial<I> as Stream>::Checkpoint> for Partial<I>
where
    I: Stream,
{
    #[inline(always)]
    fn offset_from(&self, other: &<Partial<I> as Stream>::Checkpoint) -> usize {
        self.checkpoint().offset_from(other)
    }
}

impl<I> AsBytes for Partial<I>
where
    I: AsBytes,
{
    #[inline(always)]
    fn as_bytes(&self) -> &[u8] {
        self.input.as_bytes()
    }
}

impl<I> AsBStr for Partial<I>
where
    I: AsBStr,
{
    #[inline(always)]
    fn as_bstr(&self) -> &[u8] {
        self.input.as_bstr()
    }
}

impl<I, T> Compare<T> for Partial<I>
where
    I: Compare<T>,
{
    #[inline(always)]
    fn compare(&self, t: T) -> CompareResult {
        self.input.compare(t)
    }
}

impl<I, T> FindSlice<T> for Partial<I>
where
    I: FindSlice<T>,
{
    #[inline(always)]
    fn find_slice(&self, substr: T) -> Option<crate::lib::std::ops::Range<usize>> {
        self.input.find_slice(substr)
    }
}

impl<I> UpdateSlice for Partial<I>
where
    I: UpdateSlice,
{
    #[inline(always)]
    fn update_slice(self, inner: Self::Slice) -> Self {
        Partial {
            input: I::update_slice(self.input, inner),
            partial: self.partial,
        }
    }
}
