use super::Complex;

use core::ops::Neg;
#[cfg(any(feature = "std", feature = "libm"))]
use num_traits::Float;
use num_traits::{Num, One, Pow};

macro_rules! pow_impl {
    ($U:ty, $S:ty) => {
        impl<'a, T: Clone + Num> Pow<$U> for &'a Complex<T> {
            type Output = Complex<T>;

            #[inline]
            fn pow(self, mut exp: $U) -> Self::Output {
                if exp == 0 {
                    return Complex::one();
                }
                let mut base = self.clone();

                while exp & 1 == 0 {
                    base = base.clone() * base;
                    exp >>= 1;
                }

                if exp == 1 {
                    return base;
                }

                let mut acc = base.clone();
                while exp > 1 {
                    exp >>= 1;
                    base = base.clone() * base;
                    if exp & 1 == 1 {
                        acc = acc * base.clone();
                    }
                }
                acc
            }
        }

        impl<'a, 'b, T: Clone + Num> Pow<&'b $U> for &'a Complex<T> {
            type Output = Complex<T>;

            #[inline]
            fn pow(self, exp: &$U) -> Self::Output {
                self.pow(*exp)
            }
        }

        impl<'a, T: Clone + Num + Neg<Output = T>> Pow<$S> for &'a Complex<T> {
            type Output = Complex<T>;

            #[inline]
            fn pow(self, exp: $S) -> Self::Output {
                if exp < 0 {
                    Pow::pow(&self.inv(), exp.wrapping_neg() as $U)
                } else {
                    Pow::pow(self, exp as $U)
                }
            }
        }

        impl<'a, 'b, T: Clone + Num + Neg<Output = T>> Pow<&'b $S> for &'a Complex<T> {
            type Output = Complex<T>;

            #[inline]
            fn pow(self, exp: &$S) -> Self::Output {
                self.pow(*exp)
            }
        }
    };
}

pow_impl!(u8, i8);
pow_impl!(u16, i16);
pow_impl!(u32, i32);
pow_impl!(u64, i64);
pow_impl!(usize, isize);
pow_impl!(u128, i128);

// Note: we can't add `impl<T: Float> Pow<T> for Complex<T>` because new blanket impls are a
// breaking change.  Someone could already have their own `F` and `impl Pow<F> for Complex<F>`
// which would conflict.  We can't even do this in a new semantic version, because we have to
// gate it on the "std" feature, and features can't add breaking changes either.

macro_rules! powf_impl {
    ($F:ty) => {
        #[cfg(any(feature = "std", feature = "libm"))]
        impl<'a, T: Float> Pow<$F> for &'a Complex<T>
        where
            $F: Into<T>,
        {
            type Output = Complex<T>;

            #[inline]
            fn pow(self, exp: $F) -> Self::Output {
                self.powf(exp.into())
            }
        }

        #[cfg(any(feature = "std", feature = "libm"))]
        impl<'a, 'b, T: Float> Pow<&'b $F> for &'a Complex<T>
        where
            $F: Into<T>,
        {
            type Output = Complex<T>;

            #[inline]
            fn pow(self, &exp: &$F) -> Self::Output {
                self.powf(exp.into())
            }
        }

        #[cfg(any(feature = "std", feature = "libm"))]
        impl<T: Float> Pow<$F> for Complex<T>
        where
            $F: Into<T>,
        {
            type Output = Complex<T>;

            #[inline]
            fn pow(self, exp: $F) -> Self::Output {
                self.powf(exp.into())
            }
        }

        #[cfg(any(feature = "std", feature = "libm"))]
        impl<'b, T: Float> Pow<&'b $F> for Complex<T>
        where
            $F: Into<T>,
        {
            type Output = Complex<T>;

            #[inline]
            fn pow(self, &exp: &$F) -> Self::Output {
                self.powf(exp.into())
            }
        }
    };
}

powf_impl!(f32);
powf_impl!(f64);

// These blanket impls are OK, because both the target type and the trait parameter would be
// foreign to anyone else trying to implement something that would overlap, raising E0117.

#[cfg(any(feature = "std", feature = "libm"))]
impl<'a, T: Float> Pow<Complex<T>> for &'a Complex<T> {
    type Output = Complex<T>;

    #[inline]
    fn pow(self, exp: Complex<T>) -> Self::Output {
        self.powc(exp)
    }
}

#[cfg(any(feature = "std", feature = "libm"))]
impl<'a, 'b, T: Float> Pow<&'b Complex<T>> for &'a Complex<T> {
    type Output = Complex<T>;

    #[inline]
    fn pow(self, &exp: &'b Complex<T>) -> Self::Output {
        self.powc(exp)
    }
}

#[cfg(any(feature = "std", feature = "libm"))]
impl<T: Float> Pow<Complex<T>> for Complex<T> {
    type Output = Complex<T>;

    #[inline]
    fn pow(self, exp: Complex<T>) -> Self::Output {
        self.powc(exp)
    }
}

#[cfg(any(feature = "std", feature = "libm"))]
impl<'b, T: Float> Pow<&'b Complex<T>> for Complex<T> {
    type Output = Complex<T>;

    #[inline]
    fn pow(self, &exp: &'b Complex<T>) -> Self::Output {
        self.powc(exp)
    }
}
