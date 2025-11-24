/*!
Wrapper routines for `memchr` and friends.

These routines efficiently dispatch to the best implementation based on what
the CPU supports.
*/

/// Provides a way to run a memchr-like function while amortizing the cost of
/// runtime CPU feature detection.
///
/// This works by loading a function pointer from an atomic global. Initially,
/// this global is set to a function that does CPU feature detection. For
/// example, if AVX2 is enabled, then the AVX2 implementation is used.
/// Otherwise, at least on x86_64, the SSE2 implementation is used. (And
/// in some niche cases, if SSE2 isn't available, then the architecture
/// independent fallback implementation is used.)
///
/// After the first call to this function, the atomic global is replaced with
/// the specific AVX2, SSE2 or fallback routine chosen. Subsequent calls then
/// will directly call the chosen routine instead of needing to go through the
/// CPU feature detection branching again.
///
/// This particular macro is specifically written to provide the implementation
/// of functions with the following signature:
///
/// ```ignore
/// fn memchr(needle1: u8, start: *const u8, end: *const u8) -> Option<usize>;
/// ```
///
/// Where you can also have `memchr2` and `memchr3`, but with `needle2` and
/// `needle3`, respectively. The `start` and `end` parameters correspond to the
/// start and end of the haystack, respectively.
///
/// We use raw pointers here instead of the more obvious `haystack: &[u8]` so
/// that the function is compatible with our lower level iterator logic that
/// operates on raw pointers. We use this macro to implement "raw" memchr
/// routines with the signature above, and then define memchr routines using
/// regular slices on top of them.
///
/// Note that we use `#[cfg(target_feature = "sse2")]` below even though
/// it shouldn't be strictly necessary because without it, it seems to
/// cause the compiler to blow up. I guess it can't handle a function
/// pointer being created with a sse target feature? Dunno. See the
/// `build-for-x86-64-but-non-sse-target` CI job if you want to experiment with
/// this.
///
/// # Safety
///
/// Primarily callers must ensure that `$fnty` is a correct function pointer
/// type and not something else.
///
/// Callers must also ensure that `$memchrty::$memchrfind` corresponds to a
/// routine that returns a valid function pointer when a match is found. That
/// is, a pointer that is `>= start` and `< end`.
///
/// Callers must also ensure that the `$hay_start` and `$hay_end` identifiers
/// correspond to valid pointers.
macro_rules! unsafe_ifunc {
    (
        $memchrty:ident,
        $memchrfind:ident,
        $fnty:ty,
        $retty:ty,
        $hay_start:ident,
        $hay_end:ident,
        $($needle:ident),+
    ) => {{
        #![allow(unused_unsafe)]

        use core::sync::atomic::{AtomicPtr, Ordering};

        type Fn = *mut ();
        type RealFn = $fnty;
        static FN: AtomicPtr<()> = AtomicPtr::new(detect as Fn);

        #[cfg(target_feature = "sse2")]
        #[target_feature(enable = "sse2", enable = "avx2")]
        unsafe fn find_avx2(
            $($needle: u8),+,
            $hay_start: *const u8,
            $hay_end: *const u8,
        ) -> $retty {
            use crate::arch::x86_64::avx2::memchr::$memchrty;
            $memchrty::new_unchecked($($needle),+)
                .$memchrfind($hay_start, $hay_end)
        }

        #[cfg(target_feature = "sse2")]
        #[target_feature(enable = "sse2")]
        unsafe fn find_sse2(
            $($needle: u8),+,
            $hay_start: *const u8,
            $hay_end: *const u8,
        ) -> $retty {
            use crate::arch::x86_64::sse2::memchr::$memchrty;
            $memchrty::new_unchecked($($needle),+)
                .$memchrfind($hay_start, $hay_end)
        }

        unsafe fn find_fallback(
            $($needle: u8),+,
            $hay_start: *const u8,
            $hay_end: *const u8,
        ) -> $retty {
            use crate::arch::all::memchr::$memchrty;
            $memchrty::new($($needle),+).$memchrfind($hay_start, $hay_end)
        }

        unsafe fn detect(
            $($needle: u8),+,
            $hay_start: *const u8,
            $hay_end: *const u8,
        ) -> $retty {
            let fun = {
                #[cfg(not(target_feature = "sse2"))]
                {
                    debug!(
                        "no sse2 feature available, using fallback for {}",
                        stringify!($memchrty),
                    );
                    find_fallback as RealFn
                }
                #[cfg(target_feature = "sse2")]
                {
                    use crate::arch::x86_64::{sse2, avx2};
                    if avx2::memchr::$memchrty::is_available() {
                        debug!("chose AVX2 for {}", stringify!($memchrty));
                        find_avx2 as RealFn
                    } else if sse2::memchr::$memchrty::is_available() {
                        debug!("chose SSE2 for {}", stringify!($memchrty));
                        find_sse2 as RealFn
                    } else {
                        debug!("chose fallback for {}", stringify!($memchrty));
                        find_fallback as RealFn
                    }
                }
            };
            FN.store(fun as Fn, Ordering::Relaxed);
            // SAFETY: The only thing we need to uphold here is the
            // `#[target_feature]` requirements. Since we check is_available
            // above before using the corresponding implementation, we are
            // guaranteed to only call code that is supported on the current
            // CPU.
            fun($($needle),+, $hay_start, $hay_end)
        }

        // SAFETY: By virtue of the caller contract, RealFn is a function
        // pointer, which is always safe to transmute with a *mut (). Also,
        // since we use $memchrty::is_available, it is guaranteed to be safe
        // to call $memchrty::$memchrfind.
        unsafe {
            let fun = FN.load(Ordering::Relaxed);
            core::mem::transmute::<Fn, RealFn>(fun)(
                $($needle),+,
                $hay_start,
                $hay_end,
            )
        }
    }};
}

// The routines below dispatch to AVX2, SSE2 or a fallback routine based on
// what's available in the current environment. The secret sauce here is that
// we only check for which one to use approximately once, and then "cache" that
// choice into a global function pointer. Subsequent invocations then just call
// the appropriate function directly.

/// memchr, but using raw pointers to represent the haystack.
///
/// # Safety
///
/// Pointers must be valid. See `One::find_raw`.
#[inline(always)]
pub(crate) fn memchr_raw(
    n1: u8,
    start: *const u8,
    end: *const u8,
) -> Option<*const u8> {
    // SAFETY: We provide a valid function pointer type.
    unsafe_ifunc!(
        One,
        find_raw,
        unsafe fn(u8, *const u8, *const u8) -> Option<*const u8>,
        Option<*const u8>,
        start,
        end,
        n1
    )
}

/// memrchr, but using raw pointers to represent the haystack.
///
/// # Safety
///
/// Pointers must be valid. See `One::rfind_raw`.
#[inline(always)]
pub(crate) fn memrchr_raw(
    n1: u8,
    start: *const u8,
    end: *const u8,
) -> Option<*const u8> {
    // SAFETY: We provide a valid function pointer type.
    unsafe_ifunc!(
        One,
        rfind_raw,
        unsafe fn(u8, *const u8, *const u8) -> Option<*const u8>,
        Option<*const u8>,
        start,
        end,
        n1
    )
}

/// memchr2, but using raw pointers to represent the haystack.
///
/// # Safety
///
/// Pointers must be valid. See `Two::find_raw`.
#[inline(always)]
pub(crate) fn memchr2_raw(
    n1: u8,
    n2: u8,
    start: *const u8,
    end: *const u8,
) -> Option<*const u8> {
    // SAFETY: We provide a valid function pointer type.
    unsafe_ifunc!(
        Two,
        find_raw,
        unsafe fn(u8, u8, *const u8, *const u8) -> Option<*const u8>,
        Option<*const u8>,
        start,
        end,
        n1,
        n2
    )
}

/// memrchr2, but using raw pointers to represent the haystack.
///
/// # Safety
///
/// Pointers must be valid. See `Two::rfind_raw`.
#[inline(always)]
pub(crate) fn memrchr2_raw(
    n1: u8,
    n2: u8,
    start: *const u8,
    end: *const u8,
) -> Option<*const u8> {
    // SAFETY: We provide a valid function pointer type.
    unsafe_ifunc!(
        Two,
        rfind_raw,
        unsafe fn(u8, u8, *const u8, *const u8) -> Option<*const u8>,
        Option<*const u8>,
        start,
        end,
        n1,
        n2
    )
}

/// memchr3, but using raw pointers to represent the haystack.
///
/// # Safety
///
/// Pointers must be valid. See `Three::find_raw`.
#[inline(always)]
pub(crate) fn memchr3_raw(
    n1: u8,
    n2: u8,
    n3: u8,
    start: *const u8,
    end: *const u8,
) -> Option<*const u8> {
    // SAFETY: We provide a valid function pointer type.
    unsafe_ifunc!(
        Three,
        find_raw,
        unsafe fn(u8, u8, u8, *const u8, *const u8) -> Option<*const u8>,
        Option<*const u8>,
        start,
        end,
        n1,
        n2,
        n3
    )
}

/// memrchr3, but using raw pointers to represent the haystack.
///
/// # Safety
///
/// Pointers must be valid. See `Three::rfind_raw`.
#[inline(always)]
pub(crate) fn memrchr3_raw(
    n1: u8,
    n2: u8,
    n3: u8,
    start: *const u8,
    end: *const u8,
) -> Option<*const u8> {
    // SAFETY: We provide a valid function pointer type.
    unsafe_ifunc!(
        Three,
        rfind_raw,
        unsafe fn(u8, u8, u8, *const u8, *const u8) -> Option<*const u8>,
        Option<*const u8>,
        start,
        end,
        n1,
        n2,
        n3
    )
}

/// Count all matching bytes, but using raw pointers to represent the haystack.
///
/// # Safety
///
/// Pointers must be valid. See `One::count_raw`.
#[inline(always)]
pub(crate) fn count_raw(n1: u8, start: *const u8, end: *const u8) -> usize {
    // SAFETY: We provide a valid function pointer type.
    unsafe_ifunc!(
        One,
        count_raw,
        unsafe fn(u8, *const u8, *const u8) -> usize,
        usize,
        start,
        end,
        n1
    )
}
