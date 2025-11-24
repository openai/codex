Rust Radix Trie
====

[![Unix Build Status](https://travis-ci.org/michaelsproul/rust_radix_trie.svg?branch=master)](https://travis-ci.org/michaelsproul/rust_radix_trie)
[![Windows Build Status](https://ci.appveyor.com/api/projects/status/d2voj1te0m7agfne/branch/master?svg=true)](https://ci.appveyor.com/project/michaelsproul/rust-radix-trie/branch/master)

This is a [Radix Trie][radix-wiki] implementation in Rust, building on the lessons learnt from `TrieMap` and [Sequence Trie][seq-trie]. You can read about my experience implementing this data structure [here][radix-paper].

# Help Wanted, Enquire Within

*Since writing this code I haven't used it in anger (or production) so it is no doubt in need of some maintenance, testing and optimisation love. If you would like to help out, try solving an open issue, optimise something, or just have a poke around! Thanks :)*

# Features

* Compressed nodes. Common key prefixes are stored only once.
* Trie-specific methods to look-up closest ancestors and descendants.
* Key Generic. Any type that can be serialised as a vector of bytes can be used as a key.
* Safe - no unsafe code.

# Documentation

https://docs.rs/radix_trie/

# Usage

Available on [Crates.io][] as [`radix_trie`][radix-crate].

Just add `radix_trie` to the dependencies section of your `Cargo.toml`, like so:

```toml
radix_trie = "0.2"
```

# Contributors

Made by:

* Allan Simon ([@allan-simon](https://github.com/allan-simon))
* Andrew Smith ([@andrewcsmith](https://github.com/andrewcsmith))
* Arthur Carcano ([@NougatRillettes](https://github.com/NougatRillettes))
* Devin Ragotzy ([@DevinR528](https://github.com/DevinR528))
* [@hanabi1224](https://github.com/hanabi1224)
* Jakob Dalsgaard ([@jakobdalsgaard](https://github.com/jakobdalsgaard))
* Michael Sproul ([@michaelsproul](https://github.com/michaelsproul))
* Robin Lambertz ([@roblabla](https://github.com/roblabla))
* Sergey ([@Albibek](https://github.com/Albibek))
* Stuart Hinson ([@stuarth](https://github.com/stuarth))
* Vinzent Steinberg ([@vks](https://github.com/vks))

# License

MIT License. Copyright © Michael Sproul and contributors 2015-present.

[radix-wiki]: http://en.wikipedia.org/wiki/Radix_tree
[seq-trie]: https://github.com/michaelsproul/rust_sequence_trie
[radix-paper]: https://michaelsproul.github.io/rust_radix_paper/
[crates.io]: https://crates.io/
[radix-crate]: https://crates.io/crates/radix_trie
