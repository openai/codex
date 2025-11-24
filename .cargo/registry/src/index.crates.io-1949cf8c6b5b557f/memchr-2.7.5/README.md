memchr
======
This library provides heavily optimized routines for string search primitives.

[![Build status](https://github.com/BurntSushi/memchr/workflows/ci/badge.svg)](https://github.com/BurntSushi/memchr/actions)
[![Crates.io](https://img.shields.io/crates/v/memchr.svg)](https://crates.io/crates/memchr)

Dual-licensed under MIT or the [UNLICENSE](https://unlicense.org/).


### Documentation

[https://docs.rs/memchr](https://docs.rs/memchr)


### Overview

* The top-level module provides routines for searching for 1, 2 or 3 bytes
  in the forward or reverse direction. When searching for more than one byte,
  positions are considered a match if the byte at that position matches any
  of the bytes.
* The `memmem` sub-module provides forward and reverse substring search
  routines.

In all such cases, routines operate on `&[u8]` without regard to encoding. This
is exactly what you want when searching either UTF-8 or arbitrary bytes.

### Compiling without the standard library

memchr links to the standard library by default, but you can disable the
`std` feature if you want to use it in a `#![no_std]` crate:

```toml
[dependencies]
memchr = { version = "2", default-features = false }
```

On `x86_64` platforms, when the `std` feature is disabled, the SSE2 accelerated
implementations will be used. When `std` is enabled, AVX2 accelerated
implementations will be used if the CPU is determined to support it at runtime.

SIMD accelerated routines are also available on the `wasm32` and `aarch64`
targets. The `std` feature is not required to use them.

When a SIMD version is not available, then this crate falls back to
[SWAR](https://en.wikipedia.org/wiki/SWAR) techniques.

### Minimum Rust version policy

This crate's minimum supported `rustc` version is `1.61.0`.

The current policy is that the minimum Rust version required to use this crate
can be increased in minor version updates. For example, if `crate 1.0` requires
Rust 1.20.0, then `crate 1.0.z` for all values of `z` will also require Rust
1.20.0 or newer. However, `crate 1.y` for `y > 0` may require a newer minimum
version of Rust.

In general, this crate will be conservative with respect to the minimum
supported version of Rust.


### Testing strategy

Given the complexity of the code in this crate, along with the pervasive use
of `unsafe`, this crate has an extensive testing strategy. It combines multiple
approaches:

* Hand-written tests.
* Exhaustive-style testing meant to exercise all possible branching and offset
  calculations.
* Property based testing through [`quickcheck`](https://github.com/BurntSushi/quickcheck).
* Fuzz testing through [`cargo fuzz`](https://github.com/rust-fuzz/cargo-fuzz).
* A huge suite of benchmarks that are also run as tests. Benchmarks always
  confirm that the expected result occurs.

Improvements to the testing infrastructure are very welcome.


### Algorithms used

At time of writing, this crate's implementation of substring search actually
has a few different algorithms to choose from depending on the situation.

* For very small haystacks,
  [Rabin-Karp](https://en.wikipedia.org/wiki/Rabin%E2%80%93Karp_algorithm)
  is used to reduce latency. Rabin-Karp has very small overhead and can often
  complete before other searchers have even been constructed.
* For small needles, a variant of the
  ["Generic SIMD"](http://0x80.pl/articles/simd-strfind.html#algorithm-1-generic-simd)
  algorithm is used. Instead of using the first and last bytes, a heuristic is
  used to select bytes based on a background distribution of byte frequencies.
* In all other cases,
  [Two-Way](https://en.wikipedia.org/wiki/Two-way_string-matching_algorithm)
  is used. If possible, a prefilter based on the "Generic SIMD" algorithm
  linked above is used to find candidates quickly. A dynamic heuristic is used
  to detect if the prefilter is ineffective, and if so, disables it.


### Why is the standard library's substring search so much slower?

We'll start by establishing what the difference in performance actually
is. There are two relevant benchmark classes to consider: `prebuilt` and
`oneshot`. The `prebuilt` benchmarks are designed to measure---to the extent
possible---search time only. That is, the benchmark first starts by building a
searcher and then only tracking the time for _using_ the searcher:

```
$ rebar rank benchmarks/record/x86_64/2023-08-26.csv --intersection -e memchr/memmem/prebuilt -e std/memmem/prebuilt
Engine                       Version                   Geometric mean of speed ratios  Benchmark count
------                       -------                   ------------------------------  ---------------
rust/memchr/memmem/prebuilt  2.5.0                     1.03                            53
rust/std/memmem/prebuilt     1.73.0-nightly 180dffba1  6.50                            53
```

Conversely, the `oneshot` benchmark class measures the time it takes to both
build the searcher _and_ use it:

```
$ rebar rank benchmarks/record/x86_64/2023-08-26.csv --intersection -e memchr/memmem/oneshot -e std/memmem/oneshot
Engine                      Version                   Geometric mean of speed ratios  Benchmark count
------                      -------                   ------------------------------  ---------------
rust/memchr/memmem/oneshot  2.5.0                     1.04                            53
rust/std/memmem/oneshot     1.73.0-nightly 180dffba1  5.26                            53
```

**NOTE:** Replace `rebar rank` with `rebar cmp` in the above commands to
explore the specific benchmarks and their differences.

So in both cases, this crate is quite a bit faster over a broad sampling of
benchmarks regardless of whether you measure only search time or search time
plus construction time. The difference is a little smaller when you include
construction time in your measurements.

These two different types of benchmark classes make for a nice segue into
one reason why the standard library's substring search can be slower: API
design. In the standard library, the only APIs available to you require
one to re-construct the searcher for every search. While you can benefit
from building a searcher once and iterating over all matches in a single
string, you cannot reuse that searcher to search other strings. This might
come up when, for example, searching a file one line at a time. You'll need
to re-build the searcher for every line searched, and this can [really
matter][burntsushi-bstr-blog].

**NOTE:** The `prebuilt` benchmark for the standard library can't actually
avoid measuring searcher construction at some level, because there is no API
for it. Instead, the benchmark consists of building the searcher once and then
finding all matches in a single string via an iterator. This tends to
approximate a benchmark where searcher construction isn't measured, but it
isn't perfect. While this means the comparison is not strictly
apples-to-apples, it does reflect what is maximally possible with the standard
library, and thus reflects the best that one could do in a real world scenario.

While there is more to the story than just API design here, it's important to
point out that even if the standard library's substring search were a precise
clone of this crate internally, it would still be at a disadvantage in some
workloads because of its API. (The same also applies to C's standard library
`memmem` function. There is no way to amortize construction of the searcher.
You need to pay for it on every call.)

The other reason for the difference in performance is that
the standard library has trouble using SIMD. In particular, substring search
is implemented in the `core` library, where platform specific code generally
can't exist. That's an issue because in order to utilize SIMD beyond SSE2
while maintaining portable binaries, one needs to use [dynamic CPU feature
detection][dynamic-cpu], and that in turn requires platform specific code.
While there is [an RFC for enabling target feature detection in
`core`][core-feature], it doesn't yet exist.

The bottom line here is that `core`'s substring search implementation is
limited to making use of SSE2, but not AVX.

Still though, this crate does accelerate substring search even when only SSE2
is available. The standard library could therefore adopt the techniques in this
crate just for SSE2. The reason why that hasn't happened yet isn't totally
clear to me. It likely needs a champion to push it through. The standard
library tends to be more conservative in these things. With that said, the
standard library does use some [SSE2 acceleration on `x86-64`][std-sse2] added
in [this PR][std-sse2-pr]. However, at the time of writing, it is only used
for short needles and doesn't use the frequency based heuristics found in this
crate.

**NOTE:** Another thing worth mentioning is that the standard library's
substring search routine requires that both the needle and haystack have type
`&str`. Unless you can assume that your data is valid UTF-8, building a `&str`
will come with the overhead of UTF-8 validation. This may in turn result in
overall slower searching depending on your workload. In contrast, the `memchr`
crate permits both the needle and the haystack to have type `&[u8]`, where
`&[u8]` can be created from a `&str` with zero cost. Therefore, the substring
search in this crate is strictly more flexible than what the standard library
provides.

[burntsushi-bstr-blog]: https://blog.burntsushi.net/bstr/#motivation-based-on-performance
[dynamic-cpu]: https://doc.rust-lang.org/std/arch/index.html#dynamic-cpu-feature-detection
[core-feature]: https://github.com/rust-lang/rfcs/pull/3469
[std-sse2]: https://github.com/rust-lang/rust/blob/bf9229a2e366b4c311f059014a4aa08af16de5d8/library/core/src/str/pattern.rs#L1719-L1857
[std-sse2-pr]: https://github.com/rust-lang/rust/pull/103779
