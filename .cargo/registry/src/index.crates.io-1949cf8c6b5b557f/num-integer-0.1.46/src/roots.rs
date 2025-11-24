use crate::Integer;
use core::mem;
use num_traits::{checked_pow, PrimInt};

/// Provides methods to compute an integer's square root, cube root,
/// and arbitrary `n`th root.
pub trait Roots: Integer {
    /// Returns the truncated principal `n`th root of an integer
    /// -- `if x >= 0 { ⌊ⁿ√x⌋ } else { ⌈ⁿ√x⌉ }`
    ///
    /// This is solving for `r` in `rⁿ = x`, rounding toward zero.
    /// If `x` is positive, the result will satisfy `rⁿ ≤ x < (r+1)ⁿ`.
    /// If `x` is negative and `n` is odd, then `(r-1)ⁿ < x ≤ rⁿ`.
    ///
    /// # Panics
    ///
    /// Panics if `n` is zero:
    ///
    /// ```should_panic
    /// # use num_integer::Roots;
    /// println!("can't compute ⁰√x : {}", 123.nth_root(0));
    /// ```
    ///
    /// or if `n` is even and `self` is negative:
    ///
    /// ```should_panic
    /// # use num_integer::Roots;
    /// println!("no imaginary numbers... {}", (-1).nth_root(10));
    /// ```
    ///
    /// # Examples
    ///
    /// ```
    /// use num_integer::Roots;
    ///
    /// let x: i32 = 12345;
    /// assert_eq!(x.nth_root(1), x);
    /// assert_eq!(x.nth_root(2), x.sqrt());
    /// assert_eq!(x.nth_root(3), x.cbrt());
    /// assert_eq!(x.nth_root(4), 10);
    /// assert_eq!(x.nth_root(13), 2);
    /// assert_eq!(x.nth_root(14), 1);
    /// assert_eq!(x.nth_root(std::u32::MAX), 1);
    ///
    /// assert_eq!(std::i32::MAX.nth_root(30), 2);
    /// assert_eq!(std::i32::MAX.nth_root(31), 1);
    /// assert_eq!(std::i32::MIN.nth_root(31), -2);
    /// assert_eq!((std::i32::MIN + 1).nth_root(31), -1);
    ///
    /// assert_eq!(std::u32::MAX.nth_root(31), 2);
    /// assert_eq!(std::u32::MAX.nth_root(32), 1);
    /// ```
    fn nth_root(&self, n: u32) -> Self;

    /// Returns the truncated principal square root of an integer -- `⌊√x⌋`
    ///
    /// This is solving for `r` in `r² = x`, rounding toward zero.
    /// The result will satisfy `r² ≤ x < (r+1)²`.
    ///
    /// # Panics
    ///
    /// Panics if `self` is less than zero:
    ///
    /// ```should_panic
    /// # use num_integer::Roots;
    /// println!("no imaginary numbers... {}", (-1).sqrt());
    /// ```
    ///
    /// # Examples
    ///
    /// ```
    /// use num_integer::Roots;
    ///
    /// let x: i32 = 12345;
    /// assert_eq!((x * x).sqrt(), x);
    /// assert_eq!((x * x + 1).sqrt(), x);
    /// assert_eq!((x * x - 1).sqrt(), x - 1);
    /// ```
    #[inline]
    fn sqrt(&self) -> Self {
        self.nth_root(2)
    }

    /// Returns the truncated principal cube root of an integer --
    /// `if x >= 0 { ⌊∛x⌋ } else { ⌈∛x⌉ }`
    ///
    /// This is solving for `r` in `r³ = x`, rounding toward zero.
    /// If `x` is positive, the result will satisfy `r³ ≤ x < (r+1)³`.
    /// If `x` is negative, then `(r-1)³ < x ≤ r³`.
    ///
    /// # Examples
    ///
    /// ```
    /// use num_integer::Roots;
    ///
    /// let x: i32 = 1234;
    /// assert_eq!((x * x * x).cbrt(), x);
    /// assert_eq!((x * x * x + 1).cbrt(), x);
    /// assert_eq!((x * x * x - 1).cbrt(), x - 1);
    ///
    /// assert_eq!((-(x * x * x)).cbrt(), -x);
    /// assert_eq!((-(x * x * x + 1)).cbrt(), -x);
    /// assert_eq!((-(x * x * x - 1)).cbrt(), -(x - 1));
    /// ```
    #[inline]
    fn cbrt(&self) -> Self {
        self.nth_root(3)
    }
}

/// Returns the truncated principal square root of an integer --
/// see [Roots::sqrt](trait.Roots.html#method.sqrt).
#[inline]
pub fn sqrt<T: Roots>(x: T) -> T {
    x.sqrt()
}

/// Returns the truncated principal cube root of an integer --
/// see [Roots::cbrt](trait.Roots.html#method.cbrt).
#[inline]
pub fn cbrt<T: Roots>(x: T) -> T {
    x.cbrt()
}

/// Returns the truncated principal `n`th root of an integer --
/// see [Roots::nth_root](trait.Roots.html#tymethod.nth_root).
#[inline]
pub fn nth_root<T: Roots>(x: T, n: u32) -> T {
    x.nth_root(n)
}

macro_rules! signed_roots {
    ($T:ty, $U:ty) => {
        impl Roots for $T {
            #[inline]
            fn nth_root(&self, n: u32) -> Self {
                if *self >= 0 {
                    (*self as $U).nth_root(n) as Self
                } else {
                    assert!(n.is_odd(), "even roots of a negative are imaginary");
                    -((self.wrapping_neg() as $U).nth_root(n) as Self)
                }
            }

            #[inline]
            fn sqrt(&self) -> Self {
                assert!(*self >= 0, "the square root of a negative is imaginary");
                (*self as $U).sqrt() as Self
            }

            #[inline]
            fn cbrt(&self) -> Self {
                if *self >= 0 {
                    (*self as $U).cbrt() as Self
                } else {
                    -((self.wrapping_neg() as $U).cbrt() as Self)
                }
            }
        }
    };
}

signed_roots!(i8, u8);
signed_roots!(i16, u16);
signed_roots!(i32, u32);
signed_roots!(i64, u64);
signed_roots!(i128, u128);
signed_roots!(isize, usize);

#[inline]
fn fixpoint<T, F>(mut x: T, f: F) -> T
where
    T: Integer + Copy,
    F: Fn(T) -> T,
{
    let mut xn = f(x);
    while x < xn {
        x = xn;
        xn = f(x);
    }
    while x > xn {
        x = xn;
        xn = f(x);
    }
    x
}

#[inline]
fn bits<T>() -> u32 {
    8 * mem::size_of::<T>() as u32
}

#[inline]
fn log2<T: PrimInt>(x: T) -> u32 {
    debug_assert!(x > T::zero());
    bits::<T>() - 1 - x.leading_zeros()
}

macro_rules! unsigned_roots {
    ($T:ident) => {
        impl Roots for $T {
            #[inline]
            fn nth_root(&self, n: u32) -> Self {
                fn go(a: $T, n: u32) -> $T {
                    // Specialize small roots
                    match n {
                        0 => panic!("can't find a root of degree 0!"),
                        1 => return a,
                        2 => return a.sqrt(),
                        3 => return a.cbrt(),
                        _ => (),
                    }

                    // The root of values less than 2ⁿ can only be 0 or 1.
                    if bits::<$T>() <= n || a < (1 << n) {
                        return (a > 0) as $T;
                    }

                    if bits::<$T>() > 64 {
                        // 128-bit division is slow, so do a bitwise `nth_root` until it's small enough.
                        return if a <= core::u64::MAX as $T {
                            (a as u64).nth_root(n) as $T
                        } else {
                            let lo = (a >> n).nth_root(n) << 1;
                            let hi = lo + 1;
                            // 128-bit `checked_mul` also involves division, but we can't always
                            // compute `hiⁿ` without risking overflow.  Try to avoid it though...
                            if hi.next_power_of_two().trailing_zeros() * n >= bits::<$T>() {
                                match checked_pow(hi, n as usize) {
                                    Some(x) if x <= a => hi,
                                    _ => lo,
                                }
                            } else {
                                if hi.pow(n) <= a {
                                    hi
                                } else {
                                    lo
                                }
                            }
                        };
                    }

                    #[cfg(feature = "std")]
                    #[inline]
                    fn guess(x: $T, n: u32) -> $T {
                        // for smaller inputs, `f64` doesn't justify its cost.
                        if bits::<$T>() <= 32 || x <= core::u32::MAX as $T {
                            1 << ((log2(x) + n - 1) / n)
                        } else {
                            ((x as f64).ln() / f64::from(n)).exp() as $T
                        }
                    }

                    #[cfg(not(feature = "std"))]
                    #[inline]
                    fn guess(x: $T, n: u32) -> $T {
                        1 << ((log2(x) + n - 1) / n)
                    }

                    // https://en.wikipedia.org/wiki/Nth_root_algorithm
                    let n1 = n - 1;
                    let next = |x: $T| {
                        let y = match checked_pow(x, n1 as usize) {
                            Some(ax) => a / ax,
                            None => 0,
                        };
                        (y + x * n1 as $T) / n as $T
                    };
                    fixpoint(guess(a, n), next)
                }
                go(*self, n)
            }

            #[inline]
            fn sqrt(&self) -> Self {
                fn go(a: $T) -> $T {
                    if bits::<$T>() > 64 {
                        // 128-bit division is slow, so do a bitwise `sqrt` until it's small enough.
                        return if a <= core::u64::MAX as $T {
                            (a as u64).sqrt() as $T
                        } else {
                            let lo = (a >> 2u32).sqrt() << 1;
                            let hi = lo + 1;
                            if hi * hi <= a {
                                hi
                            } else {
                                lo
                            }
                        };
                    }

                    if a < 4 {
                        return (a > 0) as $T;
                    }

                    #[cfg(feature = "std")]
                    #[inline]
                    fn guess(x: $T) -> $T {
                        (x as f64).sqrt() as $T
                    }

                    #[cfg(not(feature = "std"))]
                    #[inline]
                    fn guess(x: $T) -> $T {
                        1 << ((log2(x) + 1) / 2)
                    }

                    // https://en.wikipedia.org/wiki/Methods_of_computing_square_roots#Babylonian_method
                    let next = |x: $T| (a / x + x) >> 1;
                    fixpoint(guess(a), next)
                }
                go(*self)
            }

            #[inline]
            fn cbrt(&self) -> Self {
                fn go(a: $T) -> $T {
                    if bits::<$T>() > 64 {
                        // 128-bit division is slow, so do a bitwise `cbrt` until it's small enough.
                        return if a <= core::u64::MAX as $T {
                            (a as u64).cbrt() as $T
                        } else {
                            let lo = (a >> 3u32).cbrt() << 1;
                            let hi = lo + 1;
                            if hi * hi * hi <= a {
                                hi
                            } else {
                                lo
                            }
                        };
                    }

                    if bits::<$T>() <= 32 {
                        // Implementation based on Hacker's Delight `icbrt2`
                        let mut x = a;
                        let mut y2 = 0;
                        let mut y = 0;
                        let smax = bits::<$T>() / 3;
                        for s in (0..smax + 1).rev() {
                            let s = s * 3;
                            y2 *= 4;
                            y *= 2;
                            let b = 3 * (y2 + y) + 1;
                            if x >> s >= b {
                                x -= b << s;
                                y2 += 2 * y + 1;
                                y += 1;
                            }
                        }
                        return y;
                    }

                    if a < 8 {
                        return (a > 0) as $T;
                    }
                    if a <= core::u32::MAX as $T {
                        return (a as u32).cbrt() as $T;
                    }

                    #[cfg(feature = "std")]
                    #[inline]
                    fn guess(x: $T) -> $T {
                        (x as f64).cbrt() as $T
                    }

                    #[cfg(not(feature = "std"))]
                    #[inline]
                    fn guess(x: $T) -> $T {
                        1 << ((log2(x) + 2) / 3)
                    }

                    // https://en.wikipedia.org/wiki/Cube_root#Numerical_methods
                    let next = |x: $T| (a / (x * x) + x * 2) / 3;
                    fixpoint(guess(a), next)
                }
                go(*self)
            }
        }
    };
}

unsigned_roots!(u8);
unsigned_roots!(u16);
unsigned_roots!(u32);
unsigned_roots!(u64);
unsigned_roots!(u128);
unsigned_roots!(usize);
