mod biguint {
    use num_bigint::BigUint;
    use num_traits::{One, Zero};

    fn check<T: Into<BigUint>>(x: T, n: u32) {
        let x: BigUint = x.into();
        let root = x.nth_root(n);
        println!("check {}.nth_root({}) = {}", x, n, root);

        if n == 2 {
            assert_eq!(root, x.sqrt())
        } else if n == 3 {
            assert_eq!(root, x.cbrt())
        }

        let lo = root.pow(n);
        assert!(lo <= x);
        assert_eq!(lo.nth_root(n), root);
        if !lo.is_zero() {
            assert_eq!((&lo - 1u32).nth_root(n), &root - 1u32);
        }

        let hi = (&root + 1u32).pow(n);
        assert!(hi > x);
        assert_eq!(hi.nth_root(n), &root + 1u32);
        assert_eq!((&hi - 1u32).nth_root(n), root);
    }

    #[test]
    fn test_sqrt() {
        check(99u32, 2);
        check(100u32, 2);
        check(120u32, 2);
    }

    #[test]
    fn test_cbrt() {
        check(8u32, 3);
        check(26u32, 3);
    }

    #[test]
    fn test_nth_root() {
        check(0u32, 1);
        check(10u32, 1);
        check(100u32, 4);
    }

    #[test]
    #[should_panic]
    fn test_nth_root_n_is_zero() {
        check(4u32, 0);
    }

    #[test]
    fn test_nth_root_big() {
        let x = BigUint::from(123_456_789_u32);
        let expected = BigUint::from(6u32);

        assert_eq!(x.nth_root(10), expected);
        check(x, 10);
    }

    #[test]
    fn test_nth_root_googol() {
        let googol = BigUint::from(10u32).pow(100u32);

        // perfect divisors of 100
        for &n in &[2, 4, 5, 10, 20, 25, 50, 100] {
            let expected = BigUint::from(10u32).pow(100u32 / n);
            assert_eq!(googol.nth_root(n), expected);
            check(googol.clone(), n);
        }
    }

    #[test]
    fn test_nth_root_twos() {
        const EXP: u32 = 12;
        const LOG2: usize = 1 << EXP;
        let x = BigUint::one() << LOG2;

        // the perfect divisors are just powers of two
        for exp in 1..=EXP {
            let n = 2u32.pow(exp);
            let expected = BigUint::one() << (LOG2 / n as usize);
            assert_eq!(x.nth_root(n), expected);
            check(x.clone(), n);
        }

        // degenerate cases should return quickly
        assert!(x.nth_root(x.bits() as u32).is_one());
        assert!(x.nth_root(i32::MAX as u32).is_one());
        assert!(x.nth_root(u32::MAX).is_one());
    }

    #[test]
    fn test_roots_rand1() {
        // A random input that found regressions
        let s = "575981506858479247661989091587544744717244516135539456183849\
                 986593934723426343633698413178771587697273822147578889823552\
                 182702908597782734558103025298880194023243541613924361007059\
                 353344183590348785832467726433749431093350684849462759540710\
                 026019022227591412417064179299354183441181373862905039254106\
                 4781867";
        let x: BigUint = s.parse().unwrap();

        check(x.clone(), 2);
        check(x.clone(), 3);
        check(x.clone(), 10);
        check(x, 100);
    }
}

mod bigint {
    use num_bigint::BigInt;
    use num_traits::Signed;

    fn check(x: i64, n: u32) {
        let big_x = BigInt::from(x);
        let res = big_x.nth_root(n);

        if n == 2 {
            assert_eq!(&res, &big_x.sqrt())
        } else if n == 3 {
            assert_eq!(&res, &big_x.cbrt())
        }

        if big_x.is_negative() {
            assert!(res.pow(n) >= big_x);
            assert!((res - 1u32).pow(n) < big_x);
        } else {
            assert!(res.pow(n) <= big_x);
            assert!((res + 1u32).pow(n) > big_x);
        }
    }

    #[test]
    fn test_nth_root() {
        check(-100, 3);
    }

    #[test]
    #[should_panic]
    fn test_nth_root_x_neg_n_even() {
        check(-100, 4);
    }

    #[test]
    #[should_panic]
    fn test_sqrt_x_neg() {
        check(-4, 2);
    }

    #[test]
    fn test_cbrt() {
        check(8, 3);
        check(-8, 3);
    }
}
