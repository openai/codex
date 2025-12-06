//! Polyfills providing pure rust fallback implementations of various `f64` methods for `no_std`
//! compatibility. These implementations may not be as accurate as their built-in counterparts.
//! This must be taken into account when using floating point math in this crate.
//!
//! Implementations based on [`micromath`](https://github.com/tarcieri/micromath) crate.
//!
//! Related Rust tracking issues:
//!
//! - <https://github.com/rust-lang/rust/issues/50145>
//! - <https://github.com/rust-lang/rust/issues/137578>
use core::f64::consts::{FRAC_1_PI, PI};

#[inline]
fn mul_add(val: f64, a: f64, b: f64) -> f64 {
    val * a + b
}

#[inline]
fn round(val: f64) -> f64 {
    (val + 0.5f64.copysign(val)) as i64 as f64
}

#[inline]
fn floor(val: f64) -> f64 {
    let mut res = (val as i64) as f64;
    if val < res {
        res -= 1.0;
    }
    res
}

#[inline]
fn sin(val: f64) -> f64 {
    cos(val - PI / 2.0)
}

#[inline]
fn cos(val: f64) -> f64 {
    let mut x = val;
    x *= FRAC_1_PI / 2.0;
    x -= 0.25 + floor(x + 0.25);
    x *= 16.0 * (x.abs() - 0.5);
    x += 0.225 * x * (x.abs() - 1.0);
    x
}

pub(crate) trait F64Polyfills {
    /// Computes `(self * a) + b`.
    fn mul_add(self, a: f64, b: f64) -> f64;

    /// Returns the nearest integer to `self`. If a value is half-way between two integers, round
    /// away from `0.0`.
    fn round(self) -> f64;

    /// Returns the largest integer less than or equal to `self`.
    fn floor(self) -> f64;

    /// Approximates the sine of a number (in radians) with max error of `0.002`.
    fn sin(self) -> f64;

    /// Approximates the cosine of a number (in radians) with max error of `0.002`.
    fn cos(self) -> f64;
}

impl F64Polyfills for f64 {
    #[inline]
    fn mul_add(self, a: f64, b: f64) -> f64 {
        mul_add(self, a, b)
    }
    #[inline]
    fn round(self) -> f64 {
        round(self)
    }
    #[inline]
    fn floor(self) -> f64 {
        floor(self)
    }
    #[inline]
    fn sin(self) -> f64 {
        sin(self)
    }
    #[inline]
    fn cos(self) -> f64 {
        cos(self)
    }
}

#[cfg(test)]
mod tests {
    use core::f64::consts::{FRAC_PI_2, PI, TAU};

    use super::*;
    // explicitly use std to prevent testing against itself
    extern crate std;

    const TEST_VALUES: [f64; 24] = [
        0.0, 0.5, -0.5, 1.0, -1.0, PI, -PI, FRAC_PI_2, -FRAC_PI_2, TAU, -TAU, 9.4248, -9.4248,
        0.2528, -7.7047, -1.1596, -2.6095, 6.8435, 3.5392, 5.9725, 0.9172, 9.3539, 2.8843, -1.8483,
    ];

    const MAX_ERROR: f64 = 0.000_000_000_000_01;
    const TRIG_MAX_ERROR: f64 = 0.002;

    fn assert_with_error(computed: f64, expected: f64, max_error: f64) {
        let delta = (computed - expected).abs();
        assert!(
            delta <= max_error,
            "error exceeded max value of {max_error}: {computed} vs {expected}"
        );
    }

    #[test]
    fn f64_mul_add() {
        for chunk in TEST_VALUES.chunks(3) {
            let expected = chunk[0].mul_add(chunk[1], chunk[2]);
            let computed = mul_add(chunk[0], chunk[1], chunk[2]);
            assert_with_error(computed, expected, MAX_ERROR);
        }
    }

    #[test]
    fn f64_round() {
        for value in TEST_VALUES {
            let expected = value.round();
            let computed = round(value);
            assert_with_error(computed, expected, MAX_ERROR);
        }
    }

    #[test]
    fn f64_floor() {
        for value in TEST_VALUES {
            let expected = value.floor();
            let computed = floor(value);
            assert_with_error(computed, expected, MAX_ERROR);
        }
    }

    #[test]
    fn f64_sin() {
        for value in TEST_VALUES {
            let expected = value.sin();
            let computed = sin(value);
            assert_with_error(computed, expected, TRIG_MAX_ERROR);
        }
    }

    #[test]
    fn f64_cos() {
        for value in TEST_VALUES {
            let expected = value.cos();
            let computed = cos(value);
            assert_with_error(computed, expected, TRIG_MAX_ERROR);
        }
    }
}
