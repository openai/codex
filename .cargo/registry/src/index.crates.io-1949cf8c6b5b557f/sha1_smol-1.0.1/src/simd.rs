// Copyright (c) 2006-2009 Graydon Hoare
// Copyright (c) 2009-2013 Mozilla Foundation

// Permission is hereby granted, free of charge, to any
// person obtaining a copy of this software and associated
// documentation files (the "Software"), to deal in the
// Software without restriction, including without
// limitation the rights to use, copy, modify, merge,
// publish, distribute, sublicense, and/or sell copies of
// the Software, and to permit persons to whom the Software
// is furnished to do so, subject to the following
// conditions:

// The above copyright notice and this permission notice
// shall be included in all copies or substantial portions
// of the Software.

// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
// ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
// TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
// PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
// SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
// CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
// IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

use core::ops::{Add, BitAnd, BitOr, BitXor, Shl, Shr, Sub};

#[derive(Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub struct u32x4(pub u32, pub u32, pub u32, pub u32);

impl Add for u32x4 {
    type Output = u32x4;

    fn add(self, rhs: u32x4) -> u32x4 {
        u32x4(
            self.0.wrapping_add(rhs.0),
            self.1.wrapping_add(rhs.1),
            self.2.wrapping_add(rhs.2),
            self.3.wrapping_add(rhs.3),
        )
    }
}

impl Sub for u32x4 {
    type Output = u32x4;

    fn sub(self, rhs: u32x4) -> u32x4 {
        u32x4(
            self.0.wrapping_sub(rhs.0),
            self.1.wrapping_sub(rhs.1),
            self.2.wrapping_sub(rhs.2),
            self.3.wrapping_sub(rhs.3),
        )
    }
}

impl BitAnd for u32x4 {
    type Output = u32x4;

    fn bitand(self, rhs: u32x4) -> u32x4 {
        u32x4(
            self.0 & rhs.0,
            self.1 & rhs.1,
            self.2 & rhs.2,
            self.3 & rhs.3,
        )
    }
}

impl BitOr for u32x4 {
    type Output = u32x4;

    fn bitor(self, rhs: u32x4) -> u32x4 {
        u32x4(
            self.0 | rhs.0,
            self.1 | rhs.1,
            self.2 | rhs.2,
            self.3 | rhs.3,
        )
    }
}

impl BitXor for u32x4 {
    type Output = u32x4;

    fn bitxor(self, rhs: u32x4) -> u32x4 {
        u32x4(
            self.0 ^ rhs.0,
            self.1 ^ rhs.1,
            self.2 ^ rhs.2,
            self.3 ^ rhs.3,
        )
    }
}

impl Shl<usize> for u32x4 {
    type Output = u32x4;

    fn shl(self, amt: usize) -> u32x4 {
        u32x4(self.0 << amt, self.1 << amt, self.2 << amt, self.3 << amt)
    }
}

impl Shl<u32x4> for u32x4 {
    type Output = u32x4;

    fn shl(self, rhs: u32x4) -> u32x4 {
        u32x4(
            self.0 << rhs.0,
            self.1 << rhs.1,
            self.2 << rhs.2,
            self.3 << rhs.3,
        )
    }
}

impl Shr<usize> for u32x4 {
    type Output = u32x4;

    fn shr(self, amt: usize) -> u32x4 {
        u32x4(self.0 >> amt, self.1 >> amt, self.2 >> amt, self.3 >> amt)
    }
}

impl Shr<u32x4> for u32x4 {
    type Output = u32x4;

    fn shr(self, rhs: u32x4) -> u32x4 {
        u32x4(
            self.0 >> rhs.0,
            self.1 >> rhs.1,
            self.2 >> rhs.2,
            self.3 >> rhs.3,
        )
    }
}

#[derive(Clone, Copy)]
#[allow(non_camel_case_types)]
pub struct u64x2(pub u64, pub u64);

impl Add for u64x2 {
    type Output = u64x2;

    fn add(self, rhs: u64x2) -> u64x2 {
        u64x2(self.0.wrapping_add(rhs.0), self.1.wrapping_add(rhs.1))
    }
}
