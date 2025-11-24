# litemap [![crates.io](https://img.shields.io/crates/v/litemap)](https://crates.io/crates/litemap)

<!-- cargo-rdme start -->

## `litemap`

`litemap` is a crate providing [`LiteMap`], a highly simplistic "flat" key-value map
based off of a single sorted vector.

The main goal of this crate is to provide a map that is good enough for small
sizes, and does not carry the binary size impact of [`HashMap`](std::collections::HashMap)
or [`BTreeMap`](alloc::collections::BTreeMap).

If binary size is not a concern, [`std::collections::BTreeMap`] may be a better choice
for your use case. It behaves very similarly to [`LiteMap`] for less than 12 elements,
and upgrades itself gracefully for larger inputs.

### Performance characteristics

[`LiteMap`] is a data structure with similar characteristics as [`std::collections::BTreeMap`] but
with slightly different trade-offs as it's implemented on top of a flat storage (e.g. Vec).

* [`LiteMap`] iteration is generally faster than `BTreeMap` because of the flat storage.
* [`LiteMap`] can be pre-allocated whereas `BTreeMap` can't.
* [`LiteMap`] has a smaller memory footprint than `BTreeMap` for small collections (< 20 items).
* Lookup is `O(log(n))` like `BTreeMap`.
* Insertion is generally `O(n)`, but optimized to `O(1)` if the new item sorts greater than the current items. In `BTreeMap` it's `O(log(n))`.
* Deletion is `O(n)` whereas `BTreeMap` is `O(log(n))`.
* Bulk operations like `from_iter`, `extend` and deserialization have an optimized `O(n)` path
  for inputs that are ordered and `O(n*log(n))` complexity otherwise.

### Pluggable Backends

By default, [`LiteMap`] is backed by a [`Vec`]; however, it can be backed by any appropriate
random-access data store, giving that data store a map-like interface. See the [`store`]
module for more details.

### Const construction

[`LiteMap`] supports const construction from any store that is const-constructible, such as a
static slice, via [`LiteMap::from_sorted_store_unchecked()`]. This also makes [`LiteMap`]
suitable for use with [`databake`]. See [`impl Bake for LiteMap`] for more details.

[`impl Bake for LiteMap`]: ./struct.LiteMap.html#impl-Bake-for-LiteMap<K,+V,+S>
[`Vec`]: alloc::vec::Vec

<!-- cargo-rdme end -->

## More Information

For more information on development, authorship, contributing etc. please visit [`ICU4X home page`](https://github.com/unicode-org/icu4x).
