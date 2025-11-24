# zerotrie [![crates.io](https://img.shields.io/crates/v/zerotrie)](https://crates.io/crates/zerotrie)

<!-- cargo-rdme start -->

A data structure offering zero-copy storage and retrieval of byte strings, with a focus
on the efficient storage of ASCII strings. Strings are mapped to `usize` values.

[`ZeroTrie`] does not support mutation because doing so would require recomputing the entire
data structure. Instead, it supports conversion to and from [`LiteMap`] and [`BTreeMap`].

There are multiple variants of [`ZeroTrie`] optimized for different use cases.

## Examples

```rust
use zerotrie::ZeroTrie;

let data: &[(&str, usize)] = &[("abc", 11), ("xyz", 22), ("axyb", 33)];

let trie: ZeroTrie<Vec<u8>> = data.iter().copied().collect();

assert_eq!(trie.get("axyb"), Some(33));
assert_eq!(trie.byte_len(), 18);
```

## Internal Structure

To read about the internal structure of [`ZeroTrie`], build the docs with private modules:

```bash
cargo doc --document-private-items --all-features --no-deps --open
```

[`LiteMap`]: litemap::LiteMap
[`BTreeMap`]: alloc::collections::BTreeMap

<!-- cargo-rdme end -->

## More Information

For more information on development, authorship, contributing etc. please visit [`ICU4X home page`](https://github.com/unicode-org/icu4x).
