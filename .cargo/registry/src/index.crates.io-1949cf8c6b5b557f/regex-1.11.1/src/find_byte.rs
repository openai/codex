/// Searches for the given needle in the given haystack.
///
/// If the perf-literal feature is enabled, then this uses the super optimized
/// memchr crate. Otherwise, it uses the naive byte-at-a-time implementation.
pub(crate) fn find_byte(needle: u8, haystack: &[u8]) -> Option<usize> {
    #[cfg(not(feature = "perf-literal"))]
    fn imp(needle: u8, haystack: &[u8]) -> Option<usize> {
        haystack.iter().position(|&b| b == needle)
    }

    #[cfg(feature = "perf-literal")]
    fn imp(needle: u8, haystack: &[u8]) -> Option<usize> {
        memchr::memchr(needle, haystack)
    }

    imp(needle, haystack)
}
