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

/// Thread global state through your parsers
///
/// Use cases
/// - Recursion checks
/// - Error recovery
/// - Debugging
///
/// # Example
///
/// ```
/// # use std::cell::Cell;
/// # use winnow::prelude::*;
/// # use winnow::stream::Stateful;
/// # use winnow::ascii::alpha1;
/// # type Error = ();
///
/// #[derive(Debug)]
/// struct State<'s>(&'s mut u32);
///
/// impl<'s> State<'s> {
///     fn count(&mut self) {
///         *self.0 += 1;
///     }
/// }
///
/// type Stream<'is> = Stateful<&'is str, State<'is>>;
///
/// fn word<'s>(i: &mut Stream<'s>) -> ModalResult<&'s str> {
///   i.state.count();
///   alpha1.parse_next(i)
/// }
///
/// let data = "Hello";
/// let mut state = 0;
/// let input = Stream { input: data, state: State(&mut state) };
/// let output = word.parse(input).unwrap();
/// assert_eq!(state, 1);
/// ```
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[doc(alias = "LocatingSliceSpan")]
pub struct Stateful<I, S> {
    /// Inner input being wrapped in state
    pub input: I,
    /// User-provided state
    pub state: S,
}

impl<I, S> AsRef<I> for Stateful<I, S> {
    #[inline(always)]
    fn as_ref(&self) -> &I {
        &self.input
    }
}

impl<I, S> crate::lib::std::ops::Deref for Stateful<I, S> {
    type Target = I;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl<I: crate::lib::std::fmt::Display, S> crate::lib::std::fmt::Display for Stateful<I, S> {
    fn fmt(&self, f: &mut crate::lib::std::fmt::Formatter<'_>) -> crate::lib::std::fmt::Result {
        self.input.fmt(f)
    }
}

impl<I, S> SliceLen for Stateful<I, S>
where
    I: SliceLen,
{
    #[inline(always)]
    fn slice_len(&self) -> usize {
        self.input.slice_len()
    }
}

impl<I: Stream, S: crate::lib::std::fmt::Debug> Stream for Stateful<I, S> {
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
        #![allow(deprecated)]
        self.input.raw()
    }

    fn trace(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.input.trace(f)
    }
}

impl<I, S> Location for Stateful<I, S>
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
impl<I, E, S> Recover<E> for Stateful<I, S>
where
    I: Recover<E>,
    I: Stream,
    S: Clone + crate::lib::std::fmt::Debug,
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

impl<I, S> StreamIsPartial for Stateful<I, S>
where
    I: StreamIsPartial,
{
    type PartialState = I::PartialState;

    #[inline]
    fn complete(&mut self) -> Self::PartialState {
        self.input.complete()
    }

    #[inline]
    fn restore_partial(&mut self, state: Self::PartialState) {
        self.input.restore_partial(state);
    }

    #[inline(always)]
    fn is_partial_supported() -> bool {
        I::is_partial_supported()
    }

    #[inline(always)]
    fn is_partial(&self) -> bool {
        self.input.is_partial()
    }
}

impl<I, S> Offset for Stateful<I, S>
where
    I: Stream,
    S: Clone + crate::lib::std::fmt::Debug,
{
    #[inline(always)]
    fn offset_from(&self, start: &Self) -> usize {
        self.offset_from(&start.checkpoint())
    }
}

impl<I, S> Offset<<Stateful<I, S> as Stream>::Checkpoint> for Stateful<I, S>
where
    I: Stream,
    S: crate::lib::std::fmt::Debug,
{
    #[inline(always)]
    fn offset_from(&self, other: &<Stateful<I, S> as Stream>::Checkpoint) -> usize {
        self.checkpoint().offset_from(other)
    }
}

impl<I, S> AsBytes for Stateful<I, S>
where
    I: AsBytes,
{
    #[inline(always)]
    fn as_bytes(&self) -> &[u8] {
        self.input.as_bytes()
    }
}

impl<I, S> AsBStr for Stateful<I, S>
where
    I: AsBStr,
{
    #[inline(always)]
    fn as_bstr(&self) -> &[u8] {
        self.input.as_bstr()
    }
}

impl<I, S, U> Compare<U> for Stateful<I, S>
where
    I: Compare<U>,
{
    #[inline(always)]
    fn compare(&self, other: U) -> CompareResult {
        self.input.compare(other)
    }
}

impl<I, S, T> FindSlice<T> for Stateful<I, S>
where
    I: FindSlice<T>,
{
    #[inline(always)]
    fn find_slice(&self, substr: T) -> Option<crate::lib::std::ops::Range<usize>> {
        self.input.find_slice(substr)
    }
}

impl<I, S> UpdateSlice for Stateful<I, S>
where
    I: UpdateSlice,
    S: Clone + crate::lib::std::fmt::Debug,
{
    #[inline(always)]
    fn update_slice(mut self, inner: Self::Slice) -> Self {
        self.input = I::update_slice(self.input, inner);
        self
    }
}
