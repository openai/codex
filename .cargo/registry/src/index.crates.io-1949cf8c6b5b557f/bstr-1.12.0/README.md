bstr
====
This crate provides extension traits for `&[u8]` and `Vec<u8>` that enable
their use as byte strings, where byte strings are _conventionally_ UTF-8. This
differs from the standard library's `String` and `str` types in that they are
not required to be valid UTF-8, but may be fully or partially valid UTF-8.

[![Build status](https://github.com/BurntSushi/bstr/workflows/ci/badge.svg)](https://github.com/BurntSushi/bstr/actions)
[![crates.io](https://img.shields.io/crates/v/bstr.svg)](https://crates.io/crates/bstr)


### Documentation

https://docs.rs/bstr


### When should I use byte strings?

See this part of the documentation for more details:
<https://docs.rs/bstr/1.*/bstr/#when-should-i-use-byte-strings>.

The short story is that byte strings are useful when it is inconvenient or
incorrect to require valid UTF-8.


### Usage

`cargo add bstr`

### Examples

The following two examples exhibit both the API features of byte strings and
the I/O convenience functions provided for reading line-by-line quickly.

This first example simply shows how to efficiently iterate over lines in stdin,
and print out lines containing a particular substring:

```rust
use std::{error::Error, io::{self, Write}};
use bstr::{ByteSlice, io::BufReadExt};

fn main() -> Result<(), Box<dyn Error>> {
    let stdin = io::stdin();
    let mut stdout = io::BufWriter::new(io::stdout());

    stdin.lock().for_byte_line_with_terminator(|line| {
        if line.contains_str("Dimension") {
            stdout.write_all(line)?;
        }
        Ok(true)
    })?;
    Ok(())
}
```

This example shows how to count all of the words (Unicode-aware) in stdin,
line-by-line:

```rust
use std::{error::Error, io};
use bstr::{ByteSlice, io::BufReadExt};

fn main() -> Result<(), Box<dyn Error>> {
    let stdin = io::stdin();
    let mut words = 0;
    stdin.lock().for_byte_line_with_terminator(|line| {
        words += line.words().count();
        Ok(true)
    })?;
    println!("{}", words);
    Ok(())
}
```

This example shows how to convert a stream on stdin to uppercase without
performing UTF-8 validation _and_ amortizing allocation. On standard ASCII
text, this is quite a bit faster than what you can (easily) do with standard
library APIs. (N.B. Any invalid UTF-8 bytes are passed through unchanged.)

```rust
use std::{error::Error, io::{self, Write}};
use bstr::{ByteSlice, io::BufReadExt};

fn main() -> Result<(), Box<dyn Error>> {
    let stdin = io::stdin();
    let mut stdout = io::BufWriter::new(io::stdout());

    let mut upper = vec![];
    stdin.lock().for_byte_line_with_terminator(|line| {
        upper.clear();
        line.to_uppercase_into(&mut upper);
        stdout.write_all(&upper)?;
        Ok(true)
    })?;
    Ok(())
}
```

This example shows how to extract the first 10 visual characters (as grapheme
clusters) from each line, where invalid UTF-8 sequences are generally treated
as a single character and are passed through correctly:

```rust
use std::{error::Error, io::{self, Write}};
use bstr::{ByteSlice, io::BufReadExt};

fn main() -> Result<(), Box<dyn Error>> {
    let stdin = io::stdin();
    let mut stdout = io::BufWriter::new(io::stdout());

    stdin.lock().for_byte_line_with_terminator(|line| {
        let end = line
            .grapheme_indices()
            .map(|(_, end, _)| end)
            .take(10)
            .last()
            .unwrap_or(line.len());
        stdout.write_all(line[..end].trim_end())?;
        stdout.write_all(b"\n")?;
        Ok(true)
    })?;
    Ok(())
}
```


### Cargo features

This crates comes with a few features that control standard library, serde and
Unicode support.

* `std` - **Enabled** by default. This provides APIs that require the standard
  library, such as `Vec<u8>` and `PathBuf`. Enabling this feature also enables
  the `alloc` feature.
* `alloc` - **Enabled** by default. This provides APIs that require allocations
  via the `alloc` crate, such as `Vec<u8>`.
* `unicode` - **Enabled** by default. This provides APIs that require sizable
  Unicode data compiled into the binary. This includes, but is not limited to,
  grapheme/word/sentence segmenters. When this is disabled, basic support such
  as UTF-8 decoding is still included. Note that currently, enabling this
  feature also requires enabling the `std` feature. It is expected that this
  limitation will be lifted at some point.
* `serde` - Enables implementations of serde traits for `BStr`, and also
  `BString` when `alloc` is enabled.


### Minimum Rust version policy

This crate's minimum supported `rustc` version (MSRV) is `1.73`.

In general, this crate will be conservative with respect to the minimum
supported version of Rust. MSRV may be bumped in minor version releases.


### Future work

Since it is plausible that some of the types in this crate might end up in your
public API (e.g., `BStr` and `BString`), we will commit to being very
conservative with respect to new major version releases. It's difficult to say
precisely how conservative, but unless there is a major issue with the `1.0`
release, I wouldn't expect a `2.0` release to come out any sooner than some
period of years.

A large part of the API surface area was taken from the standard library, so
from an API design perspective, a good portion of this crate should be on solid
ground. The main differences from the standard library are in how the various
substring search routines work. The standard library provides generic
infrastructure for supporting different types of searches with a single method,
where as this library prefers to define new methods for each type of search and
drop the generic infrastructure.

Some _probable_ future considerations for APIs include, but are not limited to:

* Unicode normalization.
* More sophisticated support for dealing with Unicode case, perhaps by
  combining the use cases supported by [`caseless`](https://docs.rs/caseless)
  and [`unicase`](https://docs.rs/unicase).

Here are some examples that are _probably_ out of scope for this crate:

* Regular expressions.
* Unicode collation.

The exact scope isn't quite clear, but I expect we can iterate on it.

In general, as stated below, this crate brings lots of related APIs together
into a single crate while simultaneously attempting to keep the total number of
dependencies low. Indeed, every dependency of `bstr`, except for `memchr`, is
optional.


### High level motivation

Strictly speaking, the `bstr` crate provides very little that can't already be
achieved with the standard library `Vec<u8>`/`&[u8]` APIs and the ecosystem of
library crates. For example:

* The standard library's
  [`Utf8Error`](https://doc.rust-lang.org/std/str/struct.Utf8Error.html) can be
  used for incremental lossy decoding of `&[u8]`.
* The
  [`unicode-segmentation`](https://unicode-rs.github.io/unicode-segmentation/unicode_segmentation/index.html)
  crate can be used for iterating over graphemes (or words), but is only
  implemented for `&str` types. One could use `Utf8Error` above to implement
  grapheme iteration with the same semantics as what `bstr` provides (automatic
  Unicode replacement codepoint substitution).
* The [`twoway`](https://docs.rs/twoway) crate can be used for fast substring
  searching on `&[u8]`.

So why create `bstr`? Part of the point of the `bstr` crate is to provide a
uniform API of coupled components instead of relying on users to piece together
loosely coupled components from the crate ecosystem. For example, if you wanted
to perform a search and replace in a `Vec<u8>`, then writing the code to do
that with the `twoway` crate is not that difficult, but it's still additional
glue code you have to write. This work adds up depending on what you're doing.
Consider, for example, trimming and splitting, along with their different
variants.

In other words, `bstr` is partially a way of pushing back against the
micro-crate ecosystem that appears to be evolving. Namely, it is a goal of
`bstr` to keep its dependency list lightweight. For example, `serde` is an
optional dependency because there is no feasible alternative. In service of
this philosophy, currently, the only required dependency of `bstr` is `memchr`.


### License

This project is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   https://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   https://opensource.org/licenses/MIT)

at your option.

The data in `src/unicode/data/` is licensed under the Unicode License Agreement
([LICENSE-UNICODE](https://www.unicode.org/copyright.html#License)), although
this data is only used in tests.
