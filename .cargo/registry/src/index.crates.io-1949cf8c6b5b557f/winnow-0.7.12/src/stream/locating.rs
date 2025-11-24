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

/// Allow collecting the span of a parsed token within a slice
///
/// Converting byte offsets to line or column numbers is left up to the user, as computing column
/// numbers requires domain knowledge (are columns byte-based, codepoint-based, or grapheme-based?)
/// and O(n) iteration over the input to determine codepoint and line boundaries.
///
/// [The `line-span` crate](https://docs.rs/line-span/latest/line_span/) can help with converting
/// byte offsets to line numbers.
///
/// See [`Parser::span`][crate::Parser::span] and [`Parser::with_span`][crate::Parser::with_span] for more details
#[derive(Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
#[doc(alias = "LocatingSliceSpan")]
#[doc(alias = "Located")]
pub struct LocatingSlice<I> {
    initial: I,
    input: I,
}

impl<I> LocatingSlice<I>
where
    I: Clone + Offset,
{
    /// Wrap another Stream with span tracking
    pub fn new(input: I) -> Self {
        let initial = input.clone();
        Self { initial, input }
    }

    #[inline]
    fn previous_token_end(&self) -> usize {
        // Assumptions:
        // - Index offsets is sufficient
        // - Tokens are continuous
        self.input.offset_from(&self.initial)
    }
    #[inline]
    fn current_token_start(&self) -> usize {
        // Assumptions:
        // - Index offsets is sufficient
        self.input.offset_from(&self.initial)
    }
}

impl<I> LocatingSlice<I>
where
    I: Clone + Stream + Offset,
{
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
}

impl<I> AsRef<I> for LocatingSlice<I> {
    #[inline(always)]
    fn as_ref(&self) -> &I {
        &self.input
    }
}

impl<I: core::fmt::Debug> core::fmt::Debug for LocatingSlice<I> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.input.fmt(f)
    }
}

impl<I> crate::lib::std::ops::Deref for LocatingSlice<I> {
    type Target = I;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.input
    }
}

impl<I: crate::lib::std::fmt::Display> crate::lib::std::fmt::Display for LocatingSlice<I> {
    fn fmt(&self, f: &mut crate::lib::std::fmt::Formatter<'_>) -> crate::lib::std::fmt::Result {
        self.input.fmt(f)
    }
}

impl<I> SliceLen for LocatingSlice<I>
where
    I: SliceLen,
{
    #[inline(always)]
    fn slice_len(&self) -> usize {
        self.input.slice_len()
    }
}

impl<I: Stream> Stream for LocatingSlice<I> {
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

impl<I> Location for LocatingSlice<I>
where
    I: Clone + Offset,
{
    #[inline(always)]
    fn previous_token_end(&self) -> usize {
        self.previous_token_end()
    }
    #[inline(always)]
    fn current_token_start(&self) -> usize {
        self.current_token_start()
    }
}

#[cfg(feature = "unstable-recover")]
#[cfg(feature = "std")]
impl<I, E> Recover<E> for LocatingSlice<I>
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

impl<I> StreamIsPartial for LocatingSlice<I>
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

impl<I> Offset for LocatingSlice<I>
where
    I: Stream,
{
    #[inline(always)]
    fn offset_from(&self, other: &Self) -> usize {
        self.offset_from(&other.checkpoint())
    }
}

impl<I> Offset<<LocatingSlice<I> as Stream>::Checkpoint> for LocatingSlice<I>
where
    I: Stream,
{
    #[inline(always)]
    fn offset_from(&self, other: &<LocatingSlice<I> as Stream>::Checkpoint) -> usize {
        self.checkpoint().offset_from(other)
    }
}

impl<I> AsBytes for LocatingSlice<I>
where
    I: AsBytes,
{
    #[inline(always)]
    fn as_bytes(&self) -> &[u8] {
        self.input.as_bytes()
    }
}

impl<I> AsBStr for LocatingSlice<I>
where
    I: AsBStr,
{
    #[inline(always)]
    fn as_bstr(&self) -> &[u8] {
        self.input.as_bstr()
    }
}

impl<I, U> Compare<U> for LocatingSlice<I>
where
    I: Compare<U>,
{
    #[inline(always)]
    fn compare(&self, other: U) -> CompareResult {
        self.input.compare(other)
    }
}

impl<I, T> FindSlice<T> for LocatingSlice<I>
where
    I: FindSlice<T>,
{
    #[inline(always)]
    fn find_slice(&self, substr: T) -> Option<crate::lib::std::ops::Range<usize>> {
        self.input.find_slice(substr)
    }
}

impl<I> UpdateSlice for LocatingSlice<I>
where
    I: UpdateSlice,
{
    #[inline(always)]
    fn update_slice(mut self, inner: Self::Slice) -> Self {
        self.input = I::update_slice(self.input, inner);
        self
    }
}
