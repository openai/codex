use core::iter::Rev;

use crate::arch::generic::memchr as generic;

/// Search for the first occurrence of a byte in a slice.
///
/// This returns the index corresponding to the first occurrence of `needle` in
/// `haystack`, or `None` if one is not found. If an index is returned, it is
/// guaranteed to be less than `haystack.len()`.
///
/// While this is semantically the same as something like
/// `haystack.iter().position(|&b| b == needle)`, this routine will attempt to
/// use highly optimized vector operations that can be an order of magnitude
/// faster (or more).
///
/// # Example
///
/// This shows how to find the first position of a byte in a byte string.
///
/// ```
/// use memchr::memchr;
///
/// let haystack = b"the quick brown fox";
/// assert_eq!(memchr(b'k', haystack), Some(8));
/// ```
#[inline]
pub fn memchr(needle: u8, haystack: &[u8]) -> Option<usize> {
    // SAFETY: memchr_raw, when a match is found, always returns a valid
    // pointer between start and end.
    unsafe {
        generic::search_slice_with_raw(haystack, |start, end| {
            memchr_raw(needle, start, end)
        })
    }
}

/// Search for the last occurrence of a byte in a slice.
///
/// This returns the index corresponding to the last occurrence of `needle` in
/// `haystack`, or `None` if one is not found. If an index is returned, it is
/// guaranteed to be less than `haystack.len()`.
///
/// While this is semantically the same as something like
/// `haystack.iter().rposition(|&b| b == needle)`, this routine will attempt to
/// use highly optimized vector operations that can be an order of magnitude
/// faster (or more).
///
/// # Example
///
/// This shows how to find the last position of a byte in a byte string.
///
/// ```
/// use memchr::memrchr;
///
/// let haystack = b"the quick brown fox";
/// assert_eq!(memrchr(b'o', haystack), Some(17));
/// ```
#[inline]
pub fn memrchr(needle: u8, haystack: &[u8]) -> Option<usize> {
    // SAFETY: memrchr_raw, when a match is found, always returns a valid
    // pointer between start and end.
    unsafe {
        generic::search_slice_with_raw(haystack, |start, end| {
            memrchr_raw(needle, start, end)
        })
    }
}

/// Search for the first occurrence of two possible bytes in a haystack.
///
/// This returns the index corresponding to the first occurrence of one of the
/// needle bytes in `haystack`, or `None` if one is not found. If an index is
/// returned, it is guaranteed to be less than `haystack.len()`.
///
/// While this is semantically the same as something like
/// `haystack.iter().position(|&b| b == needle1 || b == needle2)`, this routine
/// will attempt to use highly optimized vector operations that can be an order
/// of magnitude faster (or more).
///
/// # Example
///
/// This shows how to find the first position of one of two possible bytes in a
/// haystack.
///
/// ```
/// use memchr::memchr2;
///
/// let haystack = b"the quick brown fox";
/// assert_eq!(memchr2(b'k', b'q', haystack), Some(4));
/// ```
#[inline]
pub fn memchr2(needle1: u8, needle2: u8, haystack: &[u8]) -> Option<usize> {
    // SAFETY: memchr2_raw, when a match is found, always returns a valid
    // pointer between start and end.
    unsafe {
        generic::search_slice_with_raw(haystack, |start, end| {
            memchr2_raw(needle1, needle2, start, end)
        })
    }
}

/// Search for the last occurrence of two possible bytes in a haystack.
///
/// This returns the index corresponding to the last occurrence of one of the
/// needle bytes in `haystack`, or `None` if one is not found. If an index is
/// returned, it is guaranteed to be less than `haystack.len()`.
///
/// While this is semantically the same as something like
/// `haystack.iter().rposition(|&b| b == needle1 || b == needle2)`, this
/// routine will attempt to use highly optimized vector operations that can be
/// an order of magnitude faster (or more).
///
/// # Example
///
/// This shows how to find the last position of one of two possible bytes in a
/// haystack.
///
/// ```
/// use memchr::memrchr2;
///
/// let haystack = b"the quick brown fox";
/// assert_eq!(memrchr2(b'k', b'o', haystack), Some(17));
/// ```
#[inline]
pub fn memrchr2(needle1: u8, needle2: u8, haystack: &[u8]) -> Option<usize> {
    // SAFETY: memrchr2_raw, when a match is found, always returns a valid
    // pointer between start and end.
    unsafe {
        generic::search_slice_with_raw(haystack, |start, end| {
            memrchr2_raw(needle1, needle2, start, end)
        })
    }
}

/// Search for the first occurrence of three possible bytes in a haystack.
///
/// This returns the index corresponding to the first occurrence of one of the
/// needle bytes in `haystack`, or `None` if one is not found. If an index is
/// returned, it is guaranteed to be less than `haystack.len()`.
///
/// While this is semantically the same as something like
/// `haystack.iter().position(|&b| b == needle1 || b == needle2 || b == needle3)`,
/// this routine will attempt to use highly optimized vector operations that
/// can be an order of magnitude faster (or more).
///
/// # Example
///
/// This shows how to find the first position of one of three possible bytes in
/// a haystack.
///
/// ```
/// use memchr::memchr3;
///
/// let haystack = b"the quick brown fox";
/// assert_eq!(memchr3(b'k', b'q', b'u', haystack), Some(4));
/// ```
#[inline]
pub fn memchr3(
    needle1: u8,
    needle2: u8,
    needle3: u8,
    haystack: &[u8],
) -> Option<usize> {
    // SAFETY: memchr3_raw, when a match is found, always returns a valid
    // pointer between start and end.
    unsafe {
        generic::search_slice_with_raw(haystack, |start, end| {
            memchr3_raw(needle1, needle2, needle3, start, end)
        })
    }
}

/// Search for the last occurrence of three possible bytes in a haystack.
///
/// This returns the index corresponding to the last occurrence of one of the
/// needle bytes in `haystack`, or `None` if one is not found. If an index is
/// returned, it is guaranteed to be less than `haystack.len()`.
///
/// While this is semantically the same as something like
/// `haystack.iter().rposition(|&b| b == needle1 || b == needle2 || b == needle3)`,
/// this routine will attempt to use highly optimized vector operations that
/// can be an order of magnitude faster (or more).
///
/// # Example
///
/// This shows how to find the last position of one of three possible bytes in
/// a haystack.
///
/// ```
/// use memchr::memrchr3;
///
/// let haystack = b"the quick brown fox";
/// assert_eq!(memrchr3(b'k', b'o', b'n', haystack), Some(17));
/// ```
#[inline]
pub fn memrchr3(
    needle1: u8,
    needle2: u8,
    needle3: u8,
    haystack: &[u8],
) -> Option<usize> {
    // SAFETY: memrchr3_raw, when a match is found, always returns a valid
    // pointer between start and end.
    unsafe {
        generic::search_slice_with_raw(haystack, |start, end| {
            memrchr3_raw(needle1, needle2, needle3, start, end)
        })
    }
}

/// Returns an iterator over all occurrences of the needle in a haystack.
///
/// The iterator returned implements `DoubleEndedIterator`. This means it
/// can also be used to find occurrences in reverse order.
#[inline]
pub fn memchr_iter<'h>(needle: u8, haystack: &'h [u8]) -> Memchr<'h> {
    Memchr::new(needle, haystack)
}

/// Returns an iterator over all occurrences of the needle in a haystack, in
/// reverse.
#[inline]
pub fn memrchr_iter(needle: u8, haystack: &[u8]) -> Rev<Memchr<'_>> {
    Memchr::new(needle, haystack).rev()
}

/// Returns an iterator over all occurrences of the needles in a haystack.
///
/// The iterator returned implements `DoubleEndedIterator`. This means it
/// can also be used to find occurrences in reverse order.
#[inline]
pub fn memchr2_iter<'h>(
    needle1: u8,
    needle2: u8,
    haystack: &'h [u8],
) -> Memchr2<'h> {
    Memchr2::new(needle1, needle2, haystack)
}

/// Returns an iterator over all occurrences of the needles in a haystack, in
/// reverse.
#[inline]
pub fn memrchr2_iter(
    needle1: u8,
    needle2: u8,
    haystack: &[u8],
) -> Rev<Memchr2<'_>> {
    Memchr2::new(needle1, needle2, haystack).rev()
}

/// Returns an iterator over all occurrences of the needles in a haystack.
///
/// The iterator returned implements `DoubleEndedIterator`. This means it
/// can also be used to find occurrences in reverse order.
#[inline]
pub fn memchr3_iter<'h>(
    needle1: u8,
    needle2: u8,
    needle3: u8,
    haystack: &'h [u8],
) -> Memchr3<'h> {
    Memchr3::new(needle1, needle2, needle3, haystack)
}

/// Returns an iterator over all occurrences of the needles in a haystack, in
/// reverse.
#[inline]
pub fn memrchr3_iter(
    needle1: u8,
    needle2: u8,
    needle3: u8,
    haystack: &[u8],
) -> Rev<Memchr3<'_>> {
    Memchr3::new(needle1, needle2, needle3, haystack).rev()
}

/// An iterator over all occurrences of a single byte in a haystack.
///
/// This iterator implements `DoubleEndedIterator`, which means it can also be
/// used to find occurrences in reverse order.
///
/// This iterator is created by the [`memchr_iter`] or `[memrchr_iter`]
/// functions. It can also be created with the [`Memchr::new`] method.
///
/// The lifetime parameter `'h` refers to the lifetime of the haystack being
/// searched.
#[derive(Clone, Debug)]
pub struct Memchr<'h> {
    needle1: u8,
    it: crate::arch::generic::memchr::Iter<'h>,
}

impl<'h> Memchr<'h> {
    /// Returns an iterator over all occurrences of the needle byte in the
    /// given haystack.
    ///
    /// The iterator returned implements `DoubleEndedIterator`. This means it
    /// can also be used to find occurrences in reverse order.
    #[inline]
    pub fn new(needle1: u8, haystack: &'h [u8]) -> Memchr<'h> {
        Memchr {
            needle1,
            it: crate::arch::generic::memchr::Iter::new(haystack),
        }
    }
}

impl<'h> Iterator for Memchr<'h> {
    type Item = usize;

    #[inline]
    fn next(&mut self) -> Option<usize> {
        // SAFETY: All of our implementations of memchr ensure that any
        // pointers returns will fall within the start and end bounds, and this
        // upholds the safety contract of `self.it.next`.
        unsafe {
            // NOTE: I attempted to define an enum of previously created
            // searchers and then switch on those here instead of just
            // calling `memchr_raw` (or `One::new(..).find_raw(..)`). But
            // that turned out to have a fair bit of extra overhead when
            // searching very small haystacks.
            self.it.next(|s, e| memchr_raw(self.needle1, s, e))
        }
    }

    #[inline]
    fn count(self) -> usize {
        self.it.count(|s, e| {
            // SAFETY: We rely on our generic iterator to return valid start
            // and end pointers.
            unsafe { count_raw(self.needle1, s, e) }
        })
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.it.size_hint()
    }
}

impl<'h> DoubleEndedIterator for Memchr<'h> {
    #[inline]
    fn next_back(&mut self) -> Option<usize> {
        // SAFETY: All of our implementations of memchr ensure that any
        // pointers returns will fall within the start and end bounds, and this
        // upholds the safety contract of `self.it.next_back`.
        unsafe { self.it.next_back(|s, e| memrchr_raw(self.needle1, s, e)) }
    }
}

impl<'h> core::iter::FusedIterator for Memchr<'h> {}

/// An iterator over all occurrences of two possible bytes in a haystack.
///
/// This iterator implements `DoubleEndedIterator`, which means it can also be
/// used to find occurrences in reverse order.
///
/// This iterator is created by the [`memchr2_iter`] or `[memrchr2_iter`]
/// functions. It can also be created with the [`Memchr2::new`] method.
///
/// The lifetime parameter `'h` refers to the lifetime of the haystack being
/// searched.
#[derive(Clone, Debug)]
pub struct Memchr2<'h> {
    needle1: u8,
    needle2: u8,
    it: crate::arch::generic::memchr::Iter<'h>,
}

impl<'h> Memchr2<'h> {
    /// Returns an iterator over all occurrences of the needle bytes in the
    /// given haystack.
    ///
    /// The iterator returned implements `DoubleEndedIterator`. This means it
    /// can also be used to find occurrences in reverse order.
    #[inline]
    pub fn new(needle1: u8, needle2: u8, haystack: &'h [u8]) -> Memchr2<'h> {
        Memchr2 {
            needle1,
            needle2,
            it: crate::arch::generic::memchr::Iter::new(haystack),
        }
    }
}

impl<'h> Iterator for Memchr2<'h> {
    type Item = usize;

    #[inline]
    fn next(&mut self) -> Option<usize> {
        // SAFETY: All of our implementations of memchr ensure that any
        // pointers returns will fall within the start and end bounds, and this
        // upholds the safety contract of `self.it.next`.
        unsafe {
            self.it.next(|s, e| memchr2_raw(self.needle1, self.needle2, s, e))
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.it.size_hint()
    }
}

impl<'h> DoubleEndedIterator for Memchr2<'h> {
    #[inline]
    fn next_back(&mut self) -> Option<usize> {
        // SAFETY: All of our implementations of memchr ensure that any
        // pointers returns will fall within the start and end bounds, and this
        // upholds the safety contract of `self.it.next_back`.
        unsafe {
            self.it.next_back(|s, e| {
                memrchr2_raw(self.needle1, self.needle2, s, e)
            })
        }
    }
}

impl<'h> core::iter::FusedIterator for Memchr2<'h> {}

/// An iterator over all occurrences of three possible bytes in a haystack.
///
/// This iterator implements `DoubleEndedIterator`, which means it can also be
/// used to find occurrences in reverse order.
///
/// This iterator is created by the [`memchr2_iter`] or `[memrchr2_iter`]
/// functions. It can also be created with the [`Memchr3::new`] method.
///
/// The lifetime parameter `'h` refers to the lifetime of the haystack being
/// searched.
#[derive(Clone, Debug)]
pub struct Memchr3<'h> {
    needle1: u8,
    needle2: u8,
    needle3: u8,
    it: crate::arch::generic::memchr::Iter<'h>,
}

impl<'h> Memchr3<'h> {
    /// Returns an iterator over all occurrences of the needle bytes in the
    /// given haystack.
    ///
    /// The iterator returned implements `DoubleEndedIterator`. This means it
    /// can also be used to find occurrences in reverse order.
    #[inline]
    pub fn new(
        needle1: u8,
        needle2: u8,
        needle3: u8,
        haystack: &'h [u8],
    ) -> Memchr3<'h> {
        Memchr3 {
            needle1,
            needle2,
            needle3,
            it: crate::arch::generic::memchr::Iter::new(haystack),
        }
    }
}

impl<'h> Iterator for Memchr3<'h> {
    type Item = usize;

    #[inline]
    fn next(&mut self) -> Option<usize> {
        // SAFETY: All of our implementations of memchr ensure that any
        // pointers returns will fall within the start and end bounds, and this
        // upholds the safety contract of `self.it.next`.
        unsafe {
            self.it.next(|s, e| {
                memchr3_raw(self.needle1, self.needle2, self.needle3, s, e)
            })
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.it.size_hint()
    }
}

impl<'h> DoubleEndedIterator for Memchr3<'h> {
    #[inline]
    fn next_back(&mut self) -> Option<usize> {
        // SAFETY: All of our implementations of memchr ensure that any
        // pointers returns will fall within the start and end bounds, and this
        // upholds the safety contract of `self.it.next_back`.
        unsafe {
            self.it.next_back(|s, e| {
                memrchr3_raw(self.needle1, self.needle2, self.needle3, s, e)
            })
        }
    }
}

impl<'h> core::iter::FusedIterator for Memchr3<'h> {}

/// memchr, but using raw pointers to represent the haystack.
///
/// # Safety
///
/// Pointers must be valid. See `One::find_raw`.
#[inline]
unsafe fn memchr_raw(
    needle: u8,
    start: *const u8,
    end: *const u8,
) -> Option<*const u8> {
    #[cfg(target_arch = "x86_64")]
    {
        // x86_64 does CPU feature detection at runtime in order to use AVX2
        // instructions even when the `avx2` feature isn't enabled at compile
        // time. This function also handles using a fallback if neither AVX2
        // nor SSE2 (unusual) are available.
        crate::arch::x86_64::memchr::memchr_raw(needle, start, end)
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    {
        crate::arch::wasm32::memchr::memchr_raw(needle, start, end)
    }
    #[cfg(target_arch = "aarch64")]
    {
        crate::arch::aarch64::memchr::memchr_raw(needle, start, end)
    }
    #[cfg(not(any(
        target_arch = "x86_64",
        all(target_arch = "wasm32", target_feature = "simd128"),
        target_arch = "aarch64"
    )))]
    {
        crate::arch::all::memchr::One::new(needle).find_raw(start, end)
    }
}

/// memrchr, but using raw pointers to represent the haystack.
///
/// # Safety
///
/// Pointers must be valid. See `One::rfind_raw`.
#[inline]
unsafe fn memrchr_raw(
    needle: u8,
    start: *const u8,
    end: *const u8,
) -> Option<*const u8> {
    #[cfg(target_arch = "x86_64")]
    {
        crate::arch::x86_64::memchr::memrchr_raw(needle, start, end)
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    {
        crate::arch::wasm32::memchr::memrchr_raw(needle, start, end)
    }
    #[cfg(target_arch = "aarch64")]
    {
        crate::arch::aarch64::memchr::memrchr_raw(needle, start, end)
    }
    #[cfg(not(any(
        target_arch = "x86_64",
        all(target_arch = "wasm32", target_feature = "simd128"),
        target_arch = "aarch64"
    )))]
    {
        crate::arch::all::memchr::One::new(needle).rfind_raw(start, end)
    }
}

/// memchr2, but using raw pointers to represent the haystack.
///
/// # Safety
///
/// Pointers must be valid. See `Two::find_raw`.
#[inline]
unsafe fn memchr2_raw(
    needle1: u8,
    needle2: u8,
    start: *const u8,
    end: *const u8,
) -> Option<*const u8> {
    #[cfg(target_arch = "x86_64")]
    {
        crate::arch::x86_64::memchr::memchr2_raw(needle1, needle2, start, end)
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    {
        crate::arch::wasm32::memchr::memchr2_raw(needle1, needle2, start, end)
    }
    #[cfg(target_arch = "aarch64")]
    {
        crate::arch::aarch64::memchr::memchr2_raw(needle1, needle2, start, end)
    }
    #[cfg(not(any(
        target_arch = "x86_64",
        all(target_arch = "wasm32", target_feature = "simd128"),
        target_arch = "aarch64"
    )))]
    {
        crate::arch::all::memchr::Two::new(needle1, needle2)
            .find_raw(start, end)
    }
}

/// memrchr2, but using raw pointers to represent the haystack.
///
/// # Safety
///
/// Pointers must be valid. See `Two::rfind_raw`.
#[inline]
unsafe fn memrchr2_raw(
    needle1: u8,
    needle2: u8,
    start: *const u8,
    end: *const u8,
) -> Option<*const u8> {
    #[cfg(target_arch = "x86_64")]
    {
        crate::arch::x86_64::memchr::memrchr2_raw(needle1, needle2, start, end)
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    {
        crate::arch::wasm32::memchr::memrchr2_raw(needle1, needle2, start, end)
    }
    #[cfg(target_arch = "aarch64")]
    {
        crate::arch::aarch64::memchr::memrchr2_raw(
            needle1, needle2, start, end,
        )
    }
    #[cfg(not(any(
        target_arch = "x86_64",
        all(target_arch = "wasm32", target_feature = "simd128"),
        target_arch = "aarch64"
    )))]
    {
        crate::arch::all::memchr::Two::new(needle1, needle2)
            .rfind_raw(start, end)
    }
}

/// memchr3, but using raw pointers to represent the haystack.
///
/// # Safety
///
/// Pointers must be valid. See `Three::find_raw`.
#[inline]
unsafe fn memchr3_raw(
    needle1: u8,
    needle2: u8,
    needle3: u8,
    start: *const u8,
    end: *const u8,
) -> Option<*const u8> {
    #[cfg(target_arch = "x86_64")]
    {
        crate::arch::x86_64::memchr::memchr3_raw(
            needle1, needle2, needle3, start, end,
        )
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    {
        crate::arch::wasm32::memchr::memchr3_raw(
            needle1, needle2, needle3, start, end,
        )
    }
    #[cfg(target_arch = "aarch64")]
    {
        crate::arch::aarch64::memchr::memchr3_raw(
            needle1, needle2, needle3, start, end,
        )
    }
    #[cfg(not(any(
        target_arch = "x86_64",
        all(target_arch = "wasm32", target_feature = "simd128"),
        target_arch = "aarch64"
    )))]
    {
        crate::arch::all::memchr::Three::new(needle1, needle2, needle3)
            .find_raw(start, end)
    }
}

/// memrchr3, but using raw pointers to represent the haystack.
///
/// # Safety
///
/// Pointers must be valid. See `Three::rfind_raw`.
#[inline]
unsafe fn memrchr3_raw(
    needle1: u8,
    needle2: u8,
    needle3: u8,
    start: *const u8,
    end: *const u8,
) -> Option<*const u8> {
    #[cfg(target_arch = "x86_64")]
    {
        crate::arch::x86_64::memchr::memrchr3_raw(
            needle1, needle2, needle3, start, end,
        )
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    {
        crate::arch::wasm32::memchr::memrchr3_raw(
            needle1, needle2, needle3, start, end,
        )
    }
    #[cfg(target_arch = "aarch64")]
    {
        crate::arch::aarch64::memchr::memrchr3_raw(
            needle1, needle2, needle3, start, end,
        )
    }
    #[cfg(not(any(
        target_arch = "x86_64",
        all(target_arch = "wasm32", target_feature = "simd128"),
        target_arch = "aarch64"
    )))]
    {
        crate::arch::all::memchr::Three::new(needle1, needle2, needle3)
            .rfind_raw(start, end)
    }
}

/// Count all matching bytes, but using raw pointers to represent the haystack.
///
/// # Safety
///
/// Pointers must be valid. See `One::count_raw`.
#[inline]
unsafe fn count_raw(needle: u8, start: *const u8, end: *const u8) -> usize {
    #[cfg(target_arch = "x86_64")]
    {
        crate::arch::x86_64::memchr::count_raw(needle, start, end)
    }
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    {
        crate::arch::wasm32::memchr::count_raw(needle, start, end)
    }
    #[cfg(target_arch = "aarch64")]
    {
        crate::arch::aarch64::memchr::count_raw(needle, start, end)
    }
    #[cfg(not(any(
        target_arch = "x86_64",
        all(target_arch = "wasm32", target_feature = "simd128"),
        target_arch = "aarch64"
    )))]
    {
        crate::arch::all::memchr::One::new(needle).count_raw(start, end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward1_iter() {
        crate::tests::memchr::Runner::new(1).forward_iter(
            |haystack, needles| {
                Some(memchr_iter(needles[0], haystack).collect())
            },
        )
    }

    #[test]
    fn forward1_oneshot() {
        crate::tests::memchr::Runner::new(1).forward_oneshot(
            |haystack, needles| Some(memchr(needles[0], haystack)),
        )
    }

    #[test]
    fn reverse1_iter() {
        crate::tests::memchr::Runner::new(1).reverse_iter(
            |haystack, needles| {
                Some(memrchr_iter(needles[0], haystack).collect())
            },
        )
    }

    #[test]
    fn reverse1_oneshot() {
        crate::tests::memchr::Runner::new(1).reverse_oneshot(
            |haystack, needles| Some(memrchr(needles[0], haystack)),
        )
    }

    #[test]
    fn count1_iter() {
        crate::tests::memchr::Runner::new(1).count_iter(|haystack, needles| {
            Some(memchr_iter(needles[0], haystack).count())
        })
    }

    #[test]
    fn forward2_iter() {
        crate::tests::memchr::Runner::new(2).forward_iter(
            |haystack, needles| {
                let n1 = needles.get(0).copied()?;
                let n2 = needles.get(1).copied()?;
                Some(memchr2_iter(n1, n2, haystack).collect())
            },
        )
    }

    #[test]
    fn forward2_oneshot() {
        crate::tests::memchr::Runner::new(2).forward_oneshot(
            |haystack, needles| {
                let n1 = needles.get(0).copied()?;
                let n2 = needles.get(1).copied()?;
                Some(memchr2(n1, n2, haystack))
            },
        )
    }

    #[test]
    fn reverse2_iter() {
        crate::tests::memchr::Runner::new(2).reverse_iter(
            |haystack, needles| {
                let n1 = needles.get(0).copied()?;
                let n2 = needles.get(1).copied()?;
                Some(memrchr2_iter(n1, n2, haystack).collect())
            },
        )
    }

    #[test]
    fn reverse2_oneshot() {
        crate::tests::memchr::Runner::new(2).reverse_oneshot(
            |haystack, needles| {
                let n1 = needles.get(0).copied()?;
                let n2 = needles.get(1).copied()?;
                Some(memrchr2(n1, n2, haystack))
            },
        )
    }

    #[test]
    fn forward3_iter() {
        crate::tests::memchr::Runner::new(3).forward_iter(
            |haystack, needles| {
                let n1 = needles.get(0).copied()?;
                let n2 = needles.get(1).copied()?;
                let n3 = needles.get(2).copied()?;
                Some(memchr3_iter(n1, n2, n3, haystack).collect())
            },
        )
    }

    #[test]
    fn forward3_oneshot() {
        crate::tests::memchr::Runner::new(3).forward_oneshot(
            |haystack, needles| {
                let n1 = needles.get(0).copied()?;
                let n2 = needles.get(1).copied()?;
                let n3 = needles.get(2).copied()?;
                Some(memchr3(n1, n2, n3, haystack))
            },
        )
    }

    #[test]
    fn reverse3_iter() {
        crate::tests::memchr::Runner::new(3).reverse_iter(
            |haystack, needles| {
                let n1 = needles.get(0).copied()?;
                let n2 = needles.get(1).copied()?;
                let n3 = needles.get(2).copied()?;
                Some(memrchr3_iter(n1, n2, n3, haystack).collect())
            },
        )
    }

    #[test]
    fn reverse3_oneshot() {
        crate::tests::memchr::Runner::new(3).reverse_oneshot(
            |haystack, needles| {
                let n1 = needles.get(0).copied()?;
                let n2 = needles.get(1).copied()?;
                let n3 = needles.get(2).copied()?;
                Some(memrchr3(n1, n2, n3, haystack))
            },
        )
    }

    // Prior to memchr 2.6, the memchr iterators both implemented Send and
    // Sync. But in memchr 2.6, the iterator changed to use raw pointers
    // internally and I didn't add explicit Send/Sync impls. This ended up
    // regressing the API. This test ensures we don't do that again.
    //
    // See: https://github.com/BurntSushi/memchr/issues/133
    #[test]
    fn sync_regression() {
        use core::panic::{RefUnwindSafe, UnwindSafe};

        fn assert_send_sync<T: Send + Sync + UnwindSafe + RefUnwindSafe>() {}
        assert_send_sync::<Memchr>();
        assert_send_sync::<Memchr2>();
        assert_send_sync::<Memchr3>()
    }
}
