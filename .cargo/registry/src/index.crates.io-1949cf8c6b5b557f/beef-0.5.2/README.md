# beef

[![Travis shield](https://travis-ci.org/maciejhirsz/beef.svg)](https://travis-ci.org/maciejhirsz/beef)
[![Crates.io version shield](https://img.shields.io/crates/v/beef.svg)](https://crates.io/crates/beef)
[![Crates.io license shield](https://img.shields.io/crates/l/beef.svg)](https://crates.io/crates/beef)

Faster, more compact implementation of `Cow`.

**[Changelog](https://github.com/maciejhirsz/beef/releases) -**
**[Documentation](https://docs.rs/beef/) -**
**[Cargo](https://crates.io/crates/beef) -**
**[Repository](https://github.com/maciejhirsz/beef)**

```rust
use beef::Cow;

let borrowed: Cow<str> = Cow::borrowed("Hello");
let owned: Cow<str> = Cow::owned(String::from("World"));

assert_eq!(
    format!("{} {}!", borrowed, owned),
    "Hello World!",
);
```

There are two versions of `Cow` exposed by this crate:

+ `beef::Cow` is 3 words wide: pointer, length, and capacity. It stores the ownership tag in capacity.
+ `beef::lean::Cow` is 2 words wide, storing length, capacity, and the ownership tag all in one word.

Both versions are leaner than the `std::borrow::Cow`:

```rust
use std::mem::size_of;

const WORD: usize = size_of::<usize>();

assert_eq!(size_of::<std::borrow::Cow<str>>(), 4 * WORD);
assert_eq!(size_of::<beef::Cow<str>>(), 3 * WORD);
assert_eq!(size_of::<beef::lean::Cow<str>>(), 2 * WORD);
```

## How does it work?

The standard library `Cow` is an enum with two variants:

```rust
pub enum Cow<'a, B> where
    B: 'a + ToOwned + ?Sized,
{
    Borrowed(&'a B),
    Owned(<B as ToOwned>::Owned),
}
```

For the most common pairs of values - `&str` and `String`, or `&[u8]` and `Vec<u8>` - this
means that the entire enum is 4 words wide:

```text
                                                 Padding
                                                    |
                                                    v
          +-----------+-----------+-----------+-----------+
Borrowed: | Tag       | Pointer   | Length    | XXXXXXXXX |
          +-----------+-----------+-----------+-----------+

          +-----------+-----------+-----------+-----------+
Owned:    | Tag       | Pointer   | Length    | Capacity  |
          +-----------+-----------+-----------+-----------+
```

Instead of being an enum with a tag, `beef::Cow` uses capacity to determine whether the
value it's holding is owned (capacity is greater than 0), or borrowed (capacity is 0).

`beef::lean::Cow` goes even further and puts length and capacity on a single 64 word.

```text
                 +-----------+-----------+-----------+
beef::Cow        | Pointer   | Length    | Capacity? |
                 +-----------+-----------+-----------+

                 +-----------+-----------+
beef::lean::Cow  | Pointer   | Cap | Len |
                 +-----------+-----------+
```

Any owned `Vec` or `String` that has 0 capacity is effectively treated as a borrowed
value. Since having no capacity means there is no actual allocation behind the pointer,
this is safe.

## Benchmarks

```
cargo +nightly bench
```

Microbenchmarking obtaining a `&str` reference is rather flaky and you can have widely different results. In general the following seems to hold true:

+ `beef::Cow` and `beef::lean::Cow` are faster than `std::borrow::Cow` at obtaining a reference `&T`. This makes sense since we avoid the enum tag branching.
+ The 3-word `beef::Cow` is faster at creating borrowed variants, but slower at creating owned variants than `std::borrow::Cow`.
+ The 2-word `beef::lean::Cow` is faster at both.

```
running 9 tests
test beef_as_ref            ... bench:          57 ns/iter (+/- 15)
test beef_create            ... bench:         135 ns/iter (+/- 5)
test beef_create_mixed      ... bench:         659 ns/iter (+/- 52)
test lean_beef_as_ref       ... bench:          50 ns/iter (+/- 2)
test lean_beef_create       ... bench:          77 ns/iter (+/- 3)
test lean_beef_create_mixed ... bench:         594 ns/iter (+/- 52)
test std_as_ref             ... bench:          70 ns/iter (+/- 6)
test std_create             ... bench:         142 ns/iter (+/- 7)
test std_create_mixed       ... bench:         663 ns/iter (+/- 32)
```

## License

This crate is distributed under the terms of both the MIT license
and the Apache License (Version 2.0). Choose whichever one works best for you.

See [LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT) for details.
