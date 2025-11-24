/*!
Contains architecture independent routines.

These routines are often used as a "fallback" implementation when the more
specialized architecture dependent routines are unavailable.
*/

pub mod memchr;
pub mod packedpair;
pub mod rabinkarp;
#[cfg(feature = "alloc")]
pub mod shiftor;
pub mod twoway;

/// Returns true if and only if `needle` is a prefix of `haystack`.
///
/// This uses a latency optimized variant of `memcmp` internally which *might*
/// make this faster for very short strings.
///
/// # Inlining
///
/// This routine is marked `inline(always)`. If you want to call this function
/// in a way that is not always inlined, you'll need to wrap a call to it in
/// another function that is marked as `inline(never)` or just `inline`.
#[inline(always)]
pub fn is_prefix(haystack: &[u8], needle: &[u8]) -> bool {
    needle.len() <= haystack.len()
        && is_equal(&haystack[..needle.len()], needle)
}

/// Returns true if and only if `needle` is a suffix of `haystack`.
///
/// This uses a latency optimized variant of `memcmp` internally which *might*
/// make this faster for very short strings.
///
/// # Inlining
///
/// This routine is marked `inline(always)`. If you want to call this function
/// in a way that is not always inlined, you'll need to wrap a call to it in
/// another function that is marked as `inline(never)` or just `inline`.
#[inline(always)]
pub fn is_suffix(haystack: &[u8], needle: &[u8]) -> bool {
    needle.len() <= haystack.len()
        && is_equal(&haystack[haystack.len() - needle.len()..], needle)
}

/// Compare corresponding bytes in `x` and `y` for equality.
///
/// That is, this returns true if and only if `x.len() == y.len()` and
/// `x[i] == y[i]` for all `0 <= i < x.len()`.
///
/// # Inlining
///
/// This routine is marked `inline(always)`. If you want to call this function
/// in a way that is not always inlined, you'll need to wrap a call to it in
/// another function that is marked as `inline(never)` or just `inline`.
///
/// # Motivation
///
/// Why not use slice equality instead? Well, slice equality usually results in
/// a call out to the current platform's `libc` which might not be inlineable
/// or have other overhead. This routine isn't guaranteed to be a win, but it
/// might be in some cases.
#[inline(always)]
pub fn is_equal(x: &[u8], y: &[u8]) -> bool {
    if x.len() != y.len() {
        return false;
    }
    // SAFETY: Our pointers are derived directly from borrowed slices which
    // uphold all of our safety guarantees except for length. We account for
    // length with the check above.
    unsafe { is_equal_raw(x.as_ptr(), y.as_ptr(), x.len()) }
}

/// Compare `n` bytes at the given pointers for equality.
///
/// This returns true if and only if `*x.add(i) == *y.add(i)` for all
/// `0 <= i < n`.
///
/// # Inlining
///
/// This routine is marked `inline(always)`. If you want to call this function
/// in a way that is not always inlined, you'll need to wrap a call to it in
/// another function that is marked as `inline(never)` or just `inline`.
///
/// # Motivation
///
/// Why not use slice equality instead? Well, slice equality usually results in
/// a call out to the current platform's `libc` which might not be inlineable
/// or have other overhead. This routine isn't guaranteed to be a win, but it
/// might be in some cases.
///
/// # Safety
///
/// * Both `x` and `y` must be valid for reads of up to `n` bytes.
/// * Both `x` and `y` must point to an initialized value.
/// * Both `x` and `y` must each point to an allocated object and
/// must either be in bounds or at most one byte past the end of the
/// allocated object. `x` and `y` do not need to point to the same allocated
/// object, but they may.
/// * Both `x` and `y` must be _derived from_ a pointer to their respective
/// allocated objects.
/// * The distance between `x` and `x+n` must not overflow `isize`. Similarly
/// for `y` and `y+n`.
/// * The distance being in bounds must not rely on "wrapping around" the
/// address space.
#[inline(always)]
pub unsafe fn is_equal_raw(
    mut x: *const u8,
    mut y: *const u8,
    mut n: usize,
) -> bool {
    // When we have 4 or more bytes to compare, then proceed in chunks of 4 at
    // a time using unaligned loads.
    //
    // Also, why do 4 byte loads instead of, say, 8 byte loads? The reason is
    // that this particular version of memcmp is likely to be called with tiny
    // needles. That means that if we do 8 byte loads, then a higher proportion
    // of memcmp calls will use the slower variant above. With that said, this
    // is a hypothesis and is only loosely supported by benchmarks. There's
    // likely some improvement that could be made here. The main thing here
    // though is to optimize for latency, not throughput.

    // SAFETY: The caller is responsible for ensuring the pointers we get are
    // valid and readable for at least `n` bytes. We also do unaligned loads,
    // so there's no need to ensure we're aligned. (This is justified by this
    // routine being specifically for short strings.)
    while n >= 4 {
        let vx = x.cast::<u32>().read_unaligned();
        let vy = y.cast::<u32>().read_unaligned();
        if vx != vy {
            return false;
        }
        x = x.add(4);
        y = y.add(4);
        n -= 4;
    }
    // If we don't have enough bytes to do 4-byte at a time loads, then
    // do partial loads. Note that I used to have a byte-at-a-time
    // loop here and that turned out to be quite a bit slower for the
    // memmem/pathological/defeat-simple-vector-alphabet benchmark.
    if n >= 2 {
        let vx = x.cast::<u16>().read_unaligned();
        let vy = y.cast::<u16>().read_unaligned();
        if vx != vy {
            return false;
        }
        x = x.add(2);
        y = y.add(2);
        n -= 2;
    }
    if n > 0 {
        if x.read() != y.read() {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equals_different_lengths() {
        assert!(!is_equal(b"", b"a"));
        assert!(!is_equal(b"a", b""));
        assert!(!is_equal(b"ab", b"a"));
        assert!(!is_equal(b"a", b"ab"));
    }

    #[test]
    fn equals_mismatch() {
        let one_mismatch = [
            (&b"a"[..], &b"x"[..]),
            (&b"ab"[..], &b"ax"[..]),
            (&b"abc"[..], &b"abx"[..]),
            (&b"abcd"[..], &b"abcx"[..]),
            (&b"abcde"[..], &b"abcdx"[..]),
            (&b"abcdef"[..], &b"abcdex"[..]),
            (&b"abcdefg"[..], &b"abcdefx"[..]),
            (&b"abcdefgh"[..], &b"abcdefgx"[..]),
            (&b"abcdefghi"[..], &b"abcdefghx"[..]),
            (&b"abcdefghij"[..], &b"abcdefghix"[..]),
            (&b"abcdefghijk"[..], &b"abcdefghijx"[..]),
            (&b"abcdefghijkl"[..], &b"abcdefghijkx"[..]),
            (&b"abcdefghijklm"[..], &b"abcdefghijklx"[..]),
            (&b"abcdefghijklmn"[..], &b"abcdefghijklmx"[..]),
        ];
        for (x, y) in one_mismatch {
            assert_eq!(x.len(), y.len(), "lengths should match");
            assert!(!is_equal(x, y));
            assert!(!is_equal(y, x));
        }
    }

    #[test]
    fn equals_yes() {
        assert!(is_equal(b"", b""));
        assert!(is_equal(b"a", b"a"));
        assert!(is_equal(b"ab", b"ab"));
        assert!(is_equal(b"abc", b"abc"));
        assert!(is_equal(b"abcd", b"abcd"));
        assert!(is_equal(b"abcde", b"abcde"));
        assert!(is_equal(b"abcdef", b"abcdef"));
        assert!(is_equal(b"abcdefg", b"abcdefg"));
        assert!(is_equal(b"abcdefgh", b"abcdefgh"));
        assert!(is_equal(b"abcdefghi", b"abcdefghi"));
    }

    #[test]
    fn prefix() {
        assert!(is_prefix(b"", b""));
        assert!(is_prefix(b"a", b""));
        assert!(is_prefix(b"ab", b""));
        assert!(is_prefix(b"foo", b"foo"));
        assert!(is_prefix(b"foobar", b"foo"));

        assert!(!is_prefix(b"foo", b"fob"));
        assert!(!is_prefix(b"foobar", b"fob"));
    }

    #[test]
    fn suffix() {
        assert!(is_suffix(b"", b""));
        assert!(is_suffix(b"a", b""));
        assert!(is_suffix(b"ab", b""));
        assert!(is_suffix(b"foo", b"foo"));
        assert!(is_suffix(b"foobar", b"bar"));

        assert!(!is_suffix(b"foo", b"goo"));
        assert!(!is_suffix(b"foobar", b"gar"));
    }
}
