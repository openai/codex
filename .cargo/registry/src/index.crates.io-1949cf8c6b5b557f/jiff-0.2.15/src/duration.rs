use core::time::Duration as UnsignedDuration;

use crate::{
    error::{err, ErrorContext},
    Error, SignedDuration, Span,
};

/// An internal type for abstracting over different duration types.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Duration {
    Span(Span),
    Signed(SignedDuration),
    Unsigned(UnsignedDuration),
}

impl Duration {
    /// Convert this to a signed duration.
    ///
    /// This returns an error only in the case where this is an unsigned
    /// duration with a number of whole seconds that exceeds `|i64::MIN|`.
    #[cfg_attr(feature = "perf-inline", inline(always))]
    pub(crate) fn to_signed(self) -> Result<SDuration, Error> {
        match self {
            Duration::Span(span) => Ok(SDuration::Span(span)),
            Duration::Signed(sdur) => Ok(SDuration::Absolute(sdur)),
            Duration::Unsigned(udur) => {
                let sdur =
                    SignedDuration::try_from(udur).with_context(|| {
                        err!(
                            "unsigned duration {udur:?} exceeds Jiff's limits"
                        )
                    })?;
                Ok(SDuration::Absolute(sdur))
            }
        }
    }

    /// Negates this duration.
    ///
    /// When the duration is a span, this can never fail because a span defines
    /// its min and max values such that negation is always possible.
    ///
    /// When the duration is signed, then this attempts to return a signed
    /// duration and only falling back to an unsigned duration when the number
    /// of seconds corresponds to `i64::MIN`.
    ///
    /// When the duration is unsigned, then this fails when the whole seconds
    /// exceed the absolute value of `i64::MIN`. Otherwise, a signed duration
    /// is returned.
    ///
    /// The failures for large unsigned durations here are okay because the
    /// point at which absolute durations overflow on negation, they would also
    /// cause overflow when adding or subtracting to *any* valid datetime value
    /// for *any* datetime type in this crate. So while the error message may
    /// be different, the actual end result is the same (failure).
    ///
    /// TODO: Write unit tests for this.
    #[cfg_attr(feature = "perf-inline", inline(always))]
    pub(crate) fn checked_neg(self) -> Result<Duration, Error> {
        match self {
            Duration::Span(span) => Ok(Duration::Span(span.negate())),
            Duration::Signed(sdur) => {
                // We try to stick with signed durations, but in the case
                // where negation fails, we can represent its negation using
                // an unsigned duration.
                if let Some(sdur) = sdur.checked_neg() {
                    Ok(Duration::Signed(sdur))
                } else {
                    let udur = UnsignedDuration::new(
                        i64::MIN.unsigned_abs(),
                        sdur.subsec_nanos().unsigned_abs(),
                    );
                    Ok(Duration::Unsigned(udur))
                }
            }
            Duration::Unsigned(udur) => {
                // We can permit negating i64::MIN.unsigned_abs() to
                // i64::MIN, but we need to handle it specially since
                // i64::MIN.unsigned_abs() exceeds i64::MAX.
                let sdur = if udur.as_secs() == i64::MIN.unsigned_abs() {
                    SignedDuration::new_without_nano_overflow(
                        i64::MIN,
                        // OK because `udur.subsec_nanos()` < 999_999_999.
                        -i32::try_from(udur.subsec_nanos()).unwrap(),
                    )
                } else {
                    // The negation here is always correct because it can only
                    // panic with `sdur.as_secs() == i64::MIN`, which is
                    // impossible because it must be positive.
                    //
                    // Otherwise, this is the only failure point in this entire
                    // routine. And specifically, we fail here in precisely
                    // the cases where `udur.as_secs() > |i64::MIN|`.
                    -SignedDuration::try_from(udur).with_context(|| {
                        err!("failed to negate unsigned duration {udur:?}")
                    })?
                };
                Ok(Duration::Signed(sdur))
            }
        }
    }

    /// Returns true if and only if this duration is negative.
    #[cfg_attr(feature = "perf-inline", inline(always))]
    pub(crate) fn is_negative(&self) -> bool {
        match *self {
            Duration::Span(ref span) => span.is_negative(),
            Duration::Signed(ref sdur) => sdur.is_negative(),
            Duration::Unsigned(_) => false,
        }
    }
}

impl From<Span> for Duration {
    #[inline]
    fn from(span: Span) -> Duration {
        Duration::Span(span)
    }
}

impl From<SignedDuration> for Duration {
    #[inline]
    fn from(sdur: SignedDuration) -> Duration {
        Duration::Signed(sdur)
    }
}

impl From<UnsignedDuration> for Duration {
    #[inline]
    fn from(udur: UnsignedDuration) -> Duration {
        Duration::Unsigned(udur)
    }
}

/// An internal type for abstracting over signed durations.
///
/// This is typically converted to from a `Duration`. It enables callers
/// downstream to implement datetime arithmetic on only two duration types
/// instead of doing it for three duration types (including
/// `std::time::Duration`).
///
/// The main thing making this idea work is that if an unsigned duration cannot
/// fit into a signed duration, then it would overflow any calculation on any
/// datetime type in Jiff anyway. If this weren't true, then we'd need to
/// support doing actual arithmetic with unsigned durations separately from
/// signed durations.
#[derive(Clone, Copy, Debug)]
pub(crate) enum SDuration {
    Span(Span),
    Absolute(SignedDuration),
}
