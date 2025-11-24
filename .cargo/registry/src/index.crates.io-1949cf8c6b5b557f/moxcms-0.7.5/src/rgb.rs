/*
 * // Copyright 2024 (c) the Radzivon Bartoshyk. All rights reserved.
 * //
 * // Use of this source code is governed by a BSD-style
 * // license that can be found in the LICENSE file.
 */
use crate::math::{FusedMultiplyAdd, m_clamp, m_max, m_min};
use crate::mlaf::mlaf;
use crate::{Matrix3f, Vector3, Xyz};
use num_traits::{AsPrimitive, Bounded, Float, Num, Pow, Signed};
use pxfm::{
    f_exp, f_exp2, f_exp2f, f_exp10, f_exp10f, f_expf, f_log, f_log2, f_log2f, f_log10, f_log10f,
    f_logf, f_pow, f_powf,
};
use std::cmp::Ordering;
use std::ops::{Add, AddAssign, Div, DivAssign, Index, IndexMut, Mul, MulAssign, Neg, Sub};

#[repr(C)]
#[derive(Debug, PartialOrd, PartialEq, Clone, Copy, Default)]
/// Represents any RGB values
pub struct Rgb<T> {
    /// Red component
    pub r: T,
    /// Green component
    pub g: T,
    /// Blue component
    pub b: T,
}

impl<T> Rgb<T> {
    pub fn new(r: T, g: T, b: T) -> Rgb<T> {
        Rgb { r, g, b }
    }
}

impl<T> Rgb<T>
where
    T: Copy,
{
    pub fn dup(v: T) -> Rgb<T> {
        Rgb { r: v, g: v, b: v }
    }

    #[inline]
    pub const fn to_vector(self) -> Vector3<T> {
        Vector3 {
            v: [self.r, self.g, self.b],
        }
    }
}

impl Rgb<f32> {
    #[inline(always)]
    pub fn apply(&self, matrix: Matrix3f) -> Rgb<f32> {
        let new_r = mlaf(
            mlaf(self.r * matrix.v[0][0], self.g, matrix.v[0][1]),
            self.b,
            matrix.v[0][2],
        );

        let new_g = mlaf(
            mlaf(self.r * matrix.v[1][0], self.g, matrix.v[1][1]),
            self.b,
            matrix.v[1][2],
        );

        let new_b = mlaf(
            mlaf(self.r * matrix.v[2][0], self.g, matrix.v[2][1]),
            self.b,
            matrix.v[2][2],
        );

        Rgb {
            r: new_r,
            g: new_g,
            b: new_b,
        }
    }

    #[inline(always)]
    pub fn to_xyz(&self, matrix: Matrix3f) -> Xyz {
        let new_self = self.apply(matrix);
        Xyz {
            x: new_self.r,
            y: new_self.g,
            z: new_self.b,
        }
    }

    #[inline(always)]
    pub fn is_out_of_gamut(&self) -> bool {
        !(0.0..=1.0).contains(&self.r)
            || !(0.0..=1.0).contains(&self.g)
            || !(0.0..=1.0).contains(&self.b)
    }
}

impl<T> Index<usize> for Rgb<T> {
    type Output = T;

    fn index(&self, index: usize) -> &T {
        match index {
            0 => &self.r,
            1 => &self.g,
            2 => &self.b,
            _ => panic!("Index out of bounds for Rgb"),
        }
    }
}

impl<T> IndexMut<usize> for Rgb<T> {
    fn index_mut(&mut self, index: usize) -> &mut T {
        match index {
            0 => &mut self.r,
            1 => &mut self.g,
            2 => &mut self.b,
            _ => panic!("Index out of bounds for RGB"),
        }
    }
}

macro_rules! generated_float_definition_rgb {
    ($T: ty) => {
        impl Rgb<$T> {
            #[inline]
            pub fn zeroed() -> Rgb<$T> {
                Rgb::<$T>::new(0., 0., 0.)
            }

            #[inline]
            pub fn ones() -> Rgb<$T> {
                Rgb::<$T>::new(1., 1., 1.)
            }

            #[inline]
            pub fn white() -> Rgb<$T> {
                Rgb::<$T>::ones()
            }

            #[inline]
            pub fn black() -> Rgb<$T> {
                Rgb::<$T>::zeroed()
            }
        }
    };
}

generated_float_definition_rgb!(f32);
generated_float_definition_rgb!(f64);

macro_rules! generated_integral_definition_rgb {
    ($T: ty) => {
        impl Rgb<$T> {
            #[inline]
            pub fn zeroed() -> Rgb<$T> {
                Rgb::<$T>::new(0, 0, 0)
            }

            #[inline]
            pub fn capped() -> Rgb<$T> {
                Rgb::<$T>::new(<$T>::MAX, <$T>::MAX, <$T>::MAX)
            }

            #[inline]
            pub fn white() -> Rgb<$T> {
                Rgb::<$T>::capped()
            }

            #[inline]
            pub fn black() -> Rgb<$T> {
                Rgb::<$T>::new(0, 0, 0)
            }
        }
    };
}

generated_integral_definition_rgb!(u8);
generated_integral_definition_rgb!(u16);
generated_integral_definition_rgb!(i8);
generated_integral_definition_rgb!(i16);
generated_integral_definition_rgb!(i32);
generated_integral_definition_rgb!(u32);

pub trait FusedPow<T> {
    fn f_pow(&self, power: T) -> Self;
}

pub trait FusedLog2<T> {
    fn f_log2(&self) -> Self;
}

pub trait FusedLog10<T> {
    fn f_log10(&self) -> Self;
}

pub trait FusedLog<T> {
    fn f_log(&self) -> Self;
}

pub trait FusedExp<T> {
    fn f_exp(&self) -> Self;
}

pub trait FusedExp2<T> {
    fn f_exp2(&self) -> Self;
}

pub trait FusedExp10<T> {
    fn f_exp10(&self) -> Self;
}

impl FusedPow<Rgb<f32>> for Rgb<f32> {
    fn f_pow(&self, power: Rgb<f32>) -> Rgb<f32> {
        Rgb::new(
            f_powf(self.r, power.r),
            f_powf(self.g, power.g),
            f_powf(self.b, power.b),
        )
    }
}

impl FusedPow<Rgb<f64>> for Rgb<f64> {
    fn f_pow(&self, power: Rgb<f64>) -> Rgb<f64> {
        Rgb::new(
            f_pow(self.r, power.r),
            f_pow(self.g, power.g),
            f_pow(self.b, power.b),
        )
    }
}

impl FusedLog2<Rgb<f32>> for Rgb<f32> {
    #[inline]
    fn f_log2(&self) -> Rgb<f32> {
        Rgb::new(f_log2f(self.r), f_log2f(self.g), f_log2f(self.b))
    }
}

impl FusedLog2<Rgb<f64>> for Rgb<f64> {
    #[inline]
    fn f_log2(&self) -> Rgb<f64> {
        Rgb::new(f_log2(self.r), f_log2(self.g), f_log2(self.b))
    }
}

impl FusedLog<Rgb<f32>> for Rgb<f32> {
    #[inline]
    fn f_log(&self) -> Rgb<f32> {
        Rgb::new(f_logf(self.r), f_logf(self.g), f_logf(self.b))
    }
}

impl FusedLog<Rgb<f64>> for Rgb<f64> {
    #[inline]
    fn f_log(&self) -> Rgb<f64> {
        Rgb::new(f_log(self.r), f_log(self.g), f_log(self.b))
    }
}

impl FusedLog10<Rgb<f32>> for Rgb<f32> {
    #[inline]
    fn f_log10(&self) -> Rgb<f32> {
        Rgb::new(f_log10f(self.r), f_log10f(self.g), f_log10f(self.b))
    }
}

impl FusedLog10<Rgb<f64>> for Rgb<f64> {
    #[inline]
    fn f_log10(&self) -> Rgb<f64> {
        Rgb::new(f_log10(self.r), f_log10(self.g), f_log10(self.b))
    }
}

impl FusedExp<Rgb<f32>> for Rgb<f32> {
    #[inline]
    fn f_exp(&self) -> Rgb<f32> {
        Rgb::new(f_expf(self.r), f_expf(self.g), f_expf(self.b))
    }
}

impl FusedExp<Rgb<f64>> for Rgb<f64> {
    #[inline]
    fn f_exp(&self) -> Rgb<f64> {
        Rgb::new(f_exp(self.r), f_exp(self.g), f_exp(self.b))
    }
}

impl FusedExp2<Rgb<f32>> for Rgb<f32> {
    #[inline]
    fn f_exp2(&self) -> Rgb<f32> {
        Rgb::new(f_exp2f(self.r), f_exp2f(self.g), f_exp2f(self.b))
    }
}

impl FusedExp2<Rgb<f64>> for Rgb<f64> {
    #[inline]
    fn f_exp2(&self) -> Rgb<f64> {
        Rgb::new(f_exp2(self.r), f_exp2(self.g), f_exp2(self.b))
    }
}

impl FusedExp10<Rgb<f32>> for Rgb<f32> {
    #[inline]
    fn f_exp10(&self) -> Rgb<f32> {
        Rgb::new(f_exp10f(self.r), f_exp10f(self.g), f_exp10f(self.b))
    }
}

impl FusedExp10<Rgb<f64>> for Rgb<f64> {
    #[inline]
    fn f_exp10(&self) -> Rgb<f64> {
        Rgb::new(f_exp10(self.r), f_exp10(self.g), f_exp10(self.b))
    }
}

impl<T> Rgb<T>
where
    T: Copy + AsPrimitive<f32>,
{
    pub fn euclidean_distance(&self, other: Rgb<T>) -> f32 {
        let dr = self.r.as_() - other.r.as_();
        let dg = self.g.as_() - other.g.as_();
        let db = self.b.as_() - other.b.as_();
        (dr * dr + dg * dg + db * db).sqrt()
    }
}

impl<T> Rgb<T>
where
    T: Copy + AsPrimitive<f32>,
{
    pub fn taxicab_distance(&self, other: Self) -> f32 {
        let dr = self.r.as_() - other.r.as_();
        let dg = self.g.as_() - other.g.as_();
        let db = self.b.as_() - other.b.as_();
        dr.abs() + dg.abs() + db.abs()
    }
}

impl<T> Add for Rgb<T>
where
    T: Add<Output = T>,
{
    type Output = Rgb<T>;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        Rgb::new(self.r + rhs.r, self.g + rhs.g, self.b + rhs.b)
    }
}

impl<T> Sub for Rgb<T>
where
    T: Sub<Output = T>,
{
    type Output = Rgb<T>;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        Rgb::new(self.r - rhs.r, self.g - rhs.g, self.b - rhs.b)
    }
}

impl<T: Copy + Clone> Sub<T> for Rgb<T>
where
    T: Sub<Output = T>,
{
    type Output = Rgb<T>;

    #[inline]
    fn sub(self, rhs: T) -> Self::Output {
        Rgb::new(self.r - rhs, self.g - rhs, self.b - rhs)
    }
}

impl<T: Copy + Clone> Add<T> for Rgb<T>
where
    T: Add<Output = T>,
{
    type Output = Rgb<T>;

    #[inline]
    fn add(self, rhs: T) -> Self::Output {
        Rgb::new(self.r + rhs, self.g + rhs, self.b + rhs)
    }
}

impl<T: Copy + Clone> Rgb<T>
where
    T: Signed,
{
    #[inline]
    pub fn abs(self) -> Self {
        Rgb::new(self.r.abs(), self.g.abs(), self.b.abs())
    }
}

impl<T> Div for Rgb<T>
where
    T: Div<Output = T>,
{
    type Output = Rgb<T>;

    #[inline]
    fn div(self, rhs: Self) -> Self::Output {
        Rgb::new(self.r / rhs.r, self.g / rhs.g, self.b / rhs.b)
    }
}

impl<T: Clone + Copy> Div<T> for Rgb<T>
where
    T: Div<Output = T>,
{
    type Output = Rgb<T>;

    #[inline]
    fn div(self, rhs: T) -> Self::Output {
        Rgb::new(self.r / rhs, self.g / rhs, self.b / rhs)
    }
}

impl<T> Mul for Rgb<T>
where
    T: Mul<Output = T>,
{
    type Output = Rgb<T>;

    #[inline]
    fn mul(self, rhs: Self) -> Self::Output {
        Rgb::new(self.r * rhs.r, self.g * rhs.g, self.b * rhs.b)
    }
}

impl<T: Clone + Copy> Mul<T> for Rgb<T>
where
    T: Mul<Output = T>,
{
    type Output = Rgb<T>;

    #[inline]
    fn mul(self, rhs: T) -> Self::Output {
        Rgb::new(self.r * rhs, self.g * rhs, self.b * rhs)
    }
}

impl<T> MulAssign for Rgb<T>
where
    T: MulAssign<T>,
{
    #[inline]
    fn mul_assign(&mut self, rhs: Self) {
        self.r *= rhs.r;
        self.g *= rhs.g;
        self.b *= rhs.b;
    }
}

macro_rules! generated_mul_assign_definition_rgb {
    ($T: ty) => {
        impl<T> MulAssign<$T> for Rgb<T>
        where
            T: MulAssign<$T>,
        {
            #[inline]
            fn mul_assign(&mut self, rhs: $T) {
                self.r *= rhs;
                self.g *= rhs;
                self.b *= rhs;
            }
        }
    };
}

generated_mul_assign_definition_rgb!(i8);
generated_mul_assign_definition_rgb!(u8);
generated_mul_assign_definition_rgb!(u16);
generated_mul_assign_definition_rgb!(i16);
generated_mul_assign_definition_rgb!(u32);
generated_mul_assign_definition_rgb!(i32);
generated_mul_assign_definition_rgb!(f32);
generated_mul_assign_definition_rgb!(f64);

impl<T> AddAssign for Rgb<T>
where
    T: AddAssign<T>,
{
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.r += rhs.r;
        self.g += rhs.g;
        self.b += rhs.b;
    }
}

macro_rules! generated_add_assign_definition_rgb {
    ($T: ty) => {
        impl<T: Copy> AddAssign<$T> for Rgb<T>
        where
            T: AddAssign<$T>,
        {
            #[inline]
            fn add_assign(&mut self, rhs: $T) {
                self.r += rhs;
                self.g += rhs;
                self.b += rhs;
            }
        }
    };
}

generated_add_assign_definition_rgb!(i8);
generated_add_assign_definition_rgb!(u8);
generated_add_assign_definition_rgb!(u16);
generated_add_assign_definition_rgb!(i16);
generated_add_assign_definition_rgb!(u32);
generated_add_assign_definition_rgb!(i32);
generated_add_assign_definition_rgb!(f32);
generated_add_assign_definition_rgb!(f64);

impl<T> DivAssign for Rgb<T>
where
    T: DivAssign<T>,
{
    #[inline]
    fn div_assign(&mut self, rhs: Self) {
        self.r /= rhs.r;
        self.g /= rhs.g;
        self.b /= rhs.b;
    }
}

macro_rules! generated_div_assign_definition_rgb {
    ($T: ty) => {
        impl<T: Copy> DivAssign<$T> for Rgb<T>
        where
            T: DivAssign<$T>,
        {
            #[inline]
            fn div_assign(&mut self, rhs: $T) {
                self.r /= rhs;
                self.g /= rhs;
                self.b /= rhs;
            }
        }
    };
}

generated_div_assign_definition_rgb!(u8);
generated_div_assign_definition_rgb!(i8);
generated_div_assign_definition_rgb!(u16);
generated_div_assign_definition_rgb!(i16);
generated_div_assign_definition_rgb!(u32);
generated_div_assign_definition_rgb!(i32);
generated_div_assign_definition_rgb!(f32);
generated_div_assign_definition_rgb!(f64);

impl<T> Neg for Rgb<T>
where
    T: Neg<Output = T>,
{
    type Output = Rgb<T>;

    #[inline]
    fn neg(self) -> Self::Output {
        Rgb::new(-self.r, -self.g, -self.b)
    }
}

impl<T> Rgb<T>
where
    T: FusedMultiplyAdd<T>,
{
    pub fn mla(&self, b: Rgb<T>, c: Rgb<T>) -> Rgb<T> {
        Rgb::new(
            self.r.mla(b.r, c.r),
            self.g.mla(b.g, c.g),
            self.b.mla(b.b, c.b),
        )
    }
}

impl<T> Rgb<T>
where
    T: Num + PartialOrd + Copy + Bounded,
{
    /// Clamp function to clamp each channel within a given range
    #[inline]
    #[allow(clippy::manual_clamp)]
    pub fn clamp(&self, min_value: T, max_value: T) -> Rgb<T> {
        Rgb::new(
            m_clamp(self.r, min_value, max_value),
            m_clamp(self.g, min_value, max_value),
            m_clamp(self.b, min_value, max_value),
        )
    }

    /// Min function to define min
    #[inline]
    pub fn min(&self, other_min: T) -> Rgb<T> {
        Rgb::new(
            m_min(self.r, other_min),
            m_min(self.g, other_min),
            m_min(self.b, other_min),
        )
    }

    /// Max function to define max
    #[inline]
    pub fn max(&self, other_max: T) -> Rgb<T> {
        Rgb::new(
            m_max(self.r, other_max),
            m_max(self.g, other_max),
            m_max(self.b, other_max),
        )
    }

    /// Clamp function to clamp each channel within a given range
    #[inline]
    #[allow(clippy::manual_clamp)]
    pub fn clamp_p(&self, min_value: Rgb<T>, max_value: Rgb<T>) -> Rgb<T> {
        Rgb::new(
            m_clamp(self.r, max_value.r, min_value.r),
            m_clamp(self.g, max_value.g, min_value.g),
            m_clamp(self.b, max_value.b, min_value.b),
        )
    }

    /// Min function to define min
    #[inline]
    pub fn min_p(&self, other_min: Rgb<T>) -> Rgb<T> {
        Rgb::new(
            m_min(self.r, other_min.r),
            m_min(self.g, other_min.g),
            m_min(self.b, other_min.b),
        )
    }

    /// Max function to define max
    #[inline]
    pub fn max_p(&self, other_max: Rgb<T>) -> Rgb<T> {
        Rgb::new(
            m_max(self.r, other_max.r),
            m_max(self.g, other_max.g),
            m_max(self.b, other_max.b),
        )
    }
}

impl<T> Rgb<T>
where
    T: Float + 'static,
    f32: AsPrimitive<T>,
{
    #[inline]
    pub fn sqrt(&self) -> Rgb<T> {
        let zeros = 0f32.as_();
        Rgb::new(
            if self.r.partial_cmp(&zeros).unwrap_or(Ordering::Less) == Ordering::Less {
                0f32.as_()
            } else {
                self.r.sqrt()
            },
            if self.g.partial_cmp(&zeros).unwrap_or(Ordering::Less) == Ordering::Less {
                0f32.as_()
            } else {
                self.g.sqrt()
            },
            if self.b.partial_cmp(&zeros).unwrap_or(Ordering::Less) == Ordering::Less {
                0f32.as_()
            } else {
                self.b.sqrt()
            },
        )
    }

    #[inline]
    pub fn cbrt(&self) -> Rgb<T> {
        Rgb::new(self.r.cbrt(), self.g.cbrt(), self.b.cbrt())
    }
}

impl<T> Pow<T> for Rgb<T>
where
    T: Float,
{
    type Output = Rgb<T>;

    #[inline]
    fn pow(self, rhs: T) -> Self::Output {
        Rgb::<T>::new(self.r.powf(rhs), self.g.powf(rhs), self.b.powf(rhs))
    }
}

impl<T> Pow<Rgb<T>> for Rgb<T>
where
    T: Float,
{
    type Output = Rgb<T>;

    #[inline]
    fn pow(self, rhs: Rgb<T>) -> Self::Output {
        Rgb::<T>::new(self.r.powf(rhs.r), self.g.powf(rhs.g), self.b.powf(rhs.b))
    }
}

impl<T> Rgb<T> {
    pub fn cast<V>(self) -> Rgb<V>
    where
        T: AsPrimitive<V>,
        V: Copy + 'static,
    {
        Rgb::new(self.r.as_(), self.g.as_(), self.b.as_())
    }
}

impl<T> Rgb<T>
where
    T: Float + 'static,
{
    pub fn round(self) -> Rgb<T> {
        Rgb::new(self.r.round(), self.g.round(), self.b.round())
    }
}
