//! Implementation of [`Distribution`] for various structs.

use rand09::distr::{Distribution, StandardUniform};
use rand09::Rng;

use crate::{
    Date, Duration, Month, OffsetDateTime, PrimitiveDateTime, Time, UtcDateTime, UtcOffset, Weekday,
};

impl Distribution<Time> for StandardUniform {
    #[inline]
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Time {
        Time::from_hms_nanos_ranged(rng.random(), rng.random(), rng.random(), rng.random())
    }
}

impl Distribution<Date> for StandardUniform {
    #[inline]
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Date {
        // Safety: The Julian day number is in range.
        unsafe {
            Date::from_julian_day_unchecked(
                rng.random_range(Date::MIN.to_julian_day()..=Date::MAX.to_julian_day()),
            )
        }
    }
}

impl Distribution<UtcOffset> for StandardUniform {
    #[inline]
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> UtcOffset {
        UtcOffset::from_hms_ranged(rng.random(), rng.random(), rng.random())
    }
}

impl Distribution<PrimitiveDateTime> for StandardUniform {
    #[inline]
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> PrimitiveDateTime {
        PrimitiveDateTime::new(Self.sample(rng), Self.sample(rng))
    }
}

impl Distribution<UtcDateTime> for StandardUniform {
    #[inline]
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> UtcDateTime {
        UtcDateTime::new(Self.sample(rng), Self.sample(rng))
    }
}

impl Distribution<OffsetDateTime> for StandardUniform {
    #[inline]
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> OffsetDateTime {
        let date_time: PrimitiveDateTime = Self.sample(rng);
        date_time.assume_offset(Self.sample(rng))
    }
}

impl Distribution<Duration> for StandardUniform {
    #[inline]
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Duration {
        Duration::new_ranged(rng.random(), rng.random())
    }
}

impl Distribution<Weekday> for StandardUniform {
    #[inline]
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Weekday {
        use Weekday::*;

        match rng.random_range(0u8..7) {
            0 => Monday,
            1 => Tuesday,
            2 => Wednesday,
            3 => Thursday,
            4 => Friday,
            5 => Saturday,
            val => {
                debug_assert!(val == 6);
                Sunday
            }
        }
    }
}

impl Distribution<Month> for StandardUniform {
    #[inline]
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Month {
        use Month::*;
        match rng.random_range(1u8..=12) {
            1 => January,
            2 => February,
            3 => March,
            4 => April,
            5 => May,
            6 => June,
            7 => July,
            8 => August,
            9 => September,
            10 => October,
            11 => November,
            val => {
                debug_assert!(val == 12);
                December
            }
        }
    }
}
