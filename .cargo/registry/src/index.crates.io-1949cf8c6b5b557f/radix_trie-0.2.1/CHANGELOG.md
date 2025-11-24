Changelog
====

0.2.1:
    * Implement `TrieKey` for vectors of integers (#63)
    * Released 22/10/2020

0.2.0:
    * Update `nibble_vec` to v0.1.0: faster and uses less memory!
    * Released 28/06/2020

0.1.6:

* Update to Rust 2018, add benchmarking code (#52)
* Appveyor CI for Windows (#42)
* No API changes or bugfixes

0.1.5:

* Fix another bug related to the removal of non-existent keys (#50)
* Implement `Clone` for `Trie`

0.1.4:

* Fix a panic that occurred when removing non-existent keys (#40)
* Reformat the code using the latest `rustfmt`

0.1.3:

* Add `prefix()` method for fetching the `NibbleVec` of a trie or subtrie
* Update `nibble_vec` to v0.0.4
* Make better use of lifetime elision (type signatures will look different, but are the same)

0.1.2:

* Display README on crates.io (no code changes)
