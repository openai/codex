use memchr::{memchr, memchr2, memchr3, memrchr, memrchr2, memrchr3};

mod scalar;

#[inline]
fn build_table(byteset: &[u8]) -> [u8; 256] {
    let mut table = [0u8; 256];
    for &b in byteset {
        table[b as usize] = 1;
    }
    table
}

#[inline]
pub(crate) fn find(haystack: &[u8], byteset: &[u8]) -> Option<usize> {
    match byteset.len() {
        0 => None,
        1 => memchr(byteset[0], haystack),
        2 => memchr2(byteset[0], byteset[1], haystack),
        3 => memchr3(byteset[0], byteset[1], byteset[2], haystack),
        _ => {
            let table = build_table(byteset);
            scalar::forward_search_bytes(haystack, |b| table[b as usize] != 0)
        }
    }
}

#[inline]
pub(crate) fn rfind(haystack: &[u8], byteset: &[u8]) -> Option<usize> {
    match byteset.len() {
        0 => None,
        1 => memrchr(byteset[0], haystack),
        2 => memrchr2(byteset[0], byteset[1], haystack),
        3 => memrchr3(byteset[0], byteset[1], byteset[2], haystack),
        _ => {
            let table = build_table(byteset);
            scalar::reverse_search_bytes(haystack, |b| table[b as usize] != 0)
        }
    }
}

#[inline]
pub(crate) fn find_not(haystack: &[u8], byteset: &[u8]) -> Option<usize> {
    if haystack.is_empty() {
        return None;
    }
    match byteset.len() {
        0 => Some(0),
        1 => scalar::inv_memchr(byteset[0], haystack),
        2 => scalar::forward_search_bytes(haystack, |b| {
            b != byteset[0] && b != byteset[1]
        }),
        3 => scalar::forward_search_bytes(haystack, |b| {
            b != byteset[0] && b != byteset[1] && b != byteset[2]
        }),
        _ => {
            let table = build_table(byteset);
            scalar::forward_search_bytes(haystack, |b| table[b as usize] == 0)
        }
    }
}
#[inline]
pub(crate) fn rfind_not(haystack: &[u8], byteset: &[u8]) -> Option<usize> {
    if haystack.is_empty() {
        return None;
    }
    match byteset.len() {
        0 => Some(haystack.len() - 1),
        1 => scalar::inv_memrchr(byteset[0], haystack),
        2 => scalar::reverse_search_bytes(haystack, |b| {
            b != byteset[0] && b != byteset[1]
        }),
        3 => scalar::reverse_search_bytes(haystack, |b| {
            b != byteset[0] && b != byteset[1] && b != byteset[2]
        }),
        _ => {
            let table = build_table(byteset);
            scalar::reverse_search_bytes(haystack, |b| table[b as usize] == 0)
        }
    }
}

#[cfg(all(test, feature = "std", not(miri)))]
mod tests {
    use alloc::vec::Vec;

    quickcheck::quickcheck! {
        fn qc_byteset_forward_matches_naive(
            haystack: Vec<u8>,
            needles: Vec<u8>
        ) -> bool {
            super::find(&haystack, &needles)
                == haystack.iter().position(|b| needles.contains(b))
        }
        fn qc_byteset_backwards_matches_naive(
            haystack: Vec<u8>,
            needles: Vec<u8>
        ) -> bool {
            super::rfind(&haystack, &needles)
                == haystack.iter().rposition(|b| needles.contains(b))
        }
        fn qc_byteset_forward_not_matches_naive(
            haystack: Vec<u8>,
            needles: Vec<u8>
        ) -> bool {
            super::find_not(&haystack, &needles)
                == haystack.iter().position(|b| !needles.contains(b))
        }
        fn qc_byteset_backwards_not_matches_naive(
            haystack: Vec<u8>,
            needles: Vec<u8>
        ) -> bool {
            super::rfind_not(&haystack, &needles)
                == haystack.iter().rposition(|b| !needles.contains(b))
        }
    }
}
