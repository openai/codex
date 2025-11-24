# strsim-rs

[![Crates.io](https://img.shields.io/crates/v/strsim.svg)](https://crates.io/crates/strsim)
[![Crates.io](https://img.shields.io/crates/l/strsim.svg?maxAge=2592000)](https://github.com/rapidfuzz/strsim-rs/blob/main/LICENSE)
[![CI status](https://github.com/rapidfuzz/strsim-rs/workflows/CI/badge.svg)](https://github.com/rapidfuzz/strsim-rs/actions?query=branch%3Amain)
[![unsafe forbidden](https://img.shields.io/badge/unsafe-forbidden-success.svg)](https://github.com/rust-secure-code/safety-dance/)

[Rust](https://www.rust-lang.org) implementations of [string similarity metrics]:
  - [Hamming]
  - [Levenshtein] - distance & normalized
  - [Optimal string alignment]
  - [Damerau-Levenshtein] - distance & normalized
  - [Jaro and Jaro-Winkler]
  - [Sørensen-Dice]

The normalized versions return values between `0.0` and `1.0`, where `1.0` means
an exact match.

There are also generic versions of the functions for non-string inputs.

## Installation

`strsim` is available on [crates.io](https://crates.io/crates/strsim). Add it to
your project:

```sh
cargo add strsim
```

## Usage

Go to [Docs.rs](https://docs.rs/strsim/) for the full documentation. You can
also clone the repo, and run `$ cargo doc --open`.

### Examples

```rust
extern crate strsim;

use strsim::{hamming, levenshtein, normalized_levenshtein, osa_distance,
             damerau_levenshtein, normalized_damerau_levenshtein, jaro,
             jaro_winkler, sorensen_dice};

fn main() {
    match hamming("hamming", "hammers") {
        Ok(distance) => assert_eq!(3, distance),
        Err(why) => panic!("{:?}", why)
    }

    assert_eq!(levenshtein("kitten", "sitting"), 3);

    assert!((normalized_levenshtein("kitten", "sitting") - 0.571).abs() < 0.001);

    assert_eq!(osa_distance("ac", "cba"), 3);

    assert_eq!(damerau_levenshtein("ac", "cba"), 2);

    assert!((normalized_damerau_levenshtein("levenshtein", "löwenbräu") - 0.272).abs() <
            0.001);

    assert!((jaro("Friedrich Nietzsche", "Jean-Paul Sartre") - 0.392).abs() <
            0.001);

    assert!((jaro_winkler("cheeseburger", "cheese fries") - 0.911).abs() <
            0.001);

    assert_eq!(sorensen_dice("web applications", "applications of the web"),
        0.7878787878787878);
}
```

Using the generic versions of the functions:

```rust
extern crate strsim;

use strsim::generic_levenshtein;

fn main() {
    assert_eq!(2, generic_levenshtein(&[1, 2, 3], &[0, 2, 5]));
}
```

## Contributing

If you don't want to install Rust itself, you can run `$ ./dev` for a
development CLI if you have [Docker] installed.

Benchmarks require a Nightly toolchain. Run `$ cargo +nightly bench`.

## License

[MIT](https://github.com/rapidfuzz/strsim-rs/blob/main/LICENSE)

[string similarity metrics]:http://en.wikipedia.org/wiki/String_metric
[Damerau-Levenshtein]:http://en.wikipedia.org/wiki/Damerau%E2%80%93Levenshtein_distance
[Jaro and Jaro-Winkler]:http://en.wikipedia.org/wiki/Jaro%E2%80%93Winkler_distance
[Levenshtein]:http://en.wikipedia.org/wiki/Levenshtein_distance
[Hamming]:http://en.wikipedia.org/wiki/Hamming_distance
[Optimal string alignment]:https://en.wikipedia.org/wiki/Damerau%E2%80%93Levenshtein_distance#Optimal_string_alignment_distance
[Sørensen-Dice]:http://en.wikipedia.org/wiki/S%C3%B8rensen%E2%80%93Dice_coefficient
[Docker]:https://docs.docker.com/engine/installation/
