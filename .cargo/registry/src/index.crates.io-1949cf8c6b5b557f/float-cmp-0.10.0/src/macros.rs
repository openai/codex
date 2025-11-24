#[macro_export]
macro_rules! approx_eq {
    ($typ:ty, $lhs:expr, $rhs:expr) => {
        {
            let m = <$typ as $crate::ApproxEq>::Margin::default();
            <$typ as $crate::ApproxEq>::approx_eq($lhs, $rhs, m)
        }
    };
    ($typ:ty, $lhs:expr, $rhs:expr $(, $set:ident = $val:expr)*) => {
        {
            use $crate::FloatMargin;
            let m = <$typ as $crate::ApproxEq>::Margin::zero()$(.$set($val))*;
            <$typ as $crate::ApproxEq>::approx_eq($lhs, $rhs, m)
        }
    };
    ($typ:ty, $lhs:expr, $rhs:expr, $marg:expr) => {
        {
            <$typ as $crate::ApproxEq>::approx_eq($lhs, $rhs, $marg)
        }
    };
}

#[macro_export]
macro_rules! assert_approx_eq {
    ($typ:ty, $lhs:expr, $rhs:expr) => {
        {
            match (&$lhs, &$rhs) {
                (left_val, right_val) => {
                    if !$crate::approx_eq!($typ, *left_val, *right_val) {
                        panic!(
                            r#"assertion failed: `(left approx_eq right)`
  left: `{:?}`,
 right: `{:?}`"#,
                            left_val, right_val,
                        )
                    }
                }
            }
        }
    };
    ($typ:ty, $lhs:expr, $rhs:expr $(, $set:ident = $val:expr)*) => {
        {
            match (&$lhs, &$rhs) {
                (left_val, right_val) => {
                    if !$crate::approx_eq!($typ, *left_val, *right_val $(, $set = $val)*) {
                        panic!(
                            r#"assertion failed: `(left approx_eq right)`
  left: `{:?}`,
 right: `{:?}`"#,
                            left_val, right_val,
                        )
                    }
                }
            }
        }
    };
    ($typ:ty, $lhs:expr, $rhs:expr, $marg:expr) => {
        {
            match (&$lhs, &$rhs) {
                (left_val, right_val) => {
                    if !$crate::approx_eq!($typ, *left_val, *right_val, $marg) {
                        panic!(
                            r#"assertion failed: `(left approx_eq right)`
  left: `{:?}`,
 right: `{:?}`"#,
                            left_val, right_val,
                        )
                    }
                }
            }
        }
    };
}

// Until saturating_abs() comes out of nightly, we have to code it ourselves.
macro_rules! saturating_abs_i32 {
    ($val:expr) => {
        if $val.is_negative() {
            match $val.checked_neg() {
                Some(v) => v,
                None => i32::MAX,
            }
        } else {
            $val
        }
    };
}
macro_rules! saturating_abs_i64 {
    ($val:expr) => {
        if $val.is_negative() {
            match $val.checked_neg() {
                Some(v) => v,
                None => i64::MAX,
            }
        } else {
            $val
        }
    };
}

#[test]
fn test_macro() {
    let a: f32 = 0.15 + 0.15 + 0.15;
    let b: f32 = 0.1 + 0.1 + 0.25;
    assert!(approx_eq!(f32, a, b)); // uses the default
    assert!(approx_eq!(f32, a, b, ulps = 2));
    assert!(approx_eq!(f32, a, b, epsilon = 0.00000003));
    assert!(approx_eq!(f32, a, b, epsilon = 0.00000003, ulps = 2));
    assert!(approx_eq!(f32, a, b, (0.0, 2)));

    assert_approx_eq!(f32, a, b); // uses the default
    assert_approx_eq!(f32, a, b, ulps = 2);
    assert_approx_eq!(f32, a, b, epsilon = 0.00000003);
    assert_approx_eq!(f32, a, b, epsilon = 0.00000003, ulps = 2);
    assert_approx_eq!(f32, a, b, (0.0, 2));
}

#[test]
fn test_macro_2() {
    assert!(approx_eq!(f64, 1000000_f64, 1000000.0000000003_f64));
    assert!(approx_eq!(
        f64,
        1000000_f64,
        1000000.0000000003_f64,
        ulps = 3
    ));
    assert!(approx_eq!(
        f64,
        1000000_f64,
        1000000.0000000003_f64,
        epsilon = 0.0000000004
    ));
    assert!(approx_eq!(
        f64,
        1000000_f64,
        1000000.0000000003_f64,
        (0.0000000004, 0)
    ));
    assert!(approx_eq!(
        f64,
        1000000_f64,
        1000000.0000000003_f64,
        (0.0, 3)
    ));

    assert_approx_eq!(f64, 1000000_f64, 1000000.0000000003_f64);
    assert_approx_eq!(f64, 1000000_f64, 1000000.0000000003_f64, ulps = 3);
    assert_approx_eq!(
        f64,
        1000000_f64,
        1000000.0000000003_f64,
        epsilon = 0.0000000004
    );
    assert_approx_eq!(f64, 1000000_f64, 1000000.0000000003_f64, (0.0000000004, 0));
    assert_approx_eq!(f64, 1000000_f64, 1000000.0000000003_f64, (0.0, 3));
}

#[test]
fn test_macro_3() {
    use crate::F32Margin;

    let a: f32 = 0.15 + 0.15 + 0.15;
    let b: f32 = 0.1 + 0.1 + 0.25;
    assert!(approx_eq!(
        f32,
        a,
        b,
        F32Margin {
            epsilon: 0.0,
            ulps: 2
        }
    ));
    assert!(approx_eq!(f32, a, b, F32Margin::default()));

    assert_approx_eq!(
        f32,
        a,
        b,
        F32Margin {
            epsilon: 0.0,
            ulps: 2
        }
    );
    assert_approx_eq!(f32, a, b, F32Margin::default());
}

#[test]
#[should_panic]
fn test_macro_4() {
    let a: f32 = 0.15 + 0.15 + 0.15;
    let b: f32 = 1.0;

    assert_approx_eq!(f32, a, b);
}

#[test]
#[should_panic]
fn test_macro_5() {
    let a: f32 = 0.15 + 0.15 + 0.15;
    let b: f32 = 1.0;

    assert_approx_eq!(f32, a, b, ulps = 2);
}

#[test]
#[should_panic]
fn test_macro_6() {
    let a: f32 = 0.15 + 0.15 + 0.15;
    let b: f32 = 1.0;

    assert_approx_eq!(f32, a, b, epsilon = 0.00000003);
}

#[test]
#[should_panic]
fn test_macro_7() {
    let a: f32 = 0.15 + 0.15 + 0.15;
    let b: f32 = 1.0;

    assert_approx_eq!(f32, a, b, epsilon = 0.00000003, ulps = 2);
}

#[test]
#[should_panic]
fn test_macro_8() {
    let a: f32 = 0.15 + 0.15 + 0.15;
    let b: f32 = 1.0;

    assert_approx_eq!(f32, a, b, (0.0, 2));
}
