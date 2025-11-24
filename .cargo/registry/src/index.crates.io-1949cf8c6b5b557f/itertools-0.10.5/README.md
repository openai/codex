# Itertools

Extra iterator adaptors, functions and macros.

Please read the [API documentation here](https://docs.rs/itertools/).

[![build_status](https://github.com/rust-itertools/itertools/actions/workflows/ci.yml/badge.svg)](https://github.com/rust-itertools/itertools/actions)
[![crates.io](https://img.shields.io/crates/v/itertools.svg)](https://crates.io/crates/itertools)

How to use with Cargo:

```toml
[dependencies]
itertools = "0.10.5"
```

How to use in your crate:

```rust
use itertools::Itertools;
```

## How to contribute

- Fix a bug or implement a new thing
- Include tests for your new feature, preferably a QuickCheck test
- Make a Pull Request

For new features, please first consider filing a PR to [rust-lang/rust](https://github.com/rust-lang/rust),
adding your new feature to the `Iterator` trait of the standard library, if you believe it is reasonable.
If it isn't accepted there, proposing it for inclusion in ``itertools`` is a good idea.
The reason for doing is this is so that we avoid future breakage as with ``.flatten()``.
However, if your feature involves heap allocation, such as storing elements in a ``Vec<T>``,
then it can't be accepted into ``libcore``, and you should propose it for ``itertools`` directly instead.

## License

Dual-licensed to be compatible with the Rust project.

Licensed under the Apache License, Version 2.0
https://www.apache.org/licenses/LICENSE-2.0 or the MIT license
https://opensource.org/licenses/MIT, at your
option. This file may not be copied, modified, or distributed
except according to those terms.
