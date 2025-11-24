regex
=====
This crate provides routines for searching strings for matches of a [regular
expression] (aka "regex"). The regex syntax supported by this crate is similar
to other regex engines, but it lacks several features that are not known how to
implement efficiently. This includes, but is not limited to, look-around and
backreferences. In exchange, all regex searches in this crate have worst case
`O(m * n)` time complexity, where `m` is proportional to the size of the regex
and `n` is proportional to the size of the string being searched.

[regular expression]: https://en.wikipedia.org/wiki/Regular_expression

[![Build status](https://github.com/rust-lang/regex/workflows/ci/badge.svg)](https://github.com/rust-lang/regex/actions)
[![Crates.io](https://img.shields.io/crates/v/regex.svg)](https://crates.io/crates/regex)

### Documentation

[Module documentation with examples](https://docs.rs/regex).
The module documentation also includes a comprehensive description of the
syntax supported.

Documentation with examples for the various matching functions and iterators
can be found on the
[`Regex` type](https://docs.rs/regex/*/regex/struct.Regex.html).

### Usage

To bring this crate into your repository, either add `regex` to your
`Cargo.toml`, or run `cargo add regex`.

Here's a simple example that matches a date in YYYY-MM-DD format and prints the
year, month and day:

```rust
use regex::Regex;

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

fn main() {
    let re = Regex::new(r"(\d{4})-(\d{2})-(\d{2})").unwrap();
    let hay = "On 2010-03-14, foo happened. On 2014-10-14, bar happened.";

    let mut dates = vec![];
    for (_, [year, month, day]) in re.captures_iter(hay).map(|c| c.extract()) {
        dates.push((year, month, day));
    }
    assert_eq!(dates, vec![
      ("2010", "03", "14"),
      ("2014", "10", "14"),
    ]);
}
```

### Usage: Avoid compiling the same regex in a loop

It is an anti-pattern to compile the same regular expression in a loop since
compilation is typically expensive. (It takes anywhere from a few microseconds
to a few **milliseconds** depending on the size of the regex.) Not only is
compilation itself expensive, but this also prevents optimizations that reuse
allocations internally to the matching engines.

In Rust, it can sometimes be a pain to pass regular expressions around if
they're used from inside a helper function. Instead, we recommend using the
[`once_cell`](https://crates.io/crates/once_cell) crate to ensure that
regular expressions are compiled exactly once. For example:

```rust
use {
    once_cell::sync::Lazy,
    regex::Regex,
};

fn some_helper_function(haystack: &str) -> bool {
    static RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"...").unwrap());
    RE.is_match(haystack)
}

fn main() {
    assert!(some_helper_function("abc"));
    assert!(!some_helper_function("ac"));
}
```

Specifically, in this example, the regex will be compiled when it is used for
the first time. On subsequent uses, it will reuse the previous compilation.

### Usage: match regular expressions on `&[u8]`

The main API of this crate (`regex::Regex`) requires the caller to pass a
`&str` for searching. In Rust, an `&str` is required to be valid UTF-8, which
means the main API can't be used for searching arbitrary bytes.

To match on arbitrary bytes, use the `regex::bytes::Regex` API. The API is
identical to the main API, except that it takes an `&[u8]` to search on instead
of an `&str`. The `&[u8]` APIs also permit disabling Unicode mode in the regex
even when the pattern would match invalid UTF-8. For example, `(?-u:.)` is
not allowed in `regex::Regex` but is allowed in `regex::bytes::Regex` since
`(?-u:.)` matches any byte except for `\n`. Conversely, `.` will match the
UTF-8 encoding of any Unicode scalar value except for `\n`.

This example shows how to find all null-terminated strings in a slice of bytes:

```rust
use regex::bytes::Regex;

let re = Regex::new(r"(?-u)(?<cstr>[^\x00]+)\x00").unwrap();
let text = b"foo\xFFbar\x00baz\x00";

// Extract all of the strings without the null terminator from each match.
// The unwrap is OK here since a match requires the `cstr` capture to match.
let cstrs: Vec<&[u8]> =
    re.captures_iter(text)
      .map(|c| c.name("cstr").unwrap().as_bytes())
      .collect();
assert_eq!(vec![&b"foo\xFFbar"[..], &b"baz"[..]], cstrs);
```

Notice here that the `[^\x00]+` will match any *byte* except for `NUL`,
including bytes like `\xFF` which are not valid UTF-8. When using the main API,
`[^\x00]+` would instead match any valid UTF-8 sequence except for `NUL`.

### Usage: match multiple regular expressions simultaneously

This demonstrates how to use a `RegexSet` to match multiple (possibly
overlapping) regular expressions in a single scan of the search text:

```rust
use regex::RegexSet;

let set = RegexSet::new(&[
    r"\w+",
    r"\d+",
    r"\pL+",
    r"foo",
    r"bar",
    r"barfoo",
    r"foobar",
]).unwrap();

// Iterate over and collect all of the matches.
let matches: Vec<_> = set.matches("foobar").into_iter().collect();
assert_eq!(matches, vec![0, 2, 3, 4, 6]);

// You can also test whether a particular regex matched:
let matches = set.matches("foobar");
assert!(!matches.matched(5));
assert!(matches.matched(6));
```


### Usage: regex internals as a library

The [`regex-automata` directory](./regex-automata/) contains a crate that
exposes all of the internal matching engines used by the `regex` crate. The
idea is that the `regex` crate exposes a simple API for 99% of use cases, but
`regex-automata` exposes oodles of customizable behaviors.

[Documentation for `regex-automata`.](https://docs.rs/regex-automata)


### Usage: a regular expression parser

This repository contains a crate that provides a well tested regular expression
parser, abstract syntax and a high-level intermediate representation for
convenient analysis. It provides no facilities for compilation or execution.
This may be useful if you're implementing your own regex engine or otherwise
need to do analysis on the syntax of a regular expression. It is otherwise not
recommended for general use.

[Documentation for `regex-syntax`.](https://docs.rs/regex-syntax)


### Crate features

This crate comes with several features that permit tweaking the trade off
between binary size, compilation time and runtime performance. Users of this
crate can selectively disable Unicode tables, or choose from a variety of
optimizations performed by this crate to disable.

When all of these features are disabled, runtime match performance may be much
worse, but if you're matching on short strings, or if high performance isn't
necessary, then such a configuration is perfectly serviceable. To disable
all such features, use the following `Cargo.toml` dependency configuration:

```toml
[dependencies.regex]
version = "1.3"
default-features = false
# Unless you have a specific reason not to, it's good sense to enable standard
# library support. It enables several optimizations and avoids spin locks. It
# also shouldn't meaningfully impact compile times or binary size.
features = ["std"]
```

This will reduce the dependency tree of `regex` down to two crates:
`regex-syntax` and `regex-automata`.

The full set of features one can disable are
[in the "Crate features" section of the documentation](https://docs.rs/regex/1.*/#crate-features).


### Performance

One of the goals of this crate is for the regex engine to be "fast." What that
is a somewhat nebulous goal, it is usually interpreted in one of two ways.
First, it means that all searches take worst case `O(m * n)` time, where
`m` is proportional to `len(regex)` and `n` is proportional to `len(haystack)`.
Second, it means that even aside from the time complexity constraint, regex
searches are "fast" in practice.

While the first interpretation is pretty unambiguous, the second one remains
nebulous. While nebulous, it guides this crate's architecture and the sorts of
the trade offs it makes. For example, here are some general architectural
statements that follow as a result of the goal to be "fast":

* When given the choice between faster regex searches and faster _Rust compile
times_, this crate will generally choose faster regex searches.
* When given the choice between faster regex searches and faster _regex compile
times_, this crate will generally choose faster regex searches. That is, it is
generally acceptable for `Regex::new` to get a little slower if it means that
searches get faster. (This is a somewhat delicate balance to strike, because
the speed of `Regex::new` needs to remain somewhat reasonable. But this is why
one should avoid re-compiling the same regex over and over again.)
* When given the choice between faster regex searches and simpler API
design, this crate will generally choose faster regex searches. For example,
if one didn't care about performance, we could like get rid of both of
the `Regex::is_match` and `Regex::find` APIs and instead just rely on
`Regex::captures`.

There are perhaps more ways that being "fast" influences things.

While this repository used to provide its own benchmark suite, it has since
been moved to [rebar](https://github.com/BurntSushi/rebar). The benchmarks are
quite extensive, and there are many more than what is shown in rebar's README
(which is just limited to a "curated" set meant to compare performance between
regex engines). To run all of this crate's benchmarks, first start by cloning
and installing `rebar`:

```text
$ git clone https://github.com/BurntSushi/rebar
$ cd rebar
$ cargo install --path ./
```

Then build the benchmark harness for just this crate:

```text
$ rebar build -e '^rust/regex$'
```

Run all benchmarks for this crate as tests (each benchmark is executed once to
ensure it works):

```text
$ rebar measure -e '^rust/regex$' -t
```

Record measurements for all benchmarks and save them to a CSV file:

```text
$ rebar measure -e '^rust/regex$' | tee results.csv
```

Explore benchmark timings:

```text
$ rebar cmp results.csv
```

See the `rebar` documentation for more details on how it works and how to
compare results with other regex engines.


### Hacking

The `regex` crate is, for the most part, a pretty thin wrapper around the
[`meta::Regex`](https://docs.rs/regex-automata/latest/regex_automata/meta/struct.Regex.html)
from the
[`regex-automata` crate](https://docs.rs/regex-automata/latest/regex_automata/).
Therefore, if you're looking to work on the internals of this crate, you'll
likely either want to look in `regex-syntax` (for parsing) or `regex-automata`
(for construction of finite automata and the search routines).

My [blog on regex internals](https://blog.burntsushi.net/regex-internals/)
goes into more depth.


### Minimum Rust version policy

This crate's minimum supported `rustc` version is `1.65.0`.

The policy is that the minimum Rust version required to use this crate can be
increased in minor version updates. For example, if regex 1.0 requires Rust
1.20.0, then regex 1.0.z for all values of `z` will also require Rust 1.20.0 or
newer. However, regex 1.y for `y > 0` may require a newer minimum version of
Rust.


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
