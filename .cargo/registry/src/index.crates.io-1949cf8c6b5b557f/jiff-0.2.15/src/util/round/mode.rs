use crate::{
    util::{
        rangeint::{RFrom, RInto},
        t::{NoUnits, NoUnits128, C, C128},
    },
    Unit,
};

/// The mode for dealing with the remainder when rounding datetimes or spans.
///
/// This is used in APIs like [`Span::round`](crate::Span::round) for rounding
/// spans, and APIs like [`Zoned::round`](crate::Zoned::round) for rounding
/// datetimes.
///
/// In the documentation for each variant, we refer to concepts like the
/// "smallest" unit and the "rounding increment." These are best described
/// in the documentation for what you're rounding. For example,
/// [`SpanRound::smallest`](crate::SpanRound::smallest)
/// and [`SpanRound::increment`](crate::SpanRound::increment).
///
/// # Example
///
/// This shows how to round a span with a different rounding mode than the
/// default:
///
/// ```
/// use jiff::{RoundMode, SpanRound, ToSpan, Unit};
///
/// // The default rounds like how you were taught in school:
/// assert_eq!(
///     1.hour().minutes(59).round(Unit::Hour)?,
///     2.hours().fieldwise(),
/// );
/// // But we can change the mode, e.g., truncation:
/// let options = SpanRound::new().smallest(Unit::Hour).mode(RoundMode::Trunc);
/// assert_eq!(
///     1.hour().minutes(59).round(options)?,
///     1.hour().fieldwise(),
/// );
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RoundMode {
    /// Rounds toward positive infinity.
    ///
    /// For negative spans and datetimes, this option will make the value
    /// smaller, which could be unexpected. To round away from zero, use
    /// `Expand`.
    Ceil,
    /// Rounds toward negative infinity.
    ///
    /// This mode acts like `Trunc` for positive spans and datetimes, but
    /// for negative values it will make the value larger, which could be
    /// unexpected. To round towards zero, use `Trunc`.
    Floor,
    /// Rounds away from zero like `Ceil` for positive spans and datetimes,
    /// and like `Floor` for negative spans and datetimes.
    Expand,
    /// Rounds toward zero, chopping off any fractional part of a unit.
    ///
    /// This is the default when rounding spans returned from
    /// datetime arithmetic. (But it is not the default for
    /// [`Span::round`](crate::Span::round).)
    Trunc,
    /// Rounds to the nearest allowed value like `HalfExpand`, but when there
    /// is a tie, round towards positive infinity like `Ceil`.
    HalfCeil,
    /// Rounds to the nearest allowed value like `HalfExpand`, but when there
    /// is a tie, round towards negative infinity like `Floor`.
    HalfFloor,
    /// Rounds to the nearest value allowed by the rounding increment and the
    /// smallest unit. When there is a tie, round away from zero like `Ceil`
    /// for positive spans and datetimes and like `Floor` for negative spans
    /// and datetimes.
    ///
    /// This corresponds to how rounding is often taught in school.
    ///
    /// This is the default for rounding spans and datetimes.
    HalfExpand,
    /// Rounds to the nearest allowed value like `HalfExpand`, but when there
    /// is a tie, round towards zero like `Trunc`.
    HalfTrunc,
    /// Rounds to the nearest allowed value like `HalfExpand`, but when there
    /// is a tie, round towards the value that is an even multiple of the
    /// rounding increment. For example, with a rounding increment of `3`,
    /// the number `10` would round up to `12` instead of down to `9`, because
    /// `12` is an even multiple of `3`, where as `9` is is an odd multiple.
    HalfEven,
}

impl RoundMode {
    /// Given a `quantity` in nanoseconds and an `increment` in units of
    /// `unit`, this rounds it according to this mode and returns the result
    /// in nanoseconds.
    pub(crate) fn round_by_unit_in_nanoseconds(
        self,
        quantity: impl RInto<NoUnits128>,
        unit: Unit,
        increment: impl RInto<NoUnits128>,
    ) -> NoUnits128 {
        let quantity = quantity.rinto();
        let increment = unit.nanoseconds() * increment.rinto();
        let rounded = self.round(quantity, increment);
        rounded
    }

    /// Rounds `quantity` to the nearest `increment` in units of nanoseconds.
    pub(crate) fn round(
        self,
        quantity: impl RInto<NoUnits128>,
        increment: impl RInto<NoUnits128>,
    ) -> NoUnits128 {
        // ref: https://tc39.es/proposal-temporal/#sec-temporal-roundnumbertoincrement
        fn inner(
            mode: RoundMode,
            quantity: NoUnits128,
            increment: NoUnits128,
        ) -> NoUnits128 {
            let mut quotient = quantity.div_ceil(increment);
            let remainder = quantity.rem_ceil(increment);
            if remainder == C(0) {
                return quantity;
            }
            let sign = if remainder < C(0) { C128(-1) } else { C128(1) };
            let tiebreaker = (remainder * C128(2)).abs();
            let tie = tiebreaker == increment;
            let expand_is_nearer = tiebreaker > increment;
            // ref: https://tc39.es/proposal-temporal/#sec-temporal-roundnumbertoincrement
            match mode {
                RoundMode::Ceil => {
                    if sign > C(0) {
                        quotient += sign;
                    }
                }
                RoundMode::Floor => {
                    if sign < C(0) {
                        quotient += sign;
                    }
                }
                RoundMode::Expand => {
                    quotient += sign;
                }
                RoundMode::Trunc => {}
                RoundMode::HalfCeil => {
                    if expand_is_nearer || (tie && sign > C(0)) {
                        quotient += sign;
                    }
                }
                RoundMode::HalfFloor => {
                    if expand_is_nearer || (tie && sign < C(0)) {
                        quotient += sign;
                    }
                }
                RoundMode::HalfExpand => {
                    if expand_is_nearer || tie {
                        quotient += sign;
                    }
                }
                RoundMode::HalfTrunc => {
                    if expand_is_nearer {
                        quotient += sign;
                    }
                }
                RoundMode::HalfEven => {
                    if expand_is_nearer || (tie && quotient % C(2) == C(1)) {
                        quotient += sign;
                    }
                }
            }
            // We use saturating arithmetic here because this can overflow
            // when `quantity` is the max value. Since we're rounding, we just
            // refuse to go over the maximum. I'm not 100% convinced this is
            // correct, but I think the only alternative is to return an error,
            // and I'm not sure that's ideal either.
            quotient.saturating_mul(increment)
        }
        inner(self, quantity.rinto(), increment.rinto())
    }

    pub(crate) fn round_float(
        self,
        quantity: f64,
        increment: NoUnits128,
    ) -> NoUnits128 {
        #[cfg(not(feature = "std"))]
        use crate::util::libm::Float;

        let quotient = quantity / (increment.get() as f64);
        let rounded = match self {
            RoundMode::Ceil => quotient.ceil(),
            RoundMode::Floor => quotient.floor(),
            RoundMode::Expand => {
                if quotient < 0.0 {
                    quotient.floor()
                } else {
                    quotient.ceil()
                }
            }
            RoundMode::Trunc => quotient.trunc(),
            RoundMode::HalfCeil => {
                if quotient % 1.0 == 0.5 {
                    quotient.ceil()
                } else {
                    quotient.round()
                }
            }
            RoundMode::HalfFloor => {
                if quotient % 1.0 == 0.5 {
                    quotient.floor()
                } else {
                    quotient.round()
                }
            }
            RoundMode::HalfExpand => {
                quotient.signum() * quotient.abs().round()
            }
            RoundMode::HalfTrunc => {
                if quotient % 1.0 == 0.5 {
                    quotient.trunc()
                } else {
                    quotient.round()
                }
            }
            RoundMode::HalfEven => {
                if quotient % 1.0 == 0.5 {
                    quotient.trunc() + (quotient % 2.0)
                } else {
                    quotient.round()
                }
            }
        };
        let rounded = NoUnits::new(rounded as i64).unwrap();
        NoUnits128::rfrom(rounded.saturating_mul(increment))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Some ad hoc tests I wrote while writing the rounding increment code.
    #[test]
    fn round_to_increment_half_expand_ad_hoc() {
        let round = |quantity: i64, increment: i64| -> i64 {
            let quantity = NoUnits::new(quantity).unwrap();
            let increment = NoUnits::new(increment).unwrap();
            i64::from(RoundMode::HalfExpand.round(quantity, increment))
        };
        assert_eq!(26, round(20, 13));

        assert_eq!(0, round(29, 60));
        assert_eq!(60, round(30, 60));
        assert_eq!(60, round(31, 60));

        assert_eq!(0, round(3, 7));
        assert_eq!(7, round(4, 7));
    }

    // The Temporal tests are inspired by the table from here:
    // https://tc39.es/proposal-temporal/#sec-temporal-roundnumbertoincrement
    //
    // The main difference is that our rounding function specifically does not
    // use floating point, so we tweak the values a bit.

    #[test]
    fn round_to_increment_temporal_table_ceil() {
        let round = |quantity: i64, increment: i64| -> i64 {
            let quantity = NoUnits::new(quantity).unwrap();
            let increment = NoUnits::new(increment).unwrap();
            RoundMode::Ceil.round(quantity, increment).into()
        };
        assert_eq!(-10, round(-15, 10));
        assert_eq!(0, round(-5, 10));
        assert_eq!(10, round(4, 10));
        assert_eq!(10, round(5, 10));
        assert_eq!(10, round(6, 10));
        assert_eq!(20, round(15, 10));
    }

    #[test]
    fn round_to_increment_temporal_table_floor() {
        let round = |quantity: i64, increment: i64| -> i64 {
            let quantity = NoUnits::new(quantity).unwrap();
            let increment = NoUnits::new(increment).unwrap();
            RoundMode::Floor.round(quantity, increment).into()
        };
        assert_eq!(-20, round(-15, 10));
        assert_eq!(-10, round(-5, 10));
        assert_eq!(0, round(4, 10));
        assert_eq!(0, round(5, 10));
        assert_eq!(0, round(6, 10));
        assert_eq!(10, round(15, 10));
    }

    #[test]
    fn round_to_increment_temporal_table_expand() {
        let round = |quantity: i64, increment: i64| -> i64 {
            let quantity = NoUnits::new(quantity).unwrap();
            let increment = NoUnits::new(increment).unwrap();
            RoundMode::Expand.round(quantity, increment).into()
        };
        assert_eq!(-20, round(-15, 10));
        assert_eq!(-10, round(-5, 10));
        assert_eq!(10, round(4, 10));
        assert_eq!(10, round(5, 10));
        assert_eq!(10, round(6, 10));
        assert_eq!(20, round(15, 10));
    }

    #[test]
    fn round_to_increment_temporal_table_trunc() {
        let round = |quantity: i64, increment: i64| -> i64 {
            let quantity = NoUnits::new(quantity).unwrap();
            let increment = NoUnits::new(increment).unwrap();
            RoundMode::Trunc.round(quantity, increment).into()
        };
        assert_eq!(-10, round(-15, 10));
        assert_eq!(0, round(-5, 10));
        assert_eq!(0, round(4, 10));
        assert_eq!(0, round(5, 10));
        assert_eq!(0, round(6, 10));
        assert_eq!(10, round(15, 10));
    }

    #[test]
    fn round_to_increment_temporal_table_half_ceil() {
        let round = |quantity: i64, increment: i64| -> i64 {
            let quantity = NoUnits::new(quantity).unwrap();
            let increment = NoUnits::new(increment).unwrap();
            RoundMode::HalfCeil.round(quantity, increment).into()
        };
        assert_eq!(-10, round(-15, 10));
        assert_eq!(0, round(-5, 10));
        assert_eq!(0, round(4, 10));
        assert_eq!(10, round(5, 10));
        assert_eq!(10, round(6, 10));
        assert_eq!(20, round(15, 10));
    }

    #[test]
    fn round_to_increment_temporal_table_half_floor() {
        let round = |quantity: i64, increment: i64| -> i64 {
            let quantity = NoUnits::new(quantity).unwrap();
            let increment = NoUnits::new(increment).unwrap();
            RoundMode::HalfFloor.round(quantity, increment).into()
        };
        assert_eq!(-20, round(-15, 10));
        assert_eq!(-10, round(-5, 10));
        assert_eq!(0, round(4, 10));
        assert_eq!(0, round(5, 10));
        assert_eq!(10, round(6, 10));
        assert_eq!(10, round(15, 10));
    }

    #[test]
    fn round_to_increment_temporal_table_half_expand() {
        let round = |quantity: i64, increment: i64| -> i64 {
            let quantity = NoUnits::new(quantity).unwrap();
            let increment = NoUnits::new(increment).unwrap();
            RoundMode::HalfExpand.round(quantity, increment).into()
        };
        assert_eq!(-20, round(-15, 10));
        assert_eq!(-10, round(-5, 10));
        assert_eq!(0, round(4, 10));
        assert_eq!(10, round(5, 10));
        assert_eq!(10, round(6, 10));
        assert_eq!(20, round(15, 10));
    }

    #[test]
    fn round_to_increment_temporal_table_half_trunc() {
        let round = |quantity: i64, increment: i64| -> i64 {
            let quantity = NoUnits::new(quantity).unwrap();
            let increment = NoUnits::new(increment).unwrap();
            RoundMode::HalfTrunc.round(quantity, increment).into()
        };
        assert_eq!(-10, round(-15, 10));
        assert_eq!(0, round(-5, 10));
        assert_eq!(0, round(4, 10));
        assert_eq!(0, round(5, 10));
        assert_eq!(10, round(6, 10));
        assert_eq!(10, round(15, 10));
    }

    #[test]
    fn round_to_increment_temporal_table_half_even() {
        let round = |quantity: i64, increment: i64| -> i64 {
            let quantity = NoUnits::new(quantity).unwrap();
            let increment = NoUnits::new(increment).unwrap();
            RoundMode::HalfEven.round(quantity, increment).into()
        };
        assert_eq!(-20, round(-15, 10));
        assert_eq!(0, round(-5, 10));
        assert_eq!(0, round(4, 10));
        assert_eq!(0, round(5, 10));
        assert_eq!(10, round(6, 10));
        assert_eq!(20, round(15, 10));
    }
}
