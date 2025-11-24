# wildmatch

[![build status](https://github.com/becheran/wildmatch/workflows/Build/badge.svg)](https://github.com/becheran/wildmatch/actions?workflow=Build)
[![docs](https://docs.rs/wildmatch/badge.svg)](https://docs.rs/wildmatch)
[![downloads](https://img.shields.io/crates/v/wildmatch.svg?color=orange)](https://crates.io/crates/wildmatch)
[![crate](https://badgen.net/crates/d/wildmatch)](https://crates.io/crates/wildmatch)
[![license](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![codecov](https://img.shields.io/codecov/c/github/becheran/wildmatch/master)](https://codecov.io/gh/becheran/wildmatch)

Match strings against a simple wildcard pattern. Tests a wildcard pattern `p` against an input string `s`. Returns true only when `p` matches the entirety of `s`.

See also the example described on [wikipedia](https://en.wikipedia.org/wiki/Matching_wildcards) for matching wildcards.

- `?` matches exactly one occurrence of any character.
- `*` matches arbitrary many (including zero) occurrences of any character.
- No escape characters are defined.

Can also be used with a [custom match pattern](https://docs.rs/wildmatch/latest/wildmatch/struct.WildMatchPattern.html) to define own wildcard patterns for single and multi-character matching.

For example the pattern `ca?` will match `cat` or `car`. The pattern `https://*` will match all https urls, such as `https://google.de` or `https://github.com/becheran/wildmatch`.

The following table shows a performance benchmarks between wildmatch, [regex](https://crates.io/crates/regex),[glob](https://docs.rs/glob/0.3.0/glob/struct.Pattern.html), and the [regex_lite](https://github.com/rust-lang/regex/tree/master/regex-lite) libraries:

| Benchmark         | wildmatch     | regex      | glob           | regex_lite
| ----              | ------------: | ---------: | -------------: | ---------:
| compiling/text    |    **462 ns** |  39,714 ns |   1,470 ns     | 13,210 ns
| compiling/complex |     190 ns    | 153,830 ns |     238 ns     | **60 ns**
| matching/text     |    **186 ns** |   4,065 ns |     456 ns     | 6,097 ns
| matching/complex  |    **310 ns** |  16,085 ns |   1,426 ns     | 3,773 ns

The library only depends on the rust [`stdlib`](https://doc.rust-lang.org/std/).

See the [documentation](https://docs.rs/wildmatch) for usage and more examples.
