use crate::Ratio;

use core::cmp;
use num_integer::Integer;
use num_traits::{One, Pow};

macro_rules! pow_unsigned_impl {
    (@ $exp:ty) => {
        type Output = Ratio<T>;
        #[inline]
        fn pow(self, expon: $exp) -> Ratio<T> {
            Ratio::new_raw(self.numer.pow(expon), self.denom.pow(expon))
        }
    };
    ($exp:ty) => {
        impl<T: Clone + Integer + Pow<$exp, Output = T>> Pow<$exp> for Ratio<T> {
            pow_unsigned_impl!(@ $exp);
        }
        impl<'a, T: Clone + Integer> Pow<$exp> for &'a Ratio<T>
        where
            &'a T: Pow<$exp, Output = T>,
        {
            pow_unsigned_impl!(@ $exp);
        }
        impl<'b, T: Clone + Integer + Pow<$exp, Output = T>> Pow<&'b $exp> for Ratio<T> {
            type Output = Ratio<T>;
            #[inline]
            fn pow(self, expon: &'b $exp) -> Ratio<T> {
                Pow::pow(self, *expon)
            }
        }
        impl<'a, 'b, T: Clone + Integer> Pow<&'b $exp> for &'a Ratio<T>
        where
            &'a T: Pow<$exp, Output = T>,
        {
            type Output = Ratio<T>;
            #[inline]
            fn pow(self, expon: &'b $exp) -> Ratio<T> {
                Pow::pow(self, *expon)
            }
        }
    };
}
pow_unsigned_impl!(u8);
pow_unsigned_impl!(u16);
pow_unsigned_impl!(u32);
pow_unsigned_impl!(u64);
pow_unsigned_impl!(u128);
pow_unsigned_impl!(usize);

macro_rules! pow_signed_impl {
    (@ &'b BigInt, BigUint) => {
        type Output = Ratio<T>;
        #[inline]
        fn pow(self, expon: &'b BigInt) -> Ratio<T> {
            match expon.sign() {
                Sign::NoSign => One::one(),
                Sign::Minus => {
                    Pow::pow(self, expon.magnitude()).into_recip()
                }
                Sign::Plus => Pow::pow(self, expon.magnitude()),
            }
        }
    };
    (@ $exp:ty, $unsigned:ty) => {
        type Output = Ratio<T>;
        #[inline]
        fn pow(self, expon: $exp) -> Ratio<T> {
            match expon.cmp(&0) {
                cmp::Ordering::Equal => One::one(),
                cmp::Ordering::Less => {
                    let expon = expon.wrapping_abs() as $unsigned;
                    Pow::pow(self, expon).into_recip()
                }
                cmp::Ordering::Greater => Pow::pow(self, expon as $unsigned),
            }
        }
    };
    ($exp:ty, $unsigned:ty) => {
        impl<T: Clone + Integer + Pow<$unsigned, Output = T>> Pow<$exp> for Ratio<T> {
            pow_signed_impl!(@ $exp, $unsigned);
        }
        impl<'a, T: Clone + Integer> Pow<$exp> for &'a Ratio<T>
        where
            &'a T: Pow<$unsigned, Output = T>,
        {
            pow_signed_impl!(@ $exp, $unsigned);
        }
        impl<'b, T: Clone + Integer + Pow<$unsigned, Output = T>> Pow<&'b $exp> for Ratio<T> {
            type Output = Ratio<T>;
            #[inline]
            fn pow(self, expon: &'b $exp) -> Ratio<T> {
                Pow::pow(self, *expon)
            }
        }
        impl<'a, 'b, T: Clone + Integer> Pow<&'b $exp> for &'a Ratio<T>
        where
            &'a T: Pow<$unsigned, Output = T>,
        {
            type Output = Ratio<T>;
            #[inline]
            fn pow(self, expon: &'b $exp) -> Ratio<T> {
                Pow::pow(self, *expon)
            }
        }
    };
}
pow_signed_impl!(i8, u8);
pow_signed_impl!(i16, u16);
pow_signed_impl!(i32, u32);
pow_signed_impl!(i64, u64);
pow_signed_impl!(i128, u128);
pow_signed_impl!(isize, usize);

#[cfg(feature = "num-bigint")]
mod bigint {
    use super::*;
    use num_bigint::{BigInt, BigUint, Sign};

    impl<T: Clone + Integer + for<'b> Pow<&'b BigUint, Output = T>> Pow<BigUint> for Ratio<T> {
        type Output = Ratio<T>;
        #[inline]
        fn pow(self, expon: BigUint) -> Ratio<T> {
            Pow::pow(self, &expon)
        }
    }
    impl<'a, T: Clone + Integer> Pow<BigUint> for &'a Ratio<T>
    where
        &'a T: for<'b> Pow<&'b BigUint, Output = T>,
    {
        type Output = Ratio<T>;
        #[inline]
        fn pow(self, expon: BigUint) -> Ratio<T> {
            Pow::pow(self, &expon)
        }
    }
    impl<'b, T: Clone + Integer + Pow<&'b BigUint, Output = T>> Pow<&'b BigUint> for Ratio<T> {
        pow_unsigned_impl!(@ &'b BigUint);
    }
    impl<'a, 'b, T: Clone + Integer> Pow<&'b BigUint> for &'a Ratio<T>
    where
        &'a T: Pow<&'b BigUint, Output = T>,
    {
        pow_unsigned_impl!(@ &'b BigUint);
    }

    impl<T: Clone + Integer + for<'b> Pow<&'b BigUint, Output = T>> Pow<BigInt> for Ratio<T> {
        type Output = Ratio<T>;
        #[inline]
        fn pow(self, expon: BigInt) -> Ratio<T> {
            Pow::pow(self, &expon)
        }
    }
    impl<'a, T: Clone + Integer> Pow<BigInt> for &'a Ratio<T>
    where
        &'a T: for<'b> Pow<&'b BigUint, Output = T>,
    {
        type Output = Ratio<T>;
        #[inline]
        fn pow(self, expon: BigInt) -> Ratio<T> {
            Pow::pow(self, &expon)
        }
    }
    impl<'b, T: Clone + Integer + Pow<&'b BigUint, Output = T>> Pow<&'b BigInt> for Ratio<T> {
        pow_signed_impl!(@ &'b BigInt, BigUint);
    }
    impl<'a, 'b, T: Clone + Integer> Pow<&'b BigInt> for &'a Ratio<T>
    where
        &'a T: Pow<&'b BigUint, Output = T>,
    {
        pow_signed_impl!(@ &'b BigInt, BigUint);
    }
}
