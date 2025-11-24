# tiny-keccak

An implementation of Keccak derived functions specified in [`FIPS-202`], [`SP800-185`] and [`KangarooTwelve`].

[![Build Status][travis-image]][travis-url]

[travis-image]: https://travis-ci.org/debris/tiny-keccak.svg?branch=master
[travis-url]: https://travis-ci.org/debris/tiny-keccak
[`FIPS-202`]: https://nvlpubs.nist.gov/nistpubs/FIPS/NIST.FIPS.202.pdf
[`SP800-185`]: https://nvlpubs.nist.gov/nistpubs/SpecialPublications/NIST.SP.800-185.pdf
[`KangarooTwelve`]: https://eprint.iacr.org/2016/770.pdf

[`Documentation`](https://docs.rs/tiny-keccak)

The `Keccak-f[1600]` permutation is fully unrolled; it's nearly as fast
as the Keccak team's optimized permutation.

## Usage

In your `Cargo.toml` specify what features (hash functions, you are intending to use).
Available options are: `cshake`, `fips202`, `k12`, `keccak`, `kmac`, `parallel_hash`, `sha3`,
`shake`, `sp800`, `tuple_hash`.

```toml
[dependencies]
tiny-keccak = { version = "2.0", features = ["sha3"] }
```

## Example

```rust
use tiny_keccak::Sha3;

fn main() {
    let mut sha3 = Sha3::v256();
    let mut output = [0u8; 32];
    let expected = b"\
        \x64\x4b\xcc\x7e\x56\x43\x73\x04\x09\x99\xaa\xc8\x9e\x76\x22\xf3\
        \xca\x71\xfb\xa1\xd9\x72\xfd\x94\xa3\x1c\x3b\xfb\xf2\x4e\x39\x38\
    ";

    sha3.update(b"hello");
    sha3.update(b" ");
    sha3.update(b"world");
    sha3.finalize(&mut output);

    assert_eq!(expected, &output);
}
```

## Benchmarks

Benchmarked with [rust-crypto](https://github.com/RustCrypto) sha3 on:

```
MacBook Pro (Retina, 15-inch, Mid 2015)
2,5 GHz Intel Core i7
16 GB 1600 MHz DDR3
Intel Iris Pro 1536 MB
```

Benchmark code is available [here](https://github.com/debris/tiny-keccak/blob/master/comparison/benches/sha3.rs)

```
running 4 tests
test rust_crypto_sha3_256_input_32_bytes   ... bench:         677 ns/iter (+/- 113) = 47 MB/s
test rust_crypto_sha3_256_input_4096_bytes ... bench:      17,619 ns/iter (+/- 4,174) = 232 MB/s
test tiny_keccak_sha3_256_input_32_bytes   ... bench:         569 ns/iter (+/- 204) = 56 MB/s
test tiny_keccak_sha3_256_input_4096_bytes ... bench:      17,185 ns/iter (+/- 4,575) = 238 MB/s
```
