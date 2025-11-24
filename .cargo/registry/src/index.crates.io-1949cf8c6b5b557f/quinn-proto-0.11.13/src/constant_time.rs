// This function is non-inline to prevent the optimizer from looking inside it.
#[inline(never)]
fn constant_time_ne(a: &[u8], b: &[u8]) -> u8 {
    assert!(a.len() == b.len());

    // These useless slices make the optimizer elide the bounds checks.
    // See the comment in clone_from_slice() added on Rust commit 6a7bc47.
    let len = a.len();
    let a = &a[..len];
    let b = &b[..len];

    let mut tmp = 0;
    for i in 0..len {
        tmp |= a[i] ^ b[i];
    }
    tmp // The compare with 0 must happen outside this function.
}

/// Compares byte strings in constant time.
pub(crate) fn eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && constant_time_ne(a, b) == 0
}
