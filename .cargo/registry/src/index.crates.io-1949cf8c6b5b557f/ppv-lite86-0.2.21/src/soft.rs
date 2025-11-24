//! Implement 256- and 512- bit in terms of 128-bit, for machines without native wide SIMD.

use crate::types::*;
use crate::{vec128_storage, vec256_storage, vec512_storage};
use core::marker::PhantomData;
use core::ops::*;

zerocopy::cryptocorrosion_derive_traits! {
    #[repr(transparent)]
    #[derive(Copy, Clone, Default)]
    #[allow(non_camel_case_types)]
    pub struct x2<W, G>(pub [W; 2], PhantomData<G>);
}

impl<W, G> x2<W, G> {
    #[inline(always)]
    pub fn new(xs: [W; 2]) -> Self {
        x2(xs, PhantomData)
    }
}
macro_rules! fwd_binop_x2 {
    ($trait:ident, $fn:ident) => {
        impl<W: $trait + Copy, G> $trait for x2<W, G> {
            type Output = x2<W::Output, G>;
            #[inline(always)]
            fn $fn(self, rhs: Self) -> Self::Output {
                x2::new([self.0[0].$fn(rhs.0[0]), self.0[1].$fn(rhs.0[1])])
            }
        }
    };
}
macro_rules! fwd_binop_assign_x2 {
    ($trait:ident, $fn_assign:ident) => {
        impl<W: $trait + Copy, G> $trait for x2<W, G> {
            #[inline(always)]
            fn $fn_assign(&mut self, rhs: Self) {
                (self.0[0]).$fn_assign(rhs.0[0]);
                (self.0[1]).$fn_assign(rhs.0[1]);
            }
        }
    };
}
macro_rules! fwd_unop_x2 {
    ($fn:ident) => {
        #[inline(always)]
        fn $fn(self) -> Self {
            x2::new([self.0[0].$fn(), self.0[1].$fn()])
        }
    };
}
impl<W, G> RotateEachWord32 for x2<W, G>
where
    W: Copy + RotateEachWord32,
{
    fwd_unop_x2!(rotate_each_word_right7);
    fwd_unop_x2!(rotate_each_word_right8);
    fwd_unop_x2!(rotate_each_word_right11);
    fwd_unop_x2!(rotate_each_word_right12);
    fwd_unop_x2!(rotate_each_word_right16);
    fwd_unop_x2!(rotate_each_word_right20);
    fwd_unop_x2!(rotate_each_word_right24);
    fwd_unop_x2!(rotate_each_word_right25);
}
impl<W, G> RotateEachWord64 for x2<W, G>
where
    W: Copy + RotateEachWord64,
{
    fwd_unop_x2!(rotate_each_word_right32);
}
impl<W, G> RotateEachWord128 for x2<W, G> where W: RotateEachWord128 {}
impl<W, G> BitOps0 for x2<W, G>
where
    W: BitOps0,
    G: Copy,
{
}
impl<W, G> BitOps32 for x2<W, G>
where
    W: BitOps32 + BitOps0,
    G: Copy,
{
}
impl<W, G> BitOps64 for x2<W, G>
where
    W: BitOps64 + BitOps0,
    G: Copy,
{
}
impl<W, G> BitOps128 for x2<W, G>
where
    W: BitOps128 + BitOps0,
    G: Copy,
{
}
fwd_binop_x2!(BitAnd, bitand);
fwd_binop_x2!(BitOr, bitor);
fwd_binop_x2!(BitXor, bitxor);
fwd_binop_x2!(AndNot, andnot);
fwd_binop_assign_x2!(BitAndAssign, bitand_assign);
fwd_binop_assign_x2!(BitOrAssign, bitor_assign);
fwd_binop_assign_x2!(BitXorAssign, bitxor_assign);
impl<W, G> ArithOps for x2<W, G>
where
    W: ArithOps,
    G: Copy,
{
}
fwd_binop_x2!(Add, add);
fwd_binop_assign_x2!(AddAssign, add_assign);
impl<W: Not + Copy, G> Not for x2<W, G> {
    type Output = x2<W::Output, G>;
    #[inline(always)]
    fn not(self) -> Self::Output {
        x2::new([self.0[0].not(), self.0[1].not()])
    }
}
impl<W, G> UnsafeFrom<[W; 2]> for x2<W, G> {
    #[inline(always)]
    unsafe fn unsafe_from(xs: [W; 2]) -> Self {
        x2::new(xs)
    }
}
impl<W: Copy, G> Vec2<W> for x2<W, G> {
    #[inline(always)]
    fn extract(self, i: u32) -> W {
        self.0[i as usize]
    }
    #[inline(always)]
    fn insert(mut self, w: W, i: u32) -> Self {
        self.0[i as usize] = w;
        self
    }
}
impl<W: Copy + Store<vec128_storage>, G> Store<vec256_storage> for x2<W, G> {
    #[inline(always)]
    unsafe fn unpack(p: vec256_storage) -> Self {
        let p = p.split128();
        x2::new([W::unpack(p[0]), W::unpack(p[1])])
    }
}
impl<W, G> From<x2<W, G>> for vec256_storage
where
    W: Copy,
    vec128_storage: From<W>,
{
    #[inline(always)]
    fn from(x: x2<W, G>) -> Self {
        vec256_storage::new128([x.0[0].into(), x.0[1].into()])
    }
}
impl<W, G> Swap64 for x2<W, G>
where
    W: Swap64 + Copy,
{
    fwd_unop_x2!(swap1);
    fwd_unop_x2!(swap2);
    fwd_unop_x2!(swap4);
    fwd_unop_x2!(swap8);
    fwd_unop_x2!(swap16);
    fwd_unop_x2!(swap32);
    fwd_unop_x2!(swap64);
}
impl<W: Copy, G> MultiLane<[W; 2]> for x2<W, G> {
    #[inline(always)]
    fn to_lanes(self) -> [W; 2] {
        self.0
    }
    #[inline(always)]
    fn from_lanes(lanes: [W; 2]) -> Self {
        x2::new(lanes)
    }
}
impl<W: BSwap + Copy, G> BSwap for x2<W, G> {
    #[inline(always)]
    fn bswap(self) -> Self {
        x2::new([self.0[0].bswap(), self.0[1].bswap()])
    }
}
impl<W: StoreBytes + BSwap + Copy, G> StoreBytes for x2<W, G> {
    #[inline(always)]
    unsafe fn unsafe_read_le(input: &[u8]) -> Self {
        let input = input.split_at(input.len() / 2);
        x2::new([W::unsafe_read_le(input.0), W::unsafe_read_le(input.1)])
    }
    #[inline(always)]
    unsafe fn unsafe_read_be(input: &[u8]) -> Self {
        let input = input.split_at(input.len() / 2);
        x2::new([W::unsafe_read_be(input.0), W::unsafe_read_be(input.1)])
    }
    #[inline(always)]
    fn write_le(self, out: &mut [u8]) {
        let out = out.split_at_mut(out.len() / 2);
        self.0[0].write_le(out.0);
        self.0[1].write_le(out.1);
    }
    #[inline(always)]
    fn write_be(self, out: &mut [u8]) {
        let out = out.split_at_mut(out.len() / 2);
        self.0[0].write_be(out.0);
        self.0[1].write_be(out.1);
    }
}
impl<W: Copy + LaneWords4, G: Copy> LaneWords4 for x2<W, G> {
    #[inline(always)]
    fn shuffle_lane_words2301(self) -> Self {
        Self::new([
            self.0[0].shuffle_lane_words2301(),
            self.0[1].shuffle_lane_words2301(),
        ])
    }
    #[inline(always)]
    fn shuffle_lane_words1230(self) -> Self {
        Self::new([
            self.0[0].shuffle_lane_words1230(),
            self.0[1].shuffle_lane_words1230(),
        ])
    }
    #[inline(always)]
    fn shuffle_lane_words3012(self) -> Self {
        Self::new([
            self.0[0].shuffle_lane_words3012(),
            self.0[1].shuffle_lane_words3012(),
        ])
    }
}

zerocopy::cryptocorrosion_derive_traits! {
    #[repr(transparent)]
    #[derive(Copy, Clone, Default)]
    #[allow(non_camel_case_types)]
    pub struct x4<W>(pub [W; 4]);
}

impl<W> x4<W> {
    #[inline(always)]
    pub fn new(xs: [W; 4]) -> Self {
        x4(xs)
    }
}
macro_rules! fwd_binop_x4 {
    ($trait:ident, $fn:ident) => {
        impl<W: $trait + Copy> $trait for x4<W> {
            type Output = x4<W::Output>;
            #[inline(always)]
            fn $fn(self, rhs: Self) -> Self::Output {
                x4([
                    self.0[0].$fn(rhs.0[0]),
                    self.0[1].$fn(rhs.0[1]),
                    self.0[2].$fn(rhs.0[2]),
                    self.0[3].$fn(rhs.0[3]),
                ])
            }
        }
    };
}
macro_rules! fwd_binop_assign_x4 {
    ($trait:ident, $fn_assign:ident) => {
        impl<W: $trait + Copy> $trait for x4<W> {
            #[inline(always)]
            fn $fn_assign(&mut self, rhs: Self) {
                self.0[0].$fn_assign(rhs.0[0]);
                self.0[1].$fn_assign(rhs.0[1]);
                self.0[2].$fn_assign(rhs.0[2]);
                self.0[3].$fn_assign(rhs.0[3]);
            }
        }
    };
}
macro_rules! fwd_unop_x4 {
    ($fn:ident) => {
        #[inline(always)]
        fn $fn(self) -> Self {
            x4([
                self.0[0].$fn(),
                self.0[1].$fn(),
                self.0[2].$fn(),
                self.0[3].$fn(),
            ])
        }
    };
}
impl<W> RotateEachWord32 for x4<W>
where
    W: Copy + RotateEachWord32,
{
    fwd_unop_x4!(rotate_each_word_right7);
    fwd_unop_x4!(rotate_each_word_right8);
    fwd_unop_x4!(rotate_each_word_right11);
    fwd_unop_x4!(rotate_each_word_right12);
    fwd_unop_x4!(rotate_each_word_right16);
    fwd_unop_x4!(rotate_each_word_right20);
    fwd_unop_x4!(rotate_each_word_right24);
    fwd_unop_x4!(rotate_each_word_right25);
}
impl<W> RotateEachWord64 for x4<W>
where
    W: Copy + RotateEachWord64,
{
    fwd_unop_x4!(rotate_each_word_right32);
}
impl<W> RotateEachWord128 for x4<W> where W: RotateEachWord128 {}
impl<W> BitOps0 for x4<W> where W: BitOps0 {}
impl<W> BitOps32 for x4<W> where W: BitOps32 + BitOps0 {}
impl<W> BitOps64 for x4<W> where W: BitOps64 + BitOps0 {}
impl<W> BitOps128 for x4<W> where W: BitOps128 + BitOps0 {}
fwd_binop_x4!(BitAnd, bitand);
fwd_binop_x4!(BitOr, bitor);
fwd_binop_x4!(BitXor, bitxor);
fwd_binop_x4!(AndNot, andnot);
fwd_binop_assign_x4!(BitAndAssign, bitand_assign);
fwd_binop_assign_x4!(BitOrAssign, bitor_assign);
fwd_binop_assign_x4!(BitXorAssign, bitxor_assign);
impl<W> ArithOps for x4<W> where W: ArithOps {}
fwd_binop_x4!(Add, add);
fwd_binop_assign_x4!(AddAssign, add_assign);
impl<W: Not + Copy> Not for x4<W> {
    type Output = x4<W::Output>;
    #[inline(always)]
    fn not(self) -> Self::Output {
        x4([
            self.0[0].not(),
            self.0[1].not(),
            self.0[2].not(),
            self.0[3].not(),
        ])
    }
}
impl<W> UnsafeFrom<[W; 4]> for x4<W> {
    #[inline(always)]
    unsafe fn unsafe_from(xs: [W; 4]) -> Self {
        x4(xs)
    }
}
impl<W: Copy> Vec4<W> for x4<W> {
    #[inline(always)]
    fn extract(self, i: u32) -> W {
        self.0[i as usize]
    }
    #[inline(always)]
    fn insert(mut self, w: W, i: u32) -> Self {
        self.0[i as usize] = w;
        self
    }
}
impl<W: Copy> Vec4Ext<W> for x4<W> {
    #[inline(always)]
    fn transpose4(a: Self, b: Self, c: Self, d: Self) -> (Self, Self, Self, Self)
    where
        Self: Sized,
    {
        (
            x4([a.0[0], b.0[0], c.0[0], d.0[0]]),
            x4([a.0[1], b.0[1], c.0[1], d.0[1]]),
            x4([a.0[2], b.0[2], c.0[2], d.0[2]]),
            x4([a.0[3], b.0[3], c.0[3], d.0[3]]),
        )
    }
}
impl<W: Copy + Store<vec128_storage>> Store<vec512_storage> for x4<W> {
    #[inline(always)]
    unsafe fn unpack(p: vec512_storage) -> Self {
        let p = p.split128();
        x4([
            W::unpack(p[0]),
            W::unpack(p[1]),
            W::unpack(p[2]),
            W::unpack(p[3]),
        ])
    }
}
impl<W> From<x4<W>> for vec512_storage
where
    W: Copy,
    vec128_storage: From<W>,
{
    #[inline(always)]
    fn from(x: x4<W>) -> Self {
        vec512_storage::new128([x.0[0].into(), x.0[1].into(), x.0[2].into(), x.0[3].into()])
    }
}
impl<W> Swap64 for x4<W>
where
    W: Swap64 + Copy,
{
    fwd_unop_x4!(swap1);
    fwd_unop_x4!(swap2);
    fwd_unop_x4!(swap4);
    fwd_unop_x4!(swap8);
    fwd_unop_x4!(swap16);
    fwd_unop_x4!(swap32);
    fwd_unop_x4!(swap64);
}
impl<W: Copy> MultiLane<[W; 4]> for x4<W> {
    #[inline(always)]
    fn to_lanes(self) -> [W; 4] {
        self.0
    }
    #[inline(always)]
    fn from_lanes(lanes: [W; 4]) -> Self {
        x4(lanes)
    }
}
impl<W: BSwap + Copy> BSwap for x4<W> {
    #[inline(always)]
    fn bswap(self) -> Self {
        x4([
            self.0[0].bswap(),
            self.0[1].bswap(),
            self.0[2].bswap(),
            self.0[3].bswap(),
        ])
    }
}
impl<W: StoreBytes + BSwap + Copy> StoreBytes for x4<W> {
    #[inline(always)]
    unsafe fn unsafe_read_le(input: &[u8]) -> Self {
        let n = input.len() / 4;
        x4([
            W::unsafe_read_le(&input[..n]),
            W::unsafe_read_le(&input[n..n * 2]),
            W::unsafe_read_le(&input[n * 2..n * 3]),
            W::unsafe_read_le(&input[n * 3..]),
        ])
    }
    #[inline(always)]
    unsafe fn unsafe_read_be(input: &[u8]) -> Self {
        let n = input.len() / 4;
        x4([
            W::unsafe_read_be(&input[..n]),
            W::unsafe_read_be(&input[n..n * 2]),
            W::unsafe_read_be(&input[n * 2..n * 3]),
            W::unsafe_read_be(&input[n * 3..]),
        ])
    }
    #[inline(always)]
    fn write_le(self, out: &mut [u8]) {
        let n = out.len() / 4;
        self.0[0].write_le(&mut out[..n]);
        self.0[1].write_le(&mut out[n..n * 2]);
        self.0[2].write_le(&mut out[n * 2..n * 3]);
        self.0[3].write_le(&mut out[n * 3..]);
    }
    #[inline(always)]
    fn write_be(self, out: &mut [u8]) {
        let n = out.len() / 4;
        self.0[0].write_be(&mut out[..n]);
        self.0[1].write_be(&mut out[n..n * 2]);
        self.0[2].write_be(&mut out[n * 2..n * 3]);
        self.0[3].write_be(&mut out[n * 3..]);
    }
}
impl<W: Copy + LaneWords4> LaneWords4 for x4<W> {
    #[inline(always)]
    fn shuffle_lane_words2301(self) -> Self {
        x4([
            self.0[0].shuffle_lane_words2301(),
            self.0[1].shuffle_lane_words2301(),
            self.0[2].shuffle_lane_words2301(),
            self.0[3].shuffle_lane_words2301(),
        ])
    }
    #[inline(always)]
    fn shuffle_lane_words1230(self) -> Self {
        x4([
            self.0[0].shuffle_lane_words1230(),
            self.0[1].shuffle_lane_words1230(),
            self.0[2].shuffle_lane_words1230(),
            self.0[3].shuffle_lane_words1230(),
        ])
    }
    #[inline(always)]
    fn shuffle_lane_words3012(self) -> Self {
        x4([
            self.0[0].shuffle_lane_words3012(),
            self.0[1].shuffle_lane_words3012(),
            self.0[2].shuffle_lane_words3012(),
            self.0[3].shuffle_lane_words3012(),
        ])
    }
}
