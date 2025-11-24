//! Pre-computed small tables for parsing decimal strings.

#![doc(hidden)]
#![cfg(not(feature = "compact"))]

/// Pre-computed, small powers-of-5.
pub const SMALL_INT_POW5: [u64; 28] = [
    1,
    5,
    25,
    125,
    625,
    3125,
    15625,
    78125,
    390625,
    1953125,
    9765625,
    48828125,
    244140625,
    1220703125,
    6103515625,
    30517578125,
    152587890625,
    762939453125,
    3814697265625,
    19073486328125,
    95367431640625,
    476837158203125,
    2384185791015625,
    11920928955078125,
    59604644775390625,
    298023223876953125,
    1490116119384765625,
    7450580596923828125,
];

/// Pre-computed, small powers-of-10.
pub const SMALL_INT_POW10: [u64; 20] = [
    1,
    10,
    100,
    1000,
    10000,
    100000,
    1000000,
    10000000,
    100000000,
    1000000000,
    10000000000,
    100000000000,
    1000000000000,
    10000000000000,
    100000000000000,
    1000000000000000,
    10000000000000000,
    100000000000000000,
    1000000000000000000,
    10000000000000000000,
];

/// Pre-computed, small powers-of-10.
pub const SMALL_F32_POW10: [f32; 16] =
    [1e0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8, 1e9, 1e10, 0., 0., 0., 0., 0.];

/// Pre-computed, small powers-of-10.
pub const SMALL_F64_POW10: [f64; 32] = [
    1e0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8, 1e9, 1e10, 1e11, 1e12, 1e13, 1e14, 1e15, 1e16,
    1e17, 1e18, 1e19, 1e20, 1e21, 1e22, 0., 0., 0., 0., 0., 0., 0., 0., 0.,
];

/// Pre-computed large power-of-5 for 32-bit limbs.
#[cfg(not(all(target_pointer_width = "64", not(target_arch = "sparc"))))]
pub const LARGE_POW5: [u32; 10] = [
    4279965485, 329373468, 4020270615, 2137533757, 4287402176, 1057042919, 1071430142, 2440757623,
    381945767, 46164893,
];

/// Pre-computed large power-of-5 for 64-bit limbs.
#[cfg(all(target_pointer_width = "64", not(target_arch = "sparc")))]
pub const LARGE_POW5: [u64; 5] = [
    1414648277510068013,
    9180637584431281687,
    4539964771860779200,
    10482974169319127550,
    198276706040285095,
];

/// Step for large power-of-5 for 32-bit limbs.
pub const LARGE_POW5_STEP: u32 = 135;
