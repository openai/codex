# Slab

Pre-allocated storage for a uniform data type.

[![Crates.io][crates-badge]][crates-url]
[![Build Status][ci-badge]][ci-url]

[crates-badge]: https://img.shields.io/crates/v/slab
[crates-url]: https://crates.io/crates/slab
[ci-badge]: https://img.shields.io/github/actions/workflow/status/tokio-rs/slab/ci.yml?branch=master
[ci-url]: https://github.com/tokio-rs/slab/actions

[Documentation](https://docs.rs/slab)

## Usage

To use `slab`, first add this to your `Cargo.toml`:

```toml
[dependencies]
slab = "0.4"
```

Next, add this to your crate:

```rust
use slab::Slab;

let mut slab = Slab::new();

let hello = slab.insert("hello");
let world = slab.insert("world");

assert_eq!(slab[hello], "hello");
assert_eq!(slab[world], "world");

slab[world] = "earth";
assert_eq!(slab[world], "earth");
```

See [documentation](https://docs.rs/slab) for more details.

## License

This project is licensed under the [MIT license](LICENSE).

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in `slab` by you, shall be licensed as MIT, without any additional
terms or conditions.
