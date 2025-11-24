// Adapted from https://github.com/Alexhuszagh/rust-lexical.

use crate::lexical::algorithm::*;
use crate::lexical::num::Float;

#[test]
fn float_fast_path_test() {
    // valid
    let mantissa = (1 << f32::MANTISSA_SIZE) - 1;
    let (min_exp, max_exp) = f32::exponent_limit();
    for exp in min_exp..=max_exp {
        let f = fast_path::<f32>(mantissa, exp);
        assert!(f.is_some(), "should be valid {:?}.", (mantissa, exp));
    }

    // Check slightly above valid exponents
    let f = fast_path::<f32>(123, 15);
    assert_eq!(f, Some(1.23e+17));

    // Exponent is 1 too high, pushes over the mantissa.
    let f = fast_path::<f32>(123, 16);
    assert!(f.is_none());

    // Mantissa is too large, checked_mul should overflow.
    let f = fast_path::<f32>(mantissa, 11);
    assert!(f.is_none());

    // invalid exponents
    let (min_exp, max_exp) = f32::exponent_limit();
    let f = fast_path::<f32>(mantissa, min_exp - 1);
    assert!(f.is_none(), "exponent under min_exp");

    let f = fast_path::<f32>(mantissa, max_exp + 1);
    assert!(f.is_none(), "exponent above max_exp");
}

#[test]
fn double_fast_path_test() {
    // valid
    let mantissa = (1 << f64::MANTISSA_SIZE) - 1;
    let (min_exp, max_exp) = f64::exponent_limit();
    for exp in min_exp..=max_exp {
        let f = fast_path::<f64>(mantissa, exp);
        assert!(f.is_some(), "should be valid {:?}.", (mantissa, exp));
    }

    // invalid exponents
    let (min_exp, max_exp) = f64::exponent_limit();
    let f = fast_path::<f64>(mantissa, min_exp - 1);
    assert!(f.is_none(), "exponent under min_exp");

    let f = fast_path::<f64>(mantissa, max_exp + 1);
    assert!(f.is_none(), "exponent above max_exp");

    assert_eq!(
        Some(0.04628372940652459),
        fast_path::<f64>(4628372940652459, -17)
    );
    assert_eq!(None, fast_path::<f64>(26383446160308229, -272));
}

#[test]
fn moderate_path_test() {
    let (f, valid) = moderate_path::<f64>(1234567890, -1, false);
    assert!(valid, "should be valid");
    assert_eq!(f.into_float::<f64>(), 123456789.0);

    let (f, valid) = moderate_path::<f64>(1234567891, -1, false);
    assert!(valid, "should be valid");
    assert_eq!(f.into_float::<f64>(), 123456789.1);

    let (f, valid) = moderate_path::<f64>(12345678912, -2, false);
    assert!(valid, "should be valid");
    assert_eq!(f.into_float::<f64>(), 123456789.12);

    let (f, valid) = moderate_path::<f64>(123456789123, -3, false);
    assert!(valid, "should be valid");
    assert_eq!(f.into_float::<f64>(), 123456789.123);

    let (f, valid) = moderate_path::<f64>(1234567891234, -4, false);
    assert!(valid, "should be valid");
    assert_eq!(f.into_float::<f64>(), 123456789.1234);

    let (f, valid) = moderate_path::<f64>(12345678912345, -5, false);
    assert!(valid, "should be valid");
    assert_eq!(f.into_float::<f64>(), 123456789.12345);

    let (f, valid) = moderate_path::<f64>(123456789123456, -6, false);
    assert!(valid, "should be valid");
    assert_eq!(f.into_float::<f64>(), 123456789.123456);

    let (f, valid) = moderate_path::<f64>(1234567891234567, -7, false);
    assert!(valid, "should be valid");
    assert_eq!(f.into_float::<f64>(), 123456789.1234567);

    let (f, valid) = moderate_path::<f64>(12345678912345679, -8, false);
    assert!(valid, "should be valid");
    assert_eq!(f.into_float::<f64>(), 123456789.12345679);

    let (f, valid) = moderate_path::<f64>(4628372940652459, -17, false);
    assert!(valid, "should be valid");
    assert_eq!(f.into_float::<f64>(), 0.04628372940652459);

    let (f, valid) = moderate_path::<f64>(26383446160308229, -272, false);
    assert!(valid, "should be valid");
    assert_eq!(f.into_float::<f64>(), 2.6383446160308229e-256);

    let (_, valid) = moderate_path::<f64>(26383446160308230, -272, false);
    assert!(!valid, "should be invalid");
}
