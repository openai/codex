//! Stream capability for combinators to parse
//!
//! Stream types include:
//! - `&[u8]` and [`Bytes`] for binary data
//! - `&str` (aliased as [`Str`]) and [`BStr`] for UTF-8 data
//! - [`LocatingSlice`] can track the location within the original buffer to report
//!   [spans][crate::Parser::with_span]
//! - [`Stateful`] to thread global state through your parsers
//! - [`Partial`] can mark an input as partial buffer that is being streamed into
//! - [Custom stream types][crate::_topic::stream]

use core::hash::BuildHasher;
use core::num::NonZeroUsize;

use crate::ascii::Caseless as AsciiCaseless;
use crate::error::Needed;
use crate::lib::std::iter::{Cloned, Enumerate};
use crate::lib::std::slice::Iter;
use crate::lib::std::str::from_utf8;
use crate::lib::std::str::CharIndices;
use crate::lib::std::str::FromStr;

#[allow(unused_imports)]
#[cfg(any(feature = "unstable-doc", feature = "unstable-recover"))]
use crate::error::ErrMode;

#[cfg(feature = "alloc")]
use crate::lib::std::collections::BTreeMap;
#[cfg(feature = "alloc")]
use crate::lib::std::collections::BTreeSet;
#[cfg(feature = "std")]
use crate::lib::std::collections::HashMap;
#[cfg(feature = "std")]
use crate::lib::std::collections::HashSet;
#[cfg(feature = "alloc")]
use crate::lib::std::collections::VecDeque;
#[cfg(feature = "alloc")]
use crate::lib::std::string::String;
#[cfg(feature = "alloc")]
use crate::lib::std::vec::Vec;

mod bstr;
mod bytes;
mod locating;
mod partial;
mod range;
#[cfg(feature = "unstable-recover")]
#[cfg(feature = "std")]
mod recoverable;
mod stateful;
#[cfg(test)]
mod tests;
mod token;

pub use bstr::BStr;
pub use bytes::Bytes;
pub use locating::LocatingSlice;
pub use partial::Partial;
pub use range::Range;
#[cfg(feature = "unstable-recover")]
#[cfg(feature = "std")]
pub use recoverable::Recoverable;
pub use stateful::Stateful;
pub use token::TokenSlice;

/// UTF-8 Stream
pub type Str<'i> = &'i str;

/// Abstract method to calculate the input length
pub trait SliceLen {
    /// Calculates the input length, as indicated by its name,
    /// and the name of the trait itself
    fn slice_len(&self) -> usize;
}

impl<S: SliceLen> SliceLen for AsciiCaseless<S> {
    #[inline(always)]
    fn slice_len(&self) -> usize {
        self.0.slice_len()
    }
}

impl<T> SliceLen for &[T] {
    #[inline(always)]
    fn slice_len(&self) -> usize {
        self.len()
    }
}

impl<T, const LEN: usize> SliceLen for [T; LEN] {
    #[inline(always)]
    fn slice_len(&self) -> usize {
        self.len()
    }
}

impl<T, const LEN: usize> SliceLen for &[T; LEN] {
    #[inline(always)]
    fn slice_len(&self) -> usize {
        self.len()
    }
}

impl SliceLen for &str {
    #[inline(always)]
    fn slice_len(&self) -> usize {
        self.len()
    }
}

impl SliceLen for u8 {
    #[inline(always)]
    fn slice_len(&self) -> usize {
        1
    }
}

impl SliceLen for char {
    #[inline(always)]
    fn slice_len(&self) -> usize {
        self.len_utf8()
    }
}

impl<I> SliceLen for (I, usize, usize)
where
    I: SliceLen,
{
    #[inline(always)]
    fn slice_len(&self) -> usize {
        self.0.slice_len() * 8 + self.2 - self.1
    }
}

/// Core definition for parser input state
pub trait Stream: Offset<<Self as Stream>::Checkpoint> + crate::lib::std::fmt::Debug {
    /// The smallest unit being parsed
    ///
    /// Example: `u8` for `&[u8]` or `char` for `&str`
    type Token: crate::lib::std::fmt::Debug;
    /// Sequence of `Token`s
    ///
    /// Example: `&[u8]` for `LocatingSlice<&[u8]>` or `&str` for `LocatingSlice<&str>`
    type Slice: crate::lib::std::fmt::Debug;

    /// Iterate with the offset from the current location
    type IterOffsets: Iterator<Item = (usize, Self::Token)>;

    /// A parse location within the stream
    type Checkpoint: Offset + Clone + crate::lib::std::fmt::Debug;

    /// Iterate with the offset from the current location
    fn iter_offsets(&self) -> Self::IterOffsets;

    /// Returns the offset to the end of the input
    fn eof_offset(&self) -> usize;

    /// Split off the next token from the input
    fn next_token(&mut self) -> Option<Self::Token>;
    /// Split off the next token from the input
    fn peek_token(&self) -> Option<Self::Token>;

    /// Finds the offset of the next matching token
    fn offset_for<P>(&self, predicate: P) -> Option<usize>
    where
        P: Fn(Self::Token) -> bool;
    /// Get the offset for the number of `tokens` into the stream
    ///
    /// This means "0 tokens" will return `0` offset
    fn offset_at(&self, tokens: usize) -> Result<usize, Needed>;
    /// Split off a slice of tokens from the input
    ///
    /// <div class="warning">
    ///
    /// **Note:** For inputs with variable width tokens, like `&str`'s `char`, `offset` might not correspond
    /// with the number of tokens. To get a valid offset, use:
    /// - [`Stream::eof_offset`]
    /// - [`Stream::iter_offsets`]
    /// - [`Stream::offset_for`]
    /// - [`Stream::offset_at`]
    ///
    /// </div>
    ///
    /// # Panic
    ///
    /// This will panic if
    ///
    /// * Indexes must be within bounds of the original input;
    /// * Indexes must uphold invariants of the stream, like for `str` they must lie on UTF-8
    ///   sequence boundaries.
    ///
    fn next_slice(&mut self, offset: usize) -> Self::Slice;
    /// Split off a slice of tokens from the input
    ///
    /// <div class="warning">
    ///
    /// **Note:** For inputs with variable width tokens, like `&str`'s `char`, `offset` might not correspond
    /// with the number of tokens. To get a valid offset, use:
    /// - [`Stream::eof_offset`]
    /// - [`Stream::iter_offsets`]
    /// - [`Stream::offset_for`]
    /// - [`Stream::offset_at`]
    ///
    /// </div>
    ///
    /// # Safety
    ///
    /// Callers of this function are responsible that these preconditions are satisfied:
    ///
    /// * Indexes must be within bounds of the original input;
    /// * Indexes must uphold invariants of the stream, like for `str` they must lie on UTF-8
    ///   sequence boundaries.
    ///
    unsafe fn next_slice_unchecked(&mut self, offset: usize) -> Self::Slice {
        // Inherent impl to allow callers to have `unsafe`-free code
        self.next_slice(offset)
    }
    /// Split off a slice of tokens from the input
    fn peek_slice(&self, offset: usize) -> Self::Slice;
    /// Split off a slice of tokens from the input
    ///
    /// # Safety
    ///
    /// Callers of this function are responsible that these preconditions are satisfied:
    ///
    /// * Indexes must be within bounds of the original input;
    /// * Indexes must uphold invariants of the stream, like for `str` they must lie on UTF-8
    ///   sequence boundaries.
    unsafe fn peek_slice_unchecked(&self, offset: usize) -> Self::Slice {
        // Inherent impl to allow callers to have `unsafe`-free code
        self.peek_slice(offset)
    }

    /// Advance to the end of the stream
    #[inline(always)]
    fn finish(&mut self) -> Self::Slice {
        self.next_slice(self.eof_offset())
    }
    /// Advance to the end of the stream
    #[inline(always)]
    fn peek_finish(&self) -> Self::Slice
    where
        Self: Clone,
    {
        self.peek_slice(self.eof_offset())
    }

    /// Save the current parse location within the stream
    fn checkpoint(&self) -> Self::Checkpoint;
    /// Revert the stream to a prior [`Self::Checkpoint`]
    ///
    /// # Panic
    ///
    /// May panic if an invalid [`Self::Checkpoint`] is provided
    fn reset(&mut self, checkpoint: &Self::Checkpoint);

    /// Deprecated for callers as of 0.7.10, instead call [`Stream::trace`]
    #[deprecated(since = "0.7.10", note = "Replaced with `Stream::trace`")]
    fn raw(&self) -> &dyn crate::lib::std::fmt::Debug;

    /// Write out a single-line summary of the current parse location
    fn trace(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        #![allow(deprecated)]
        write!(f, "{:#?}", self.raw())
    }
}

impl<'i, T> Stream for &'i [T]
where
    T: Clone + crate::lib::std::fmt::Debug,
{
    type Token = T;
    type Slice = &'i [T];

    type IterOffsets = Enumerate<Cloned<Iter<'i, T>>>;

    type Checkpoint = Checkpoint<Self, Self>;

    #[inline(always)]
    fn iter_offsets(&self) -> Self::IterOffsets {
        self.iter().cloned().enumerate()
    }
    #[inline(always)]
    fn eof_offset(&self) -> usize {
        self.len()
    }

    #[inline(always)]
    fn next_token(&mut self) -> Option<Self::Token> {
        let (token, next) = self.split_first()?;
        *self = next;
        Some(token.clone())
    }

    #[inline(always)]
    fn peek_token(&self) -> Option<Self::Token> {
        if self.is_empty() {
            None
        } else {
            Some(self[0].clone())
        }
    }

    #[inline(always)]
    fn offset_for<P>(&self, predicate: P) -> Option<usize>
    where
        P: Fn(Self::Token) -> bool,
    {
        self.iter().position(|b| predicate(b.clone()))
    }
    #[inline(always)]
    fn offset_at(&self, tokens: usize) -> Result<usize, Needed> {
        if let Some(needed) = tokens.checked_sub(self.len()).and_then(NonZeroUsize::new) {
            Err(Needed::Size(needed))
        } else {
            Ok(tokens)
        }
    }
    #[inline(always)]
    fn next_slice(&mut self, offset: usize) -> Self::Slice {
        let (slice, next) = self.split_at(offset);
        *self = next;
        slice
    }
    #[inline(always)]
    unsafe fn next_slice_unchecked(&mut self, offset: usize) -> Self::Slice {
        #[cfg(debug_assertions)]
        self.peek_slice(offset);

        // SAFETY: `Stream::next_slice_unchecked` requires `offset` to be in bounds
        let slice = unsafe { self.get_unchecked(..offset) };
        // SAFETY: `Stream::next_slice_unchecked` requires `offset` to be in bounds
        let next = unsafe { self.get_unchecked(offset..) };
        *self = next;
        slice
    }
    #[inline(always)]
    fn peek_slice(&self, offset: usize) -> Self::Slice {
        &self[..offset]
    }
    #[inline(always)]
    unsafe fn peek_slice_unchecked(&self, offset: usize) -> Self::Slice {
        #[cfg(debug_assertions)]
        self.peek_slice(offset);

        // SAFETY: `Stream::next_slice_unchecked` requires `offset` to be in bounds
        let slice = unsafe { self.get_unchecked(..offset) };
        slice
    }

    #[inline(always)]
    fn checkpoint(&self) -> Self::Checkpoint {
        Checkpoint::<_, Self>::new(*self)
    }
    #[inline(always)]
    fn reset(&mut self, checkpoint: &Self::Checkpoint) {
        *self = checkpoint.inner;
    }

    #[inline(always)]
    fn raw(&self) -> &dyn crate::lib::std::fmt::Debug {
        self
    }

    fn trace(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl<'i> Stream for &'i str {
    type Token = char;
    type Slice = &'i str;

    type IterOffsets = CharIndices<'i>;

    type Checkpoint = Checkpoint<Self, Self>;

    #[inline(always)]
    fn iter_offsets(&self) -> Self::IterOffsets {
        self.char_indices()
    }
    #[inline(always)]
    fn eof_offset(&self) -> usize {
        self.len()
    }

    #[inline(always)]
    fn next_token(&mut self) -> Option<Self::Token> {
        let mut iter = self.chars();
        let c = iter.next()?;
        *self = iter.as_str();
        Some(c)
    }

    #[inline(always)]
    fn peek_token(&self) -> Option<Self::Token> {
        self.chars().next()
    }

    #[inline(always)]
    fn offset_for<P>(&self, predicate: P) -> Option<usize>
    where
        P: Fn(Self::Token) -> bool,
    {
        for (o, c) in self.iter_offsets() {
            if predicate(c) {
                return Some(o);
            }
        }
        None
    }
    #[inline]
    fn offset_at(&self, tokens: usize) -> Result<usize, Needed> {
        let mut cnt = 0;
        for (offset, _) in self.iter_offsets() {
            if cnt == tokens {
                return Ok(offset);
            }
            cnt += 1;
        }

        if cnt == tokens {
            Ok(self.eof_offset())
        } else {
            Err(Needed::Unknown)
        }
    }
    #[inline(always)]
    fn next_slice(&mut self, offset: usize) -> Self::Slice {
        let (slice, next) = self.split_at(offset);
        *self = next;
        slice
    }
    #[inline(always)]
    unsafe fn next_slice_unchecked(&mut self, offset: usize) -> Self::Slice {
        #[cfg(debug_assertions)]
        self.peek_slice(offset);

        // SAFETY: `Stream::next_slice_unchecked` requires `offset` to be in bounds and on a UTF-8
        // sequence boundary
        let slice = unsafe { self.get_unchecked(..offset) };
        // SAFETY: `Stream::next_slice_unchecked` requires `offset` to be in bounds and on a UTF-8
        // sequence boundary
        let next = unsafe { self.get_unchecked(offset..) };
        *self = next;
        slice
    }
    #[inline(always)]
    fn peek_slice(&self, offset: usize) -> Self::Slice {
        &self[..offset]
    }
    #[inline(always)]
    unsafe fn peek_slice_unchecked(&self, offset: usize) -> Self::Slice {
        #[cfg(debug_assertions)]
        self.peek_slice(offset);

        // SAFETY: `Stream::next_slice_unchecked` requires `offset` to be in bounds
        let slice = unsafe { self.get_unchecked(..offset) };
        slice
    }

    #[inline(always)]
    fn checkpoint(&self) -> Self::Checkpoint {
        Checkpoint::<_, Self>::new(*self)
    }
    #[inline(always)]
    fn reset(&mut self, checkpoint: &Self::Checkpoint) {
        *self = checkpoint.inner;
    }

    #[inline(always)]
    fn raw(&self) -> &dyn crate::lib::std::fmt::Debug {
        self
    }
}

impl<I> Stream for (I, usize)
where
    I: Stream<Token = u8> + Clone,
{
    type Token = bool;
    type Slice = (I::Slice, usize, usize);

    type IterOffsets = BitOffsets<I>;

    type Checkpoint = Checkpoint<(I::Checkpoint, usize), Self>;

    #[inline(always)]
    fn iter_offsets(&self) -> Self::IterOffsets {
        BitOffsets {
            i: self.clone(),
            o: 0,
        }
    }
    #[inline(always)]
    fn eof_offset(&self) -> usize {
        let offset = self.0.eof_offset() * 8;
        if offset == 0 {
            0
        } else {
            offset - self.1
        }
    }

    #[inline(always)]
    fn next_token(&mut self) -> Option<Self::Token> {
        next_bit(self)
    }

    #[inline(always)]
    fn peek_token(&self) -> Option<Self::Token> {
        peek_bit(self)
    }

    #[inline(always)]
    fn offset_for<P>(&self, predicate: P) -> Option<usize>
    where
        P: Fn(Self::Token) -> bool,
    {
        self.iter_offsets()
            .find_map(|(o, b)| predicate(b).then_some(o))
    }
    #[inline(always)]
    fn offset_at(&self, tokens: usize) -> Result<usize, Needed> {
        if let Some(needed) = tokens
            .checked_sub(self.eof_offset())
            .and_then(NonZeroUsize::new)
        {
            Err(Needed::Size(needed))
        } else {
            Ok(tokens)
        }
    }
    #[inline(always)]
    fn next_slice(&mut self, offset: usize) -> Self::Slice {
        let byte_offset = (offset + self.1) / 8;
        let end_offset = (offset + self.1) % 8;
        let s = self.0.next_slice(byte_offset);
        let start_offset = self.1;
        self.1 = end_offset;
        (s, start_offset, end_offset)
    }
    #[inline(always)]
    fn peek_slice(&self, offset: usize) -> Self::Slice {
        let byte_offset = (offset + self.1) / 8;
        let end_offset = (offset + self.1) % 8;
        let s = self.0.peek_slice(byte_offset);
        let start_offset = self.1;
        (s, start_offset, end_offset)
    }

    #[inline(always)]
    fn checkpoint(&self) -> Self::Checkpoint {
        Checkpoint::<_, Self>::new((self.0.checkpoint(), self.1))
    }
    #[inline(always)]
    fn reset(&mut self, checkpoint: &Self::Checkpoint) {
        self.0.reset(&checkpoint.inner.0);
        self.1 = checkpoint.inner.1;
    }

    #[inline(always)]
    fn raw(&self) -> &dyn crate::lib::std::fmt::Debug {
        &self.0
    }
}

/// Iterator for [bit][crate::binary::bits] stream (`(I, usize)`)
pub struct BitOffsets<I> {
    i: (I, usize),
    o: usize,
}

impl<I> Iterator for BitOffsets<I>
where
    I: Stream<Token = u8> + Clone,
{
    type Item = (usize, bool);
    fn next(&mut self) -> Option<Self::Item> {
        let b = next_bit(&mut self.i)?;
        let o = self.o;

        self.o += 1;

        Some((o, b))
    }
}

fn next_bit<I>(i: &mut (I, usize)) -> Option<bool>
where
    I: Stream<Token = u8> + Clone,
{
    if i.eof_offset() == 0 {
        return None;
    }
    let offset = i.1;

    let mut next_i = i.0.clone();
    let byte = next_i.next_token()?;
    let bit = (byte >> offset) & 0x1 == 0x1;

    let next_offset = offset + 1;
    if next_offset == 8 {
        i.0 = next_i;
        i.1 = 0;
        Some(bit)
    } else {
        i.1 = next_offset;
        Some(bit)
    }
}

fn peek_bit<I>(i: &(I, usize)) -> Option<bool>
where
    I: Stream<Token = u8> + Clone,
{
    if i.eof_offset() == 0 {
        return None;
    }
    let offset = i.1;

    let mut next_i = i.0.clone();
    let byte = next_i.next_token()?;
    let bit = (byte >> offset) & 0x1 == 0x1;

    let next_offset = offset + 1;
    if next_offset == 8 {
        Some(bit)
    } else {
        Some(bit)
    }
}

/// Current parse locations offset
///
/// See [`LocatingSlice`] for adding location tracking to your [`Stream`]
pub trait Location {
    /// Previous token's end offset
    fn previous_token_end(&self) -> usize;
    /// Current token's start offset
    fn current_token_start(&self) -> usize;
}

/// Capture top-level errors in the middle of parsing so parsing can resume
///
/// See [`Recoverable`] for adding error recovery tracking to your [`Stream`]
#[cfg(feature = "unstable-recover")]
#[cfg(feature = "std")]
pub trait Recover<E>: Stream {
    /// Capture a top-level error
    ///
    /// May return `Err(err)` if recovery is not possible (e.g. if [`Recover::is_recovery_supported`]
    /// returns `false`).
    fn record_err(
        &mut self,
        token_start: &Self::Checkpoint,
        err_start: &Self::Checkpoint,
        err: E,
    ) -> Result<(), E>;

    /// Report whether the [`Stream`] can save off errors for recovery
    fn is_recovery_supported() -> bool;
}

#[cfg(feature = "unstable-recover")]
#[cfg(feature = "std")]
impl<'a, T, E> Recover<E> for &'a [T]
where
    &'a [T]: Stream,
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

#[cfg(feature = "unstable-recover")]
#[cfg(feature = "std")]
impl<E> Recover<E> for &str {
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

#[cfg(feature = "unstable-recover")]
#[cfg(feature = "std")]
impl<I, E> Recover<E> for (I, usize)
where
    I: Recover<E>,
    I: Stream<Token = u8> + Clone,
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

/// Marks the input as being the complete buffer or a partial buffer for streaming input
///
/// See [`Partial`] for marking a presumed complete buffer type as a streaming buffer.
pub trait StreamIsPartial: Sized {
    /// Whether the stream is currently partial or complete
    type PartialState;

    /// Mark the stream is complete
    #[must_use]
    fn complete(&mut self) -> Self::PartialState;

    /// Restore the stream back to its previous state
    fn restore_partial(&mut self, state: Self::PartialState);

    /// Report whether the [`Stream`] is can ever be incomplete
    fn is_partial_supported() -> bool;

    /// Report whether the [`Stream`] is currently incomplete
    #[inline(always)]
    fn is_partial(&self) -> bool {
        Self::is_partial_supported()
    }
}

impl<T> StreamIsPartial for &[T] {
    type PartialState = ();

    #[inline]
    fn complete(&mut self) -> Self::PartialState {}

    #[inline]
    fn restore_partial(&mut self, _state: Self::PartialState) {}

    #[inline(always)]
    fn is_partial_supported() -> bool {
        false
    }
}

impl StreamIsPartial for &str {
    type PartialState = ();

    #[inline]
    fn complete(&mut self) -> Self::PartialState {
        // Already complete
    }

    #[inline]
    fn restore_partial(&mut self, _state: Self::PartialState) {}

    #[inline(always)]
    fn is_partial_supported() -> bool {
        false
    }
}

impl<I> StreamIsPartial for (I, usize)
where
    I: StreamIsPartial,
{
    type PartialState = I::PartialState;

    #[inline]
    fn complete(&mut self) -> Self::PartialState {
        self.0.complete()
    }

    #[inline]
    fn restore_partial(&mut self, state: Self::PartialState) {
        self.0.restore_partial(state);
    }

    #[inline(always)]
    fn is_partial_supported() -> bool {
        I::is_partial_supported()
    }

    #[inline(always)]
    fn is_partial(&self) -> bool {
        self.0.is_partial()
    }
}

/// Useful functions to calculate the offset between slices and show a hexdump of a slice
pub trait Offset<Start = Self> {
    /// Offset between the first byte of `start` and the first byte of `self`a
    ///
    /// <div class="warning">
    ///
    /// **Note:** This is an offset, not an index, and may point to the end of input
    /// (`start.len()`) when `self` is exhausted.
    ///
    /// </div>
    fn offset_from(&self, start: &Start) -> usize;
}

impl<T> Offset for &[T] {
    #[inline]
    fn offset_from(&self, start: &Self) -> usize {
        let fst = (*start).as_ptr();
        let snd = (*self).as_ptr();

        debug_assert!(
            fst <= snd,
            "`Offset::offset_from({snd:?}, {fst:?})` only accepts slices of `self`"
        );
        (snd as usize - fst as usize) / crate::lib::std::mem::size_of::<T>()
    }
}

impl<'a, T> Offset<<&'a [T] as Stream>::Checkpoint> for &'a [T]
where
    T: Clone + crate::lib::std::fmt::Debug,
{
    #[inline(always)]
    fn offset_from(&self, other: &<&'a [T] as Stream>::Checkpoint) -> usize {
        self.checkpoint().offset_from(other)
    }
}

impl Offset for &str {
    #[inline(always)]
    fn offset_from(&self, start: &Self) -> usize {
        self.as_bytes().offset_from(&start.as_bytes())
    }
}

impl<'a> Offset<<&'a str as Stream>::Checkpoint> for &'a str {
    #[inline(always)]
    fn offset_from(&self, other: &<&'a str as Stream>::Checkpoint) -> usize {
        self.checkpoint().offset_from(other)
    }
}

impl<I> Offset for (I, usize)
where
    I: Offset,
{
    #[inline(always)]
    fn offset_from(&self, start: &Self) -> usize {
        self.0.offset_from(&start.0) * 8 + self.1 - start.1
    }
}

impl<I> Offset<<(I, usize) as Stream>::Checkpoint> for (I, usize)
where
    I: Stream<Token = u8> + Clone,
{
    #[inline(always)]
    fn offset_from(&self, other: &<(I, usize) as Stream>::Checkpoint) -> usize {
        self.checkpoint().offset_from(other)
    }
}

impl<I, S> Offset for Checkpoint<I, S>
where
    I: Offset,
{
    #[inline(always)]
    fn offset_from(&self, start: &Self) -> usize {
        self.inner.offset_from(&start.inner)
    }
}

/// Helper trait for types that can be viewed as a byte slice
pub trait AsBytes {
    /// Casts the input type to a byte slice
    fn as_bytes(&self) -> &[u8];
}

impl AsBytes for &[u8] {
    #[inline(always)]
    fn as_bytes(&self) -> &[u8] {
        self
    }
}

/// Helper trait for types that can be viewed as a byte slice
pub trait AsBStr {
    /// Casts the input type to a byte slice
    fn as_bstr(&self) -> &[u8];
}

impl AsBStr for &[u8] {
    #[inline(always)]
    fn as_bstr(&self) -> &[u8] {
        self
    }
}

impl AsBStr for &str {
    #[inline(always)]
    fn as_bstr(&self) -> &[u8] {
        (*self).as_bytes()
    }
}

/// Result of [`Compare::compare`]
#[derive(Debug, Eq, PartialEq)]
pub enum CompareResult {
    /// Comparison was successful
    ///
    /// `usize` is the end of the successful match within the buffer.
    /// This is most relevant for caseless UTF-8 where `Compare::compare`'s parameter might be a different
    /// length than the match within the buffer.
    Ok(usize),
    /// We need more data to be sure
    Incomplete,
    /// Comparison failed
    Error,
}

/// Abstracts comparison operations
pub trait Compare<T> {
    /// Compares self to another value for equality
    fn compare(&self, t: T) -> CompareResult;
}

impl<'b> Compare<&'b [u8]> for &[u8] {
    #[inline]
    fn compare(&self, t: &'b [u8]) -> CompareResult {
        if t.iter().zip(*self).any(|(a, b)| a != b) {
            CompareResult::Error
        } else if self.len() < t.slice_len() {
            CompareResult::Incomplete
        } else {
            CompareResult::Ok(t.slice_len())
        }
    }
}

impl<'b> Compare<AsciiCaseless<&'b [u8]>> for &[u8] {
    #[inline]
    fn compare(&self, t: AsciiCaseless<&'b [u8]>) -> CompareResult {
        if t.0
            .iter()
            .zip(*self)
            .any(|(a, b)| !a.eq_ignore_ascii_case(b))
        {
            CompareResult::Error
        } else if self.len() < t.slice_len() {
            CompareResult::Incomplete
        } else {
            CompareResult::Ok(t.slice_len())
        }
    }
}

impl<const LEN: usize> Compare<[u8; LEN]> for &[u8] {
    #[inline(always)]
    fn compare(&self, t: [u8; LEN]) -> CompareResult {
        self.compare(&t[..])
    }
}

impl<const LEN: usize> Compare<AsciiCaseless<[u8; LEN]>> for &[u8] {
    #[inline(always)]
    fn compare(&self, t: AsciiCaseless<[u8; LEN]>) -> CompareResult {
        self.compare(AsciiCaseless(&t.0[..]))
    }
}

impl<'b, const LEN: usize> Compare<&'b [u8; LEN]> for &[u8] {
    #[inline(always)]
    fn compare(&self, t: &'b [u8; LEN]) -> CompareResult {
        self.compare(&t[..])
    }
}

impl<'b, const LEN: usize> Compare<AsciiCaseless<&'b [u8; LEN]>> for &[u8] {
    #[inline(always)]
    fn compare(&self, t: AsciiCaseless<&'b [u8; LEN]>) -> CompareResult {
        self.compare(AsciiCaseless(&t.0[..]))
    }
}

impl<'b> Compare<&'b str> for &[u8] {
    #[inline(always)]
    fn compare(&self, t: &'b str) -> CompareResult {
        self.compare(t.as_bytes())
    }
}

impl<'b> Compare<AsciiCaseless<&'b str>> for &[u8] {
    #[inline(always)]
    fn compare(&self, t: AsciiCaseless<&'b str>) -> CompareResult {
        self.compare(AsciiCaseless(t.0.as_bytes()))
    }
}

impl Compare<u8> for &[u8] {
    #[inline]
    fn compare(&self, t: u8) -> CompareResult {
        match self.first().copied() {
            Some(c) if t == c => CompareResult::Ok(t.slice_len()),
            Some(_) => CompareResult::Error,
            None => CompareResult::Incomplete,
        }
    }
}

impl Compare<AsciiCaseless<u8>> for &[u8] {
    #[inline]
    fn compare(&self, t: AsciiCaseless<u8>) -> CompareResult {
        match self.first() {
            Some(c) if t.0.eq_ignore_ascii_case(c) => CompareResult::Ok(t.slice_len()),
            Some(_) => CompareResult::Error,
            None => CompareResult::Incomplete,
        }
    }
}

impl Compare<char> for &[u8] {
    #[inline(always)]
    fn compare(&self, t: char) -> CompareResult {
        self.compare(t.encode_utf8(&mut [0; 4]).as_bytes())
    }
}

impl Compare<AsciiCaseless<char>> for &[u8] {
    #[inline(always)]
    fn compare(&self, t: AsciiCaseless<char>) -> CompareResult {
        self.compare(AsciiCaseless(t.0.encode_utf8(&mut [0; 4]).as_bytes()))
    }
}

impl<'b> Compare<&'b str> for &str {
    #[inline(always)]
    fn compare(&self, t: &'b str) -> CompareResult {
        self.as_bytes().compare(t.as_bytes())
    }
}

impl<'b> Compare<AsciiCaseless<&'b str>> for &str {
    #[inline(always)]
    fn compare(&self, t: AsciiCaseless<&'b str>) -> CompareResult {
        self.as_bytes().compare(t.as_bytes())
    }
}

impl Compare<char> for &str {
    #[inline(always)]
    fn compare(&self, t: char) -> CompareResult {
        self.as_bytes().compare(t)
    }
}

impl Compare<AsciiCaseless<char>> for &str {
    #[inline(always)]
    fn compare(&self, t: AsciiCaseless<char>) -> CompareResult {
        self.as_bytes().compare(t)
    }
}

/// Look for a slice in self
pub trait FindSlice<T> {
    /// Returns the offset of the slice if it is found
    fn find_slice(&self, substr: T) -> Option<crate::lib::std::ops::Range<usize>>;
}

impl<'s> FindSlice<&'s [u8]> for &[u8] {
    #[inline(always)]
    fn find_slice(&self, substr: &'s [u8]) -> Option<crate::lib::std::ops::Range<usize>> {
        memmem(self, substr)
    }
}

impl<'s> FindSlice<(&'s [u8],)> for &[u8] {
    #[inline(always)]
    fn find_slice(&self, substr: (&'s [u8],)) -> Option<crate::lib::std::ops::Range<usize>> {
        memmem(self, substr.0)
    }
}

impl<'s> FindSlice<(&'s [u8], &'s [u8])> for &[u8] {
    #[inline(always)]
    fn find_slice(
        &self,
        substr: (&'s [u8], &'s [u8]),
    ) -> Option<crate::lib::std::ops::Range<usize>> {
        memmem2(self, substr)
    }
}

impl<'s> FindSlice<(&'s [u8], &'s [u8], &'s [u8])> for &[u8] {
    #[inline(always)]
    fn find_slice(
        &self,
        substr: (&'s [u8], &'s [u8], &'s [u8]),
    ) -> Option<crate::lib::std::ops::Range<usize>> {
        memmem3(self, substr)
    }
}

impl FindSlice<char> for &[u8] {
    #[inline(always)]
    fn find_slice(&self, substr: char) -> Option<crate::lib::std::ops::Range<usize>> {
        let mut b = [0; 4];
        let substr = substr.encode_utf8(&mut b);
        self.find_slice(&*substr)
    }
}

impl FindSlice<(char,)> for &[u8] {
    #[inline(always)]
    fn find_slice(&self, substr: (char,)) -> Option<crate::lib::std::ops::Range<usize>> {
        let mut b = [0; 4];
        let substr0 = substr.0.encode_utf8(&mut b);
        self.find_slice((&*substr0,))
    }
}

impl FindSlice<(char, char)> for &[u8] {
    #[inline(always)]
    fn find_slice(&self, substr: (char, char)) -> Option<crate::lib::std::ops::Range<usize>> {
        let mut b = [0; 4];
        let substr0 = substr.0.encode_utf8(&mut b);
        let mut b = [0; 4];
        let substr1 = substr.1.encode_utf8(&mut b);
        self.find_slice((&*substr0, &*substr1))
    }
}

impl FindSlice<(char, char, char)> for &[u8] {
    #[inline(always)]
    fn find_slice(&self, substr: (char, char, char)) -> Option<crate::lib::std::ops::Range<usize>> {
        let mut b = [0; 4];
        let substr0 = substr.0.encode_utf8(&mut b);
        let mut b = [0; 4];
        let substr1 = substr.1.encode_utf8(&mut b);
        let mut b = [0; 4];
        let substr2 = substr.2.encode_utf8(&mut b);
        self.find_slice((&*substr0, &*substr1, &*substr2))
    }
}

impl FindSlice<u8> for &[u8] {
    #[inline(always)]
    fn find_slice(&self, substr: u8) -> Option<crate::lib::std::ops::Range<usize>> {
        memchr(substr, self).map(|i| i..i + 1)
    }
}

impl FindSlice<(u8,)> for &[u8] {
    #[inline(always)]
    fn find_slice(&self, substr: (u8,)) -> Option<crate::lib::std::ops::Range<usize>> {
        memchr(substr.0, self).map(|i| i..i + 1)
    }
}

impl FindSlice<(u8, u8)> for &[u8] {
    #[inline(always)]
    fn find_slice(&self, substr: (u8, u8)) -> Option<crate::lib::std::ops::Range<usize>> {
        memchr2(substr, self).map(|i| i..i + 1)
    }
}

impl FindSlice<(u8, u8, u8)> for &[u8] {
    #[inline(always)]
    fn find_slice(&self, substr: (u8, u8, u8)) -> Option<crate::lib::std::ops::Range<usize>> {
        memchr3(substr, self).map(|i| i..i + 1)
    }
}

impl<'s> FindSlice<&'s str> for &[u8] {
    #[inline(always)]
    fn find_slice(&self, substr: &'s str) -> Option<crate::lib::std::ops::Range<usize>> {
        self.find_slice(substr.as_bytes())
    }
}

impl<'s> FindSlice<(&'s str,)> for &[u8] {
    #[inline(always)]
    fn find_slice(&self, substr: (&'s str,)) -> Option<crate::lib::std::ops::Range<usize>> {
        memmem(self, substr.0.as_bytes())
    }
}

impl<'s> FindSlice<(&'s str, &'s str)> for &[u8] {
    #[inline(always)]
    fn find_slice(&self, substr: (&'s str, &'s str)) -> Option<crate::lib::std::ops::Range<usize>> {
        memmem2(self, (substr.0.as_bytes(), substr.1.as_bytes()))
    }
}

impl<'s> FindSlice<(&'s str, &'s str, &'s str)> for &[u8] {
    #[inline(always)]
    fn find_slice(
        &self,
        substr: (&'s str, &'s str, &'s str),
    ) -> Option<crate::lib::std::ops::Range<usize>> {
        memmem3(
            self,
            (
                substr.0.as_bytes(),
                substr.1.as_bytes(),
                substr.2.as_bytes(),
            ),
        )
    }
}

impl<'s> FindSlice<&'s str> for &str {
    #[inline(always)]
    fn find_slice(&self, substr: &'s str) -> Option<crate::lib::std::ops::Range<usize>> {
        self.as_bytes().find_slice(substr)
    }
}

impl<'s> FindSlice<(&'s str,)> for &str {
    #[inline(always)]
    fn find_slice(&self, substr: (&'s str,)) -> Option<crate::lib::std::ops::Range<usize>> {
        self.as_bytes().find_slice(substr)
    }
}

impl<'s> FindSlice<(&'s str, &'s str)> for &str {
    #[inline(always)]
    fn find_slice(&self, substr: (&'s str, &'s str)) -> Option<crate::lib::std::ops::Range<usize>> {
        self.as_bytes().find_slice(substr)
    }
}

impl<'s> FindSlice<(&'s str, &'s str, &'s str)> for &str {
    #[inline(always)]
    fn find_slice(
        &self,
        substr: (&'s str, &'s str, &'s str),
    ) -> Option<crate::lib::std::ops::Range<usize>> {
        self.as_bytes().find_slice(substr)
    }
}

impl FindSlice<char> for &str {
    #[inline(always)]
    fn find_slice(&self, substr: char) -> Option<crate::lib::std::ops::Range<usize>> {
        self.as_bytes().find_slice(substr)
    }
}

impl FindSlice<(char,)> for &str {
    #[inline(always)]
    fn find_slice(&self, substr: (char,)) -> Option<crate::lib::std::ops::Range<usize>> {
        self.as_bytes().find_slice(substr)
    }
}

impl FindSlice<(char, char)> for &str {
    #[inline(always)]
    fn find_slice(&self, substr: (char, char)) -> Option<crate::lib::std::ops::Range<usize>> {
        self.as_bytes().find_slice(substr)
    }
}

impl FindSlice<(char, char, char)> for &str {
    #[inline(always)]
    fn find_slice(&self, substr: (char, char, char)) -> Option<crate::lib::std::ops::Range<usize>> {
        self.as_bytes().find_slice(substr)
    }
}

/// Used to integrate `str`'s `parse()` method
pub trait ParseSlice<R> {
    /// Succeeds if `parse()` succeeded
    ///
    /// The byte slice implementation will first convert it to a `&str`, then apply the `parse()`
    /// function
    fn parse_slice(&self) -> Option<R>;
}

impl<R: FromStr> ParseSlice<R> for &[u8] {
    #[inline(always)]
    fn parse_slice(&self) -> Option<R> {
        from_utf8(self).ok().and_then(|s| s.parse().ok())
    }
}

impl<R: FromStr> ParseSlice<R> for &str {
    #[inline(always)]
    fn parse_slice(&self) -> Option<R> {
        self.parse().ok()
    }
}

/// Convert a `Stream` into an appropriate `Output` type
pub trait UpdateSlice: Stream {
    /// Convert an `Output` type to be used as `Stream`
    fn update_slice(self, inner: Self::Slice) -> Self;
}

impl<T> UpdateSlice for &[T]
where
    T: Clone + crate::lib::std::fmt::Debug,
{
    #[inline(always)]
    fn update_slice(self, inner: Self::Slice) -> Self {
        inner
    }
}

impl UpdateSlice for &str {
    #[inline(always)]
    fn update_slice(self, inner: Self::Slice) -> Self {
        inner
    }
}

/// Ensure checkpoint details are kept private
pub struct Checkpoint<T, S> {
    inner: T,
    stream: core::marker::PhantomData<S>,
}

impl<T, S> Checkpoint<T, S> {
    fn new(inner: T) -> Self {
        Self {
            inner,
            stream: Default::default(),
        }
    }
}

impl<T: Copy, S> Copy for Checkpoint<T, S> {}

impl<T: Clone, S> Clone for Checkpoint<T, S> {
    #[inline(always)]
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            stream: Default::default(),
        }
    }
}

impl<T: PartialOrd, S> PartialOrd for Checkpoint<T, S> {
    #[inline(always)]
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        self.inner.partial_cmp(&other.inner)
    }
}

impl<T: Ord, S> Ord for Checkpoint<T, S> {
    #[inline(always)]
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.inner.cmp(&other.inner)
    }
}

impl<T: PartialEq, S> PartialEq for Checkpoint<T, S> {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.inner.eq(&other.inner)
    }
}

impl<T: Eq, S> Eq for Checkpoint<T, S> {}

impl<T: crate::lib::std::fmt::Debug, S> crate::lib::std::fmt::Debug for Checkpoint<T, S> {
    fn fmt(&self, f: &mut crate::lib::std::fmt::Formatter<'_>) -> crate::lib::std::fmt::Result {
        self.inner.fmt(f)
    }
}

/// Abstracts something which can extend an `Extend`.
/// Used to build modified input slices in `escaped_transform`
pub trait Accumulate<T>: Sized {
    /// Create a new `Extend` of the correct type
    fn initial(capacity: Option<usize>) -> Self;
    /// Accumulate the input into an accumulator
    fn accumulate(&mut self, acc: T);
}

impl<T> Accumulate<T> for () {
    #[inline(always)]
    fn initial(_capacity: Option<usize>) -> Self {}
    #[inline(always)]
    fn accumulate(&mut self, _acc: T) {}
}

impl<T> Accumulate<T> for usize {
    #[inline(always)]
    fn initial(_capacity: Option<usize>) -> Self {
        0
    }
    #[inline(always)]
    fn accumulate(&mut self, _acc: T) {
        *self += 1;
    }
}

#[cfg(feature = "alloc")]
impl<T> Accumulate<T> for Vec<T> {
    #[inline(always)]
    fn initial(capacity: Option<usize>) -> Self {
        match capacity {
            Some(capacity) => Vec::with_capacity(clamp_capacity::<T>(capacity)),
            None => Vec::new(),
        }
    }
    #[inline(always)]
    fn accumulate(&mut self, acc: T) {
        self.push(acc);
    }
}

#[cfg(feature = "alloc")]
impl<'i, T: Clone> Accumulate<&'i [T]> for Vec<T> {
    #[inline(always)]
    fn initial(capacity: Option<usize>) -> Self {
        match capacity {
            Some(capacity) => Vec::with_capacity(clamp_capacity::<T>(capacity)),
            None => Vec::new(),
        }
    }
    #[inline(always)]
    fn accumulate(&mut self, acc: &'i [T]) {
        self.extend(acc.iter().cloned());
    }
}

#[cfg(feature = "alloc")]
impl Accumulate<char> for String {
    #[inline(always)]
    fn initial(capacity: Option<usize>) -> Self {
        match capacity {
            Some(capacity) => String::with_capacity(clamp_capacity::<char>(capacity)),
            None => String::new(),
        }
    }
    #[inline(always)]
    fn accumulate(&mut self, acc: char) {
        self.push(acc);
    }
}

#[cfg(feature = "alloc")]
impl<'i> Accumulate<&'i str> for String {
    #[inline(always)]
    fn initial(capacity: Option<usize>) -> Self {
        match capacity {
            Some(capacity) => String::with_capacity(clamp_capacity::<char>(capacity)),
            None => String::new(),
        }
    }
    #[inline(always)]
    fn accumulate(&mut self, acc: &'i str) {
        self.push_str(acc);
    }
}

#[cfg(feature = "alloc")]
impl<K, V> Accumulate<(K, V)> for BTreeMap<K, V>
where
    K: crate::lib::std::cmp::Ord,
{
    #[inline(always)]
    fn initial(_capacity: Option<usize>) -> Self {
        BTreeMap::new()
    }
    #[inline(always)]
    fn accumulate(&mut self, (key, value): (K, V)) {
        self.insert(key, value);
    }
}

#[cfg(feature = "std")]
impl<K, V, S> Accumulate<(K, V)> for HashMap<K, V, S>
where
    K: crate::lib::std::cmp::Eq + crate::lib::std::hash::Hash,
    S: BuildHasher + Default,
{
    #[inline(always)]
    fn initial(capacity: Option<usize>) -> Self {
        let h = S::default();
        match capacity {
            Some(capacity) => {
                HashMap::with_capacity_and_hasher(clamp_capacity::<(K, V)>(capacity), h)
            }
            None => HashMap::with_hasher(h),
        }
    }
    #[inline(always)]
    fn accumulate(&mut self, (key, value): (K, V)) {
        self.insert(key, value);
    }
}

#[cfg(feature = "alloc")]
impl<K> Accumulate<K> for BTreeSet<K>
where
    K: crate::lib::std::cmp::Ord,
{
    #[inline(always)]
    fn initial(_capacity: Option<usize>) -> Self {
        BTreeSet::new()
    }
    #[inline(always)]
    fn accumulate(&mut self, key: K) {
        self.insert(key);
    }
}

#[cfg(feature = "std")]
impl<K, S> Accumulate<K> for HashSet<K, S>
where
    K: crate::lib::std::cmp::Eq + crate::lib::std::hash::Hash,
    S: BuildHasher + Default,
{
    #[inline(always)]
    fn initial(capacity: Option<usize>) -> Self {
        let h = S::default();
        match capacity {
            Some(capacity) => HashSet::with_capacity_and_hasher(clamp_capacity::<K>(capacity), h),
            None => HashSet::with_hasher(h),
        }
    }
    #[inline(always)]
    fn accumulate(&mut self, key: K) {
        self.insert(key);
    }
}

#[cfg(feature = "alloc")]
impl<'i, T: Clone> Accumulate<&'i [T]> for VecDeque<T> {
    #[inline(always)]
    fn initial(capacity: Option<usize>) -> Self {
        match capacity {
            Some(capacity) => VecDeque::with_capacity(clamp_capacity::<T>(capacity)),
            None => VecDeque::new(),
        }
    }
    #[inline(always)]
    fn accumulate(&mut self, acc: &'i [T]) {
        self.extend(acc.iter().cloned());
    }
}

#[cfg(feature = "alloc")]
#[inline]
pub(crate) fn clamp_capacity<T>(capacity: usize) -> usize {
    /// Don't pre-allocate more than 64KiB when calling `Vec::with_capacity`.
    ///
    /// Pre-allocating memory is a nice optimization but count fields can't
    /// always be trusted. We should clamp initial capacities to some reasonable
    /// amount. This reduces the risk of a bogus count value triggering a panic
    /// due to an OOM error.
    ///
    /// This does not affect correctness. `winnow` will always read the full number
    /// of elements regardless of the capacity cap.
    const MAX_INITIAL_CAPACITY_BYTES: usize = 65536;

    let max_initial_capacity =
        MAX_INITIAL_CAPACITY_BYTES / crate::lib::std::mem::size_of::<T>().max(1);
    capacity.min(max_initial_capacity)
}

/// Helper trait to convert numbers to usize.
///
/// By default, usize implements `From<u8>` and `From<u16>` but not
/// `From<u32>` and `From<u64>` because that would be invalid on some
/// platforms. This trait implements the conversion for platforms
/// with 32 and 64 bits pointer platforms
pub trait ToUsize {
    /// converts self to usize
    fn to_usize(&self) -> usize;
}

impl ToUsize for u8 {
    #[inline(always)]
    fn to_usize(&self) -> usize {
        *self as usize
    }
}

impl ToUsize for u16 {
    #[inline(always)]
    fn to_usize(&self) -> usize {
        *self as usize
    }
}

impl ToUsize for usize {
    #[inline(always)]
    fn to_usize(&self) -> usize {
        *self
    }
}

#[cfg(any(target_pointer_width = "32", target_pointer_width = "64"))]
impl ToUsize for u32 {
    #[inline(always)]
    fn to_usize(&self) -> usize {
        *self as usize
    }
}

#[cfg(target_pointer_width = "64")]
impl ToUsize for u64 {
    #[inline(always)]
    fn to_usize(&self) -> usize {
        *self as usize
    }
}

/// Transforms a token into a char for basic string parsing
#[allow(clippy::len_without_is_empty)]
#[allow(clippy::wrong_self_convention)]
pub trait AsChar {
    /// Makes a char from self
    ///
    /// # Example
    ///
    /// ```
    /// use winnow::prelude::*;
    ///
    /// assert_eq!('a'.as_char(), 'a');
    /// assert_eq!(u8::MAX.as_char(), std::char::from_u32(u8::MAX as u32).unwrap());
    /// ```
    fn as_char(self) -> char;

    /// Tests that self is an alphabetic character
    ///
    /// <div class="warning">
    ///
    /// **Warning:** for `&str` it matches alphabetic
    /// characters outside of the 52 ASCII letters
    ///
    /// </div>
    fn is_alpha(self) -> bool;

    /// Tests that self is an alphabetic character
    /// or a decimal digit
    fn is_alphanum(self) -> bool;
    /// Tests that self is a decimal digit
    fn is_dec_digit(self) -> bool;
    /// Tests that self is an hex digit
    fn is_hex_digit(self) -> bool;
    /// Tests that self is an octal digit
    fn is_oct_digit(self) -> bool;
    /// Gets the len in bytes for self
    fn len(self) -> usize;
    /// Tests that self is ASCII space or tab
    fn is_space(self) -> bool;
    /// Tests if byte is ASCII newline: \n
    fn is_newline(self) -> bool;
}

impl AsChar for u8 {
    #[inline(always)]
    fn as_char(self) -> char {
        self as char
    }
    #[inline]
    fn is_alpha(self) -> bool {
        matches!(self, 0x41..=0x5A | 0x61..=0x7A)
    }
    #[inline]
    fn is_alphanum(self) -> bool {
        self.is_alpha() || self.is_dec_digit()
    }
    #[inline]
    fn is_dec_digit(self) -> bool {
        matches!(self, 0x30..=0x39)
    }
    #[inline]
    fn is_hex_digit(self) -> bool {
        matches!(self, 0x30..=0x39 | 0x41..=0x46 | 0x61..=0x66)
    }
    #[inline]
    fn is_oct_digit(self) -> bool {
        matches!(self, 0x30..=0x37)
    }
    #[inline]
    fn len(self) -> usize {
        1
    }
    #[inline]
    fn is_space(self) -> bool {
        self == b' ' || self == b'\t'
    }
    #[inline]
    fn is_newline(self) -> bool {
        self == b'\n'
    }
}

impl AsChar for &u8 {
    #[inline(always)]
    fn as_char(self) -> char {
        (*self).as_char()
    }
    #[inline(always)]
    fn is_alpha(self) -> bool {
        (*self).is_alpha()
    }
    #[inline(always)]
    fn is_alphanum(self) -> bool {
        (*self).is_alphanum()
    }
    #[inline(always)]
    fn is_dec_digit(self) -> bool {
        (*self).is_dec_digit()
    }
    #[inline(always)]
    fn is_hex_digit(self) -> bool {
        (*self).is_hex_digit()
    }
    #[inline(always)]
    fn is_oct_digit(self) -> bool {
        (*self).is_oct_digit()
    }
    #[inline(always)]
    fn len(self) -> usize {
        (*self).len()
    }
    #[inline(always)]
    fn is_space(self) -> bool {
        (*self).is_space()
    }
    #[inline(always)]
    fn is_newline(self) -> bool {
        (*self).is_newline()
    }
}

impl AsChar for char {
    #[inline(always)]
    fn as_char(self) -> char {
        self
    }
    #[inline]
    fn is_alpha(self) -> bool {
        self.is_ascii_alphabetic()
    }
    #[inline]
    fn is_alphanum(self) -> bool {
        self.is_alpha() || self.is_dec_digit()
    }
    #[inline]
    fn is_dec_digit(self) -> bool {
        self.is_ascii_digit()
    }
    #[inline]
    fn is_hex_digit(self) -> bool {
        self.is_ascii_hexdigit()
    }
    #[inline]
    fn is_oct_digit(self) -> bool {
        self.is_digit(8)
    }
    #[inline]
    fn len(self) -> usize {
        self.len_utf8()
    }
    #[inline]
    fn is_space(self) -> bool {
        self == ' ' || self == '\t'
    }
    #[inline]
    fn is_newline(self) -> bool {
        self == '\n'
    }
}

impl AsChar for &char {
    #[inline(always)]
    fn as_char(self) -> char {
        (*self).as_char()
    }
    #[inline(always)]
    fn is_alpha(self) -> bool {
        (*self).is_alpha()
    }
    #[inline(always)]
    fn is_alphanum(self) -> bool {
        (*self).is_alphanum()
    }
    #[inline(always)]
    fn is_dec_digit(self) -> bool {
        (*self).is_dec_digit()
    }
    #[inline(always)]
    fn is_hex_digit(self) -> bool {
        (*self).is_hex_digit()
    }
    #[inline(always)]
    fn is_oct_digit(self) -> bool {
        (*self).is_oct_digit()
    }
    #[inline(always)]
    fn len(self) -> usize {
        (*self).len()
    }
    #[inline(always)]
    fn is_space(self) -> bool {
        (*self).is_space()
    }
    #[inline(always)]
    fn is_newline(self) -> bool {
        (*self).is_newline()
    }
}

/// Check if a token is in a set of possible tokens
///
/// While this can be implemented manually, you can also build up sets using:
/// - `b'c'` and `'c'`
/// - `b""`
/// - `|c| true`
/// - `b'a'..=b'z'`, `'a'..='z'` (etc for each [range type][std::ops])
/// - `(set1, set2, ...)`
///
/// # Example
///
/// For example, you could implement `hex_digit0` as:
/// ```
/// # use winnow::prelude::*;
/// # use winnow::{error::ErrMode, error::ContextError};
/// # use winnow::token::take_while;
/// fn hex_digit1<'s>(input: &mut &'s str) -> ModalResult<&'s str, ContextError> {
///     take_while(1.., ('a'..='f', 'A'..='F', '0'..='9')).parse_next(input)
/// }
///
/// assert_eq!(hex_digit1.parse_peek("21cZ"), Ok(("Z", "21c")));
/// assert!(hex_digit1.parse_peek("H2").is_err());
/// assert!(hex_digit1.parse_peek("").is_err());
/// ```
pub trait ContainsToken<T> {
    /// Returns true if self contains the token
    fn contains_token(&self, token: T) -> bool;
}

impl ContainsToken<u8> for u8 {
    #[inline(always)]
    fn contains_token(&self, token: u8) -> bool {
        *self == token
    }
}

impl ContainsToken<&u8> for u8 {
    #[inline(always)]
    fn contains_token(&self, token: &u8) -> bool {
        self.contains_token(*token)
    }
}

impl ContainsToken<char> for u8 {
    #[inline(always)]
    fn contains_token(&self, token: char) -> bool {
        self.as_char() == token
    }
}

impl ContainsToken<&char> for u8 {
    #[inline(always)]
    fn contains_token(&self, token: &char) -> bool {
        self.contains_token(*token)
    }
}

impl<C: AsChar> ContainsToken<C> for char {
    #[inline(always)]
    fn contains_token(&self, token: C) -> bool {
        *self == token.as_char()
    }
}

impl<C, F: Fn(C) -> bool> ContainsToken<C> for F {
    #[inline(always)]
    fn contains_token(&self, token: C) -> bool {
        self(token)
    }
}

impl<C1: AsChar, C2: AsChar + Clone> ContainsToken<C1> for crate::lib::std::ops::Range<C2> {
    #[inline(always)]
    fn contains_token(&self, token: C1) -> bool {
        let start = self.start.clone().as_char();
        let end = self.end.clone().as_char();
        (start..end).contains(&token.as_char())
    }
}

impl<C1: AsChar, C2: AsChar + Clone> ContainsToken<C1>
    for crate::lib::std::ops::RangeInclusive<C2>
{
    #[inline(always)]
    fn contains_token(&self, token: C1) -> bool {
        let start = self.start().clone().as_char();
        let end = self.end().clone().as_char();
        (start..=end).contains(&token.as_char())
    }
}

impl<C1: AsChar, C2: AsChar + Clone> ContainsToken<C1> for crate::lib::std::ops::RangeFrom<C2> {
    #[inline(always)]
    fn contains_token(&self, token: C1) -> bool {
        let start = self.start.clone().as_char();
        (start..).contains(&token.as_char())
    }
}

impl<C1: AsChar, C2: AsChar + Clone> ContainsToken<C1> for crate::lib::std::ops::RangeTo<C2> {
    #[inline(always)]
    fn contains_token(&self, token: C1) -> bool {
        let end = self.end.clone().as_char();
        (..end).contains(&token.as_char())
    }
}

impl<C1: AsChar, C2: AsChar + Clone> ContainsToken<C1>
    for crate::lib::std::ops::RangeToInclusive<C2>
{
    #[inline(always)]
    fn contains_token(&self, token: C1) -> bool {
        let end = self.end.clone().as_char();
        (..=end).contains(&token.as_char())
    }
}

impl<C1: AsChar> ContainsToken<C1> for crate::lib::std::ops::RangeFull {
    #[inline(always)]
    fn contains_token(&self, _token: C1) -> bool {
        true
    }
}

impl<C: AsChar> ContainsToken<C> for &'_ [u8] {
    #[inline]
    fn contains_token(&self, token: C) -> bool {
        let token = token.as_char();
        self.iter().any(|t| t.as_char() == token)
    }
}

impl<C: AsChar> ContainsToken<C> for &'_ [char] {
    #[inline]
    fn contains_token(&self, token: C) -> bool {
        let token = token.as_char();
        self.iter().any(|t| *t == token)
    }
}

impl<const LEN: usize, C: AsChar> ContainsToken<C> for &'_ [u8; LEN] {
    #[inline]
    fn contains_token(&self, token: C) -> bool {
        let token = token.as_char();
        self.iter().any(|t| t.as_char() == token)
    }
}

impl<const LEN: usize, C: AsChar> ContainsToken<C> for &'_ [char; LEN] {
    #[inline]
    fn contains_token(&self, token: C) -> bool {
        let token = token.as_char();
        self.iter().any(|t| *t == token)
    }
}

impl<const LEN: usize, C: AsChar> ContainsToken<C> for [u8; LEN] {
    #[inline]
    fn contains_token(&self, token: C) -> bool {
        let token = token.as_char();
        self.iter().any(|t| t.as_char() == token)
    }
}

impl<const LEN: usize, C: AsChar> ContainsToken<C> for [char; LEN] {
    #[inline]
    fn contains_token(&self, token: C) -> bool {
        let token = token.as_char();
        self.iter().any(|t| *t == token)
    }
}

impl<T> ContainsToken<T> for () {
    #[inline(always)]
    fn contains_token(&self, _token: T) -> bool {
        false
    }
}

macro_rules! impl_contains_token_for_tuple {
  ($($haystack:ident),+) => (
    #[allow(non_snake_case)]
    impl<T, $($haystack),+> ContainsToken<T> for ($($haystack),+,)
    where
    T: Clone,
      $($haystack: ContainsToken<T>),+
    {
    #[inline]
      fn contains_token(&self, token: T) -> bool {
        let ($(ref $haystack),+,) = *self;
        $($haystack.contains_token(token.clone()) || )+ false
      }
    }
  )
}

macro_rules! impl_contains_token_for_tuples {
    ($haystack1:ident, $($haystack:ident),+) => {
        impl_contains_token_for_tuples!(__impl $haystack1; $($haystack),+);
    };
    (__impl $($haystack:ident),+; $haystack1:ident $(,$haystack2:ident)*) => {
        impl_contains_token_for_tuple!($($haystack),+);
        impl_contains_token_for_tuples!(__impl $($haystack),+, $haystack1; $($haystack2),*);
    };
    (__impl $($haystack:ident),+;) => {
        impl_contains_token_for_tuple!($($haystack),+);
    }
}

impl_contains_token_for_tuples!(
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12, F13, F14, F15, F16, F17, F18, F19, F20, F21
);

#[cfg(feature = "simd")]
#[inline(always)]
fn memchr(token: u8, slice: &[u8]) -> Option<usize> {
    memchr::memchr(token, slice)
}

#[cfg(feature = "simd")]
#[inline(always)]
fn memchr2(token: (u8, u8), slice: &[u8]) -> Option<usize> {
    memchr::memchr2(token.0, token.1, slice)
}

#[cfg(feature = "simd")]
#[inline(always)]
fn memchr3(token: (u8, u8, u8), slice: &[u8]) -> Option<usize> {
    memchr::memchr3(token.0, token.1, token.2, slice)
}

#[cfg(not(feature = "simd"))]
#[inline(always)]
fn memchr(token: u8, slice: &[u8]) -> Option<usize> {
    slice.iter().position(|t| *t == token)
}

#[cfg(not(feature = "simd"))]
#[inline(always)]
fn memchr2(token: (u8, u8), slice: &[u8]) -> Option<usize> {
    slice.iter().position(|t| *t == token.0 || *t == token.1)
}

#[cfg(not(feature = "simd"))]
#[inline(always)]
fn memchr3(token: (u8, u8, u8), slice: &[u8]) -> Option<usize> {
    slice
        .iter()
        .position(|t| *t == token.0 || *t == token.1 || *t == token.2)
}

#[inline(always)]
fn memmem(slice: &[u8], literal: &[u8]) -> Option<crate::lib::std::ops::Range<usize>> {
    match literal.len() {
        0 => Some(0..0),
        1 => memchr(literal[0], slice).map(|i| i..i + 1),
        _ => memmem_(slice, literal),
    }
}

#[inline(always)]
fn memmem2(slice: &[u8], literal: (&[u8], &[u8])) -> Option<crate::lib::std::ops::Range<usize>> {
    match (literal.0.len(), literal.1.len()) {
        (0, _) | (_, 0) => Some(0..0),
        (1, 1) => memchr2((literal.0[0], literal.1[0]), slice).map(|i| i..i + 1),
        _ => memmem2_(slice, literal),
    }
}

#[inline(always)]
fn memmem3(
    slice: &[u8],
    literal: (&[u8], &[u8], &[u8]),
) -> Option<crate::lib::std::ops::Range<usize>> {
    match (literal.0.len(), literal.1.len(), literal.2.len()) {
        (0, _, _) | (_, 0, _) | (_, _, 0) => Some(0..0),
        (1, 1, 1) => memchr3((literal.0[0], literal.1[0], literal.2[0]), slice).map(|i| i..i + 1),
        _ => memmem3_(slice, literal),
    }
}

#[cfg(feature = "simd")]
#[inline(always)]
fn memmem_(slice: &[u8], literal: &[u8]) -> Option<crate::lib::std::ops::Range<usize>> {
    let &prefix = match literal.first() {
        Some(x) => x,
        None => return Some(0..0),
    };
    #[allow(clippy::manual_find)] // faster this way
    for i in memchr::memchr_iter(prefix, slice) {
        if slice[i..].starts_with(literal) {
            let i_end = i + literal.len();
            return Some(i..i_end);
        }
    }
    None
}

#[cfg(feature = "simd")]
fn memmem2_(slice: &[u8], literal: (&[u8], &[u8])) -> Option<crate::lib::std::ops::Range<usize>> {
    let prefix = match (literal.0.first(), literal.1.first()) {
        (Some(&a), Some(&b)) => (a, b),
        _ => return Some(0..0),
    };
    #[allow(clippy::manual_find)] // faster this way
    for i in memchr::memchr2_iter(prefix.0, prefix.1, slice) {
        let subslice = &slice[i..];
        if subslice.starts_with(literal.0) {
            let i_end = i + literal.0.len();
            return Some(i..i_end);
        }
        if subslice.starts_with(literal.1) {
            let i_end = i + literal.1.len();
            return Some(i..i_end);
        }
    }
    None
}

#[cfg(feature = "simd")]
fn memmem3_(
    slice: &[u8],
    literal: (&[u8], &[u8], &[u8]),
) -> Option<crate::lib::std::ops::Range<usize>> {
    let prefix = match (literal.0.first(), literal.1.first(), literal.2.first()) {
        (Some(&a), Some(&b), Some(&c)) => (a, b, c),
        _ => return Some(0..0),
    };
    #[allow(clippy::manual_find)] // faster this way
    for i in memchr::memchr3_iter(prefix.0, prefix.1, prefix.2, slice) {
        let subslice = &slice[i..];
        if subslice.starts_with(literal.0) {
            let i_end = i + literal.0.len();
            return Some(i..i_end);
        }
        if subslice.starts_with(literal.1) {
            let i_end = i + literal.1.len();
            return Some(i..i_end);
        }
        if subslice.starts_with(literal.2) {
            let i_end = i + literal.2.len();
            return Some(i..i_end);
        }
    }
    None
}

#[cfg(not(feature = "simd"))]
fn memmem_(slice: &[u8], literal: &[u8]) -> Option<crate::lib::std::ops::Range<usize>> {
    for i in 0..slice.len() {
        let subslice = &slice[i..];
        if subslice.starts_with(literal) {
            let i_end = i + literal.len();
            return Some(i..i_end);
        }
    }
    None
}

#[cfg(not(feature = "simd"))]
fn memmem2_(slice: &[u8], literal: (&[u8], &[u8])) -> Option<crate::lib::std::ops::Range<usize>> {
    for i in 0..slice.len() {
        let subslice = &slice[i..];
        if subslice.starts_with(literal.0) {
            let i_end = i + literal.0.len();
            return Some(i..i_end);
        }
        if subslice.starts_with(literal.1) {
            let i_end = i + literal.1.len();
            return Some(i..i_end);
        }
    }
    None
}

#[cfg(not(feature = "simd"))]
fn memmem3_(
    slice: &[u8],
    literal: (&[u8], &[u8], &[u8]),
) -> Option<crate::lib::std::ops::Range<usize>> {
    for i in 0..slice.len() {
        let subslice = &slice[i..];
        if subslice.starts_with(literal.0) {
            let i_end = i + literal.0.len();
            return Some(i..i_end);
        }
        if subslice.starts_with(literal.1) {
            let i_end = i + literal.1.len();
            return Some(i..i_end);
        }
        if subslice.starts_with(literal.2) {
            let i_end = i + literal.2.len();
            return Some(i..i_end);
        }
    }
    None
}
