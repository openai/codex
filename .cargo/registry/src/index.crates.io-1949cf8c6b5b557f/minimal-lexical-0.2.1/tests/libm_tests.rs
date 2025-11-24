#![cfg(all(not(feature = "std"), feature = "compact"))]

// These are adapted from libm, a port of musl libc's libm to Rust.
// libm can be found online [here](https://github.com/rust-lang/libm),
// and is similarly licensed under an Apache2.0/MIT license

use core::f64;
use minimal_lexical::libm;

#[test]
fn fabsf_sanity_test() {
    assert_eq!(libm::fabsf(-1.0), 1.0);
    assert_eq!(libm::fabsf(2.8), 2.8);
}

/// The spec: https://en.cppreference.com/w/cpp/numeric/math/fabs
#[test]
fn fabsf_spec_test() {
    assert!(libm::fabsf(f32::NAN).is_nan());
    for f in [0.0, -0.0].iter().copied() {
        assert_eq!(libm::fabsf(f), 0.0);
    }
    for f in [f32::INFINITY, f32::NEG_INFINITY].iter().copied() {
        assert_eq!(libm::fabsf(f), f32::INFINITY);
    }
}

#[test]
fn sqrtf_sanity_test() {
    assert_eq!(libm::sqrtf(100.0), 10.0);
    assert_eq!(libm::sqrtf(4.0), 2.0);
}

/// The spec: https://en.cppreference.com/w/cpp/numeric/math/sqrt
#[test]
fn sqrtf_spec_test() {
    // Not Asserted: FE_INVALID exception is raised if argument is negative.
    assert!(libm::sqrtf(-1.0).is_nan());
    assert!(libm::sqrtf(f32::NAN).is_nan());
    for f in [0.0, -0.0, f32::INFINITY].iter().copied() {
        assert_eq!(libm::sqrtf(f), f);
    }
}

const POS_ZERO: &[f64] = &[0.0];
const NEG_ZERO: &[f64] = &[-0.0];
const POS_ONE: &[f64] = &[1.0];
const NEG_ONE: &[f64] = &[-1.0];
const POS_FLOATS: &[f64] = &[99.0 / 70.0, f64::consts::E, f64::consts::PI];
const NEG_FLOATS: &[f64] = &[-99.0 / 70.0, -f64::consts::E, -f64::consts::PI];
const POS_SMALL_FLOATS: &[f64] = &[(1.0 / 2.0), f64::MIN_POSITIVE, f64::EPSILON];
const NEG_SMALL_FLOATS: &[f64] = &[-(1.0 / 2.0), -f64::MIN_POSITIVE, -f64::EPSILON];
const POS_EVENS: &[f64] = &[2.0, 6.0, 8.0, 10.0, 22.0, 100.0, f64::MAX];
const NEG_EVENS: &[f64] = &[f64::MIN, -100.0, -22.0, -10.0, -8.0, -6.0, -2.0];
const POS_ODDS: &[f64] = &[3.0, 7.0];
const NEG_ODDS: &[f64] = &[-7.0, -3.0];
const NANS: &[f64] = &[f64::NAN];
const POS_INF: &[f64] = &[f64::INFINITY];
const NEG_INF: &[f64] = &[f64::NEG_INFINITY];

const ALL: &[&[f64]] = &[
    POS_ZERO,
    NEG_ZERO,
    NANS,
    NEG_SMALL_FLOATS,
    POS_SMALL_FLOATS,
    NEG_FLOATS,
    POS_FLOATS,
    NEG_EVENS,
    POS_EVENS,
    NEG_ODDS,
    POS_ODDS,
    NEG_INF,
    POS_INF,
    NEG_ONE,
    POS_ONE,
];
const POS: &[&[f64]] = &[POS_ZERO, POS_ODDS, POS_ONE, POS_FLOATS, POS_EVENS, POS_INF];
const NEG: &[&[f64]] = &[NEG_ZERO, NEG_ODDS, NEG_ONE, NEG_FLOATS, NEG_EVENS, NEG_INF];

fn powd(base: f64, exponent: f64, expected: f64) {
    let res = libm::powd(base, exponent);
    assert!(
        if expected.is_nan() {
            res.is_nan()
        } else {
            libm::powd(base, exponent) == expected
        },
        "{} ** {} was {} instead of {}",
        base,
        exponent,
        res,
        expected
    );
}

fn powd_test_sets_as_base(sets: &[&[f64]], exponent: f64, expected: f64) {
    sets.iter().for_each(|s| s.iter().for_each(|val| powd(*val, exponent, expected)));
}

fn powd_test_sets_as_exponent(base: f64, sets: &[&[f64]], expected: f64) {
    sets.iter().for_each(|s| s.iter().for_each(|val| powd(base, *val, expected)));
}

fn powd_test_sets(sets: &[&[f64]], computed: &dyn Fn(f64) -> f64, expected: &dyn Fn(f64) -> f64) {
    sets.iter().for_each(|s| {
        s.iter().for_each(|val| {
            let exp = expected(*val);
            let res = computed(*val);

            assert!(
                if exp.is_nan() {
                    res.is_nan()
                } else {
                    exp == res
                },
                "test for {} was {} instead of {}",
                val,
                res,
                exp
            );
        })
    });
}

#[test]
fn powd_zero_as_exponent() {
    powd_test_sets_as_base(ALL, 0.0, 1.0);
    powd_test_sets_as_base(ALL, -0.0, 1.0);
}

#[test]
fn powd_one_as_base() {
    powd_test_sets_as_exponent(1.0, ALL, 1.0);
}

#[test]
fn powd_nan_inputs() {
    // NAN as the base:
    // (NAN ^ anything *but 0* should be NAN)
    powd_test_sets_as_exponent(f64::NAN, &ALL[2..], f64::NAN);

    // NAN as the exponent:
    // (anything *but 1* ^ NAN should be NAN)
    powd_test_sets_as_base(&ALL[..(ALL.len() - 2)], f64::NAN, f64::NAN);
}

#[test]
fn powd_infinity_as_base() {
    // Positive Infinity as the base:
    // (+Infinity ^ positive anything but 0 and NAN should be +Infinity)
    powd_test_sets_as_exponent(f64::INFINITY, &POS[1..], f64::INFINITY);

    // (+Infinity ^ negative anything except 0 and NAN should be 0.0)
    powd_test_sets_as_exponent(f64::INFINITY, &NEG[1..], 0.0);

    // Negative Infinity as the base:
    // (-Infinity ^ positive odd ints should be -Infinity)
    powd_test_sets_as_exponent(f64::NEG_INFINITY, &[POS_ODDS], f64::NEG_INFINITY);

    // (-Infinity ^ anything but odd ints should be == -0 ^ (-anything))
    // We can lump in pos/neg odd ints here because they don't seem to
    // cause panics (div by zero) in release mode (I think).
    powd_test_sets(ALL, &|v: f64| libm::powd(f64::NEG_INFINITY, v), &|v: f64| libm::powd(-0.0, -v));
}

#[test]
fn infinity_as_exponent() {
    // Positive/Negative base greater than 1:
    // (pos/neg > 1 ^ Infinity should be Infinity - note this excludes NAN as the base)
    powd_test_sets_as_base(&ALL[5..(ALL.len() - 2)], f64::INFINITY, f64::INFINITY);

    // (pos/neg > 1 ^ -Infinity should be 0.0)
    powd_test_sets_as_base(&ALL[5..ALL.len() - 2], f64::NEG_INFINITY, 0.0);

    // Positive/Negative base less than 1:
    let base_below_one = &[POS_ZERO, NEG_ZERO, NEG_SMALL_FLOATS, POS_SMALL_FLOATS];

    // (pos/neg < 1 ^ Infinity should be 0.0 - this also excludes NAN as the base)
    powd_test_sets_as_base(base_below_one, f64::INFINITY, 0.0);

    // (pos/neg < 1 ^ -Infinity should be Infinity)
    powd_test_sets_as_base(base_below_one, f64::NEG_INFINITY, f64::INFINITY);

    // Positive/Negative 1 as the base:
    // (pos/neg 1 ^ Infinity should be 1)
    powd_test_sets_as_base(&[NEG_ONE, POS_ONE], f64::INFINITY, 1.0);

    // (pos/neg 1 ^ -Infinity should be 1)
    powd_test_sets_as_base(&[NEG_ONE, POS_ONE], f64::NEG_INFINITY, 1.0);
}

#[test]
fn powd_zero_as_base() {
    // Positive Zero as the base:
    // (+0 ^ anything positive but 0 and NAN should be +0)
    powd_test_sets_as_exponent(0.0, &POS[1..], 0.0);

    // (+0 ^ anything negative but 0 and NAN should be Infinity)
    // (this should panic because we're dividing by zero)
    powd_test_sets_as_exponent(0.0, &NEG[1..], f64::INFINITY);

    // Negative Zero as the base:
    // (-0 ^ anything positive but 0, NAN, and odd ints should be +0)
    powd_test_sets_as_exponent(-0.0, &POS[3..], 0.0);

    // (-0 ^ anything negative but 0, NAN, and odd ints should be Infinity)
    // (should panic because of divide by zero)
    powd_test_sets_as_exponent(-0.0, &NEG[3..], f64::INFINITY);

    // (-0 ^ positive odd ints should be -0)
    powd_test_sets_as_exponent(-0.0, &[POS_ODDS], -0.0);

    // (-0 ^ negative odd ints should be -Infinity)
    // (should panic because of divide by zero)
    powd_test_sets_as_exponent(-0.0, &[NEG_ODDS], f64::NEG_INFINITY);
}

#[test]
fn special_cases() {
    // One as the exponent:
    // (anything ^ 1 should be anything - i.e. the base)
    powd_test_sets(ALL, &|v: f64| libm::powd(v, 1.0), &|v: f64| v);

    // Negative One as the exponent:
    // (anything ^ -1 should be 1/anything)
    powd_test_sets(ALL, &|v: f64| libm::powd(v, -1.0), &|v: f64| 1.0 / v);

    // Factoring -1 out:
    // (negative anything ^ integer should be (-1 ^ integer) * (positive anything ^ integer))
    [POS_ZERO, NEG_ZERO, POS_ONE, NEG_ONE, POS_EVENS, NEG_EVENS].iter().for_each(|int_set| {
        int_set.iter().for_each(|int| {
            powd_test_sets(ALL, &|v: f64| libm::powd(-v, *int), &|v: f64| {
                libm::powd(-1.0, *int) * libm::powd(v, *int)
            });
        })
    });

    // Negative base (imaginary results):
    // (-anything except 0 and Infinity ^ non-integer should be NAN)
    NEG[1..(NEG.len() - 1)].iter().for_each(|set| {
        set.iter().for_each(|val| {
            powd_test_sets(&ALL[3..7], &|v: f64| libm::powd(*val, v), &|_| f64::NAN);
        })
    });
}

#[test]
fn normal_cases() {
    assert_eq!(libm::powd(2.0, 20.0), (1 << 20) as f64);
    assert_eq!(libm::powd(-1.0, 9.0), -1.0);
    assert!(libm::powd(-1.0, 2.2).is_nan());
    assert!(libm::powd(-1.0, -1.14).is_nan());
}

#[test]
fn fabsd_sanity_test() {
    assert_eq!(libm::fabsd(-1.0), 1.0);
    assert_eq!(libm::fabsd(2.8), 2.8);
}

/// The spec: https://en.cppreference.com/w/cpp/numeric/math/fabs
#[test]
fn fabsd_spec_test() {
    assert!(libm::fabsd(f64::NAN).is_nan());
    for f in [0.0, -0.0].iter().copied() {
        assert_eq!(libm::fabsd(f), 0.0);
    }
    for f in [f64::INFINITY, f64::NEG_INFINITY].iter().copied() {
        assert_eq!(libm::fabsd(f), f64::INFINITY);
    }
}

#[test]
fn sqrtd_sanity_test() {
    assert_eq!(libm::sqrtd(100.0), 10.0);
    assert_eq!(libm::sqrtd(4.0), 2.0);
}

/// The spec: https://en.cppreference.com/w/cpp/numeric/math/sqrt
#[test]
fn sqrtd_spec_test() {
    // Not Asserted: FE_INVALID exception is raised if argument is negative.
    assert!(libm::sqrtd(-1.0).is_nan());
    assert!(libm::sqrtd(f64::NAN).is_nan());
    for f in [0.0, -0.0, f64::INFINITY].iter().copied() {
        assert_eq!(libm::sqrtd(f), f);
    }
}
