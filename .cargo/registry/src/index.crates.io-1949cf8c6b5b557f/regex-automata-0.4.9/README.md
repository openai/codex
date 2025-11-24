regex-automata
==============
This crate exposes a variety of regex engines used by the `regex` crate.
It provides a vast, sprawling and "expert" level API to each regex engine.
The regex engines provided by this crate focus heavily on finite automata
implementations and specifically guarantee worst case `O(m * n)` time
complexity for all searches. (Where `m ~ len(regex)` and `n ~ len(haystack)`.)

[![Build status](https://github.com/rust-lang/regex/workflows/ci/badge.svg)](https://github.com/rust-lang/regex/actions)
[![Crates.io](https://img.shields.io/crates/v/regex-automata.svg)](https://crates.io/crates/regex-automata)


### Documentation

https://docs.rs/regex-automata


### Example

This example shows how to search for matches of multiple regexes, where each
regex uses the same capture group names to parse different key-value formats.

```rust
use regex_automata::{meta::Regex, PatternID};

let re = Regex::new_many(&[
    r#"(?m)^(?<key>[[:word:]]+)=(?<val>[[:word:]]+)$"#,
    r#"(?m)^(?<key>[[:word:]]+)="(?<val>[^"]+)"$"#,
    r#"(?m)^(?<key>[[:word:]]+)='(?<val>[^']+)'$"#,
    r#"(?m)^(?<key>[[:word:]]+):\s*(?<val>[[:word:]]+)$"#,
]).unwrap();
let hay = r#"
best_album="Blow Your Face Out"
best_quote='"then as it was, then again it will be"'
best_year=1973
best_simpsons_episode: HOMR
"#;
let mut kvs = vec![];
for caps in re.captures_iter(hay) {
    // N.B. One could use capture indices '1' and '2' here
    // as well. Capture indices are local to each pattern.
    // (Just like names are.)
    let key = &hay[caps.get_group_by_name("key").unwrap()];
    let val = &hay[caps.get_group_by_name("val").unwrap()];
    kvs.push((key, val));
}
assert_eq!(kvs, vec![
    ("best_album", "Blow Your Face Out"),
    ("best_quote", "\"then as it was, then again it will be\""),
    ("best_year", "1973"),
    ("best_simpsons_episode", "HOMR"),
]);
```


### Safety

**I welcome audits of `unsafe` code.**

This crate tries to be extremely conservative in its use of `unsafe`, but does
use it in a few spots. In general, I am very open to removing uses of `unsafe`
if it doesn't result in measurable performance regressions and doesn't result
in significantly more complex code.

Below is an outline of how `unsafe` is used in this crate.

* `util::pool::Pool` makes use of `unsafe` to implement a fast path for
accessing an element of the pool. The fast path applies to the first thread
that uses the pool. In effect, the fast path is fast because it avoid a mutex
lock. `unsafe` is also used in the no-std version of `Pool` to implement a spin
lock for synchronization.
* `util::lazy::Lazy` uses `unsafe` to implement a variant of
`once_cell::sync::Lazy` that works in no-std environments. A no-std no-alloc
implementation is also provided that requires use of `unsafe`.
* The `dfa` module makes extensive use of `unsafe` to support zero-copy
deserialization of DFAs. The high level problem is that you need to get from
`&[u8]` to the internal representation of a DFA without doing any copies.
This is required for support in no-std no-alloc environments. It also makes
deserialization extremely cheap.
* The `dfa` and `hybrid` modules use `unsafe` to explicitly elide bounds checks
in the core search loops. This makes the codegen tighter and typically leads to
consistent 5-10% performance improvements on some workloads.

In general, the above reflect the only uses of `unsafe` throughout the entire
`regex` crate. At present, there are no plans to meaningfully expand the use
of `unsafe`. With that said, one thing folks have been asking for is cheap
deserialization of a `regex::Regex`. My sense is that this feature will require
a lot more `unsafe` in places to support zero-copy deserialization. It is
unclear at this point whether this will be pursued.


### Motivation

I started out building this crate because I wanted to re-work the `regex`
crate internals to make it more amenable to optimizations. It turns out that
there are a lot of different ways to build regex engines and even more ways to
compose them. Moreover, heuristic literal optimizations are often tricky to
get correct, but the fruit they bear is attractive. All of these things were
difficult to expand upon without risking the introduction of more bugs. So I
decided to tear things down and start fresh.

In the course of doing so, I ended up designing strong boundaries between each
component so that each component could be reasoned and tested independently.
This also made it somewhat natural to expose the components as a library unto
itself. Namely, folks have been asking for more capabilities in the regex
crate for a long time, but these capabilities usually come with additional API
complexity that I didn't want to introduce in the `regex` crate proper. But
exposing them in an "expert" level crate like `regex-automata` seemed quite
fine.

In the end, I do still somewhat consider this crate an experiment. It is
unclear whether the strong boundaries between components will be an impediment
to ongoing development or not. De-coupling tends to lead to slower development
in my experience, and when you mix in the added cost of not introducing
breaking changes all of the time, things can get quite complicated. But, I
don't think anyone has ever release the internals of a regex engine as a
library before. So it will be interesting to see how it plays out!
