// crate minimums: sse2, x86_64

use crate::types::*;
use core::arch::x86_64::{__m128i, __m256i};

mod sse2;

#[derive(Copy, Clone)]
pub struct YesS3;
#[derive(Copy, Clone)]
pub struct NoS3;

#[derive(Copy, Clone)]
pub struct YesS4;
#[derive(Copy, Clone)]
pub struct NoS4;

#[derive(Copy, Clone)]
pub struct YesA1;
#[derive(Copy, Clone)]
pub struct NoA1;

#[derive(Copy, Clone)]
pub struct YesA2;
#[derive(Copy, Clone)]
pub struct NoA2;

#[derive(Copy, Clone)]
pub struct YesNI;
#[derive(Copy, Clone)]
pub struct NoNI;

use core::marker::PhantomData;

#[derive(Copy, Clone)]
pub struct SseMachine<S3, S4, NI>(PhantomData<(S3, S4, NI)>);
impl<S3: Copy, S4: Copy, NI: Copy> Machine for SseMachine<S3, S4, NI>
where
    sse2::u128x1_sse2<S3, S4, NI>: Swap64,
    sse2::u64x2_sse2<S3, S4, NI>: BSwap + RotateEachWord32 + MultiLane<[u64; 2]> + Vec2<u64>,
    sse2::u32x4_sse2<S3, S4, NI>: BSwap + RotateEachWord32 + MultiLane<[u32; 4]> + Vec4<u32>,
    sse2::u64x4_sse2<S3, S4, NI>: BSwap + Words4,
    sse2::u128x1_sse2<S3, S4, NI>: BSwap,
    sse2::u128x2_sse2<S3, S4, NI>: Into<sse2::u64x2x2_sse2<S3, S4, NI>>,
    sse2::u128x2_sse2<S3, S4, NI>: Into<sse2::u64x4_sse2<S3, S4, NI>>,
    sse2::u128x2_sse2<S3, S4, NI>: Into<sse2::u32x4x2_sse2<S3, S4, NI>>,
    sse2::u128x4_sse2<S3, S4, NI>: Into<sse2::u64x2x4_sse2<S3, S4, NI>>,
    sse2::u128x4_sse2<S3, S4, NI>: Into<sse2::u32x4x4_sse2<S3, S4, NI>>,
{
    type u32x4 = sse2::u32x4_sse2<S3, S4, NI>;
    type u64x2 = sse2::u64x2_sse2<S3, S4, NI>;
    type u128x1 = sse2::u128x1_sse2<S3, S4, NI>;

    type u32x4x2 = sse2::u32x4x2_sse2<S3, S4, NI>;
    type u64x2x2 = sse2::u64x2x2_sse2<S3, S4, NI>;
    type u64x4 = sse2::u64x4_sse2<S3, S4, NI>;
    type u128x2 = sse2::u128x2_sse2<S3, S4, NI>;

    type u32x4x4 = sse2::u32x4x4_sse2<S3, S4, NI>;
    type u64x2x4 = sse2::u64x2x4_sse2<S3, S4, NI>;
    type u128x4 = sse2::u128x4_sse2<S3, S4, NI>;

    #[inline(always)]
    unsafe fn instance() -> Self {
        SseMachine(PhantomData)
    }
}

#[derive(Copy, Clone)]
pub struct Avx2Machine<NI>(PhantomData<NI>);
impl<NI: Copy> Machine for Avx2Machine<NI>
where
    sse2::u128x1_sse2<YesS3, YesS4, NI>: BSwap + Swap64,
    sse2::u64x2_sse2<YesS3, YesS4, NI>: BSwap + RotateEachWord32 + MultiLane<[u64; 2]> + Vec2<u64>,
    sse2::u32x4_sse2<YesS3, YesS4, NI>: BSwap + RotateEachWord32 + MultiLane<[u32; 4]> + Vec4<u32>,
    sse2::u64x4_sse2<YesS3, YesS4, NI>: BSwap + Words4,
{
    type u32x4 = sse2::u32x4_sse2<YesS3, YesS4, NI>;
    type u64x2 = sse2::u64x2_sse2<YesS3, YesS4, NI>;
    type u128x1 = sse2::u128x1_sse2<YesS3, YesS4, NI>;

    type u32x4x2 = sse2::avx2::u32x4x2_avx2<NI>;
    type u64x2x2 = sse2::u64x2x2_sse2<YesS3, YesS4, NI>;
    type u64x4 = sse2::u64x4_sse2<YesS3, YesS4, NI>;
    type u128x2 = sse2::u128x2_sse2<YesS3, YesS4, NI>;

    type u32x4x4 = sse2::avx2::u32x4x4_avx2<NI>;
    type u64x2x4 = sse2::u64x2x4_sse2<YesS3, YesS4, NI>;
    type u128x4 = sse2::u128x4_sse2<YesS3, YesS4, NI>;

    #[inline(always)]
    unsafe fn instance() -> Self {
        Avx2Machine(PhantomData)
    }
}

pub type SSE2 = SseMachine<NoS3, NoS4, NoNI>;
pub type SSSE3 = SseMachine<YesS3, NoS4, NoNI>;
pub type SSE41 = SseMachine<YesS3, YesS4, NoNI>;
/// AVX but not AVX2: only 128-bit integer operations, but use VEX versions of everything
/// to avoid expensive SSE/VEX conflicts.
pub type AVX = SseMachine<YesS3, YesS4, NoNI>;
pub type AVX2 = Avx2Machine<NoNI>;

zerocopy::cryptocorrosion_derive_traits! {
    #[repr(C)]
    /// Generic wrapper for unparameterized storage of any of the possible impls.
    /// Converting into and out of this type should be essentially free, although it may be more
    /// aligned than a particular impl requires.
    #[allow(non_camel_case_types)]
    #[derive(Copy, Clone)]
    pub union vec128_storage {
        u32x4: [u32; 4],
        u64x2: [u64; 2],
        u128x1: [u128; 1],
        sse2: __m128i,
    }
}

impl Store<vec128_storage> for vec128_storage {
    #[inline(always)]
    unsafe fn unpack(p: vec128_storage) -> Self {
        p
    }
}
impl<'a> From<&'a vec128_storage> for &'a [u32; 4] {
    #[inline(always)]
    fn from(x: &'a vec128_storage) -> Self {
        unsafe { &x.u32x4 }
    }
}
impl From<[u32; 4]> for vec128_storage {
    #[inline(always)]
    fn from(u32x4: [u32; 4]) -> Self {
        vec128_storage { u32x4 }
    }
}
impl Default for vec128_storage {
    #[inline(always)]
    fn default() -> Self {
        vec128_storage { u128x1: [0] }
    }
}
impl Eq for vec128_storage {}
impl PartialEq for vec128_storage {
    #[inline(always)]
    fn eq(&self, rhs: &Self) -> bool {
        unsafe { self.u128x1 == rhs.u128x1 }
    }
}

#[allow(non_camel_case_types)]
#[derive(Copy, Clone)]
pub union vec256_storage {
    u32x8: [u32; 8],
    u64x4: [u64; 4],
    u128x2: [u128; 2],
    sse2: [vec128_storage; 2],
    avx: __m256i,
}
impl From<[u64; 4]> for vec256_storage {
    #[inline(always)]
    fn from(u64x4: [u64; 4]) -> Self {
        vec256_storage { u64x4 }
    }
}
impl Default for vec256_storage {
    #[inline(always)]
    fn default() -> Self {
        vec256_storage { u128x2: [0, 0] }
    }
}
impl vec256_storage {
    #[inline(always)]
    pub fn new128(xs: [vec128_storage; 2]) -> Self {
        Self { sse2: xs }
    }
    #[inline(always)]
    pub fn split128(self) -> [vec128_storage; 2] {
        unsafe { self.sse2 }
    }
}
impl Eq for vec256_storage {}
impl PartialEq for vec256_storage {
    #[inline(always)]
    fn eq(&self, rhs: &Self) -> bool {
        unsafe { self.sse2 == rhs.sse2 }
    }
}

#[allow(non_camel_case_types)]
#[derive(Copy, Clone)]
pub union vec512_storage {
    u32x16: [u32; 16],
    u64x8: [u64; 8],
    u128x4: [u128; 4],
    sse2: [vec128_storage; 4],
    avx: [vec256_storage; 2],
}
impl Default for vec512_storage {
    #[inline(always)]
    fn default() -> Self {
        vec512_storage {
            u128x4: [0, 0, 0, 0],
        }
    }
}
impl vec512_storage {
    #[inline(always)]
    pub fn new128(xs: [vec128_storage; 4]) -> Self {
        Self { sse2: xs }
    }
    #[inline(always)]
    pub fn split128(self) -> [vec128_storage; 4] {
        unsafe { self.sse2 }
    }
}
impl Eq for vec512_storage {}
impl PartialEq for vec512_storage {
    #[inline(always)]
    fn eq(&self, rhs: &Self) -> bool {
        unsafe { self.avx == rhs.avx }
    }
}

macro_rules! impl_into {
    ($storage:ident, $array:ty, $name:ident) => {
        impl From<$storage> for $array {
            #[inline(always)]
            fn from(vec: $storage) -> Self {
                unsafe { vec.$name }
            }
        }
    };
}
impl_into!(vec128_storage, [u32; 4], u32x4);
impl_into!(vec128_storage, [u64; 2], u64x2);
impl_into!(vec128_storage, [u128; 1], u128x1);
impl_into!(vec256_storage, [u32; 8], u32x8);
impl_into!(vec256_storage, [u64; 4], u64x4);
impl_into!(vec256_storage, [u128; 2], u128x2);
impl_into!(vec512_storage, [u32; 16], u32x16);
impl_into!(vec512_storage, [u64; 8], u64x8);
impl_into!(vec512_storage, [u128; 4], u128x4);

/// Generate the full set of optimized implementations to take advantage of the most important
/// hardware feature sets.
///
/// This dispatcher is suitable for maximizing throughput.
#[macro_export]
macro_rules! dispatch {
    ($mach:ident, $MTy:ident, { $([$pub:tt$(($krate:tt))*])* fn $name:ident($($arg:ident: $argty:ty),* $(,)*) -> $ret:ty $body:block }) => {
        #[cfg(feature = "std")]
        $($pub$(($krate))*)* fn $name($($arg: $argty),*) -> $ret {
            #[inline(always)]
            fn fn_impl<$MTy: $crate::Machine>($mach: $MTy, $($arg: $argty),*) -> $ret $body
            use std::arch::x86_64::*;
            #[target_feature(enable = "avx2")]
            unsafe fn impl_avx2($($arg: $argty),*) -> $ret {
                let ret = fn_impl($crate::x86_64::AVX2::instance(), $($arg),*);
                _mm256_zeroupper();
                ret
            }
            #[target_feature(enable = "avx")]
            #[target_feature(enable = "sse4.1")]
            #[target_feature(enable = "ssse3")]
            unsafe fn impl_avx($($arg: $argty),*) -> $ret {
                let ret = fn_impl($crate::x86_64::AVX::instance(), $($arg),*);
                _mm256_zeroupper();
                ret
            }
            #[target_feature(enable = "sse4.1")]
            #[target_feature(enable = "ssse3")]
            unsafe fn impl_sse41($($arg: $argty),*) -> $ret {
                fn_impl($crate::x86_64::SSE41::instance(), $($arg),*)
            }
            #[target_feature(enable = "ssse3")]
            unsafe fn impl_ssse3($($arg: $argty),*) -> $ret {
                fn_impl($crate::x86_64::SSSE3::instance(), $($arg),*)
            }
            #[target_feature(enable = "sse2")]
            unsafe fn impl_sse2($($arg: $argty),*) -> $ret {
                fn_impl($crate::x86_64::SSE2::instance(), $($arg),*)
            }
            unsafe {
                if is_x86_feature_detected!("avx2") {
                    impl_avx2($($arg),*)
                } else if is_x86_feature_detected!("avx") {
                    impl_avx($($arg),*)
                } else if is_x86_feature_detected!("sse4.1") {
                    impl_sse41($($arg),*)
                } else if is_x86_feature_detected!("ssse3") {
                    impl_ssse3($($arg),*)
                } else if is_x86_feature_detected!("sse2") {
                    impl_sse2($($arg),*)
                } else {
                    unimplemented!()
                }
            }
        }
        #[cfg(not(feature = "std"))]
        #[inline(always)]
        $($pub$(($krate))*)* fn $name($($arg: $argty),*) -> $ret {
            unsafe fn fn_impl<$MTy: $crate::Machine>($mach: $MTy, $($arg: $argty),*) -> $ret $body
            unsafe {
                if cfg!(target_feature = "avx2") {
                    fn_impl($crate::x86_64::AVX2::instance(), $($arg),*)
                } else if cfg!(target_feature = "avx") {
                    fn_impl($crate::x86_64::AVX::instance(), $($arg),*)
                } else if cfg!(target_feature = "sse4.1") {
                    fn_impl($crate::x86_64::SSE41::instance(), $($arg),*)
                } else if cfg!(target_feature = "ssse3") {
                    fn_impl($crate::x86_64::SSSE3::instance(), $($arg),*)
                } else {
                    fn_impl($crate::x86_64::SSE2::instance(), $($arg),*)
                }
            }
        }
    };
    ($mach:ident, $MTy:ident, { $([$pub:tt $(($krate:tt))*])* fn $name:ident($($arg:ident: $argty:ty),* $(,)*) $body:block }) => {
        dispatch!($mach, $MTy, {
            $([$pub $(($krate))*])* fn $name($($arg: $argty),*) -> () $body
        });
    }
}

/// Generate only the basic implementations necessary to be able to operate efficiently on 128-bit
/// vectors on this platfrom. For x86-64, that would mean SSE2 and AVX.
///
/// This dispatcher is suitable for vector operations that do not benefit from advanced hardware
/// features (e.g. because they are done infrequently), so minimizing their contribution to code
/// size is more important.
#[macro_export]
macro_rules! dispatch_light128 {
    ($mach:ident, $MTy:ident, { $([$pub:tt$(($krate:tt))*])* fn $name:ident($($arg:ident: $argty:ty),* $(,)*) -> $ret:ty $body:block }) => {
        #[cfg(feature = "std")]
        $($pub $(($krate))*)* fn $name($($arg: $argty),*) -> $ret {
            #[inline(always)]
            fn fn_impl<$MTy: $crate::Machine>($mach: $MTy, $($arg: $argty),*) -> $ret $body
            use std::arch::x86_64::*;
            #[target_feature(enable = "avx")]
            unsafe fn impl_avx($($arg: $argty),*) -> $ret {
                fn_impl($crate::x86_64::AVX::instance(), $($arg),*)
            }
            #[target_feature(enable = "sse2")]
            unsafe fn impl_sse2($($arg: $argty),*) -> $ret {
                fn_impl($crate::x86_64::SSE2::instance(), $($arg),*)
            }
            unsafe {
                if is_x86_feature_detected!("avx") {
                    impl_avx($($arg),*)
                } else if is_x86_feature_detected!("sse2") {
                    impl_sse2($($arg),*)
                } else {
                    unimplemented!()
                }
            }
        }
        #[cfg(not(feature = "std"))]
        #[inline(always)]
        $($pub$(($krate))*)* fn $name($($arg: $argty),*) -> $ret {
            unsafe fn fn_impl<$MTy: $crate::Machine>($mach: $MTy, $($arg: $argty),*) -> $ret $body
            unsafe {
                if cfg!(target_feature = "avx2") {
                    fn_impl($crate::x86_64::AVX2::instance(), $($arg),*)
                } else if cfg!(target_feature = "avx") {
                    fn_impl($crate::x86_64::AVX::instance(), $($arg),*)
                } else if cfg!(target_feature = "sse4.1") {
                    fn_impl($crate::x86_64::SSE41::instance(), $($arg),*)
                } else if cfg!(target_feature = "ssse3") {
                    fn_impl($crate::x86_64::SSSE3::instance(), $($arg),*)
                } else {
                    fn_impl($crate::x86_64::SSE2::instance(), $($arg),*)
                }
            }
        }
    };
    ($mach:ident, $MTy:ident, { $([$pub:tt$(($krate:tt))*])* fn $name:ident($($arg:ident: $argty:ty),* $(,)*) $body:block }) => {
        dispatch_light128!($mach, $MTy, {
            $([$pub $(($krate))*])* fn $name($($arg: $argty),*) -> () $body
        });
    }
}

/// Generate only the basic implementations necessary to be able to operate efficiently on 256-bit
/// vectors on this platfrom. For x86-64, that would mean SSE2, AVX, and AVX2.
///
/// This dispatcher is suitable for vector operations that do not benefit from advanced hardware
/// features (e.g. because they are done infrequently), so minimizing their contribution to code
/// size is more important.
#[macro_export]
macro_rules! dispatch_light256 {
    ($mach:ident, $MTy:ident, { $([$pub:tt$(($krate:tt))*])* fn $name:ident($($arg:ident: $argty:ty),* $(,)*) -> $ret:ty $body:block }) => {
        #[cfg(feature = "std")]
        $([$pub $(($krate))*])* fn $name($($arg: $argty),*) -> $ret {
            #[inline(always)]
            fn fn_impl<$MTy: $crate::Machine>($mach: $MTy, $($arg: $argty),*) -> $ret $body
            use std::arch::x86_64::*;
            #[target_feature(enable = "avx")]
            unsafe fn impl_avx($($arg: $argty),*) -> $ret {
                fn_impl($crate::x86_64::AVX::instance(), $($arg),*)
            }
            #[target_feature(enable = "sse2")]
            unsafe fn impl_sse2($($arg: $argty),*) -> $ret {
                fn_impl($crate::x86_64::SSE2::instance(), $($arg),*)
            }
            unsafe {
                if is_x86_feature_detected!("avx") {
                    impl_avx($($arg),*)
                } else if is_x86_feature_detected!("sse2") {
                    impl_sse2($($arg),*)
                } else {
                    unimplemented!()
                }
            }
        }
        #[cfg(not(feature = "std"))]
        #[inline(always)]
        $($pub$(($krate))*)* fn $name($($arg: $argty),*) -> $ret {
            unsafe fn fn_impl<$MTy: $crate::Machine>($mach: $MTy, $($arg: $argty),*) -> $ret $body
            unsafe {
                if cfg!(target_feature = "avx2") {
                    fn_impl($crate::x86_64::AVX2::instance(), $($arg),*)
                } else if cfg!(target_feature = "avx") {
                    fn_impl($crate::x86_64::AVX::instance(), $($arg),*)
                } else if cfg!(target_feature = "sse4.1") {
                    fn_impl($crate::x86_64::SSE41::instance(), $($arg),*)
                } else if cfg!(target_feature = "ssse3") {
                    fn_impl($crate::x86_64::SSSE3::instance(), $($arg),*)
                } else {
                    fn_impl($crate::x86_64::SSE2::instance(), $($arg),*)
                }
            }
        }
    };
    ($mach:ident, $MTy:ident, { $([$pub:tt$(($krate:tt))*])* fn $name:ident($($arg:ident: $argty:ty),* $(,)*) $body:block }) => {
        dispatch_light256!($mach, $MTy, {
            $([$pub $(($krate))*])* fn $name($($arg: $argty),*) -> () $body
        });
    }
}
