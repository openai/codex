use nix::sys::time::{TimeSpec, TimeVal, TimeValLike};
use std::time::Duration;

#[test]
pub fn test_timespec() {
    assert_ne!(TimeSpec::seconds(1), TimeSpec::zero());
    assert_eq!(
        TimeSpec::seconds(1) + TimeSpec::seconds(2),
        TimeSpec::seconds(3)
    );
    assert_eq!(
        TimeSpec::minutes(3) + TimeSpec::seconds(2),
        TimeSpec::seconds(182)
    );
}

#[test]
pub fn test_timespec_from() {
    let duration = Duration::new(123, 123_456_789);
    let timespec = TimeSpec::nanoseconds(123_123_456_789);

    assert_eq!(TimeSpec::from(duration), timespec);
    assert_eq!(Duration::from(timespec), duration);
}

#[test]
pub fn test_timespec_neg() {
    let a = TimeSpec::seconds(1) + TimeSpec::nanoseconds(123);
    let b = TimeSpec::seconds(-1) + TimeSpec::nanoseconds(-123);

    assert_eq!(a, -b);
}

#[test]
pub fn test_timespec_ord() {
    assert_eq!(TimeSpec::seconds(1), TimeSpec::nanoseconds(1_000_000_000));
    assert!(TimeSpec::seconds(1) < TimeSpec::nanoseconds(1_000_000_001));
    assert!(TimeSpec::seconds(1) > TimeSpec::nanoseconds(999_999_999));
    assert!(TimeSpec::seconds(-1) < TimeSpec::nanoseconds(-999_999_999));
    assert!(TimeSpec::seconds(-1) > TimeSpec::nanoseconds(-1_000_000_001));
}

#[test]
pub fn test_timespec_fmt() {
    assert_eq!(TimeSpec::zero().to_string(), "0 seconds");
    assert_eq!(TimeSpec::seconds(42).to_string(), "42 seconds");
    assert_eq!(TimeSpec::milliseconds(42).to_string(), "0.042 seconds");
    assert_eq!(TimeSpec::microseconds(42).to_string(), "0.000042 seconds");
    assert_eq!(TimeSpec::nanoseconds(42).to_string(), "0.000000042 seconds");
    assert_eq!(TimeSpec::seconds(-86401).to_string(), "-86401 seconds");
}

#[test]
pub fn test_timeval() {
    assert_ne!(TimeVal::seconds(1), TimeVal::zero());
    assert_eq!(
        TimeVal::seconds(1) + TimeVal::seconds(2),
        TimeVal::seconds(3)
    );
    assert_eq!(
        TimeVal::minutes(3) + TimeVal::seconds(2),
        TimeVal::seconds(182)
    );
}

#[test]
pub fn test_timeval_ord() {
    assert_eq!(TimeVal::seconds(1), TimeVal::microseconds(1_000_000));
    assert!(TimeVal::seconds(1) < TimeVal::microseconds(1_000_001));
    assert!(TimeVal::seconds(1) > TimeVal::microseconds(999_999));
    assert!(TimeVal::seconds(-1) < TimeVal::microseconds(-999_999));
    assert!(TimeVal::seconds(-1) > TimeVal::microseconds(-1_000_001));
}

#[test]
pub fn test_timeval_neg() {
    let a = TimeVal::seconds(1) + TimeVal::microseconds(123);
    let b = TimeVal::seconds(-1) + TimeVal::microseconds(-123);

    assert_eq!(a, -b);
}

#[test]
pub fn test_timeval_fmt() {
    assert_eq!(TimeVal::zero().to_string(), "0 seconds");
    assert_eq!(TimeVal::seconds(42).to_string(), "42 seconds");
    assert_eq!(TimeVal::milliseconds(42).to_string(), "0.042 seconds");
    assert_eq!(TimeVal::microseconds(42).to_string(), "0.000042 seconds");
    assert_eq!(TimeVal::nanoseconds(1402).to_string(), "0.000001 seconds");
    assert_eq!(TimeVal::seconds(-86401).to_string(), "-86401 seconds");
}
