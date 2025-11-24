use crate::error::FromRecoverableError;
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

/// Allow recovering from parse errors, capturing them as the parser continues
///
/// Generally, this will be used indirectly via
/// [`RecoverableParser::recoverable_parse`][crate::RecoverableParser::recoverable_parse].
#[derive(Clone, Debug)]
pub struct Recoverable<I, E>
where
    I: Stream,
{
    input: I,
    errors: Vec<E>,
    is_recoverable: bool,
}

impl<I, E> Default for Recoverable<I, E>
where
    I: Default + Stream,
{
    #[inline]
    fn default() -> Self {
        Self::new(I::default())
    }
}

impl<I, E> Recoverable<I, E>
where
    I: Stream,
{
    /// Track recoverable errors with the stream
    #[inline]
    pub fn new(input: I) -> Self {
        Self {
            input,
            errors: Default::default(),
            is_recoverable: true,
        }
    }

    /// Act as a normal stream
    #[inline]
    pub fn unrecoverable(input: I) -> Self {
        Self {
            input,
            errors: Default::default(),
            is_recoverable: false,
        }
    }

    /// Access the current input and errors
    #[inline]
    pub fn into_parts(self) -> (I, Vec<E>) {
        (self.input, self.errors)
    }
}

impl<I, E> AsRef<I> for Recoverable<I, E>
where
    I: Stream,
{
    #[inline(always)]
    fn as_ref(&self) -> &I {
        &self.input
    }
}

impl<I, E> crate::lib::std::ops::Deref for Recoverable<I, E>
where
    I: Stream,
{
    type Target = I;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.input
    }
}

impl<I: crate::lib::std::fmt::Display, E> crate::lib::std::fmt::Display for Recoverable<I, E>
where
    I: Stream,
{
    fn fmt(&self, f: &mut crate::lib::std::fmt::Formatter<'_>) -> crate::lib::std::fmt::Result {
        crate::lib::std::fmt::Display::fmt(&self.input, f)
    }
}

impl<I, E> SliceLen for Recoverable<I, E>
where
    I: SliceLen,
    I: Stream,
{
    #[inline(always)]
    fn slice_len(&self) -> usize {
        self.input.slice_len()
    }
}

impl<I, E: crate::lib::std::fmt::Debug> Stream for Recoverable<I, E>
where
    I: Stream,
{
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

impl<I, E> Location for Recoverable<I, E>
where
    I: Location,
    I: Stream,
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

impl<I, E, R> Recover<E> for Recoverable<I, R>
where
    I: Stream,
    R: FromRecoverableError<Self, E>,
    R: crate::lib::std::fmt::Debug,
    E: crate::error::ParserError<Self>,
{
    fn record_err(
        &mut self,
        token_start: &Self::Checkpoint,
        err_start: &Self::Checkpoint,
        err: E,
    ) -> Result<(), E> {
        if self.is_recoverable {
            if err.is_incomplete() {
                Err(err)
            } else {
                self.errors
                    .push(R::from_recoverable_error(token_start, err_start, self, err));
                Ok(())
            }
        } else {
            Err(err)
        }
    }

    /// Report whether the [`Stream`] can save off errors for recovery
    #[inline(always)]
    fn is_recovery_supported() -> bool {
        true
    }
}

impl<I, E> StreamIsPartial for Recoverable<I, E>
where
    I: StreamIsPartial,
    I: Stream,
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

impl<I, E> Offset for Recoverable<I, E>
where
    I: Stream,
    E: crate::lib::std::fmt::Debug,
{
    #[inline(always)]
    fn offset_from(&self, other: &Self) -> usize {
        self.offset_from(&other.checkpoint())
    }
}

impl<I, E> Offset<<Recoverable<I, E> as Stream>::Checkpoint> for Recoverable<I, E>
where
    I: Stream,
    E: crate::lib::std::fmt::Debug,
{
    #[inline(always)]
    fn offset_from(&self, other: &<Recoverable<I, E> as Stream>::Checkpoint) -> usize {
        self.checkpoint().offset_from(other)
    }
}

impl<I, E> AsBytes for Recoverable<I, E>
where
    I: Stream,
    I: AsBytes,
{
    #[inline(always)]
    fn as_bytes(&self) -> &[u8] {
        self.input.as_bytes()
    }
}

impl<I, E> AsBStr for Recoverable<I, E>
where
    I: Stream,
    I: AsBStr,
{
    #[inline(always)]
    fn as_bstr(&self) -> &[u8] {
        self.input.as_bstr()
    }
}

impl<I, E, U> Compare<U> for Recoverable<I, E>
where
    I: Stream,
    I: Compare<U>,
{
    #[inline(always)]
    fn compare(&self, other: U) -> CompareResult {
        self.input.compare(other)
    }
}

impl<I, E, T> FindSlice<T> for Recoverable<I, E>
where
    I: Stream,
    I: FindSlice<T>,
{
    #[inline(always)]
    fn find_slice(&self, substr: T) -> Option<crate::lib::std::ops::Range<usize>> {
        self.input.find_slice(substr)
    }
}

impl<I, E> UpdateSlice for Recoverable<I, E>
where
    I: Stream,
    I: UpdateSlice,
    E: crate::lib::std::fmt::Debug,
{
    #[inline(always)]
    fn update_slice(mut self, inner: Self::Slice) -> Self {
        self.input = I::update_slice(self.input, inner);
        self
    }
}
