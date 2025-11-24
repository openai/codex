# 0.4.11 (August 8, 2025)

* Fix `Slab::get_disjoint_mut` out of bounds (#152)

# 0.4.10 (June 15, 2025)

* Add `Slab::get_disjoint_mut` (#149)
* Drop build script and `autocfg` dependency (#150)
* Fix redundant import warning in no_std builds (#143)
* Fix `clippy::needless_lifetimes` warning (#147)
* Internal CI improvements (#141, #146)

# 0.4.9 (August 22, 2023)

* Avoid reallocations in `Slab::clone_from` (#137)

# 0.4.8 (January 20, 2023)

* Fixed documentation about overflow (#124)
* Document panic in `get2_mut` (#131)
* Refactoring (#129, #132)

# 0.4.7 (July 19, 2022)

* Use `#[track_caller]` on Rust 1.46+ (#119)
* Make `Slab::new` const on Rust 1.39+ (#119)

# 0.4.6 (April 2, 2022)

* Add `Slab::vacant_key` (#114)
* Fix stacked borrows violation in `Slab::get2_unchecked_mut` (#115)

# 0.4.5 (October 13, 2021)

* Add alternate debug output for listing items in the slab (#108)
* Fix typo in debug output of IntoIter (#109)
* Impl 'Clone' for 'Iter' (#110)

# 0.4.4 (August 06, 2021)

* Fix panic in `FromIterator` impl (#102)
* Fix compatibility with older clippy versions (#104)
* Add `try_remove` method (#89)
* Implement `ExactSizeIterator` and `FusedIterator` for iterators (#92)

# 0.4.3 (April 20, 2021)

* Add no_std support for Rust 1.36 and above (#71).
* Add `get2_mut` and `get2_unchecked_mut` methods (#65).
* Make `shrink_to_fit()` remove trailing vacant entries (#62).
* Implement `FromIterator<(usize, T)>` (#62).
* Implement `IntoIterator<Item = (usize, T)>` (#62).
* Provide `size_hint()` of the iterators (#62).
* Make all iterators reversible (#62).
* Add `key_of()` method (#61)
* Add `compact()` method (#60)
* Add support for serde (#85)

# 0.4.2 (January 11, 2019)

* Add `Slab::drain` (#56).

# 0.4.1 (July 15, 2018)

* Improve `reserve` and `reserve_exact` (#37).
* Implement `Default` for `Slab` (#43).
