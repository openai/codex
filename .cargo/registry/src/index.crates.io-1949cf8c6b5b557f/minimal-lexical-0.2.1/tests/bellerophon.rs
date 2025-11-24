#![cfg(feature = "compact")]
#![allow(dead_code)]

use minimal_lexical::bellerophon::bellerophon;
use minimal_lexical::extended_float::{extended_to_float, ExtendedFloat};
use minimal_lexical::num::Float;
use minimal_lexical::number::Number;

pub fn bellerophon_test<F: Float + core::fmt::Debug>(
    xmant: u64,
    xexp: i32,
    many_digits: bool,
    ymant: u64,
    yexp: i32,
) {
    let num = Number {
        exponent: xexp,
        mantissa: xmant,
        many_digits,
    };
    let xfp = bellerophon::<F>(&num);
    let yfp = ExtendedFloat {
        mant: ymant,
        exp: yexp,
    };
    // Given us useful error messages if the floats are valid.
    if xfp.exp >= 0 && yfp.exp >= 0 {
        assert!(
            xfp == yfp,
            "x != y, xfp={:?}, yfp={:?}, x={:?}, y={:?}",
            xfp,
            yfp,
            extended_to_float::<F>(xfp),
            extended_to_float::<F>(yfp)
        );
    } else {
        assert_eq!(xfp, yfp);
    }
}

pub fn compute_float32(q: i32, w: u64) -> (i32, u64) {
    let num = Number {
        exponent: q,
        mantissa: w,
        many_digits: false,
    };
    let fp = bellerophon::<f32>(&num);
    (fp.exp, fp.mant)
}

pub fn compute_float64(q: i32, w: u64) -> (i32, u64) {
    let num = Number {
        exponent: q,
        mantissa: w,
        many_digits: false,
    };
    let fp = bellerophon::<f64>(&num);
    (fp.exp, fp.mant)
}
