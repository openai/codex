//! Extension traits.

mod digit_count;
#[cfg(feature = "std")]
mod instant;
mod numerical_duration;
mod numerical_std_duration;
#[cfg(feature = "std")]
mod systemtime;

pub(crate) use self::digit_count::DigitCount;
#[cfg(feature = "std")]
pub use self::instant::InstantExt;
pub use self::numerical_duration::NumericalDuration;
pub use self::numerical_std_duration::NumericalStdDuration;
#[cfg(feature = "std")]
pub use self::systemtime::SystemTimeExt;
