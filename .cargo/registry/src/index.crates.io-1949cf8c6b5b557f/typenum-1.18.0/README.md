[![crates.io](https://img.shields.io/crates/v/typenum.svg)](https://crates.io/crates/typenum)
[![Build Status](https://github.com/paholg/typenum/actions/workflows/check.yml/badge.svg)](https://github.com/paholg/typenum/actions/workflows/check.yml)

Typenum
=====

Typenum is a Rust library for type-level numbers evaluated at compile time. It
currently supports bits, unsigned integers, and signed integers.

Typenum depends only on libcore, and so is suitable for use on any platform!

For the full documentation, go [here](https://docs.rs/typenum).

### Importing

While `typenum` is divided into several modules, they are all re-exported
through the crate root, so you can import anything contained herein with `use
typenum::whatever;`, ignoring the crate structure.

You may also find it useful to treat the `consts` module as a prelude,
performing a glob import.

### Example

Here is a trivial example of `typenum`'s use:

```rust
use typenum::{Sum, Exp, Integer, N2, P3, P4};

type X = Sum<P3, P4>;
assert_eq!(<X as Integer>::to_i32(), 7);

type Y = Exp<N2, P3>;
assert_eq!(<Y as Integer>::to_i32(), -8);
```

For a non-trivial example of its use, see one of the crates that depends on
it. The full list is
[here](https://crates.io/crates/typenum/reverse_dependencies). Of note are
[dimensioned](https://crates.io/crates/dimensioned/) which does compile-time
type checking for arbitrary unit systems and
[generic-array](https://crates.io/crates/generic-array/) which provides arrays
whose length you can generically refer to.

### Error messages


Typenum's error messages aren't great, and can be difficult to parse. The good
news is that the fine folks at Auxon have written a tool to help with it. Please
take a look at [tnfilt](https://github.com/auxoncorp/tnfilt).

### License

Licensed under either of

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
   http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
