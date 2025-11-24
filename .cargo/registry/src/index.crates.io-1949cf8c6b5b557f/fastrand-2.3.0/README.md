# fastrand

[![Build](https://github.com/smol-rs/fastrand/workflows/CI/badge.svg)](
https://github.com/smol-rs/fastrand/actions)
[![License](https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue.svg)](
https://github.com/smol-rs/fastrand)
[![Cargo](https://img.shields.io/crates/v/fastrand.svg)](
https://crates.io/crates/fastrand)
[![Documentation](https://docs.rs/fastrand/badge.svg)](
https://docs.rs/fastrand)

A simple and fast random number generator.

The implementation uses [Wyrand](https://github.com/wangyi-fudan/wyhash), a simple and fast
generator but **not** cryptographically secure.

## Examples

Flip a coin:

```rust
if fastrand::bool() {
    println!("heads");
} else {
    println!("tails");
}
```

Generate a random `i32`:

```rust
let num = fastrand::i32(..);
```

Choose a random element in an array:

```rust
let v = vec![1, 2, 3, 4, 5];
let i = fastrand::usize(..v.len());
let elem = v[i];
```

Sample values from an array with `O(n)` complexity (`n` is the length of array):

```rust
fastrand::choose_multiple([1, 4, 5], 2);
fastrand::choose_multiple(0..20, 12);
```

Shuffle an array:

```rust
let mut v = vec![1, 2, 3, 4, 5];
fastrand::shuffle(&mut v);
```

Generate a random `Vec` or `String`:

```rust
use std::iter::repeat_with;

let v: Vec<i32> = repeat_with(|| fastrand::i32(..)).take(10).collect();
let s: String = repeat_with(fastrand::alphanumeric).take(10).collect();
```

To get reproducible results on every run, initialize the generator with a seed:

```rust
// Pick an arbitrary number as seed.
fastrand::seed(7);

// Now this prints the same number on every run:
println!("{}", fastrand::u32(..));
```

To be more efficient, create a new `Rng` instance instead of using the thread-local
generator:

```rust
use std::iter::repeat_with;

let rng = fastrand::Rng::new();
let mut bytes: Vec<u8> = repeat_with(|| rng.u8(..)).take(10_000).collect();
```

This crate aims to expose a core set of useful randomness primitives. For more niche algorithms, consider using the [`fastrand-contrib`] crate alongside this one.

# Features

- `std` (enabled by default): Enables the `std` library. This is required for the global
  generator and global entropy. Without this feature, [`Rng`] can only be instantiated using
  the [`with_seed`](https://docs.rs/fastrand/latest/fastrand/struct.Rng.html#method.with_seed) method.
- `js`: Assumes that WebAssembly targets are being run in a JavaScript environment.

[`fastrand-contrib`]: https://crates.io/crates/fastrand-contrib

## License

Licensed under either of

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

#### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
