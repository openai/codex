//! Hints to the compiler that affects how code should be emitted or optimized.

#![expect(
    dead_code,
    reason = "may be used in the future and has minimal overhead"
)]

/// Indicate that a given branch is **not** likely to be taken, relatively speaking.
#[inline(always)]
#[cold]
pub(crate) const fn cold_path() {}

/// Indicate that a given condition is likely to be true.
#[inline(always)]
pub(crate) const fn likely(b: bool) -> bool {
    if !b {
        cold_path();
    }
    b
}

/// Indicate that a given condition is likely to be false.
#[inline(always)]
pub(crate) const fn unlikely(b: bool) -> bool {
    if b {
        cold_path();
    }
    b
}
