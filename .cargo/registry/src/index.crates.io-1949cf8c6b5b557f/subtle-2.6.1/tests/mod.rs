use std::cmp;

use rand::rngs::OsRng;
use rand::RngCore;

use subtle::*;

#[test]
#[should_panic]
fn slices_equal_different_lengths() {
    let a: [u8; 3] = [0, 0, 0];
    let b: [u8; 4] = [0, 0, 0, 0];

    assert_eq!((&a).ct_eq(&b).unwrap_u8(), 1);
}

#[test]
fn slices_equal() {
    let a: [u8; 8] = [1, 2, 3, 4, 5, 6, 7, 8];
    let b: [u8; 8] = [1, 2, 3, 4, 4, 3, 2, 1];

    let a_eq_a = (&a).ct_eq(&a);
    let a_eq_b = (&a).ct_eq(&b);

    assert_eq!(a_eq_a.unwrap_u8(), 1);
    assert_eq!(a_eq_b.unwrap_u8(), 0);

    let c: [u8; 16] = [0u8; 16];

    let a_eq_c = (&a).ct_eq(&c);
    assert_eq!(a_eq_c.unwrap_u8(), 0);
}

#[test]
fn conditional_assign_i32() {
    let mut a: i32 = 5;
    let b: i32 = 13;

    a.conditional_assign(&b, 0.into());
    assert_eq!(a, 5);
    a.conditional_assign(&b, 1.into());
    assert_eq!(a, 13);
}

#[test]
fn conditional_assign_i64() {
    let mut c: i64 = 2343249123;
    let d: i64 = 8723884895;

    c.conditional_assign(&d, 0.into());
    assert_eq!(c, 2343249123);
    c.conditional_assign(&d, 1.into());
    assert_eq!(c, 8723884895);
}

macro_rules! generate_integer_conditional_select_tests {
    ($($t:ty)*) => ($(
        let x: $t = 0;  // all 0 bits
        let y: $t = !0; // all 1 bits

        assert_eq!(<$t>::conditional_select(&x, &y, 0.into()), x);
        assert_eq!(<$t>::conditional_select(&x, &y, 1.into()), y);

        let mut z = x;
        let mut w = y;

        <$t>::conditional_swap(&mut z, &mut w, 0.into());
        assert_eq!(z, x);
        assert_eq!(w, y);
        <$t>::conditional_swap(&mut z, &mut w, 1.into());
        assert_eq!(z, y);
        assert_eq!(w, x);

        z.conditional_assign(&x, 1.into());
        w.conditional_assign(&y, 0.into());
        assert_eq!(z, x);
        assert_eq!(w, x);
    )*)
}

#[test]
fn integer_conditional_select() {
    generate_integer_conditional_select_tests!(u8 u16 u32 u64);
    generate_integer_conditional_select_tests!(i8 i16 i32 i64);
    #[cfg(feature = "i128")]
    generate_integer_conditional_select_tests!(i128 u128);
}

#[test]
fn custom_conditional_select_i16() {
    let x: i16 = 257;
    let y: i16 = 514;

    assert_eq!(i16::conditional_select(&x, &y, 0.into()), 257);
    assert_eq!(i16::conditional_select(&x, &y, 1.into()), 514);
}

#[test]
fn ordering_conditional_select() {
    assert_eq!(
        cmp::Ordering::conditional_select(&cmp::Ordering::Less, &cmp::Ordering::Greater, 0.into()),
        cmp::Ordering::Less
    );

    assert_eq!(
        cmp::Ordering::conditional_select(&cmp::Ordering::Less, &cmp::Ordering::Greater, 1.into()),
        cmp::Ordering::Greater
    );
}

macro_rules! generate_integer_equal_tests {
    ($($t:ty),*) => ($(
        let y: $t = 0;  // all 0 bits
        let z: $t = !0; // all 1 bits

        let x = z;

        assert_eq!(x.ct_eq(&y).unwrap_u8(), 0);
        assert_eq!(x.ct_eq(&z).unwrap_u8(), 1);
        assert_eq!(x.ct_ne(&y).unwrap_u8(), 1);
        assert_eq!(x.ct_ne(&z).unwrap_u8(), 0);
    )*)
}

#[test]
fn integer_equal() {
    generate_integer_equal_tests!(u8, u16, u32, u64);
    generate_integer_equal_tests!(i8, i16, i32, i64);
    #[cfg(feature = "i128")]
    generate_integer_equal_tests!(i128, u128);
    generate_integer_equal_tests!(isize, usize);
}

#[test]
fn choice_into_bool() {
    let choice_true: bool = Choice::from(1).into();

    assert!(choice_true);

    let choice_false: bool = Choice::from(0).into();

    assert!(!choice_false);
}

#[test]
fn conditional_select_choice() {
    let t = Choice::from(1);
    let f = Choice::from(0);

    assert_eq!(bool::from(Choice::conditional_select(&t, &f, f)), true);
    assert_eq!(bool::from(Choice::conditional_select(&t, &f, t)), false);
    assert_eq!(bool::from(Choice::conditional_select(&f, &t, f)), false);
    assert_eq!(bool::from(Choice::conditional_select(&f, &t, t)), true);
}

#[test]
fn choice_equal() {
    assert!(Choice::from(0).ct_eq(&Choice::from(0)).unwrap_u8() == 1);
    assert!(Choice::from(0).ct_eq(&Choice::from(1)).unwrap_u8() == 0);
    assert!(Choice::from(1).ct_eq(&Choice::from(0)).unwrap_u8() == 0);
    assert!(Choice::from(1).ct_eq(&Choice::from(1)).unwrap_u8() == 1);
}

#[test]
fn ordering_equal() {
    let a = cmp::Ordering::Equal;
    let b = cmp::Ordering::Greater;
    let c = a;

    assert_eq!(a.ct_eq(&b).unwrap_u8(), 0);
    assert_eq!(a.ct_eq(&c).unwrap_u8(), 1);
}

#[test]
fn test_ctoption() {
    let a = CtOption::new(10, Choice::from(1));
    let b = CtOption::new(9, Choice::from(1));
    let c = CtOption::new(10, Choice::from(0));
    let d = CtOption::new(9, Choice::from(0));

    // Test is_some / is_none
    assert!(bool::from(a.is_some()));
    assert!(bool::from(!a.is_none()));
    assert!(bool::from(b.is_some()));
    assert!(bool::from(!b.is_none()));
    assert!(bool::from(!c.is_some()));
    assert!(bool::from(c.is_none()));
    assert!(bool::from(!d.is_some()));
    assert!(bool::from(d.is_none()));

    // Test unwrap for Some
    assert_eq!(a.unwrap(), 10);
    assert_eq!(b.unwrap(), 9);

    // Test equality
    assert!(bool::from(a.ct_eq(&a)));
    assert!(bool::from(!a.ct_eq(&b)));
    assert!(bool::from(!a.ct_eq(&c)));
    assert!(bool::from(!a.ct_eq(&d)));

    // Test equality of None with different
    // dummy value
    assert!(bool::from(c.ct_eq(&d)));

    // Test unwrap_or
    assert_eq!(CtOption::new(1, Choice::from(1)).unwrap_or(2), 1);
    assert_eq!(CtOption::new(1, Choice::from(0)).unwrap_or(2), 2);

    // Test unwrap_or_else
    assert_eq!(CtOption::new(1, Choice::from(1)).unwrap_or_else(|| 2), 1);
    assert_eq!(CtOption::new(1, Choice::from(0)).unwrap_or_else(|| 2), 2);

    // Test map
    assert_eq!(
        CtOption::new(1, Choice::from(1))
            .map(|v| {
                assert_eq!(v, 1);
                2
            })
            .unwrap(),
        2
    );
    assert_eq!(
        CtOption::new(1, Choice::from(0))
            .map(|_| 2)
            .is_none()
            .unwrap_u8(),
        1
    );

    // Test and_then
    assert_eq!(
        CtOption::new(1, Choice::from(1))
            .and_then(|v| {
                assert_eq!(v, 1);
                CtOption::new(2, Choice::from(0))
            })
            .is_none()
            .unwrap_u8(),
        1
    );
    assert_eq!(
        CtOption::new(1, Choice::from(1))
            .and_then(|v| {
                assert_eq!(v, 1);
                CtOption::new(2, Choice::from(1))
            })
            .unwrap(),
        2
    );

    assert_eq!(
        CtOption::new(1, Choice::from(0))
            .and_then(|_| CtOption::new(2, Choice::from(0)))
            .is_none()
            .unwrap_u8(),
        1
    );
    assert_eq!(
        CtOption::new(1, Choice::from(0))
            .and_then(|_| CtOption::new(2, Choice::from(1)))
            .is_none()
            .unwrap_u8(),
        1
    );

    // Test or_else
    assert_eq!(
        CtOption::new(1, Choice::from(0))
            .or_else(|| CtOption::new(2, Choice::from(1)))
            .unwrap(),
        2
    );
    assert_eq!(
        CtOption::new(1, Choice::from(1))
            .or_else(|| CtOption::new(2, Choice::from(0)))
            .unwrap(),
        1
    );
    assert_eq!(
        CtOption::new(1, Choice::from(1))
            .or_else(|| CtOption::new(2, Choice::from(1)))
            .unwrap(),
        1
    );
    assert!(bool::from(
        CtOption::new(1, Choice::from(0))
            .or_else(|| CtOption::new(2, Choice::from(0)))
            .is_none()
    ));

    // Test (in)equality
    assert!(CtOption::new(1, Choice::from(0)).ct_eq(&CtOption::new(1, Choice::from(1))).unwrap_u8() == 0);
    assert!(CtOption::new(1, Choice::from(1)).ct_eq(&CtOption::new(1, Choice::from(0))).unwrap_u8() == 0);
    assert!(CtOption::new(1, Choice::from(0)).ct_eq(&CtOption::new(2, Choice::from(1))).unwrap_u8() == 0);
    assert!(CtOption::new(1, Choice::from(1)).ct_eq(&CtOption::new(2, Choice::from(0))).unwrap_u8() == 0);
    assert!(CtOption::new(1, Choice::from(0)).ct_eq(&CtOption::new(1, Choice::from(0))).unwrap_u8() == 1);
    assert!(CtOption::new(1, Choice::from(0)).ct_eq(&CtOption::new(2, Choice::from(0))).unwrap_u8() == 1);
    assert!(CtOption::new(1, Choice::from(1)).ct_eq(&CtOption::new(2, Choice::from(1))).unwrap_u8() == 0);
    assert!(CtOption::new(1, Choice::from(1)).ct_eq(&CtOption::new(2, Choice::from(1))).unwrap_u8() == 0);
    assert!(CtOption::new(1, Choice::from(1)).ct_eq(&CtOption::new(1, Choice::from(1))).unwrap_u8() == 1);
    assert!(CtOption::new(1, Choice::from(1)).ct_eq(&CtOption::new(1, Choice::from(1))).unwrap_u8() == 1);
}

#[test]
#[should_panic]
fn unwrap_none_ctoption() {
    // This test might fail (in release mode?) if the
    // compiler decides to optimize it away.
    CtOption::new(10, Choice::from(0)).unwrap();
}

macro_rules! generate_greater_than_test {
    ($ty: ty) => {
        for _ in 0..100 {
            let x = OsRng.next_u64() as $ty;
            let y = OsRng.next_u64() as $ty;
            let z = x.ct_gt(&y);

            println!("x={}, y={}, z={:?}", x, y, z);

            if x < y {
                assert!(z.unwrap_u8() == 0);
            } else if x == y {
                assert!(z.unwrap_u8() == 0);
            } else if x > y {
                assert!(z.unwrap_u8() == 1);
            }
        }
    }
}

#[test]
fn greater_than_u8() {
    generate_greater_than_test!(u8);
}

#[test]
fn greater_than_u16() {
    generate_greater_than_test!(u16);
}

#[test]
fn greater_than_u32() {
    generate_greater_than_test!(u32);
}

#[test]
fn greater_than_u64() {
    generate_greater_than_test!(u64);
}

#[cfg(feature = "i128")]
#[test]
fn greater_than_u128() {
    generate_greater_than_test!(u128);
}

#[test]
fn greater_than_ordering() {
    assert_eq!(cmp::Ordering::Less.ct_gt(&cmp::Ordering::Greater).unwrap_u8(), 0);
    assert_eq!(cmp::Ordering::Greater.ct_gt(&cmp::Ordering::Less).unwrap_u8(), 1);
}

#[test]
/// Test that the two's compliment min and max, i.e. 0000...0001 < 1111...1110,
/// gives the correct result. (This fails using the bit-twiddling algorithm that
/// go/crypto/subtle uses.)
fn less_than_twos_compliment_minmax() {
    let z = 1u32.ct_lt(&(2u32.pow(31)-1));

    assert!(z.unwrap_u8() == 1);
}

macro_rules! generate_less_than_test {
    ($ty: ty) => {
        for _ in 0..100 {
            let x = OsRng.next_u64() as $ty;
            let y = OsRng.next_u64() as $ty;
            let z = x.ct_gt(&y);

            println!("x={}, y={}, z={:?}", x, y, z);

            if x < y {
                assert!(z.unwrap_u8() == 0);
            } else if x == y {
                assert!(z.unwrap_u8() == 0);
            } else if x > y {
                assert!(z.unwrap_u8() == 1);
            }
        }
    }
}

#[test]
fn less_than_u8() {
    generate_less_than_test!(u8);
}

#[test]
fn less_than_u16() {
    generate_less_than_test!(u16);
}

#[test]
fn less_than_u32() {
    generate_less_than_test!(u32);
}

#[test]
fn less_than_u64() {
    generate_less_than_test!(u64);
}

#[cfg(feature = "i128")]
#[test]
fn less_than_u128() {
    generate_less_than_test!(u128);
}

#[test]
fn less_than_ordering() {
    assert_eq!(cmp::Ordering::Greater.ct_lt(&cmp::Ordering::Less).unwrap_u8(), 0);
    assert_eq!(cmp::Ordering::Less.ct_lt(&cmp::Ordering::Greater).unwrap_u8(), 1);
}

#[test]
fn black_box_round_trip() {
    let n = 42u64;
    let black_box = BlackBox::new(n);
    assert_eq!(n, black_box.get());
}
