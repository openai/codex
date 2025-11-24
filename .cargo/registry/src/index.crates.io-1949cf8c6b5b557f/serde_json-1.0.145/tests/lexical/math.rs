// Adapted from https://github.com/Alexhuszagh/rust-lexical.

use crate::lexical::math::{Limb, Math};
use std::cmp;

#[derive(Clone, Default)]
struct Bigint {
    data: Vec<Limb>,
}

impl Math for Bigint {
    fn data(&self) -> &Vec<Limb> {
        &self.data
    }

    fn data_mut(&mut self) -> &mut Vec<Limb> {
        &mut self.data
    }
}

#[cfg(fast_arithmetic = "32")]
pub(crate) fn from_u32(x: &[u32]) -> Vec<Limb> {
    x.iter().cloned().collect()
}

#[cfg(fast_arithmetic = "64")]
pub(crate) fn from_u32(x: &[u32]) -> Vec<Limb> {
    let mut v = Vec::<Limb>::default();
    for xi in x.chunks(2) {
        match xi.len() {
            1 => v.push(xi[0] as u64),
            2 => v.push(((xi[1] as u64) << 32) | (xi[0] as u64)),
            _ => unreachable!(),
        }
    }

    v
}

#[test]
fn compare_test() {
    // Simple
    let x = Bigint {
        data: from_u32(&[1]),
    };
    let y = Bigint {
        data: from_u32(&[2]),
    };
    assert_eq!(x.compare(&y), cmp::Ordering::Less);
    assert_eq!(x.compare(&x), cmp::Ordering::Equal);
    assert_eq!(y.compare(&x), cmp::Ordering::Greater);

    // Check asymmetric
    let x = Bigint {
        data: from_u32(&[5, 1]),
    };
    let y = Bigint {
        data: from_u32(&[2]),
    };
    assert_eq!(x.compare(&y), cmp::Ordering::Greater);
    assert_eq!(x.compare(&x), cmp::Ordering::Equal);
    assert_eq!(y.compare(&x), cmp::Ordering::Less);

    // Check when we use reverse ordering properly.
    let x = Bigint {
        data: from_u32(&[5, 1, 9]),
    };
    let y = Bigint {
        data: from_u32(&[6, 2, 8]),
    };
    assert_eq!(x.compare(&y), cmp::Ordering::Greater);
    assert_eq!(x.compare(&x), cmp::Ordering::Equal);
    assert_eq!(y.compare(&x), cmp::Ordering::Less);

    // Complex scenario, check it properly uses reverse ordering.
    let x = Bigint {
        data: from_u32(&[0, 1, 9]),
    };
    let y = Bigint {
        data: from_u32(&[4294967295, 0, 9]),
    };
    assert_eq!(x.compare(&y), cmp::Ordering::Greater);
    assert_eq!(x.compare(&x), cmp::Ordering::Equal);
    assert_eq!(y.compare(&x), cmp::Ordering::Less);
}

#[test]
fn hi64_test() {
    assert_eq!(Bigint::from_u64(0xA).hi64(), (0xA000000000000000, false));
    assert_eq!(Bigint::from_u64(0xAB).hi64(), (0xAB00000000000000, false));
    assert_eq!(
        Bigint::from_u64(0xAB00000000).hi64(),
        (0xAB00000000000000, false)
    );
    assert_eq!(
        Bigint::from_u64(0xA23456789A).hi64(),
        (0xA23456789A000000, false)
    );
}

#[test]
fn bit_length_test() {
    let x = Bigint {
        data: from_u32(&[0, 0, 0, 1]),
    };
    assert_eq!(x.bit_length(), 97);

    let x = Bigint {
        data: from_u32(&[0, 0, 0, 3]),
    };
    assert_eq!(x.bit_length(), 98);

    let x = Bigint {
        data: from_u32(&[1 << 31]),
    };
    assert_eq!(x.bit_length(), 32);
}

#[test]
fn iadd_small_test() {
    // Overflow check (single)
    // This should set all the internal data values to 0, the top
    // value to (1<<31), and the bottom value to (4>>1).
    // This is because the max_value + 1 leads to all 0s, we set the
    // topmost bit to 1.
    let mut x = Bigint {
        data: from_u32(&[4294967295]),
    };
    x.iadd_small(5);
    assert_eq!(x.data, from_u32(&[4, 1]));

    // No overflow, single value
    let mut x = Bigint {
        data: from_u32(&[5]),
    };
    x.iadd_small(7);
    assert_eq!(x.data, from_u32(&[12]));

    // Single carry, internal overflow
    let mut x = Bigint::from_u64(0x80000000FFFFFFFF);
    x.iadd_small(7);
    assert_eq!(x.data, from_u32(&[6, 0x80000001]));

    // Double carry, overflow
    let mut x = Bigint::from_u64(0xFFFFFFFFFFFFFFFF);
    x.iadd_small(7);
    assert_eq!(x.data, from_u32(&[6, 0, 1]));
}

#[test]
fn imul_small_test() {
    // No overflow check, 1-int.
    let mut x = Bigint {
        data: from_u32(&[5]),
    };
    x.imul_small(7);
    assert_eq!(x.data, from_u32(&[35]));

    // No overflow check, 2-ints.
    let mut x = Bigint::from_u64(0x4000000040000);
    x.imul_small(5);
    assert_eq!(x.data, from_u32(&[0x00140000, 0x140000]));

    // Overflow, 1 carry.
    let mut x = Bigint {
        data: from_u32(&[0x33333334]),
    };
    x.imul_small(5);
    assert_eq!(x.data, from_u32(&[4, 1]));

    // Overflow, 1 carry, internal.
    let mut x = Bigint::from_u64(0x133333334);
    x.imul_small(5);
    assert_eq!(x.data, from_u32(&[4, 6]));

    // Overflow, 2 carries.
    let mut x = Bigint::from_u64(0x3333333333333334);
    x.imul_small(5);
    assert_eq!(x.data, from_u32(&[4, 0, 1]));
}

#[test]
fn shl_test() {
    // Pattern generated via `''.join(["1" +"0"*i for i in range(20)])`
    let mut big = Bigint {
        data: from_u32(&[0xD2210408]),
    };
    big.ishl(5);
    assert_eq!(big.data, from_u32(&[0x44208100, 0x1A]));
    big.ishl(32);
    assert_eq!(big.data, from_u32(&[0, 0x44208100, 0x1A]));
    big.ishl(27);
    assert_eq!(big.data, from_u32(&[0, 0, 0xD2210408]));

    // 96-bits of previous pattern
    let mut big = Bigint {
        data: from_u32(&[0x20020010, 0x8040100, 0xD2210408]),
    };
    big.ishl(5);
    assert_eq!(big.data, from_u32(&[0x400200, 0x802004, 0x44208101, 0x1A]));
    big.ishl(32);
    assert_eq!(
        big.data,
        from_u32(&[0, 0x400200, 0x802004, 0x44208101, 0x1A])
    );
    big.ishl(27);
    assert_eq!(
        big.data,
        from_u32(&[0, 0, 0x20020010, 0x8040100, 0xD2210408])
    );
}
