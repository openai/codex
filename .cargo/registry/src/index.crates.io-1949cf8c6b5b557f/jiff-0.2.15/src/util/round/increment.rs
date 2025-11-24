/*!
This module provides logic for validating rounding increments.

Each of the types we support rounding for have their own logic for how the
rounding increment is validated. For example, when rounding timestamps, only
rounding increments up to hours are supported. But when rounding datetimes,
rounding increments up to days are supported. Similarly, rounding increments
for time units must divide evenly into 1 unit of the next highest unit.
*/

use crate::{
    error::{err, Error},
    util::{
        rangeint::RFrom,
        t::{self, Constant, C},
    },
    Unit,
};

/// Validates the given rounding increment for the given unit.
///
/// This validation ensures the rounding increment is valid for rounding spans.
pub(crate) fn for_span(
    unit: Unit,
    increment: i64,
) -> Result<t::NoUnits128, Error> {
    // Indexed by `Unit`.
    static LIMIT: &[Constant] = &[
        t::NANOS_PER_MICRO,
        t::MICROS_PER_MILLI,
        t::MILLIS_PER_SECOND,
        t::SECONDS_PER_MINUTE,
        t::MINUTES_PER_HOUR,
        t::HOURS_PER_CIVIL_DAY,
    ];
    // We allow any kind of increment for calendar units, but for time units,
    // they have to divide evenly into the next highest unit (and also be less
    // than that). The reason for this is that calendar units vary, where as
    // for time units, given a balanced span, you know that time units will
    // always spill over into days so that hours/minutes/... will never exceed
    // 24/60/...
    if unit >= Unit::Day {
        // We specifically go from NoUnits to NoUnits128 here instead of
        // directly to NoUnits128 to ensure our increment bounds match the
        // bounds of i64 and not i128.
        Ok(t::NoUnits128::rfrom(t::NoUnits::new_unchecked(increment)))
    } else {
        get_with_limit(unit, increment, "span", LIMIT)
    }
}

/// Validates the given rounding increment for the given unit.
///
/// This validation ensures the rounding increment is valid for rounding
/// datetimes (both civil and time zone aware).
pub(crate) fn for_datetime(
    unit: Unit,
    increment: i64,
) -> Result<t::NoUnits128, Error> {
    // Indexed by `Unit`.
    static LIMIT: &[Constant] = &[
        t::NANOS_PER_MICRO,
        t::MICROS_PER_MILLI,
        t::MILLIS_PER_SECOND,
        t::SECONDS_PER_MINUTE,
        t::MINUTES_PER_HOUR,
        t::HOURS_PER_CIVIL_DAY,
        Constant(2),
    ];
    get_with_limit(unit, increment, "datetime", LIMIT)
}

/// Validates the given rounding increment for the given unit.
///
/// This validation ensures the rounding increment is valid for rounding
/// civil times.
pub(crate) fn for_time(
    unit: Unit,
    increment: i64,
) -> Result<t::NoUnits128, Error> {
    // Indexed by `Unit`.
    static LIMIT: &[Constant] = &[
        t::NANOS_PER_MICRO,
        t::MICROS_PER_MILLI,
        t::MILLIS_PER_SECOND,
        t::SECONDS_PER_MINUTE,
        t::MINUTES_PER_HOUR,
        t::HOURS_PER_CIVIL_DAY,
    ];
    get_with_limit(unit, increment, "time", LIMIT)
}

/// Validates the given rounding increment for the given unit.
///
/// This validation ensures the rounding increment is valid for rounding
/// timestamps.
pub(crate) fn for_timestamp(
    unit: Unit,
    increment: i64,
) -> Result<t::NoUnits128, Error> {
    // Indexed by `Unit`.
    static MAX: &[Constant] = &[
        t::NANOS_PER_CIVIL_DAY,
        t::MICROS_PER_CIVIL_DAY,
        t::MILLIS_PER_CIVIL_DAY,
        t::SECONDS_PER_CIVIL_DAY,
        t::MINUTES_PER_CIVIL_DAY,
        t::HOURS_PER_CIVIL_DAY,
    ];
    get_with_max(unit, increment, "timestamp", MAX)
}

fn get_with_limit(
    unit: Unit,
    increment: i64,
    what: &'static str,
    limit: &[t::Constant],
) -> Result<t::NoUnits128, Error> {
    // OK because `NoUnits` specifically allows any `i64` value.
    let increment = t::NoUnits::new_unchecked(increment);
    if increment <= C(0) {
        return Err(err!(
            "rounding increment {increment} for {unit} must be \
             greater than zero",
            unit = unit.plural(),
        ));
    }
    let Some(must_divide) = limit.get(unit as usize) else {
        return Err(err!(
            "{what} rounding does not support {unit}",
            unit = unit.plural()
        ));
    };
    let must_divide = t::NoUnits::rfrom(*must_divide);
    if increment >= must_divide || must_divide % increment != C(0) {
        Err(err!(
            "increment {increment} for rounding {what} to {unit} \
             must be 1) less than {must_divide}, 2) divide into \
             it evenly and 3) greater than zero",
            unit = unit.plural(),
        ))
    } else {
        Ok(t::NoUnits128::rfrom(increment))
    }
}

fn get_with_max(
    unit: Unit,
    increment: i64,
    what: &'static str,
    max: &[t::Constant],
) -> Result<t::NoUnits128, Error> {
    // OK because `NoUnits` specifically allows any `i64` value.
    let increment = t::NoUnits::new_unchecked(increment);
    if increment <= C(0) {
        return Err(err!(
            "rounding increment {increment} for {unit} must be \
             greater than zero",
            unit = unit.plural(),
        ));
    }
    let Some(must_divide) = max.get(unit as usize) else {
        return Err(err!(
            "{what} rounding does not support {unit}",
            unit = unit.plural()
        ));
    };
    let must_divide = t::NoUnits::rfrom(*must_divide);
    if increment > must_divide || must_divide % increment != C(0) {
        Err(err!(
            "increment {increment} for rounding {what} to {unit} \
             must be 1) less than or equal to {must_divide}, \
             2) divide into it evenly and 3) greater than zero",
            unit = unit.plural(),
        ))
    } else {
        Ok(t::NoUnits128::rfrom(increment))
    }
}
