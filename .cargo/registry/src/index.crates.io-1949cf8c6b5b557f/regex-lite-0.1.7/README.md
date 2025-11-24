regex-lite
==========
This crate provides a **lightweight** regex engine for searching strings. The
regex syntax supported by this crate is nearly identical to what is found in
the `regex` crate. Like the `regex` crate, all regex searches in this crate
have worst case `O(m * n)` time complexity, where `m` is proportional to the
size of the regex and `n` is proportional to the size of the string being
searched.

[![Build status](https://github.com/rust-lang/regex/workflows/ci/badge.svg)](https://github.com/rust-lang/regex/actions)
[![Crates.io](https://img.shields.io/crates/v/regex-lite.svg)](https://crates.io/crates/regex-lite)


### Documentation

https://docs.rs/regex-lite


### Usage

To bring this crate into your repository, either add `regex-lite` to your
`Cargo.toml`, or run `cargo add regex-lite`.

Here's a simple example that matches a date in YYYY-MM-DD format and prints the
year, month and day:

```rust
use regex_lite::Regex;

fn main() {
    let re = Regex::new(r"(?x)
(?P<year>\d{4})  # the year
-
(?P<month>\d{2}) # the month
-
(?P<day>\d{2})   # the day
").unwrap();
    let caps = re.captures("2010-03-14").unwrap();

    assert_eq!("2010", &caps["year"]);
    assert_eq!("03", &caps["month"]);
    assert_eq!("14", &caps["day"]);
}
```

If you have lots of dates in text that you'd like to iterate over, then it's
easy to adapt the above example with an iterator:

```rust
use regex::Regex;

const TO_SEARCH: &'static str = "
On 2010-03-14, foo happened. On 2014-10-14, bar happened.
";

fn main() {
    let re = Regex::new(r"(\d{4})-(\d{2})-(\d{2})").unwrap();

    for caps in re.captures_iter(TO_SEARCH) {
        // Note that all of the unwraps are actually OK for this regex
        // because the only way for the regex to match is if all of the
        // capture groups match. This is not true in general though!
        println!("year: {}, month: {}, day: {}",
                 caps.get(1).unwrap().as_str(),
                 caps.get(2).unwrap().as_str(),
                 caps.get(3).unwrap().as_str());
    }
}
```

This example outputs:

```text
year: 2010, month: 03, day: 14
year: 2014, month: 10, day: 14
```


### Minimum Rust version policy

This crate's minimum supported `rustc` version is `1.65.0`.

The policy is that the minimum Rust version required to use this crate can be
increased in semver compatible updates.


### Motivation

The primary purpose of this crate is to provide an alternative regex engine
for folks that are unhappy with the binary size and compilation time of the
primary `regex` crate. The `regex-lite` crate does the absolute minimum possible
to act as a drop-in replacement to the `regex` crate's `Regex` type. It avoids
a lot of complexity by choosing not to optimize searches and to opt out of
functionality such as robust Unicode support. By keeping the code simpler
and smaller, we get binary sizes and compile times that are substantially
better than even the `regex` crate with all of its features disabled.

To make the benefits a bit more concrete, here are the results of one
experiment I did. For `regex`, I disabled all features except for `std`:

* `regex 1.7.3`: 1.41s compile time, 373KB relative size increase
* `regex 1.8.1`: 1.46s compile time, 410KB relative size increase
* `regex 1.9.0`: 1.93s compile time, 565KB relative size increase
* `regex-lite 0.1.0`: 0.73s compile time, 94KB relative size increase

The main reason why `regex-lite` does so much better than `regex` when all of
`regex`'s features are disabled is because of irreducible complexity. There are
certain parts of the code in `regex` that can't be arbitrarily divided based
on binary size and compile time goals. It's instead more sustainable to just
maintain an entirely separate crate.

Ideas for improving the binary size and compile times of this crate even more
are most welcome.


### License

This project is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   https://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   https://opensource.org/licenses/MIT)

at your option.

The data in `regex-syntax/src/unicode_tables/` is licensed under the Unicode
License Agreement
([LICENSE-UNICODE](https://www.unicode.org/copyright.html#License)).
