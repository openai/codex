#[macro_use]
pub(crate) mod memchr;
pub(crate) mod packedpair;
#[macro_use]
pub(crate) mod substring;

// For debugging, particularly in CI, print out the byte order of the current
// target.
#[test]
fn byte_order() {
    #[cfg(target_endian = "little")]
    std::eprintln!("LITTLE ENDIAN");
    #[cfg(target_endian = "big")]
    std::eprintln!("BIG ENDIAN");
}
