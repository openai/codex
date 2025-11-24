//! Opaque implementations of [`Parser`]

use crate::combinator::trace;
use crate::combinator::trace_result;
use crate::combinator::DisplayDebug;
#[cfg(feature = "unstable-recover")]
#[cfg(feature = "std")]
use crate::error::FromRecoverableError;
use crate::error::{AddContext, FromExternalError, ParserError};
use crate::lib::std::borrow::Borrow;
use crate::lib::std::ops::Range;
#[cfg(feature = "unstable-recover")]
#[cfg(feature = "std")]
use crate::stream::Recover;
use crate::stream::StreamIsPartial;
use crate::stream::{Location, Stream};
use crate::*;

/// [`Parser`] implementation for [`Parser::by_ref`]
pub struct ByRef<'p, P, I, O, E> {
    pub(crate) p: &'p mut P,
    pub(crate) i: core::marker::PhantomData<I>,
    pub(crate) o: core::marker::PhantomData<O>,
    pub(crate) e: core::marker::PhantomData<E>,
}

impl<I, O, E, P> Parser<I, O, E> for ByRef<'_, P, I, O, E>
where
    P: Parser<I, O, E>,
{
    #[inline(always)]
    fn parse_next(&mut self, i: &mut I) -> Result<O, E> {
        self.p.parse_next(i)
    }
}

/// [`Parser`] implementation for [`Parser::map`]
pub struct Map<F, G, I, O, O2, E>
where
    F: Parser<I, O, E>,
    G: FnMut(O) -> O2,
{
    pub(crate) parser: F,
    pub(crate) map: G,
    pub(crate) i: core::marker::PhantomData<I>,
    pub(crate) o: core::marker::PhantomData<O>,
    pub(crate) o2: core::marker::PhantomData<O2>,
    pub(crate) e: core::marker::PhantomData<E>,
}

impl<F, G, I, O, O2, E> Parser<I, O2, E> for Map<F, G, I, O, O2, E>
where
    F: Parser<I, O, E>,
    G: FnMut(O) -> O2,
{
    #[inline]
    fn parse_next(&mut self, i: &mut I) -> Result<O2, E> {
        match self.parser.parse_next(i) {
            Err(e) => Err(e),
            Ok(o) => Ok((self.map)(o)),
        }
    }
}

/// [`Parser`] implementation for [`Parser::try_map`]
pub struct TryMap<F, G, I, O, O2, E, E2>
where
    F: Parser<I, O, E>,
    G: FnMut(O) -> Result<O2, E2>,
    I: Stream,
    E: FromExternalError<I, E2>,
    E: ParserError<I>,
{
    pub(crate) parser: F,
    pub(crate) map: G,
    pub(crate) i: core::marker::PhantomData<I>,
    pub(crate) o: core::marker::PhantomData<O>,
    pub(crate) o2: core::marker::PhantomData<O2>,
    pub(crate) e: core::marker::PhantomData<E>,
    pub(crate) e2: core::marker::PhantomData<E2>,
}

impl<F, G, I, O, O2, E, E2> Parser<I, O2, E> for TryMap<F, G, I, O, O2, E, E2>
where
    F: Parser<I, O, E>,
    G: FnMut(O) -> Result<O2, E2>,
    I: Stream,
    E: FromExternalError<I, E2>,
    E: ParserError<I>,
{
    #[inline]
    fn parse_next(&mut self, input: &mut I) -> Result<O2, E> {
        let start = input.checkpoint();
        let o = self.parser.parse_next(input)?;
        let res = (self.map)(o).map_err(|err| {
            input.reset(&start);
            E::from_external_error(input, err)
        });
        trace_result("verify", &res);
        res
    }
}

/// [`Parser`] implementation for [`Parser::verify_map`]
pub struct VerifyMap<F, G, I, O, O2, E>
where
    F: Parser<I, O, E>,
    G: FnMut(O) -> Option<O2>,
    I: Stream,
    E: ParserError<I>,
{
    pub(crate) parser: F,
    pub(crate) map: G,
    pub(crate) i: core::marker::PhantomData<I>,
    pub(crate) o: core::marker::PhantomData<O>,
    pub(crate) o2: core::marker::PhantomData<O2>,
    pub(crate) e: core::marker::PhantomData<E>,
}

impl<F, G, I, O, O2, E> Parser<I, O2, E> for VerifyMap<F, G, I, O, O2, E>
where
    F: Parser<I, O, E>,
    G: FnMut(O) -> Option<O2>,
    I: Stream,
    E: ParserError<I>,
{
    #[inline]
    fn parse_next(&mut self, input: &mut I) -> Result<O2, E> {
        let start = input.checkpoint();
        let o = self.parser.parse_next(input)?;
        let res = (self.map)(o).ok_or_else(|| {
            input.reset(&start);
            ParserError::from_input(input)
        });
        trace_result("verify", &res);
        res
    }
}

/// [`Parser`] implementation for [`Parser::and_then`]
pub struct AndThen<F, G, I, O, O2, E>
where
    F: Parser<I, O, E>,
    G: Parser<O, O2, E>,
    O: StreamIsPartial,
    I: Stream,
{
    pub(crate) outer: F,
    pub(crate) inner: G,
    pub(crate) i: core::marker::PhantomData<I>,
    pub(crate) o: core::marker::PhantomData<O>,
    pub(crate) o2: core::marker::PhantomData<O2>,
    pub(crate) e: core::marker::PhantomData<E>,
}

impl<F, G, I, O, O2, E> Parser<I, O2, E> for AndThen<F, G, I, O, O2, E>
where
    F: Parser<I, O, E>,
    G: Parser<O, O2, E>,
    O: StreamIsPartial,
    I: Stream,
{
    #[inline(always)]
    fn parse_next(&mut self, i: &mut I) -> Result<O2, E> {
        let start = i.checkpoint();
        let mut o = self.outer.parse_next(i)?;
        let _ = o.complete();
        let o2 = self.inner.parse_next(&mut o).map_err(|err| {
            i.reset(&start);
            err
        })?;
        Ok(o2)
    }
}

/// [`Parser`] implementation for [`Parser::parse_to`]
pub struct ParseTo<P, I, O, O2, E>
where
    P: Parser<I, O, E>,
    I: Stream,
    O: crate::stream::ParseSlice<O2>,
    E: ParserError<I>,
{
    pub(crate) p: P,
    pub(crate) i: core::marker::PhantomData<I>,
    pub(crate) o: core::marker::PhantomData<O>,
    pub(crate) o2: core::marker::PhantomData<O2>,
    pub(crate) e: core::marker::PhantomData<E>,
}

impl<P, I, O, O2, E> Parser<I, O2, E> for ParseTo<P, I, O, O2, E>
where
    P: Parser<I, O, E>,
    I: Stream,
    O: crate::stream::ParseSlice<O2>,
    E: ParserError<I>,
{
    #[inline]
    fn parse_next(&mut self, i: &mut I) -> Result<O2, E> {
        let start = i.checkpoint();
        let o = self.p.parse_next(i)?;
        let res = o.parse_slice().ok_or_else(|| {
            i.reset(&start);
            ParserError::from_input(i)
        });
        trace_result("verify", &res);
        res
    }
}

/// [`Parser`] implementation for [`Parser::flat_map`]
pub struct FlatMap<F, G, H, I, O, O2, E>
where
    F: Parser<I, O, E>,
    G: FnMut(O) -> H,
    H: Parser<I, O2, E>,
{
    pub(crate) f: F,
    pub(crate) g: G,
    pub(crate) h: core::marker::PhantomData<H>,
    pub(crate) i: core::marker::PhantomData<I>,
    pub(crate) o: core::marker::PhantomData<O>,
    pub(crate) o2: core::marker::PhantomData<O2>,
    pub(crate) e: core::marker::PhantomData<E>,
}

impl<F, G, H, I, O, O2, E> Parser<I, O2, E> for FlatMap<F, G, H, I, O, O2, E>
where
    F: Parser<I, O, E>,
    G: FnMut(O) -> H,
    H: Parser<I, O2, E>,
{
    #[inline(always)]
    fn parse_next(&mut self, i: &mut I) -> Result<O2, E> {
        let o = self.f.parse_next(i)?;
        (self.g)(o).parse_next(i)
    }
}

/// [`Parser`] implementation for [`Parser::complete_err`]
pub struct CompleteErr<P, I, O, E> {
    pub(crate) p: P,
    pub(crate) i: core::marker::PhantomData<I>,
    pub(crate) o: core::marker::PhantomData<O>,
    pub(crate) e: core::marker::PhantomData<E>,
}

impl<P, I, O, E> Parser<I, O, E> for CompleteErr<P, I, O, E>
where
    P: Parser<I, O, E>,
    I: Stream,
    E: ParserError<I>,
{
    #[inline]
    fn parse_next(&mut self, input: &mut I) -> Result<O, E> {
        trace("complete_err", |input: &mut I| {
            match (self.p).parse_next(input) {
                Err(err) => match err.needed() {
                    Some(_) => Err(ParserError::from_input(input)),
                    None => Err(err),
                },
                rest => rest,
            }
        })
        .parse_next(input)
    }
}

/// [`Parser`] implementation for [`Parser::verify`]
pub struct Verify<F, G, I, O, O2, E>
where
    F: Parser<I, O, E>,
    G: FnMut(&O2) -> bool,
    I: Stream,
    O: Borrow<O2>,
    O2: ?Sized,
    E: ParserError<I>,
{
    pub(crate) parser: F,
    pub(crate) filter: G,
    pub(crate) i: core::marker::PhantomData<I>,
    pub(crate) o: core::marker::PhantomData<O>,
    pub(crate) o2: core::marker::PhantomData<O2>,
    pub(crate) e: core::marker::PhantomData<E>,
}

impl<F, G, I, O, O2, E> Parser<I, O, E> for Verify<F, G, I, O, O2, E>
where
    F: Parser<I, O, E>,
    G: FnMut(&O2) -> bool,
    I: Stream,
    O: Borrow<O2>,
    O2: ?Sized,
    E: ParserError<I>,
{
    #[inline]
    fn parse_next(&mut self, input: &mut I) -> Result<O, E> {
        let start = input.checkpoint();
        let o = self.parser.parse_next(input)?;
        let res = (self.filter)(o.borrow()).then_some(o).ok_or_else(|| {
            input.reset(&start);
            ParserError::from_input(input)
        });
        trace_result("verify", &res);
        res
    }
}

/// [`Parser`] implementation for [`Parser::value`]
pub struct Value<F, I, O, O2, E>
where
    F: Parser<I, O, E>,
    O2: Clone,
{
    pub(crate) parser: F,
    pub(crate) val: O2,
    pub(crate) i: core::marker::PhantomData<I>,
    pub(crate) o: core::marker::PhantomData<O>,
    pub(crate) e: core::marker::PhantomData<E>,
}

impl<F, I, O, O2, E> Parser<I, O2, E> for Value<F, I, O, O2, E>
where
    F: Parser<I, O, E>,
    O2: Clone,
{
    #[inline]
    fn parse_next(&mut self, input: &mut I) -> Result<O2, E> {
        (self.parser).parse_next(input).map(|_| self.val.clone())
    }
}

/// [`Parser`] implementation for [`Parser::default_value`]
pub struct DefaultValue<F, I, O, O2, E>
where
    F: Parser<I, O, E>,
    O2: core::default::Default,
{
    pub(crate) parser: F,
    pub(crate) o2: core::marker::PhantomData<O2>,
    pub(crate) i: core::marker::PhantomData<I>,
    pub(crate) o: core::marker::PhantomData<O>,
    pub(crate) e: core::marker::PhantomData<E>,
}

impl<F, I, O, O2, E> Parser<I, O2, E> for DefaultValue<F, I, O, O2, E>
where
    F: Parser<I, O, E>,
    O2: core::default::Default,
{
    #[inline]
    fn parse_next(&mut self, input: &mut I) -> Result<O2, E> {
        (self.parser).parse_next(input).map(|_| O2::default())
    }
}

/// [`Parser`] implementation for [`Parser::void`]
pub struct Void<F, I, O, E>
where
    F: Parser<I, O, E>,
{
    pub(crate) parser: F,
    pub(crate) i: core::marker::PhantomData<I>,
    pub(crate) o: core::marker::PhantomData<O>,
    pub(crate) e: core::marker::PhantomData<E>,
}

impl<F, I, O, E> Parser<I, (), E> for Void<F, I, O, E>
where
    F: Parser<I, O, E>,
{
    #[inline(always)]
    fn parse_next(&mut self, input: &mut I) -> Result<(), E> {
        (self.parser).parse_next(input).map(|_| ())
    }
}

/// [`Parser`] implementation for [`Parser::take`]
pub struct Take<F, I, O, E>
where
    F: Parser<I, O, E>,
    I: Stream,
{
    pub(crate) parser: F,
    pub(crate) i: core::marker::PhantomData<I>,
    pub(crate) o: core::marker::PhantomData<O>,
    pub(crate) e: core::marker::PhantomData<E>,
}

impl<I, O, E, F> Parser<I, <I as Stream>::Slice, E> for Take<F, I, O, E>
where
    F: Parser<I, O, E>,
    I: Stream,
{
    #[inline]
    fn parse_next(&mut self, input: &mut I) -> Result<<I as Stream>::Slice, E> {
        let checkpoint = input.checkpoint();
        match (self.parser).parse_next(input) {
            Ok(_) => {
                let offset = input.offset_from(&checkpoint);
                input.reset(&checkpoint);
                let taken = input.next_slice(offset);
                Ok(taken)
            }
            Err(e) => Err(e),
        }
    }
}

/// [`Parser`] implementation for [`Parser::with_taken`]
pub struct WithTaken<F, I, O, E>
where
    F: Parser<I, O, E>,
    I: Stream,
{
    pub(crate) parser: F,
    pub(crate) i: core::marker::PhantomData<I>,
    pub(crate) o: core::marker::PhantomData<O>,
    pub(crate) e: core::marker::PhantomData<E>,
}

impl<F, I, O, E> Parser<I, (O, <I as Stream>::Slice), E> for WithTaken<F, I, O, E>
where
    F: Parser<I, O, E>,
    I: Stream,
{
    #[inline]
    fn parse_next(&mut self, input: &mut I) -> Result<(O, <I as Stream>::Slice), E> {
        let checkpoint = input.checkpoint();
        match (self.parser).parse_next(input) {
            Ok(result) => {
                let offset = input.offset_from(&checkpoint);
                input.reset(&checkpoint);
                let taken = input.next_slice(offset);
                Ok((result, taken))
            }
            Err(e) => Err(e),
        }
    }
}

/// [`Parser`] implementation for [`Parser::span`]
pub struct Span<F, I, O, E>
where
    F: Parser<I, O, E>,
    I: Stream + Location,
{
    pub(crate) parser: F,
    pub(crate) i: core::marker::PhantomData<I>,
    pub(crate) o: core::marker::PhantomData<O>,
    pub(crate) e: core::marker::PhantomData<E>,
}

impl<I, O, E, F> Parser<I, Range<usize>, E> for Span<F, I, O, E>
where
    F: Parser<I, O, E>,
    I: Stream + Location,
{
    #[inline]
    fn parse_next(&mut self, input: &mut I) -> Result<Range<usize>, E> {
        let start = input.current_token_start();
        self.parser.parse_next(input).map(move |_| {
            let end = input.previous_token_end();
            start..end
        })
    }
}

/// [`Parser`] implementation for [`Parser::with_span`]
pub struct WithSpan<F, I, O, E>
where
    F: Parser<I, O, E>,
    I: Stream + Location,
{
    pub(crate) parser: F,
    pub(crate) i: core::marker::PhantomData<I>,
    pub(crate) o: core::marker::PhantomData<O>,
    pub(crate) e: core::marker::PhantomData<E>,
}

impl<F, I, O, E> Parser<I, (O, Range<usize>), E> for WithSpan<F, I, O, E>
where
    F: Parser<I, O, E>,
    I: Stream + Location,
{
    #[inline]
    fn parse_next(&mut self, input: &mut I) -> Result<(O, Range<usize>), E> {
        let start = input.current_token_start();
        self.parser.parse_next(input).map(move |output| {
            let end = input.previous_token_end();
            (output, (start..end))
        })
    }
}

/// [`Parser`] implementation for [`Parser::output_into`]
pub struct OutputInto<F, I, O, O2, E>
where
    F: Parser<I, O, E>,
    O: Into<O2>,
{
    pub(crate) parser: F,
    pub(crate) i: core::marker::PhantomData<I>,
    pub(crate) o: core::marker::PhantomData<O>,
    pub(crate) o2: core::marker::PhantomData<O2>,
    pub(crate) e: core::marker::PhantomData<E>,
}

impl<F, I, O, O2, E> Parser<I, O2, E> for OutputInto<F, I, O, O2, E>
where
    F: Parser<I, O, E>,
    O: Into<O2>,
{
    #[inline]
    fn parse_next(&mut self, i: &mut I) -> Result<O2, E> {
        self.parser.parse_next(i).map(|o| o.into())
    }
}

/// [`Parser`] implementation for [`Parser::err_into`]
pub struct ErrInto<F, I, O, E, E2>
where
    F: Parser<I, O, E>,
    E: Into<E2>,
{
    pub(crate) parser: F,
    pub(crate) i: core::marker::PhantomData<I>,
    pub(crate) o: core::marker::PhantomData<O>,
    pub(crate) e: core::marker::PhantomData<E>,
    pub(crate) e2: core::marker::PhantomData<E2>,
}

impl<F, I, O, E, E2> Parser<I, O, E2> for ErrInto<F, I, O, E, E2>
where
    F: Parser<I, O, E>,
    E: Into<E2>,
{
    #[inline]
    fn parse_next(&mut self, i: &mut I) -> Result<O, E2> {
        self.parser.parse_next(i).map_err(|err| err.into())
    }
}

/// [`Parser`] implementation for [`Parser::context`]
pub struct Context<F, I, O, E, C>
where
    F: Parser<I, O, E>,
    I: Stream,
    E: AddContext<I, C>,
    E: ParserError<I>,
    C: Clone + crate::lib::std::fmt::Debug,
{
    pub(crate) parser: F,
    pub(crate) context: C,
    pub(crate) i: core::marker::PhantomData<I>,
    pub(crate) o: core::marker::PhantomData<O>,
    pub(crate) e: core::marker::PhantomData<E>,
}

impl<F, I, O, E, C> Parser<I, O, E> for Context<F, I, O, E, C>
where
    F: Parser<I, O, E>,
    I: Stream,
    E: AddContext<I, C>,
    E: ParserError<I>,
    C: Clone + crate::lib::std::fmt::Debug,
{
    #[inline]
    fn parse_next(&mut self, i: &mut I) -> Result<O, E> {
        let context = self.context.clone();
        trace(DisplayDebug(self.context.clone()), move |i: &mut I| {
            let start = i.checkpoint();
            (self.parser)
                .parse_next(i)
                .map_err(|err| err.add_context(i, &start, context.clone()))
        })
        .parse_next(i)
    }
}

/// [`Parser`] implementation for [`Parser::context`]
pub struct ContextWith<P, I, O, E, F, C, FI>
where
    P: Parser<I, O, E>,
    I: Stream,
    E: AddContext<I, C>,
    E: ParserError<I>,
    F: Fn() -> FI + Clone,
    C: crate::lib::std::fmt::Debug,
    FI: Iterator<Item = C>,
{
    pub(crate) parser: P,
    pub(crate) context: F,
    pub(crate) i: core::marker::PhantomData<I>,
    pub(crate) o: core::marker::PhantomData<O>,
    pub(crate) e: core::marker::PhantomData<E>,
    pub(crate) c: core::marker::PhantomData<C>,
    pub(crate) fi: core::marker::PhantomData<FI>,
}

impl<P, I, O, E, F, C, FI> Parser<I, O, E> for ContextWith<P, I, O, E, F, C, FI>
where
    P: Parser<I, O, E>,
    I: Stream,
    E: AddContext<I, C>,
    E: ParserError<I>,
    F: Fn() -> FI + Clone,
    C: crate::lib::std::fmt::Debug,
    FI: Iterator<Item = C>,
{
    #[inline]
    fn parse_next(&mut self, i: &mut I) -> Result<O, E> {
        let context = self.context.clone();
        let start = i.checkpoint();
        (self.parser).parse_next(i).map_err(|mut err| {
            for context in context() {
                err = err.add_context(i, &start, context);
            }
            err
        })
    }
}

/// [`Parser`] implementation for [`Parser::map_err`]
pub struct MapErr<F, G, I, O, E, E2>
where
    F: Parser<I, O, E>,
    G: FnMut(E) -> E2,
{
    pub(crate) parser: F,
    pub(crate) map: G,
    pub(crate) i: core::marker::PhantomData<I>,
    pub(crate) o: core::marker::PhantomData<O>,
    pub(crate) e: core::marker::PhantomData<E>,
    pub(crate) e2: core::marker::PhantomData<E2>,
}

impl<F, G, I, O, E, E2> Parser<I, O, E2> for MapErr<F, G, I, O, E, E2>
where
    F: Parser<I, O, E>,
    G: FnMut(E) -> E2,
{
    #[inline]
    fn parse_next(&mut self, i: &mut I) -> Result<O, E2> {
        match self.parser.parse_next(i) {
            Err(e) => Err((self.map)(e)),
            Ok(o) => Ok(o),
        }
    }
}

/// [`Parser`] implementation for [`Parser::retry_after`]
#[cfg(feature = "unstable-recover")]
#[cfg(feature = "std")]
pub struct RetryAfter<P, R, I, O, E>
where
    P: Parser<I, O, E>,
    R: Parser<I, (), E>,
    I: Stream,
    I: Recover<E>,
    E: ParserError<I> + FromRecoverableError<I, E>,
{
    pub(crate) parser: P,
    pub(crate) recover: R,
    pub(crate) i: core::marker::PhantomData<I>,
    pub(crate) o: core::marker::PhantomData<O>,
    pub(crate) e: core::marker::PhantomData<E>,
}

#[cfg(feature = "unstable-recover")]
#[cfg(feature = "std")]
impl<P, R, I, O, E> Parser<I, O, E> for RetryAfter<P, R, I, O, E>
where
    P: Parser<I, O, E>,
    R: Parser<I, (), E>,
    I: Stream,
    I: Recover<E>,
    E: ParserError<I> + FromRecoverableError<I, E>,
{
    #[inline(always)]
    fn parse_next(&mut self, i: &mut I) -> Result<O, E> {
        if I::is_recovery_supported() {
            retry_after_inner(&mut self.parser, &mut self.recover, i)
        } else {
            self.parser.parse_next(i)
        }
    }
}

#[cfg(feature = "unstable-recover")]
#[cfg(feature = "std")]
fn retry_after_inner<P, R, I, O, E>(parser: &mut P, recover: &mut R, i: &mut I) -> Result<O, E>
where
    P: Parser<I, O, E>,
    R: Parser<I, (), E>,
    I: Stream,
    I: Recover<E>,
    E: ParserError<I> + FromRecoverableError<I, E>,
{
    loop {
        let token_start = i.checkpoint();
        let mut err = match parser.parse_next(i) {
            Ok(o) => {
                return Ok(o);
            }
            Err(e) if e.is_incomplete() => return Err(e),
            Err(err) => err,
        };
        let err_start = i.checkpoint();
        let err_start_eof_offset = i.eof_offset();
        if recover.parse_next(i).is_ok() {
            let i_eof_offset = i.eof_offset();
            if err_start_eof_offset == i_eof_offset {
                // Didn't advance so bubble the error up
            } else if let Err(err_) = i.record_err(&token_start, &err_start, err) {
                err = err_;
            } else {
                continue;
            }
        }

        i.reset(&err_start);
        err = E::from_recoverable_error(&token_start, &err_start, i, err);
        return Err(err);
    }
}

/// [`Parser`] implementation for [`Parser::resume_after`]
#[cfg(feature = "unstable-recover")]
#[cfg(feature = "std")]
pub struct ResumeAfter<P, R, I, O, E>
where
    P: Parser<I, O, E>,
    R: Parser<I, (), E>,
    I: Stream,
    I: Recover<E>,
    E: ParserError<I> + FromRecoverableError<I, E>,
{
    pub(crate) parser: P,
    pub(crate) recover: R,
    pub(crate) i: core::marker::PhantomData<I>,
    pub(crate) o: core::marker::PhantomData<O>,
    pub(crate) e: core::marker::PhantomData<E>,
}

#[cfg(feature = "unstable-recover")]
#[cfg(feature = "std")]
impl<P, R, I, O, E> Parser<I, Option<O>, E> for ResumeAfter<P, R, I, O, E>
where
    P: Parser<I, O, E>,
    R: Parser<I, (), E>,
    I: Stream,
    I: Recover<E>,
    E: ParserError<I> + FromRecoverableError<I, E>,
{
    #[inline(always)]
    fn parse_next(&mut self, i: &mut I) -> Result<Option<O>, E> {
        if I::is_recovery_supported() {
            resume_after_inner(&mut self.parser, &mut self.recover, i)
        } else {
            self.parser.parse_next(i).map(Some)
        }
    }
}

#[cfg(feature = "unstable-recover")]
#[cfg(feature = "std")]
fn resume_after_inner<P, R, I, O, E>(
    parser: &mut P,
    recover: &mut R,
    i: &mut I,
) -> Result<Option<O>, E>
where
    P: Parser<I, O, E>,
    R: Parser<I, (), E>,
    I: Stream,
    I: Recover<E>,
    E: ParserError<I> + FromRecoverableError<I, E>,
{
    let token_start = i.checkpoint();
    let mut err = match parser.parse_next(i) {
        Ok(o) => {
            return Ok(Some(o));
        }
        Err(e) if e.is_incomplete() => return Err(e),
        Err(err) => err,
    };
    let err_start = i.checkpoint();
    if recover.parse_next(i).is_ok() {
        if let Err(err_) = i.record_err(&token_start, &err_start, err) {
            err = err_;
        } else {
            return Ok(None);
        }
    }

    i.reset(&err_start);
    err = FromRecoverableError::from_recoverable_error(&token_start, &err_start, i, err);
    Err(err)
}
