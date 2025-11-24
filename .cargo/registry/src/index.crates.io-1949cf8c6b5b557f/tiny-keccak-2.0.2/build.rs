#[cfg(not(any(
    feature = "keccak",
    feature = "shake",
    feature = "sha3",
    feature = "cshake",
    feature = "kmac",
    feature = "tuple_hash",
    feature = "parallel_hash",
    feature = "k12",
    feature = "fips202",
    feature = "sp800"
)))]
compile_error!(
    "You need to specify at least one hash function you intend to use. \
    Available options:\n\
    keccak, shake, sha3, cshake, kmac, tuple_hash, parallel_hash, k12, fips202, sp800\n\
    e.g.\n\
    tiny-keccak = { version = \"2.0.0\", features = [\"sha3\"] }"
);

fn main() {
}
