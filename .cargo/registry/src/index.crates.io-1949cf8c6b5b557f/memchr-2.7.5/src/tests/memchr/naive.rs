pub(crate) fn memchr(n1: u8, haystack: &[u8]) -> Option<usize> {
    haystack.iter().position(|&b| b == n1)
}

pub(crate) fn memchr2(n1: u8, n2: u8, haystack: &[u8]) -> Option<usize> {
    haystack.iter().position(|&b| b == n1 || b == n2)
}

pub(crate) fn memchr3(
    n1: u8,
    n2: u8,
    n3: u8,
    haystack: &[u8],
) -> Option<usize> {
    haystack.iter().position(|&b| b == n1 || b == n2 || b == n3)
}

pub(crate) fn memrchr(n1: u8, haystack: &[u8]) -> Option<usize> {
    haystack.iter().rposition(|&b| b == n1)
}

pub(crate) fn memrchr2(n1: u8, n2: u8, haystack: &[u8]) -> Option<usize> {
    haystack.iter().rposition(|&b| b == n1 || b == n2)
}

pub(crate) fn memrchr3(
    n1: u8,
    n2: u8,
    n3: u8,
    haystack: &[u8],
) -> Option<usize> {
    haystack.iter().rposition(|&b| b == n1 || b == n2 || b == n3)
}
