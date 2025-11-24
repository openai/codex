mod all;
mod bitflags_match;
mod bits;
mod clear;
mod complement;
mod contains;
mod difference;
mod empty;
mod eq;
mod extend;
mod flags;
mod fmt;
mod from_bits;
mod from_bits_retain;
mod from_bits_truncate;
mod from_name;
mod insert;
mod intersection;
mod intersects;
mod is_all;
mod is_empty;
mod iter;
mod parser;
mod remove;
mod symmetric_difference;
mod truncate;
mod union;
mod unknown;

bitflags! {
    #[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
    pub struct TestFlags: u8 {
        /// 1
        const A = 1;

        /// 1 << 1
        const B = 1 << 1;

        /// 1 << 2
        const C = 1 << 2;

        /// 1 | (1 << 1) | (1 << 2)
        const ABC = Self::A.bits() | Self::B.bits() | Self::C.bits();
    }

    #[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
    pub struct TestFlagsInvert: u8 {
        /// 1 | (1 << 1) | (1 << 2)
        const ABC = Self::A.bits() | Self::B.bits() | Self::C.bits();

        /// 1
        const A = 1;

        /// 1 << 1
        const B = 1 << 1;

        /// 1 << 2
        const C = 1 << 2;
    }

    #[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
    pub struct TestZero: u8 {
        /// 0
        const ZERO = 0;
    }

    #[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
    pub struct TestZeroOne: u8 {
        /// 0
        const ZERO = 0;

        /// 1
        const ONE = 1;
    }

    #[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
    pub struct TestUnicode: u8 {
        /// 1
        const 一 = 1;

        /// 2
        const 二 = 1 << 1;
    }

    #[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
    pub struct TestEmpty: u8 {}

    #[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
    pub struct TestOverlapping: u8 {
        /// 1 | (1 << 1)
        const AB = 1 | (1 << 1);

        /// (1 << 1) | (1 << 2)
        const BC = (1 << 1) | (1 << 2);
    }

    #[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
    pub struct TestOverlappingFull: u8 {
        /// 1
        const A = 1;

        /// 1
        const B = 1;

        /// 1
        const C = 1;

        /// 2
        const D = 1 << 1;
    }

    #[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
    pub struct TestExternal: u8 {
        /// 1
        const A = 1;

        /// 1 << 1
        const B = 1 << 1;

        /// 1 << 2
        const C = 1 << 2;

        /// 1 | (1 << 1) | (1 << 2)
        const ABC = Self::A.bits() | Self::B.bits() | Self::C.bits();

        /// External
        const _ = !0;
    }

    #[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
    pub struct TestExternalFull: u8 {
        /// External
        const _ = !0;
    }
}
