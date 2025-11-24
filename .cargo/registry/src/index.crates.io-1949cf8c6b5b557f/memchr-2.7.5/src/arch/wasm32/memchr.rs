/*!
Wrapper routines for `memchr` and friends.

These routines choose the best implementation at compile time. (This is
different from `x86_64` because it is expected that `simd128` is almost always
available for `wasm32` targets.)
*/

macro_rules! defraw {
    ($ty:ident, $find:ident, $start:ident, $end:ident, $($needles:ident),+) => {{
        use crate::arch::wasm32::simd128::memchr::$ty;

        debug!("chose simd128 for {}", stringify!($ty));
        debug_assert!($ty::is_available());
        // SAFETY: We know that wasm memchr is always available whenever
        // code is compiled for `wasm32` with the `simd128` target feature
        // enabled.
        $ty::new_unchecked($($needles),+).$find($start, $end)
    }}
}

/// memchr, but using raw pointers to represent the haystack.
///
/// # Safety
///
/// Pointers must be valid. See `One::find_raw`.
#[inline(always)]
pub(crate) unsafe fn memchr_raw(
    n1: u8,
    start: *const u8,
    end: *const u8,
) -> Option<*const u8> {
    defraw!(One, find_raw, start, end, n1)
}

/// memrchr, but using raw pointers to represent the haystack.
///
/// # Safety
///
/// Pointers must be valid. See `One::rfind_raw`.
#[inline(always)]
pub(crate) unsafe fn memrchr_raw(
    n1: u8,
    start: *const u8,
    end: *const u8,
) -> Option<*const u8> {
    defraw!(One, rfind_raw, start, end, n1)
}

/// memchr2, but using raw pointers to represent the haystack.
///
/// # Safety
///
/// Pointers must be valid. See `Two::find_raw`.
#[inline(always)]
pub(crate) unsafe fn memchr2_raw(
    n1: u8,
    n2: u8,
    start: *const u8,
    end: *const u8,
) -> Option<*const u8> {
    defraw!(Two, find_raw, start, end, n1, n2)
}

/// memrchr2, but using raw pointers to represent the haystack.
///
/// # Safety
///
/// Pointers must be valid. See `Two::rfind_raw`.
#[inline(always)]
pub(crate) unsafe fn memrchr2_raw(
    n1: u8,
    n2: u8,
    start: *const u8,
    end: *const u8,
) -> Option<*const u8> {
    defraw!(Two, rfind_raw, start, end, n1, n2)
}

/// memchr3, but using raw pointers to represent the haystack.
///
/// # Safety
///
/// Pointers must be valid. See `Three::find_raw`.
#[inline(always)]
pub(crate) unsafe fn memchr3_raw(
    n1: u8,
    n2: u8,
    n3: u8,
    start: *const u8,
    end: *const u8,
) -> Option<*const u8> {
    defraw!(Three, find_raw, start, end, n1, n2, n3)
}

/// memrchr3, but using raw pointers to represent the haystack.
///
/// # Safety
///
/// Pointers must be valid. See `Three::rfind_raw`.
#[inline(always)]
pub(crate) unsafe fn memrchr3_raw(
    n1: u8,
    n2: u8,
    n3: u8,
    start: *const u8,
    end: *const u8,
) -> Option<*const u8> {
    defraw!(Three, rfind_raw, start, end, n1, n2, n3)
}

/// Count all matching bytes, but using raw pointers to represent the haystack.
///
/// # Safety
///
/// Pointers must be valid. See `One::count_raw`.
#[inline(always)]
pub(crate) unsafe fn count_raw(
    n1: u8,
    start: *const u8,
    end: *const u8,
) -> usize {
    defraw!(One, count_raw, start, end, n1)
}
