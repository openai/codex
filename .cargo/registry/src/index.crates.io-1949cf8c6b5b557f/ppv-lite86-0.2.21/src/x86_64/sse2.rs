use crate::soft::{x2, x4};
use crate::types::*;
use crate::vec128_storage;
use crate::x86_64::Avx2Machine;
use crate::x86_64::SseMachine as Machine86;
use crate::x86_64::{NoS3, NoS4, YesS3, YesS4};
use core::arch::x86_64::*;
use core::marker::PhantomData;
use core::ops::{
    Add, AddAssign, BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not,
};
use zerocopy::transmute;

macro_rules! impl_binop {
    ($vec:ident, $trait:ident, $fn:ident, $impl_fn:ident) => {
        impl<S3, S4, NI> $trait for $vec<S3, S4, NI> {
            type Output = Self;
            #[inline(always)]
            fn $fn(self, rhs: Self) -> Self::Output {
                Self::new(unsafe { $impl_fn(self.x, rhs.x) })
            }
        }
    };
}

macro_rules! impl_binop_assign {
    ($vec:ident, $trait:ident, $fn_assign:ident, $fn:ident) => {
        impl<S3, S4, NI> $trait for $vec<S3, S4, NI>
        where
            $vec<S3, S4, NI>: Copy,
        {
            #[inline(always)]
            fn $fn_assign(&mut self, rhs: Self) {
                *self = self.$fn(rhs);
            }
        }
    };
}

macro_rules! def_vec {
    ($vec:ident, $word:ident) => {
        zerocopy::cryptocorrosion_derive_traits! {
            #[repr(transparent)]
            #[allow(non_camel_case_types)]
            #[derive(Copy, Clone)]
            pub struct $vec<S3, S4, NI> {
                x: __m128i,
                s3: PhantomData<S3>,
                s4: PhantomData<S4>,
                ni: PhantomData<NI>,
            }
        }

        impl<S3, S4, NI> Store<vec128_storage> for $vec<S3, S4, NI> {
            #[inline(always)]
            unsafe fn unpack(x: vec128_storage) -> Self {
                Self::new(x.sse2)
            }
        }
        impl<S3, S4, NI> From<$vec<S3, S4, NI>> for vec128_storage {
            #[inline(always)]
            fn from(x: $vec<S3, S4, NI>) -> Self {
                vec128_storage { sse2: x.x }
            }
        }
        impl<S3, S4, NI> $vec<S3, S4, NI> {
            #[inline(always)]
            fn new(x: __m128i) -> Self {
                $vec {
                    x,
                    s3: PhantomData,
                    s4: PhantomData,
                    ni: PhantomData,
                }
            }
        }

        impl<S3, S4, NI> StoreBytes for $vec<S3, S4, NI>
        where
            Self: BSwap,
        {
            #[inline(always)]
            unsafe fn unsafe_read_le(input: &[u8]) -> Self {
                assert_eq!(input.len(), 16);
                Self::new(_mm_loadu_si128(input.as_ptr() as *const _))
            }
            #[inline(always)]
            unsafe fn unsafe_read_be(input: &[u8]) -> Self {
                assert_eq!(input.len(), 16);
                Self::new(_mm_loadu_si128(input.as_ptr() as *const _)).bswap()
            }
            #[inline(always)]
            fn write_le(self, out: &mut [u8]) {
                assert_eq!(out.len(), 16);
                unsafe { _mm_storeu_si128(out.as_mut_ptr() as *mut _, self.x) }
            }
            #[inline(always)]
            fn write_be(self, out: &mut [u8]) {
                assert_eq!(out.len(), 16);
                let x = self.bswap().x;
                unsafe {
                    _mm_storeu_si128(out.as_mut_ptr() as *mut _, x);
                }
            }
        }

        impl<S3, S4, NI> Default for $vec<S3, S4, NI> {
            #[inline(always)]
            fn default() -> Self {
                Self::new(unsafe { _mm_setzero_si128() })
            }
        }

        impl<S3, S4, NI> Not for $vec<S3, S4, NI> {
            type Output = Self;
            #[inline(always)]
            fn not(self) -> Self::Output {
                unsafe {
                    let ff = _mm_set1_epi64x(-1i64);
                    self ^ Self::new(ff)
                }
            }
        }

        impl<S3: Copy, S4: Copy, NI: Copy> BitOps0 for $vec<S3, S4, NI> {}
        impl_binop!($vec, BitAnd, bitand, _mm_and_si128);
        impl_binop!($vec, BitOr, bitor, _mm_or_si128);
        impl_binop!($vec, BitXor, bitxor, _mm_xor_si128);
        impl_binop_assign!($vec, BitAndAssign, bitand_assign, bitand);
        impl_binop_assign!($vec, BitOrAssign, bitor_assign, bitor);
        impl_binop_assign!($vec, BitXorAssign, bitxor_assign, bitxor);
        impl<S3: Copy, S4: Copy, NI: Copy> AndNot for $vec<S3, S4, NI> {
            type Output = Self;
            #[inline(always)]
            fn andnot(self, rhs: Self) -> Self {
                Self::new(unsafe { _mm_andnot_si128(self.x, rhs.x) })
            }
        }
    };
}

macro_rules! impl_bitops32 {
    ($vec:ident) => {
        impl<S3: Copy, S4: Copy, NI: Copy> BitOps32 for $vec<S3, S4, NI> where
            $vec<S3, S4, NI>: RotateEachWord32
        {
        }
    };
}

macro_rules! impl_bitops64 {
    ($vec:ident) => {
        impl_bitops32!($vec);
        impl<S3: Copy, S4: Copy, NI: Copy> BitOps64 for $vec<S3, S4, NI> where
            $vec<S3, S4, NI>: RotateEachWord64 + RotateEachWord32
        {
        }
    };
}

macro_rules! impl_bitops128 {
    ($vec:ident) => {
        impl_bitops64!($vec);
        impl<S3: Copy, S4: Copy, NI: Copy> BitOps128 for $vec<S3, S4, NI> where
            $vec<S3, S4, NI>: RotateEachWord128
        {
        }
    };
}

macro_rules! rotr_32_s3 {
    ($name:ident, $k0:expr, $k1:expr) => {
        #[inline(always)]
        fn $name(self) -> Self {
            Self::new(unsafe { _mm_shuffle_epi8(self.x, _mm_set_epi64x($k0, $k1)) })
        }
    };
}
macro_rules! rotr_32 {
    ($name:ident, $i:expr) => {
        #[inline(always)]
        fn $name(self) -> Self {
            Self::new(unsafe {
                _mm_or_si128(
                    _mm_srli_epi32(self.x, $i as i32),
                    _mm_slli_epi32(self.x, 32 - $i as i32),
                )
            })
        }
    };
}
impl<S4: Copy, NI: Copy> RotateEachWord32 for u32x4_sse2<YesS3, S4, NI> {
    rotr_32!(rotate_each_word_right7, 7);
    rotr_32_s3!(
        rotate_each_word_right8,
        0x0c0f_0e0d_080b_0a09,
        0x0407_0605_0003_0201
    );
    rotr_32!(rotate_each_word_right11, 11);
    rotr_32!(rotate_each_word_right12, 12);
    rotr_32_s3!(
        rotate_each_word_right16,
        0x0d0c_0f0e_0908_0b0a,
        0x0504_0706_0100_0302
    );
    rotr_32!(rotate_each_word_right20, 20);
    rotr_32_s3!(
        rotate_each_word_right24,
        0x0e0d_0c0f_0a09_080b,
        0x0605_0407_0201_0003
    );
    rotr_32!(rotate_each_word_right25, 25);
}
impl<S4: Copy, NI: Copy> RotateEachWord32 for u32x4_sse2<NoS3, S4, NI> {
    rotr_32!(rotate_each_word_right7, 7);
    rotr_32!(rotate_each_word_right8, 8);
    rotr_32!(rotate_each_word_right11, 11);
    rotr_32!(rotate_each_word_right12, 12);
    #[inline(always)]
    fn rotate_each_word_right16(self) -> Self {
        Self::new(swap16_s2(self.x))
    }
    rotr_32!(rotate_each_word_right20, 20);
    rotr_32!(rotate_each_word_right24, 24);
    rotr_32!(rotate_each_word_right25, 25);
}

macro_rules! rotr_64_s3 {
    ($name:ident, $k0:expr, $k1:expr) => {
        #[inline(always)]
        fn $name(self) -> Self {
            Self::new(unsafe { _mm_shuffle_epi8(self.x, _mm_set_epi64x($k0, $k1)) })
        }
    };
}
macro_rules! rotr_64 {
    ($name:ident, $i:expr) => {
        #[inline(always)]
        fn $name(self) -> Self {
            Self::new(unsafe {
                _mm_or_si128(
                    _mm_srli_epi64(self.x, $i as i32),
                    _mm_slli_epi64(self.x, 64 - $i as i32),
                )
            })
        }
    };
}
impl<S4: Copy, NI: Copy> RotateEachWord32 for u64x2_sse2<YesS3, S4, NI> {
    rotr_64!(rotate_each_word_right7, 7);
    rotr_64_s3!(
        rotate_each_word_right8,
        0x080f_0e0d_0c0b_0a09,
        0x0007_0605_0403_0201
    );
    rotr_64!(rotate_each_word_right11, 11);
    rotr_64!(rotate_each_word_right12, 12);
    rotr_64_s3!(
        rotate_each_word_right16,
        0x0908_0f0e_0d0c_0b0a,
        0x0100_0706_0504_0302
    );
    rotr_64!(rotate_each_word_right20, 20);
    rotr_64_s3!(
        rotate_each_word_right24,
        0x0a09_080f_0e0d_0c0b,
        0x0201_0007_0605_0403
    );
    rotr_64!(rotate_each_word_right25, 25);
}
impl<S4: Copy, NI: Copy> RotateEachWord32 for u64x2_sse2<NoS3, S4, NI> {
    rotr_64!(rotate_each_word_right7, 7);
    rotr_64!(rotate_each_word_right8, 8);
    rotr_64!(rotate_each_word_right11, 11);
    rotr_64!(rotate_each_word_right12, 12);
    #[inline(always)]
    fn rotate_each_word_right16(self) -> Self {
        Self::new(swap16_s2(self.x))
    }
    rotr_64!(rotate_each_word_right20, 20);
    rotr_64!(rotate_each_word_right24, 24);
    rotr_64!(rotate_each_word_right25, 25);
}
impl<S3: Copy, S4: Copy, NI: Copy> RotateEachWord64 for u64x2_sse2<S3, S4, NI> {
    #[inline(always)]
    fn rotate_each_word_right32(self) -> Self {
        Self::new(unsafe { _mm_shuffle_epi32(self.x, 0b10110001) })
    }
}

macro_rules! rotr_128 {
    ($name:ident, $i:expr) => {
        #[inline(always)]
        fn $name(self) -> Self {
            Self::new(unsafe {
                _mm_or_si128(
                    _mm_srli_si128(self.x, $i as i32),
                    _mm_slli_si128(self.x, 128 - $i as i32),
                )
            })
        }
    };
}
// TODO: completely unoptimized
impl<S3: Copy, S4: Copy, NI: Copy> RotateEachWord32 for u128x1_sse2<S3, S4, NI> {
    rotr_128!(rotate_each_word_right7, 7);
    rotr_128!(rotate_each_word_right8, 8);
    rotr_128!(rotate_each_word_right11, 11);
    rotr_128!(rotate_each_word_right12, 12);
    rotr_128!(rotate_each_word_right16, 16);
    rotr_128!(rotate_each_word_right20, 20);
    rotr_128!(rotate_each_word_right24, 24);
    rotr_128!(rotate_each_word_right25, 25);
}
// TODO: completely unoptimized
impl<S3: Copy, S4: Copy, NI: Copy> RotateEachWord64 for u128x1_sse2<S3, S4, NI> {
    rotr_128!(rotate_each_word_right32, 32);
}
impl<S3: Copy, S4: Copy, NI: Copy> RotateEachWord128 for u128x1_sse2<S3, S4, NI> {}

def_vec!(u32x4_sse2, u32);
def_vec!(u64x2_sse2, u64);
def_vec!(u128x1_sse2, u128);

impl<S3, NI> MultiLane<[u32; 4]> for u32x4_sse2<S3, YesS4, NI> {
    #[inline(always)]
    fn to_lanes(self) -> [u32; 4] {
        unsafe {
            let x = _mm_cvtsi128_si64(self.x) as u64;
            let y = _mm_extract_epi64(self.x, 1) as u64;
            [x as u32, (x >> 32) as u32, y as u32, (y >> 32) as u32]
        }
    }
    #[inline(always)]
    fn from_lanes(xs: [u32; 4]) -> Self {
        unsafe {
            let mut x = _mm_cvtsi64_si128((xs[0] as u64 | ((xs[1] as u64) << 32)) as i64);
            x = _mm_insert_epi64(x, (xs[2] as u64 | ((xs[3] as u64) << 32)) as i64, 1);
            Self::new(x)
        }
    }
}
impl<S3, NI> MultiLane<[u32; 4]> for u32x4_sse2<S3, NoS4, NI> {
    #[inline(always)]
    fn to_lanes(self) -> [u32; 4] {
        unsafe {
            let x = _mm_cvtsi128_si64(self.x) as u64;
            let y = _mm_cvtsi128_si64(_mm_shuffle_epi32(self.x, 0b11101110)) as u64;
            [x as u32, (x >> 32) as u32, y as u32, (y >> 32) as u32]
        }
    }
    #[inline(always)]
    fn from_lanes(xs: [u32; 4]) -> Self {
        unsafe {
            let x = (xs[0] as u64 | ((xs[1] as u64) << 32)) as i64;
            let y = (xs[2] as u64 | ((xs[3] as u64) << 32)) as i64;
            let x = _mm_cvtsi64_si128(x);
            let y = _mm_slli_si128(_mm_cvtsi64_si128(y), 8);
            Self::new(_mm_or_si128(x, y))
        }
    }
}
impl<S3, NI> MultiLane<[u64; 2]> for u64x2_sse2<S3, YesS4, NI> {
    #[inline(always)]
    fn to_lanes(self) -> [u64; 2] {
        unsafe {
            [
                _mm_cvtsi128_si64(self.x) as u64,
                _mm_extract_epi64(self.x, 1) as u64,
            ]
        }
    }
    #[inline(always)]
    fn from_lanes(xs: [u64; 2]) -> Self {
        unsafe {
            let mut x = _mm_cvtsi64_si128(xs[0] as i64);
            x = _mm_insert_epi64(x, xs[1] as i64, 1);
            Self::new(x)
        }
    }
}
impl<S3, NI> MultiLane<[u64; 2]> for u64x2_sse2<S3, NoS4, NI> {
    #[inline(always)]
    fn to_lanes(self) -> [u64; 2] {
        unsafe {
            [
                _mm_cvtsi128_si64(self.x) as u64,
                _mm_cvtsi128_si64(_mm_srli_si128(self.x, 8)) as u64,
            ]
        }
    }
    #[inline(always)]
    fn from_lanes(xs: [u64; 2]) -> Self {
        unsafe {
            let x = _mm_cvtsi64_si128(xs[0] as i64);
            let y = _mm_slli_si128(_mm_cvtsi64_si128(xs[1] as i64), 8);
            Self::new(_mm_or_si128(x, y))
        }
    }
}
impl<S3, S4, NI> MultiLane<[u128; 1]> for u128x1_sse2<S3, S4, NI> {
    #[inline(always)]
    fn to_lanes(self) -> [u128; 1] {
        unimplemented!()
    }
    #[inline(always)]
    fn from_lanes(xs: [u128; 1]) -> Self {
        unimplemented!("{:?}", xs)
    }
}

impl<S3, S4, NI> MultiLane<[u64; 4]> for u64x4_sse2<S3, S4, NI>
where
    u64x2_sse2<S3, S4, NI>: MultiLane<[u64; 2]> + Copy,
{
    #[inline(always)]
    fn to_lanes(self) -> [u64; 4] {
        let (a, b) = (self.0[0].to_lanes(), self.0[1].to_lanes());
        [a[0], a[1], b[0], b[1]]
    }
    #[inline(always)]
    fn from_lanes(xs: [u64; 4]) -> Self {
        let (a, b) = (
            u64x2_sse2::from_lanes([xs[0], xs[1]]),
            u64x2_sse2::from_lanes([xs[2], xs[3]]),
        );
        x2::new([a, b])
    }
}

macro_rules! impl_into {
    ($from:ident, $to:ident) => {
        impl<S3, S4, NI> From<$from<S3, S4, NI>> for $to<S3, S4, NI> {
            #[inline(always)]
            fn from(x: $from<S3, S4, NI>) -> Self {
                $to::new(x.x)
            }
        }
    };
}

impl_into!(u128x1_sse2, u32x4_sse2);
impl_into!(u128x1_sse2, u64x2_sse2);

impl_bitops32!(u32x4_sse2);
impl_bitops64!(u64x2_sse2);
impl_bitops128!(u128x1_sse2);

impl<S3: Copy, S4: Copy, NI: Copy> ArithOps for u32x4_sse2<S3, S4, NI> where
    u32x4_sse2<S3, S4, NI>: BSwap
{
}
impl<S3: Copy, S4: Copy, NI: Copy> ArithOps for u64x2_sse2<S3, S4, NI> where
    u64x2_sse2<S3, S4, NI>: BSwap
{
}
impl_binop!(u32x4_sse2, Add, add, _mm_add_epi32);
impl_binop!(u64x2_sse2, Add, add, _mm_add_epi64);
impl_binop_assign!(u32x4_sse2, AddAssign, add_assign, add);
impl_binop_assign!(u64x2_sse2, AddAssign, add_assign, add);

impl<S3: Copy, S4: Copy, NI: Copy> u32x4<Machine86<S3, S4, NI>> for u32x4_sse2<S3, S4, NI>
where
    u32x4_sse2<S3, S4, NI>: RotateEachWord32 + BSwap + MultiLane<[u32; 4]> + Vec4<u32>,
    Machine86<S3, S4, NI>: Machine,
{
}
impl<S3: Copy, S4: Copy, NI: Copy> u64x2<Machine86<S3, S4, NI>> for u64x2_sse2<S3, S4, NI>
where
    u64x2_sse2<S3, S4, NI>:
        RotateEachWord64 + RotateEachWord32 + BSwap + MultiLane<[u64; 2]> + Vec2<u64>,
    Machine86<S3, S4, NI>: Machine,
{
}
impl<S3: Copy, S4: Copy, NI: Copy> u128x1<Machine86<S3, S4, NI>> for u128x1_sse2<S3, S4, NI>
where
    u128x1_sse2<S3, S4, NI>: Swap64 + RotateEachWord64 + RotateEachWord32 + BSwap,
    Machine86<S3, S4, NI>: Machine,
    u128x1_sse2<S3, S4, NI>: Into<<Machine86<S3, S4, NI> as Machine>::u32x4>,
    u128x1_sse2<S3, S4, NI>: Into<<Machine86<S3, S4, NI> as Machine>::u64x2>,
{
}

impl<NI: Copy> u32x4<Avx2Machine<NI>> for u32x4_sse2<YesS3, YesS4, NI>
where
    u32x4_sse2<YesS3, YesS4, NI>: RotateEachWord32 + BSwap + MultiLane<[u32; 4]> + Vec4<u32>,
    Machine86<YesS3, YesS4, NI>: Machine,
{
}
impl<NI: Copy> u64x2<Avx2Machine<NI>> for u64x2_sse2<YesS3, YesS4, NI>
where
    u64x2_sse2<YesS3, YesS4, NI>:
        RotateEachWord64 + RotateEachWord32 + BSwap + MultiLane<[u64; 2]> + Vec2<u64>,
    Machine86<YesS3, YesS4, NI>: Machine,
{
}
impl<NI: Copy> u128x1<Avx2Machine<NI>> for u128x1_sse2<YesS3, YesS4, NI>
where
    u128x1_sse2<YesS3, YesS4, NI>: Swap64 + RotateEachWord64 + RotateEachWord32 + BSwap,
    Machine86<YesS3, YesS4, NI>: Machine,
    u128x1_sse2<YesS3, YesS4, NI>: Into<<Machine86<YesS3, YesS4, NI> as Machine>::u32x4>,
    u128x1_sse2<YesS3, YesS4, NI>: Into<<Machine86<YesS3, YesS4, NI> as Machine>::u64x2>,
{
}

impl<S3, S4, NI> UnsafeFrom<[u32; 4]> for u32x4_sse2<S3, S4, NI> {
    #[inline(always)]
    unsafe fn unsafe_from(xs: [u32; 4]) -> Self {
        Self::new(_mm_set_epi32(
            xs[3] as i32,
            xs[2] as i32,
            xs[1] as i32,
            xs[0] as i32,
        ))
    }
}

impl<S3, NI> Vec4<u32> for u32x4_sse2<S3, YesS4, NI>
where
    Self: MultiLane<[u32; 4]>,
{
    #[inline(always)]
    fn extract(self, i: u32) -> u32 {
        self.to_lanes()[i as usize]
    }
    #[inline(always)]
    fn insert(self, v: u32, i: u32) -> Self {
        Self::new(unsafe {
            match i {
                0 => _mm_insert_epi32(self.x, v as i32, 0),
                1 => _mm_insert_epi32(self.x, v as i32, 1),
                2 => _mm_insert_epi32(self.x, v as i32, 2),
                3 => _mm_insert_epi32(self.x, v as i32, 3),
                _ => unreachable!(),
            }
        })
    }
}
impl<S3, NI> Vec4<u32> for u32x4_sse2<S3, NoS4, NI>
where
    Self: MultiLane<[u32; 4]>,
{
    #[inline(always)]
    fn extract(self, i: u32) -> u32 {
        self.to_lanes()[i as usize]
    }
    #[inline(always)]
    fn insert(self, v: u32, i: u32) -> Self {
        Self::new(unsafe {
            match i {
                0 => {
                    let x = _mm_andnot_si128(_mm_cvtsi32_si128(-1), self.x);
                    _mm_or_si128(x, _mm_cvtsi32_si128(v as i32))
                }
                1 => {
                    let mut x = _mm_shuffle_epi32(self.x, 0b0111_1000);
                    x = _mm_slli_si128(x, 4);
                    x = _mm_or_si128(x, _mm_cvtsi32_si128(v as i32));
                    _mm_shuffle_epi32(x, 0b1110_0001)
                }
                2 => {
                    let mut x = _mm_shuffle_epi32(self.x, 0b1011_0100);
                    x = _mm_slli_si128(x, 4);
                    x = _mm_or_si128(x, _mm_cvtsi32_si128(v as i32));
                    _mm_shuffle_epi32(x, 0b1100_1001)
                }
                3 => {
                    let mut x = _mm_slli_si128(self.x, 4);
                    x = _mm_or_si128(x, _mm_cvtsi32_si128(v as i32));
                    _mm_shuffle_epi32(x, 0b0011_1001)
                }
                _ => unreachable!(),
            }
        })
    }
}

impl<S3, S4, NI> LaneWords4 for u32x4_sse2<S3, S4, NI> {
    #[inline(always)]
    fn shuffle_lane_words2301(self) -> Self {
        self.shuffle2301()
    }
    #[inline(always)]
    fn shuffle_lane_words1230(self) -> Self {
        self.shuffle1230()
    }
    #[inline(always)]
    fn shuffle_lane_words3012(self) -> Self {
        self.shuffle3012()
    }
}

impl<S3, S4, NI> Words4 for u32x4_sse2<S3, S4, NI> {
    #[inline(always)]
    fn shuffle2301(self) -> Self {
        Self::new(unsafe { _mm_shuffle_epi32(self.x, 0b0100_1110) })
    }
    #[inline(always)]
    fn shuffle1230(self) -> Self {
        Self::new(unsafe { _mm_shuffle_epi32(self.x, 0b1001_0011) })
    }
    #[inline(always)]
    fn shuffle3012(self) -> Self {
        Self::new(unsafe { _mm_shuffle_epi32(self.x, 0b0011_1001) })
    }
}

impl<S4, NI> Words4 for u64x4_sse2<YesS3, S4, NI> {
    #[inline(always)]
    fn shuffle2301(self) -> Self {
        x2::new([u64x2_sse2::new(self.0[1].x), u64x2_sse2::new(self.0[0].x)])
    }
    #[inline(always)]
    fn shuffle3012(self) -> Self {
        unsafe {
            x2::new([
                u64x2_sse2::new(_mm_alignr_epi8(self.0[1].x, self.0[0].x, 8)),
                u64x2_sse2::new(_mm_alignr_epi8(self.0[0].x, self.0[1].x, 8)),
            ])
        }
    }
    #[inline(always)]
    fn shuffle1230(self) -> Self {
        unsafe {
            x2::new([
                u64x2_sse2::new(_mm_alignr_epi8(self.0[0].x, self.0[1].x, 8)),
                u64x2_sse2::new(_mm_alignr_epi8(self.0[1].x, self.0[0].x, 8)),
            ])
        }
    }
}
impl<S4, NI> Words4 for u64x4_sse2<NoS3, S4, NI> {
    #[inline(always)]
    fn shuffle2301(self) -> Self {
        x2::new([u64x2_sse2::new(self.0[1].x), u64x2_sse2::new(self.0[0].x)])
    }
    #[inline(always)]
    fn shuffle3012(self) -> Self {
        unsafe {
            let a = _mm_srli_si128(self.0[0].x, 8);
            let b = _mm_slli_si128(self.0[0].x, 8);
            let c = _mm_srli_si128(self.0[1].x, 8);
            let d = _mm_slli_si128(self.0[1].x, 8);
            let da = _mm_or_si128(d, a);
            let bc = _mm_or_si128(b, c);
            x2::new([u64x2_sse2::new(da), u64x2_sse2::new(bc)])
        }
    }
    #[inline(always)]
    fn shuffle1230(self) -> Self {
        unsafe {
            let a = _mm_srli_si128(self.0[0].x, 8);
            let b = _mm_slli_si128(self.0[0].x, 8);
            let c = _mm_srli_si128(self.0[1].x, 8);
            let d = _mm_slli_si128(self.0[1].x, 8);
            let da = _mm_or_si128(d, a);
            let bc = _mm_or_si128(b, c);
            x2::new([u64x2_sse2::new(bc), u64x2_sse2::new(da)])
        }
    }
}

impl<S3, S4, NI> UnsafeFrom<[u64; 2]> for u64x2_sse2<S3, S4, NI> {
    #[inline(always)]
    unsafe fn unsafe_from(xs: [u64; 2]) -> Self {
        Self::new(_mm_set_epi64x(xs[1] as i64, xs[0] as i64))
    }
}

impl<S3, NI> Vec2<u64> for u64x2_sse2<S3, YesS4, NI> {
    #[inline(always)]
    fn extract(self, i: u32) -> u64 {
        unsafe {
            match i {
                0 => _mm_cvtsi128_si64(self.x) as u64,
                1 => _mm_extract_epi64(self.x, 1) as u64,
                _ => unreachable!(),
            }
        }
    }
    #[inline(always)]
    fn insert(self, x: u64, i: u32) -> Self {
        Self::new(unsafe {
            match i {
                0 => _mm_insert_epi64(self.x, x as i64, 0),
                1 => _mm_insert_epi64(self.x, x as i64, 1),
                _ => unreachable!(),
            }
        })
    }
}
impl<S3, NI> Vec2<u64> for u64x2_sse2<S3, NoS4, NI> {
    #[inline(always)]
    fn extract(self, i: u32) -> u64 {
        unsafe {
            match i {
                0 => _mm_cvtsi128_si64(self.x) as u64,
                1 => _mm_cvtsi128_si64(_mm_shuffle_epi32(self.x, 0b11101110)) as u64,
                _ => unreachable!(),
            }
        }
    }
    #[inline(always)]
    fn insert(self, x: u64, i: u32) -> Self {
        Self::new(unsafe {
            match i {
                0 => _mm_or_si128(
                    _mm_andnot_si128(_mm_cvtsi64_si128(-1), self.x),
                    _mm_cvtsi64_si128(x as i64),
                ),
                1 => _mm_or_si128(
                    _mm_move_epi64(self.x),
                    _mm_slli_si128(_mm_cvtsi64_si128(x as i64), 8),
                ),
                _ => unreachable!(),
            }
        })
    }
}

impl<S4, NI> BSwap for u32x4_sse2<YesS3, S4, NI> {
    #[inline(always)]
    fn bswap(self) -> Self {
        Self::new(unsafe {
            let k = _mm_set_epi64x(0x0c0d_0e0f_0809_0a0b, 0x0405_0607_0001_0203);
            _mm_shuffle_epi8(self.x, k)
        })
    }
}
#[inline(always)]
fn bswap32_s2(x: __m128i) -> __m128i {
    unsafe {
        let mut y = _mm_unpacklo_epi8(x, _mm_setzero_si128());
        y = _mm_shufflehi_epi16(y, 0b0001_1011);
        y = _mm_shufflelo_epi16(y, 0b0001_1011);
        let mut z = _mm_unpackhi_epi8(x, _mm_setzero_si128());
        z = _mm_shufflehi_epi16(z, 0b0001_1011);
        z = _mm_shufflelo_epi16(z, 0b0001_1011);
        _mm_packus_epi16(y, z)
    }
}
impl<S4, NI> BSwap for u32x4_sse2<NoS3, S4, NI> {
    #[inline(always)]
    fn bswap(self) -> Self {
        Self::new(bswap32_s2(self.x))
    }
}

impl<S4, NI> BSwap for u64x2_sse2<YesS3, S4, NI> {
    #[inline(always)]
    fn bswap(self) -> Self {
        Self::new(unsafe {
            let k = _mm_set_epi64x(0x0809_0a0b_0c0d_0e0f, 0x0001_0203_0405_0607);
            _mm_shuffle_epi8(self.x, k)
        })
    }
}
impl<S4, NI> BSwap for u64x2_sse2<NoS3, S4, NI> {
    #[inline(always)]
    fn bswap(self) -> Self {
        Self::new(unsafe { bswap32_s2(_mm_shuffle_epi32(self.x, 0b1011_0001)) })
    }
}

impl<S4, NI> BSwap for u128x1_sse2<YesS3, S4, NI> {
    #[inline(always)]
    fn bswap(self) -> Self {
        Self::new(unsafe {
            let k = _mm_set_epi64x(0x0f0e_0d0c_0b0a_0908, 0x0706_0504_0302_0100);
            _mm_shuffle_epi8(self.x, k)
        })
    }
}
impl<S4, NI> BSwap for u128x1_sse2<NoS3, S4, NI> {
    #[inline(always)]
    fn bswap(self) -> Self {
        unimplemented!()
    }
}

macro_rules! swapi {
    ($x:expr, $i:expr, $k:expr) => {
        unsafe {
            const K: u8 = $k;
            let k = _mm_set1_epi8(K as i8);
            u128x1_sse2::new(_mm_or_si128(
                _mm_srli_epi16(_mm_and_si128($x.x, k), $i),
                _mm_and_si128(_mm_slli_epi16($x.x, $i), k),
            ))
        }
    };
}
#[inline(always)]
fn swap16_s2(x: __m128i) -> __m128i {
    unsafe { _mm_shufflehi_epi16(_mm_shufflelo_epi16(x, 0b1011_0001), 0b1011_0001) }
}
impl<S4, NI> Swap64 for u128x1_sse2<YesS3, S4, NI> {
    #[inline(always)]
    fn swap1(self) -> Self {
        swapi!(self, 1, 0xaa)
    }
    #[inline(always)]
    fn swap2(self) -> Self {
        swapi!(self, 2, 0xcc)
    }
    #[inline(always)]
    fn swap4(self) -> Self {
        swapi!(self, 4, 0xf0)
    }
    #[inline(always)]
    fn swap8(self) -> Self {
        u128x1_sse2::new(unsafe {
            let k = _mm_set_epi64x(0x0e0f_0c0d_0a0b_0809, 0x0607_0405_0203_0001);
            _mm_shuffle_epi8(self.x, k)
        })
    }
    #[inline(always)]
    fn swap16(self) -> Self {
        u128x1_sse2::new(unsafe {
            let k = _mm_set_epi64x(0x0d0c_0f0e_0908_0b0a, 0x0504_0706_0100_0302);
            _mm_shuffle_epi8(self.x, k)
        })
    }
    #[inline(always)]
    fn swap32(self) -> Self {
        u128x1_sse2::new(unsafe { _mm_shuffle_epi32(self.x, 0b1011_0001) })
    }
    #[inline(always)]
    fn swap64(self) -> Self {
        u128x1_sse2::new(unsafe { _mm_shuffle_epi32(self.x, 0b0100_1110) })
    }
}
impl<S4, NI> Swap64 for u128x1_sse2<NoS3, S4, NI> {
    #[inline(always)]
    fn swap1(self) -> Self {
        swapi!(self, 1, 0xaa)
    }
    #[inline(always)]
    fn swap2(self) -> Self {
        swapi!(self, 2, 0xcc)
    }
    #[inline(always)]
    fn swap4(self) -> Self {
        swapi!(self, 4, 0xf0)
    }
    #[inline(always)]
    fn swap8(self) -> Self {
        u128x1_sse2::new(unsafe {
            _mm_or_si128(_mm_slli_epi16(self.x, 8), _mm_srli_epi16(self.x, 8))
        })
    }
    #[inline(always)]
    fn swap16(self) -> Self {
        u128x1_sse2::new(swap16_s2(self.x))
    }
    #[inline(always)]
    fn swap32(self) -> Self {
        u128x1_sse2::new(unsafe { _mm_shuffle_epi32(self.x, 0b1011_0001) })
    }
    #[inline(always)]
    fn swap64(self) -> Self {
        u128x1_sse2::new(unsafe { _mm_shuffle_epi32(self.x, 0b0100_1110) })
    }
}

#[derive(Copy, Clone)]
pub struct G0;
#[derive(Copy, Clone)]
pub struct G1;

#[allow(non_camel_case_types)]
pub type u32x4x2_sse2<S3, S4, NI> = x2<u32x4_sse2<S3, S4, NI>, G0>;
#[allow(non_camel_case_types)]
pub type u64x2x2_sse2<S3, S4, NI> = x2<u64x2_sse2<S3, S4, NI>, G0>;
#[allow(non_camel_case_types)]
pub type u64x4_sse2<S3, S4, NI> = x2<u64x2_sse2<S3, S4, NI>, G1>;
#[allow(non_camel_case_types)]
pub type u128x2_sse2<S3, S4, NI> = x2<u128x1_sse2<S3, S4, NI>, G0>;

#[allow(non_camel_case_types)]
pub type u32x4x4_sse2<S3, S4, NI> = x4<u32x4_sse2<S3, S4, NI>>;
#[allow(non_camel_case_types)]
pub type u64x2x4_sse2<S3, S4, NI> = x4<u64x2_sse2<S3, S4, NI>>;
#[allow(non_camel_case_types)]
pub type u128x4_sse2<S3, S4, NI> = x4<u128x1_sse2<S3, S4, NI>>;

impl<S3, S4, NI> Vector<[u32; 16]> for u32x4x4_sse2<S3, S4, NI> {
    #[inline(always)]
    fn to_scalars(self) -> [u32; 16] {
        transmute!(self)
    }
}

impl<S3: Copy, S4: Copy, NI: Copy> u32x4x2<Machine86<S3, S4, NI>> for u32x4x2_sse2<S3, S4, NI>
where
    u32x4_sse2<S3, S4, NI>: RotateEachWord32 + BSwap,
    Machine86<S3, S4, NI>: Machine,
    u32x4x2_sse2<S3, S4, NI>: MultiLane<[<Machine86<S3, S4, NI> as Machine>::u32x4; 2]>,
    u32x4x2_sse2<S3, S4, NI>: Vec2<<Machine86<S3, S4, NI> as Machine>::u32x4>,
{
}
impl<S3: Copy, S4: Copy, NI: Copy> u64x2x2<Machine86<S3, S4, NI>> for u64x2x2_sse2<S3, S4, NI>
where
    u64x2_sse2<S3, S4, NI>: RotateEachWord64 + RotateEachWord32 + BSwap,
    Machine86<S3, S4, NI>: Machine,
    u64x2x2_sse2<S3, S4, NI>: MultiLane<[<Machine86<S3, S4, NI> as Machine>::u64x2; 2]>,
    u64x2x2_sse2<S3, S4, NI>: Vec2<<Machine86<S3, S4, NI> as Machine>::u64x2>,
{
}
impl<S3: Copy, S4: Copy, NI: Copy> u64x4<Machine86<S3, S4, NI>> for u64x4_sse2<S3, S4, NI>
where
    u64x2_sse2<S3, S4, NI>: RotateEachWord64 + RotateEachWord32 + BSwap,
    Machine86<S3, S4, NI>: Machine,
    u64x4_sse2<S3, S4, NI>: MultiLane<[u64; 4]> + Vec4<u64> + Words4,
{
}
impl<S3: Copy, S4: Copy, NI: Copy> u128x2<Machine86<S3, S4, NI>> for u128x2_sse2<S3, S4, NI>
where
    u128x1_sse2<S3, S4, NI>: Swap64 + BSwap,
    Machine86<S3, S4, NI>: Machine,
    u128x2_sse2<S3, S4, NI>: MultiLane<[<Machine86<S3, S4, NI> as Machine>::u128x1; 2]>,
    u128x2_sse2<S3, S4, NI>: Vec2<<Machine86<S3, S4, NI> as Machine>::u128x1>,
    u128x2_sse2<S3, S4, NI>: Into<<Machine86<S3, S4, NI> as Machine>::u32x4x2>,
    u128x2_sse2<S3, S4, NI>: Into<<Machine86<S3, S4, NI> as Machine>::u64x2x2>,
    u128x2_sse2<S3, S4, NI>: Into<<Machine86<S3, S4, NI> as Machine>::u64x4>,
{
}

impl<NI: Copy> u32x4x2<Avx2Machine<NI>> for u32x4x2_sse2<YesS3, YesS4, NI>
where
    u32x4_sse2<YesS3, YesS4, NI>: RotateEachWord32 + BSwap,
    Avx2Machine<NI>: Machine,
    u32x4x2_sse2<YesS3, YesS4, NI>: MultiLane<[<Avx2Machine<NI> as Machine>::u32x4; 2]>,
    u32x4x2_sse2<YesS3, YesS4, NI>: Vec2<<Avx2Machine<NI> as Machine>::u32x4>,
{
}
impl<NI: Copy> u64x2x2<Avx2Machine<NI>> for u64x2x2_sse2<YesS3, YesS4, NI>
where
    u64x2_sse2<YesS3, YesS4, NI>: RotateEachWord64 + RotateEachWord32 + BSwap,
    Avx2Machine<NI>: Machine,
    u64x2x2_sse2<YesS3, YesS4, NI>: MultiLane<[<Avx2Machine<NI> as Machine>::u64x2; 2]>,
    u64x2x2_sse2<YesS3, YesS4, NI>: Vec2<<Avx2Machine<NI> as Machine>::u64x2>,
{
}
impl<NI: Copy> u64x4<Avx2Machine<NI>> for u64x4_sse2<YesS3, YesS4, NI>
where
    u64x2_sse2<YesS3, YesS4, NI>: RotateEachWord64 + RotateEachWord32 + BSwap,
    Avx2Machine<NI>: Machine,
    u64x4_sse2<YesS3, YesS4, NI>: MultiLane<[u64; 4]> + Vec4<u64> + Words4,
{
}
impl<NI: Copy> u128x2<Avx2Machine<NI>> for u128x2_sse2<YesS3, YesS4, NI>
where
    u128x1_sse2<YesS3, YesS4, NI>: Swap64 + BSwap,
    Avx2Machine<NI>: Machine,
    u128x2_sse2<YesS3, YesS4, NI>: MultiLane<[<Avx2Machine<NI> as Machine>::u128x1; 2]>,
    u128x2_sse2<YesS3, YesS4, NI>: Vec2<<Avx2Machine<NI> as Machine>::u128x1>,
    u128x2_sse2<YesS3, YesS4, NI>: Into<<Avx2Machine<NI> as Machine>::u32x4x2>,
    u128x2_sse2<YesS3, YesS4, NI>: Into<<Avx2Machine<NI> as Machine>::u64x2x2>,
    u128x2_sse2<YesS3, YesS4, NI>: Into<<Avx2Machine<NI> as Machine>::u64x4>,
{
}

impl<S3, S4, NI> Vec4<u64> for u64x4_sse2<S3, S4, NI>
where
    u64x2_sse2<S3, S4, NI>: Copy + Vec2<u64>,
{
    #[inline(always)]
    fn extract(self, i: u32) -> u64 {
        match i {
            0 => self.0[0].extract(0),
            1 => self.0[0].extract(1),
            2 => self.0[1].extract(0),
            3 => self.0[1].extract(1),
            _ => panic!(),
        }
    }
    #[inline(always)]
    fn insert(mut self, w: u64, i: u32) -> Self {
        match i {
            0 => self.0[0] = self.0[0].insert(w, 0),
            1 => self.0[0] = self.0[0].insert(w, 1),
            2 => self.0[1] = self.0[1].insert(w, 0),
            3 => self.0[1] = self.0[1].insert(w, 1),
            _ => panic!(),
        };
        self
    }
}

impl<S3: Copy, S4: Copy, NI: Copy> u32x4x4<Machine86<S3, S4, NI>> for u32x4x4_sse2<S3, S4, NI>
where
    u32x4_sse2<S3, S4, NI>: RotateEachWord32 + BSwap,
    Machine86<S3, S4, NI>: Machine,
    u32x4x4_sse2<S3, S4, NI>: MultiLane<[<Machine86<S3, S4, NI> as Machine>::u32x4; 4]>,
    u32x4x4_sse2<S3, S4, NI>: Vec4<<Machine86<S3, S4, NI> as Machine>::u32x4>,
    u32x4x4_sse2<S3, S4, NI>: Vec4Ext<<Machine86<S3, S4, NI> as Machine>::u32x4>,
    u32x4x4_sse2<S3, S4, NI>: Vector<[u32; 16]>,
{
}
impl<S3: Copy, S4: Copy, NI: Copy> u64x2x4<Machine86<S3, S4, NI>> for u64x2x4_sse2<S3, S4, NI>
where
    u64x2_sse2<S3, S4, NI>: RotateEachWord64 + RotateEachWord32 + BSwap,
    Machine86<S3, S4, NI>: Machine,
    u64x2x4_sse2<S3, S4, NI>: MultiLane<[<Machine86<S3, S4, NI> as Machine>::u64x2; 4]>,
    u64x2x4_sse2<S3, S4, NI>: Vec4<<Machine86<S3, S4, NI> as Machine>::u64x2>,
{
}
impl<S3: Copy, S4: Copy, NI: Copy> u128x4<Machine86<S3, S4, NI>> for u128x4_sse2<S3, S4, NI>
where
    u128x1_sse2<S3, S4, NI>: Swap64 + BSwap,
    Machine86<S3, S4, NI>: Machine,
    u128x4_sse2<S3, S4, NI>: MultiLane<[<Machine86<S3, S4, NI> as Machine>::u128x1; 4]>,
    u128x4_sse2<S3, S4, NI>: Vec4<<Machine86<S3, S4, NI> as Machine>::u128x1>,
    u128x4_sse2<S3, S4, NI>: Into<<Machine86<S3, S4, NI> as Machine>::u32x4x4>,
    u128x4_sse2<S3, S4, NI>: Into<<Machine86<S3, S4, NI> as Machine>::u64x2x4>,
{
}

impl<NI: Copy> u64x2x4<Avx2Machine<NI>> for u64x2x4_sse2<YesS3, YesS4, NI>
where
    u64x2_sse2<YesS3, YesS4, NI>: RotateEachWord64 + RotateEachWord32 + BSwap,
    Avx2Machine<NI>: Machine,
    u64x2x4_sse2<YesS3, YesS4, NI>: MultiLane<[<Avx2Machine<NI> as Machine>::u64x2; 4]>,
    u64x2x4_sse2<YesS3, YesS4, NI>: Vec4<<Avx2Machine<NI> as Machine>::u64x2>,
{
}
impl<NI: Copy> u128x4<Avx2Machine<NI>> for u128x4_sse2<YesS3, YesS4, NI>
where
    u128x1_sse2<YesS3, YesS4, NI>: Swap64 + BSwap,
    Avx2Machine<NI>: Machine,
    u128x4_sse2<YesS3, YesS4, NI>: MultiLane<[<Avx2Machine<NI> as Machine>::u128x1; 4]>,
    u128x4_sse2<YesS3, YesS4, NI>: Vec4<<Avx2Machine<NI> as Machine>::u128x1>,
    u128x4_sse2<YesS3, YesS4, NI>: Into<<Avx2Machine<NI> as Machine>::u32x4x4>,
    u128x4_sse2<YesS3, YesS4, NI>: Into<<Avx2Machine<NI> as Machine>::u64x2x4>,
{
}

macro_rules! impl_into_x {
    ($from:ident, $to:ident) => {
        impl<S3: Copy, S4: Copy, NI: Copy, Gf, Gt> From<x2<$from<S3, S4, NI>, Gf>>
            for x2<$to<S3, S4, NI>, Gt>
        {
            #[inline(always)]
            fn from(x: x2<$from<S3, S4, NI>, Gf>) -> Self {
                x2::new([$to::from(x.0[0]), $to::from(x.0[1])])
            }
        }
        impl<S3: Copy, S4: Copy, NI: Copy> From<x4<$from<S3, S4, NI>>> for x4<$to<S3, S4, NI>> {
            #[inline(always)]
            fn from(x: x4<$from<S3, S4, NI>>) -> Self {
                x4::new([
                    $to::from(x.0[0]),
                    $to::from(x.0[1]),
                    $to::from(x.0[2]),
                    $to::from(x.0[3]),
                ])
            }
        }
    };
}
impl_into_x!(u128x1_sse2, u64x2_sse2);
impl_into_x!(u128x1_sse2, u32x4_sse2);

///// Debugging

use core::fmt::{Debug, Formatter, Result};

impl<W: PartialEq, G> PartialEq for x2<W, G> {
    #[inline(always)]
    fn eq(&self, rhs: &Self) -> bool {
        self.0[0] == rhs.0[0] && self.0[1] == rhs.0[1]
    }
}

#[allow(unused)]
#[inline(always)]
unsafe fn eq128_s4(x: __m128i, y: __m128i) -> bool {
    let q = _mm_shuffle_epi32(_mm_cmpeq_epi64(x, y), 0b1100_0110);
    _mm_cvtsi128_si64(q) == -1
}

#[inline(always)]
unsafe fn eq128_s2(x: __m128i, y: __m128i) -> bool {
    let q = _mm_cmpeq_epi32(x, y);
    let p = _mm_cvtsi128_si64(_mm_srli_si128(q, 8));
    let q = _mm_cvtsi128_si64(q);
    (p & q) == -1
}

impl<S3, S4, NI> PartialEq for u32x4_sse2<S3, S4, NI> {
    #[inline(always)]
    fn eq(&self, rhs: &Self) -> bool {
        unsafe { eq128_s2(self.x, rhs.x) }
    }
}
impl<S3, S4, NI> Debug for u32x4_sse2<S3, S4, NI>
where
    Self: Copy + MultiLane<[u32; 4]>,
{
    #[cold]
    fn fmt(&self, fmt: &mut Formatter) -> Result {
        fmt.write_fmt(format_args!("{:08x?}", &self.to_lanes()))
    }
}

impl<S3, S4, NI> PartialEq for u64x2_sse2<S3, S4, NI> {
    #[inline(always)]
    fn eq(&self, rhs: &Self) -> bool {
        unsafe { eq128_s2(self.x, rhs.x) }
    }
}
impl<S3, S4, NI> Debug for u64x2_sse2<S3, S4, NI>
where
    Self: Copy + MultiLane<[u64; 2]>,
{
    #[cold]
    fn fmt(&self, fmt: &mut Formatter) -> Result {
        fmt.write_fmt(format_args!("{:016x?}", &self.to_lanes()))
    }
}

impl<S3, S4, NI> Debug for u64x4_sse2<S3, S4, NI>
where
    u64x2_sse2<S3, S4, NI>: Copy + MultiLane<[u64; 2]>,
{
    #[cold]
    fn fmt(&self, fmt: &mut Formatter) -> Result {
        let (a, b) = (self.0[0].to_lanes(), self.0[1].to_lanes());
        fmt.write_fmt(format_args!("{:016x?}", &[a[0], a[1], b[0], b[1]]))
    }
}

#[cfg(test)]
#[cfg(target_arch = "x86_64")]
mod test {
    use super::*;
    use crate::x86_64::{SSE2, SSE41, SSSE3};
    use crate::Machine;

    #[test]
    #[cfg_attr(not(target_feature = "ssse3"), ignore)]
    fn test_bswap32_s2_vs_s3() {
        let xs = [0x0f0e_0d0c, 0x0b0a_0908, 0x0706_0504, 0x0302_0100];
        let ys = [0x0c0d_0e0f, 0x0809_0a0b, 0x0405_0607, 0x0001_0203];

        let s2 = unsafe { SSE2::instance() };
        let s3 = unsafe { SSSE3::instance() };

        let x_s2 = {
            let x_s2: <SSE2 as Machine>::u32x4 = s2.vec(xs);
            x_s2.bswap()
        };

        let x_s3 = {
            let x_s3: <SSSE3 as Machine>::u32x4 = s3.vec(xs);
            x_s3.bswap()
        };

        assert_eq!(x_s2, transmute!(x_s3));
        assert_eq!(x_s2, s2.vec(ys));
    }

    #[test]
    #[cfg_attr(not(target_feature = "ssse3"), ignore)]
    fn test_bswap64_s2_vs_s3() {
        let xs = [0x0f0e_0d0c_0b0a_0908, 0x0706_0504_0302_0100];
        let ys = [0x0809_0a0b_0c0d_0e0f, 0x0001_0203_0405_0607];

        let s2 = unsafe { SSE2::instance() };
        let s3 = unsafe { SSSE3::instance() };

        let x_s2 = {
            let x_s2: <SSE2 as Machine>::u64x2 = s2.vec(xs);
            x_s2.bswap()
        };

        let x_s3 = {
            let x_s3: <SSSE3 as Machine>::u64x2 = s3.vec(xs);
            x_s3.bswap()
        };

        assert_eq!(x_s2, s2.vec(ys));
        assert_eq!(x_s3, transmute!(x_s3));
    }

    #[test]
    #[cfg_attr(not(target_feature = "ssse3"), ignore)]
    fn test_shuffle32_s2_vs_s3() {
        let xs = [0x0, 0x1, 0x2, 0x3];
        let ys = [0x2, 0x3, 0x0, 0x1];
        let zs = [0x1, 0x2, 0x3, 0x0];

        let s2 = unsafe { SSE2::instance() };
        let s3 = unsafe { SSSE3::instance() };

        let x_s2 = {
            let x_s2: <SSE2 as Machine>::u32x4 = s2.vec(xs);
            x_s2.shuffle2301()
        };
        let x_s3 = {
            let x_s3: <SSSE3 as Machine>::u32x4 = s3.vec(xs);
            x_s3.shuffle2301()
        };
        assert_eq!(x_s2, s2.vec(ys));
        assert_eq!(x_s3, transmute!(x_s3));

        let x_s2 = {
            let x_s2: <SSE2 as Machine>::u32x4 = s2.vec(xs);
            x_s2.shuffle3012()
        };
        let x_s3 = {
            let x_s3: <SSSE3 as Machine>::u32x4 = s3.vec(xs);
            x_s3.shuffle3012()
        };
        assert_eq!(x_s2, s2.vec(zs));
        assert_eq!(x_s3, transmute!(x_s3));

        let x_s2 = x_s2.shuffle1230();
        let x_s3 = x_s3.shuffle1230();
        assert_eq!(x_s2, s2.vec(xs));
        assert_eq!(x_s3, transmute!(x_s3));
    }

    #[test]
    #[cfg_attr(not(target_feature = "ssse3"), ignore)]
    fn test_shuffle64_s2_vs_s3() {
        let xs = [0x0, 0x1, 0x2, 0x3];
        let ys = [0x2, 0x3, 0x0, 0x1];
        let zs = [0x1, 0x2, 0x3, 0x0];

        let s2 = unsafe { SSE2::instance() };
        let s3 = unsafe { SSSE3::instance() };

        let x_s2 = {
            let x_s2: <SSE2 as Machine>::u64x4 = s2.vec(xs);
            x_s2.shuffle2301()
        };
        let x_s3 = {
            let x_s3: <SSSE3 as Machine>::u64x4 = s3.vec(xs);
            x_s3.shuffle2301()
        };
        assert_eq!(x_s2, s2.vec(ys));
        assert_eq!(x_s3, transmute!(x_s3));

        let x_s2 = {
            let x_s2: <SSE2 as Machine>::u64x4 = s2.vec(xs);
            x_s2.shuffle3012()
        };
        let x_s3 = {
            let x_s3: <SSSE3 as Machine>::u64x4 = s3.vec(xs);
            x_s3.shuffle3012()
        };
        assert_eq!(x_s2, s2.vec(zs));
        assert_eq!(x_s3, transmute!(x_s3));

        let x_s2 = x_s2.shuffle1230();
        let x_s3 = x_s3.shuffle1230();
        assert_eq!(x_s2, s2.vec(xs));
        assert_eq!(x_s3, transmute!(x_s3));
    }

    #[cfg_attr(not(all(target_feature = "ssse3", target_feature = "sse4.1")), ignore)]
    #[test]
    fn test_lanes_u32x4() {
        let xs = [0x1, 0x2, 0x3, 0x4];

        let s2 = unsafe { SSE2::instance() };
        let s3 = unsafe { SSSE3::instance() };
        let s4 = unsafe { SSE41::instance() };

        {
            let x_s2: <SSE2 as Machine>::u32x4 = s2.vec(xs);
            let y_s2 = <SSE2 as Machine>::u32x4::from_lanes(xs);
            assert_eq!(x_s2, y_s2);
            assert_eq!(xs, y_s2.to_lanes());
        }

        {
            let x_s3: <SSSE3 as Machine>::u32x4 = s3.vec(xs);
            let y_s3 = <SSSE3 as Machine>::u32x4::from_lanes(xs);
            assert_eq!(x_s3, y_s3);
            assert_eq!(xs, y_s3.to_lanes());
        }

        {
            let x_s4: <SSE41 as Machine>::u32x4 = s4.vec(xs);
            let y_s4 = <SSE41 as Machine>::u32x4::from_lanes(xs);
            assert_eq!(x_s4, y_s4);
            assert_eq!(xs, y_s4.to_lanes());
        }
    }

    #[test]
    #[cfg_attr(not(all(target_feature = "ssse3", target_feature = "sse4.1")), ignore)]
    fn test_lanes_u64x2() {
        let xs = [0x1, 0x2];

        let s2 = unsafe { SSE2::instance() };
        let s3 = unsafe { SSSE3::instance() };
        let s4 = unsafe { SSE41::instance() };

        {
            let x_s2: <SSE2 as Machine>::u64x2 = s2.vec(xs);
            let y_s2 = <SSE2 as Machine>::u64x2::from_lanes(xs);
            assert_eq!(x_s2, y_s2);
            assert_eq!(xs, y_s2.to_lanes());
        }

        {
            let x_s3: <SSSE3 as Machine>::u64x2 = s3.vec(xs);
            let y_s3 = <SSSE3 as Machine>::u64x2::from_lanes(xs);
            assert_eq!(x_s3, y_s3);
            assert_eq!(xs, y_s3.to_lanes());
        }

        {
            let x_s4: <SSE41 as Machine>::u64x2 = s4.vec(xs);
            let y_s4 = <SSE41 as Machine>::u64x2::from_lanes(xs);
            assert_eq!(x_s4, y_s4);
            assert_eq!(xs, y_s4.to_lanes());
        }
    }

    #[test]
    fn test_vec4_u32x4_s2() {
        let xs = [1, 2, 3, 4];
        let s2 = unsafe { SSE2::instance() };
        let x_s2: <SSE2 as Machine>::u32x4 = s2.vec(xs);
        assert_eq!(x_s2.extract(0), 1);
        assert_eq!(x_s2.extract(1), 2);
        assert_eq!(x_s2.extract(2), 3);
        assert_eq!(x_s2.extract(3), 4);
        assert_eq!(x_s2.insert(0xf, 0), s2.vec([0xf, 2, 3, 4]));
        assert_eq!(x_s2.insert(0xf, 1), s2.vec([1, 0xf, 3, 4]));
        assert_eq!(x_s2.insert(0xf, 2), s2.vec([1, 2, 0xf, 4]));
        assert_eq!(x_s2.insert(0xf, 3), s2.vec([1, 2, 3, 0xf]));
    }

    #[test]
    #[cfg_attr(not(all(target_feature = "ssse3", target_feature = "sse4.1")), ignore)]
    fn test_vec4_u32x4_s4() {
        let xs = [1, 2, 3, 4];
        let s4 = unsafe { SSE41::instance() };
        let x_s4: <SSE41 as Machine>::u32x4 = s4.vec(xs);
        assert_eq!(x_s4.extract(0), 1);
        assert_eq!(x_s4.extract(1), 2);
        assert_eq!(x_s4.extract(2), 3);
        assert_eq!(x_s4.extract(3), 4);
        assert_eq!(x_s4.insert(0xf, 0), s4.vec([0xf, 2, 3, 4]));
        assert_eq!(x_s4.insert(0xf, 1), s4.vec([1, 0xf, 3, 4]));
        assert_eq!(x_s4.insert(0xf, 2), s4.vec([1, 2, 0xf, 4]));
        assert_eq!(x_s4.insert(0xf, 3), s4.vec([1, 2, 3, 0xf]));
    }

    #[test]
    fn test_vec2_u64x2_s2() {
        let xs = [0x1, 0x2];
        let s2 = unsafe { SSE2::instance() };
        let x_s2: <SSE2 as Machine>::u64x2 = s2.vec(xs);
        assert_eq!(x_s2.extract(0), 1);
        assert_eq!(x_s2.extract(1), 2);
        assert_eq!(x_s2.insert(0xf, 0), s2.vec([0xf, 2]));
        assert_eq!(x_s2.insert(0xf, 1), s2.vec([1, 0xf]));
    }

    #[test]
    #[cfg_attr(not(all(target_feature = "ssse3", target_feature = "sse4.1")), ignore)]
    fn test_vec4_u64x2_s4() {
        let xs = [0x1, 0x2];
        let s4 = unsafe { SSE41::instance() };
        let x_s4: <SSE41 as Machine>::u64x2 = s4.vec(xs);
        assert_eq!(x_s4.extract(0), 1);
        assert_eq!(x_s4.extract(1), 2);
        assert_eq!(x_s4.insert(0xf, 0), s4.vec([0xf, 2]));
        assert_eq!(x_s4.insert(0xf, 1), s4.vec([1, 0xf]));
    }
}

pub mod avx2 {
    #![allow(non_camel_case_types)]
    use crate::soft::{x2, x4};
    use crate::types::*;
    use crate::x86_64::sse2::{u128x1_sse2, u32x4_sse2, G0};
    use crate::x86_64::{vec256_storage, vec512_storage, Avx2Machine, YesS3, YesS4};
    use core::arch::x86_64::*;
    use core::marker::PhantomData;
    use core::ops::*;
    use zerocopy::transmute;

    zerocopy::cryptocorrosion_derive_traits! {
        #[repr(transparent)]
        #[derive(Copy, Clone)]
        pub struct u32x4x2_avx2<NI> {
            x: __m256i,
            ni: PhantomData<NI>,
        }
    }

    impl<NI> u32x4x2_avx2<NI> {
        #[inline(always)]
        fn new(x: __m256i) -> Self {
            Self { x, ni: PhantomData }
        }
    }

    impl<NI> u32x4x2<Avx2Machine<NI>> for u32x4x2_avx2<NI> where NI: Copy {}
    impl<NI> Store<vec256_storage> for u32x4x2_avx2<NI> {
        #[inline(always)]
        unsafe fn unpack(p: vec256_storage) -> Self {
            Self::new(p.avx)
        }
    }
    impl<NI> StoreBytes for u32x4x2_avx2<NI> {
        #[inline(always)]
        unsafe fn unsafe_read_le(input: &[u8]) -> Self {
            assert_eq!(input.len(), 32);
            Self::new(_mm256_loadu_si256(input.as_ptr() as *const _))
        }
        #[inline(always)]
        unsafe fn unsafe_read_be(input: &[u8]) -> Self {
            Self::unsafe_read_le(input).bswap()
        }
        #[inline(always)]
        fn write_le(self, out: &mut [u8]) {
            unsafe {
                assert_eq!(out.len(), 32);
                _mm256_storeu_si256(out.as_mut_ptr() as *mut _, self.x)
            }
        }
        #[inline(always)]
        fn write_be(self, out: &mut [u8]) {
            self.bswap().write_le(out)
        }
    }
    impl<NI> MultiLane<[u32x4_sse2<YesS3, YesS4, NI>; 2]> for u32x4x2_avx2<NI> {
        #[inline(always)]
        fn to_lanes(self) -> [u32x4_sse2<YesS3, YesS4, NI>; 2] {
            unsafe {
                [
                    u32x4_sse2::new(_mm256_extracti128_si256(self.x, 0)),
                    u32x4_sse2::new(_mm256_extracti128_si256(self.x, 1)),
                ]
            }
        }
        #[inline(always)]
        fn from_lanes(x: [u32x4_sse2<YesS3, YesS4, NI>; 2]) -> Self {
            Self::new(unsafe { _mm256_setr_m128i(x[0].x, x[1].x) })
        }
    }
    impl<NI> Vec2<u32x4_sse2<YesS3, YesS4, NI>> for u32x4x2_avx2<NI> {
        #[inline(always)]
        fn extract(self, i: u32) -> u32x4_sse2<YesS3, YesS4, NI> {
            unsafe {
                match i {
                    0 => u32x4_sse2::new(_mm256_extracti128_si256(self.x, 0)),
                    1 => u32x4_sse2::new(_mm256_extracti128_si256(self.x, 1)),
                    _ => panic!(),
                }
            }
        }
        #[inline(always)]
        fn insert(self, w: u32x4_sse2<YesS3, YesS4, NI>, i: u32) -> Self {
            Self::new(unsafe {
                match i {
                    0 => _mm256_inserti128_si256(self.x, w.x, 0),
                    1 => _mm256_inserti128_si256(self.x, w.x, 1),
                    _ => panic!(),
                }
            })
        }
    }
    impl<NI> BitOps32 for u32x4x2_avx2<NI> where NI: Copy {}
    impl<NI> ArithOps for u32x4x2_avx2<NI> where NI: Copy {}
    macro_rules! shuf_lane_bytes {
        ($name:ident, $k0:expr, $k1:expr) => {
            #[inline(always)]
            fn $name(self) -> Self {
                Self::new(unsafe {
                    _mm256_shuffle_epi8(self.x, _mm256_set_epi64x($k0, $k1, $k0, $k1))
                })
            }
        };
    }
    macro_rules! rotr_32 {
        ($name:ident, $i:expr) => {
            #[inline(always)]
            fn $name(self) -> Self {
                Self::new(unsafe {
                    _mm256_or_si256(
                        _mm256_srli_epi32(self.x, $i as i32),
                        _mm256_slli_epi32(self.x, 32 - $i as i32),
                    )
                })
            }
        };
    }
    impl<NI: Copy> RotateEachWord32 for u32x4x2_avx2<NI> {
        rotr_32!(rotate_each_word_right7, 7);
        shuf_lane_bytes!(
            rotate_each_word_right8,
            0x0c0f_0e0d_080b_0a09,
            0x0407_0605_0003_0201
        );
        rotr_32!(rotate_each_word_right11, 11);
        rotr_32!(rotate_each_word_right12, 12);
        shuf_lane_bytes!(
            rotate_each_word_right16,
            0x0d0c_0f0e_0908_0b0a,
            0x0504_0706_0100_0302
        );
        rotr_32!(rotate_each_word_right20, 20);
        shuf_lane_bytes!(
            rotate_each_word_right24,
            0x0e0d_0c0f_0a09_080b,
            0x0605_0407_0201_0003
        );
        rotr_32!(rotate_each_word_right25, 25);
    }
    impl<NI> BitOps0 for u32x4x2_avx2<NI> where NI: Copy {}
    impl<NI> From<u32x4x2_avx2<NI>> for vec256_storage {
        #[inline(always)]
        fn from(x: u32x4x2_avx2<NI>) -> Self {
            Self { avx: x.x }
        }
    }

    macro_rules! impl_assign {
        ($vec:ident, $Assign:ident, $assign_fn:ident, $bin_fn:ident) => {
            impl<NI> $Assign for $vec<NI>
            where
                NI: Copy,
            {
                #[inline(always)]
                fn $assign_fn(&mut self, rhs: Self) {
                    *self = self.$bin_fn(rhs);
                }
            }
        };
    }
    impl_assign!(u32x4x2_avx2, BitXorAssign, bitxor_assign, bitxor);
    impl_assign!(u32x4x2_avx2, BitOrAssign, bitor_assign, bitor);
    impl_assign!(u32x4x2_avx2, BitAndAssign, bitand_assign, bitand);
    impl_assign!(u32x4x2_avx2, AddAssign, add_assign, add);

    macro_rules! impl_bitop {
        ($vec:ident, $Op:ident, $op_fn:ident, $impl_fn:ident) => {
            impl<NI> $Op for $vec<NI> {
                type Output = Self;
                #[inline(always)]
                fn $op_fn(self, rhs: Self) -> Self::Output {
                    Self::new(unsafe { $impl_fn(self.x, rhs.x) })
                }
            }
        };
    }
    impl_bitop!(u32x4x2_avx2, BitXor, bitxor, _mm256_xor_si256);
    impl_bitop!(u32x4x2_avx2, BitOr, bitor, _mm256_or_si256);
    impl_bitop!(u32x4x2_avx2, BitAnd, bitand, _mm256_and_si256);
    impl_bitop!(u32x4x2_avx2, AndNot, andnot, _mm256_andnot_si256);
    impl_bitop!(u32x4x2_avx2, Add, add, _mm256_add_epi32);

    impl<NI> Not for u32x4x2_avx2<NI> {
        type Output = Self;
        #[inline(always)]
        fn not(self) -> Self::Output {
            unsafe {
                let f = _mm256_set1_epi8(-0x7f);
                Self::new(f) ^ self
            }
        }
    }

    impl<NI> BSwap for u32x4x2_avx2<NI> {
        shuf_lane_bytes!(bswap, 0x0c0d_0e0f_0809_0a0b, 0x0405_0607_0001_0203);
    }

    impl<NI> From<x2<u128x1_sse2<YesS3, YesS4, NI>, G0>> for u32x4x2_avx2<NI>
    where
        NI: Copy,
    {
        #[inline(always)]
        fn from(x: x2<u128x1_sse2<YesS3, YesS4, NI>, G0>) -> Self {
            Self::new(unsafe { _mm256_setr_m128i(x.0[0].x, x.0[1].x) })
        }
    }

    impl<NI> LaneWords4 for u32x4x2_avx2<NI> {
        #[inline(always)]
        fn shuffle_lane_words1230(self) -> Self {
            Self::new(unsafe { _mm256_shuffle_epi32(self.x, 0b1001_0011) })
        }
        #[inline(always)]
        fn shuffle_lane_words2301(self) -> Self {
            Self::new(unsafe { _mm256_shuffle_epi32(self.x, 0b0100_1110) })
        }
        #[inline(always)]
        fn shuffle_lane_words3012(self) -> Self {
            Self::new(unsafe { _mm256_shuffle_epi32(self.x, 0b0011_1001) })
        }
    }

    ///////////////////////////////////////////////////////////////////////////////////////////

    pub type u32x4x4_avx2<NI> = x2<u32x4x2_avx2<NI>, G0>;
    impl<NI: Copy> u32x4x4<Avx2Machine<NI>> for u32x4x4_avx2<NI> {}

    impl<NI: Copy> Store<vec512_storage> for u32x4x4_avx2<NI> {
        #[inline(always)]
        unsafe fn unpack(p: vec512_storage) -> Self {
            Self::new([
                u32x4x2_avx2::unpack(p.avx[0]),
                u32x4x2_avx2::unpack(p.avx[1]),
            ])
        }
    }
    impl<NI: Copy> MultiLane<[u32x4_sse2<YesS3, YesS4, NI>; 4]> for u32x4x4_avx2<NI> {
        #[inline(always)]
        fn to_lanes(self) -> [u32x4_sse2<YesS3, YesS4, NI>; 4] {
            let [a, b] = self.0[0].to_lanes();
            let [c, d] = self.0[1].to_lanes();
            [a, b, c, d]
        }
        #[inline(always)]
        fn from_lanes(x: [u32x4_sse2<YesS3, YesS4, NI>; 4]) -> Self {
            let ab = u32x4x2_avx2::from_lanes([x[0], x[1]]);
            let cd = u32x4x2_avx2::from_lanes([x[2], x[3]]);
            Self::new([ab, cd])
        }
    }
    impl<NI: Copy> Vec4<u32x4_sse2<YesS3, YesS4, NI>> for u32x4x4_avx2<NI> {
        #[inline(always)]
        fn extract(self, i: u32) -> u32x4_sse2<YesS3, YesS4, NI> {
            match i {
                0 => self.0[0].extract(0),
                1 => self.0[0].extract(1),
                2 => self.0[1].extract(0),
                3 => self.0[1].extract(1),
                _ => panic!(),
            }
        }
        #[inline(always)]
        fn insert(self, w: u32x4_sse2<YesS3, YesS4, NI>, i: u32) -> Self {
            Self::new(match i {
                0 | 1 => [self.0[0].insert(w, i), self.0[1]],
                2 | 3 => [self.0[0], self.0[1].insert(w, i - 2)],
                _ => panic!(),
            })
        }
    }
    impl<NI: Copy> Vec4Ext<u32x4_sse2<YesS3, YesS4, NI>> for u32x4x4_avx2<NI> {
        #[inline(always)]
        fn transpose4(a: Self, b: Self, c: Self, d: Self) -> (Self, Self, Self, Self) {
            /*
             * a00:a01 a10:a11
             * b00:b01 b10:b11
             * c00:c01 c10:c11
             * d00:d01 d10:d11
             *       =>
             * a00:b00 c00:d00
             * a01:b01 c01:d01
             * a10:b10 c10:d10
             * a11:b11 c11:d11
             */
            unsafe {
                let ab00 = u32x4x2_avx2::new(_mm256_permute2x128_si256(a.0[0].x, b.0[0].x, 0x20));
                let ab01 = u32x4x2_avx2::new(_mm256_permute2x128_si256(a.0[0].x, b.0[0].x, 0x31));
                let ab10 = u32x4x2_avx2::new(_mm256_permute2x128_si256(a.0[1].x, b.0[1].x, 0x20));
                let ab11 = u32x4x2_avx2::new(_mm256_permute2x128_si256(a.0[1].x, b.0[1].x, 0x31));
                let cd00 = u32x4x2_avx2::new(_mm256_permute2x128_si256(c.0[0].x, d.0[0].x, 0x20));
                let cd01 = u32x4x2_avx2::new(_mm256_permute2x128_si256(c.0[0].x, d.0[0].x, 0x31));
                let cd10 = u32x4x2_avx2::new(_mm256_permute2x128_si256(c.0[1].x, d.0[1].x, 0x20));
                let cd11 = u32x4x2_avx2::new(_mm256_permute2x128_si256(c.0[1].x, d.0[1].x, 0x31));
                (
                    Self::new([ab00, cd00]),
                    Self::new([ab01, cd01]),
                    Self::new([ab10, cd10]),
                    Self::new([ab11, cd11]),
                )
            }
        }
    }
    impl<NI: Copy> Vector<[u32; 16]> for u32x4x4_avx2<NI> {
        #[inline(always)]
        fn to_scalars(self) -> [u32; 16] {
            transmute!(self)
        }
    }
    impl<NI: Copy> From<u32x4x4_avx2<NI>> for vec512_storage {
        #[inline(always)]
        fn from(x: u32x4x4_avx2<NI>) -> Self {
            Self {
                avx: [
                    vec256_storage { avx: x.0[0].x },
                    vec256_storage { avx: x.0[1].x },
                ],
            }
        }
    }
    impl<NI: Copy> From<x4<u128x1_sse2<YesS3, YesS4, NI>>> for u32x4x4_avx2<NI> {
        #[inline(always)]
        fn from(x: x4<u128x1_sse2<YesS3, YesS4, NI>>) -> Self {
            Self::new(unsafe {
                [
                    u32x4x2_avx2::new(_mm256_setr_m128i(x.0[0].x, x.0[1].x)),
                    u32x4x2_avx2::new(_mm256_setr_m128i(x.0[2].x, x.0[3].x)),
                ]
            })
        }
    }
}
