# Changelog

## [v0.12.5](https://github.com/jeromefroe/lru-rs/tree/0.12.5) - 2024-10-30

- Upgrade hashbrown dependency to 0.15.

## [v0.12.4](https://github.com/jeromefroe/lru-rs/tree/0.12.4) - 2024-07-30

- Add methods that take a reference to the key that should be inserted.

## [v0.12.3](https://github.com/jeromefroe/lru-rs/tree/0.12.3) - 2024-02-24

- Add `get_key_value_mut` method.

## [v0.12.2](https://github.com/jeromefroe/lru-rs/tree/0.12.2) - 2024-01-28

- Add `clone` method.

## [v0.12.1](https://github.com/jeromefroe/lru-rs/tree/0.12.1) - 2023-11-21

- Add `get_key_value` method.

## [v0.12.0](https://github.com/jeromefroe/lru-rs/tree/0.12.0) - 2023-10-03

- Add lifetime specifier to `try_get_or_insert_mut`.
- Add `BuildHasher` trait bound to `Debug` for `LruCache`.

## [v0.11.1](https://github.com/jeromefroe/lru-rs/tree/0.11.1) - 2023-09-05

- Add `try_get_or_insert_mut` method.

## [v0.11.0](https://github.com/jeromefroe/lru-rs/tree/0.11.0) - 2023-07-11

- Update dependency on hashbrown to 0.14 and update MSRV to 1.64.0.

## [v0.10.1](https://github.com/jeromefroe/lru-rs/tree/0.10.1) - 2023-06-29

- Add `try_get_or_insert` method.

## [v0.10.0](https://github.com/jeromefroe/lru-rs/tree/0.10.0) - 2023-03-04

- Remove `KeyRef` from the public API.

## [v0.9.0](https://github.com/jeromefroe/lru-rs/tree/0.9.0) - 2022-12-31

- Update dependency on hashbrown to 0.13.
- Change `LruCache.map` to hold a pointer, rather than owned `LruEntry`.

## [v0.8.1](https://github.com/jeromefroe/lru-rs/tree/0.8.1) - 2022-10-01

- Add "caching" and "no-std" to list of categories.

## [v0.8.0](https://github.com/jeromefroe/lru-rs/tree/0.8.0) - 2022-09-11

- Use `NonZeroUsize` for LRU capacity instead of usize.
- Add `get_or_insert_mut` method which is similiar to `get_or_insert` but returns a mutable
  reference instead of an immutable reference.
- Add `LruCache::promote` and `LruCache::demote` API to manipulate LRU order of the entry directly.

## [v0.7.8](https://github.com/jeromefroe/lru-rs/tree/0.7.8) - 2022-07-19

- Update dependency on hashbrown to 0.12.

## [v0.7.7](https://github.com/jeromefroe/lru-rs/tree/0.7.7) - 2022-06-14

- Use `FnOnce` instead of `Fn` for trait bound of `fn` in `get_or_insert`.

## [v0.7.6](https://github.com/jeromefroe/lru-rs/tree/0.7.6) - 2022-05-25

- Add `pop_entry` method.

## [v0.7.5](https://github.com/jeromefroe/lru-rs/tree/0.7.5) - 2022-04-04

- Add `push` method.

## [v0.7.4](https://github.com/jeromefroe/lru-rs/tree/0.7.4) - 2022-04-03

- Implement `IntoIterator` trait and fix lifetime bug in `get_or_insert`.

## [v0.7.3](https://github.com/jeromefroe/lru-rs/tree/0.7.3) - 2022-02-28

- Add `get_or_insert` method.

## [v0.7.2](https://github.com/jeromefroe/lru-rs/tree/0.7.2) - 2021-12-28

- Explicitly implement Borrow for Box for non-nightly.

## [v0.7.1](https://github.com/jeromefroe/lru-rs/tree/0.7.1) - 2021-12-18

- Fix lifetime of iterators.

## [v0.7.0](https://github.com/jeromefroe/lru-rs/tree/0.7.0) - 2021-09-14

- Explicitly implement Borrow for String and Vec types for non-nightly.

## [v0.6.6](https://github.com/jeromefroe/lru-rs/tree/0.6.6) - 2021-07-28

- Update dependency on hashbrown to 0.11.2.

## [v0.6.5](https://github.com/jeromefroe/lru-rs/tree/0.6.5) - 2021-02-12

- Add `unbounded_with_hasher` constructor.

## [v0.6.4](https://github.com/jeromefroe/lru-rs/tree/0.6.4) - 2021-02-03

- Fix memory leak when keys contain heap allocated data and ensure send/sync bounds apply to
  all conforming hashers.

## [v0.6.3](https://github.com/jeromefroe/lru-rs/tree/0.6.3) - 2020-12-19

- Fix memory leak in `clear` and `resize` methods.

## [v0.6.2](https://github.com/jeromefroe/lru-rs/tree/0.6.2) - 2020-12-12

- Rename `optin_builtin_traits` to `auto_traits`.

## [v0.6.1](https://github.com/jeromefroe/lru-rs/tree/0.6.1) - 2020-11-02

- Update dependency on hashbrown to 0.9.

## [v0.6.0](https://github.com/jeromefroe/lru-rs/tree/0.6.0) - 2020-08-02

- Update dependency on hashbrown to 0.8.

## [v0.5.3](https://github.com/jeromefroe/lru-rs/tree/0.5.3) - 2020-07-06

- Fix bug that causes crash when putting an item into a zero-capacity cache.

## [v0.5.2](https://github.com/jeromefroe/lru-rs/tree/0.5.2) - 2020-06-17

- Fix nightly feature.

## [v0.5.1](https://github.com/jeromefroe/lru-rs/tree/0.5.1) - 2020-06-02

- Fix memory leak whereby old entries wouldn't be dropped when cache is full.

## [v0.5.0](https://github.com/jeromefroe/lru-rs/tree/0.5.0) - 2020-05-28

- Stop gating the `alloc` crate behind the `nightly` flag.

## [v0.4.5](https://github.com/jeromefroe/lru-rs/tree/0.4.5) - 2020-05-25

- Use `as_mut_ptr` in `drop` to fix memory leak.

## [v0.4.4](https://github.com/jeromefroe/lru-rs/tree/0.4.4) - 2020-05-19

- Use `mem::MaybeUninit` for key and value fields of nodes and not the nodes themselves.

## [v0.4.3](https://github.com/jeromefroe/lru-rs/tree/0.4.3) - 2019-12-10

- Add back import of alloc crate on nightly which was accidentally removed.

## [v0.4.2](https://github.com/jeromefroe/lru-rs/tree/0.4.2) - 2019-12-08

- Make hasbrown usage optional and add MSRV documentation.

## [v0.4.1](https://github.com/jeromefroe/lru-rs/tree/0.4.1) - 2019-11-26

- Use `mem::MaybeUninit` instead of `mem::uninitialized`.

## [v0.4.0](https://github.com/jeromefroe/lru-rs/tree/0.4.0) - 2019-10-28

- Use `Borrow` trait in `contains` and `pop` methods.

## [v0.3.1](https://github.com/jeromefroe/lru-rs/tree/0.3.1) - 2019-10-08

- Implement `Debug` for `LruCache`.

## [v0.3.0](https://github.com/jeromefroe/lru-rs/tree/0.3.0) - 2019-10-06

- Update the signature of the `peek` methods to use the `Borrow` trait and add a `peek_mut` method.

## [v0.2.0](https://github.com/jeromefroe/lru-rs/tree/0.2.0) - 2019-09-27

- Release a new minor version because of an accidental breaking change in the previous release ([#50](https://github.com/jeromefroe/lru-rs/issues/50)).

## [v0.1.18](https://github.com/jeromefroe/lru-rs/tree/0.1.18) - 2019-09-25

- Add borrowed type support for get_mut.

## [v0.1.17](https://github.com/jeromefroe/lru-rs/tree/0.1.17) - 2019-08-21

- Return Option from put method which contains old value if it exists.

## [v0.1.16](https://github.com/jeromefroe/lru-rs/tree/0.1.16) - 2019-07-25

- Implement Borrow trait for KeyRef with nightly OIBIT feature.

## [v0.1.15](https://github.com/jeromefroe/lru-rs/tree/0.1.15) - 2019-04-13

- Make crate no_std compatible with nightly feature.

## [v0.1.14](https://github.com/jeromefroe/lru-rs/tree/0.1.14) - 2019-04-13

- Implement `IterMut` to be able to get a mutable iterator for the cache.

## [v0.1.13](https://github.com/jeromefroe/lru-rs/tree/0.1.13) - 2019-03-12

- Bug fix to ensure that popped items are released.

## [v0.1.12](https://github.com/jeromefroe/lru-rs/tree/0.1.12) - 2019-03-04

- Replace standard HashMap with hashbrown HashMap.

## [v0.1.11](https://github.com/jeromefroe/lru-rs/tree/0.1.11) - 2018-12-10

- Implement `Iterator` trait for the cache.

## [v0.1.10](https://github.com/jeromefroe/lru-rs/tree/0.1.10) - 2018-11-07

- Add `peek_lru` method to get the least recently used element.

## [v0.1.9](https://github.com/jeromefroe/lru-rs/tree/0.1.9) - 2018-10-30

- Add `with_hasher` constructor to allow callers to use a custom hash function.

## [v0.1.8](https://github.com/jeromefroe/lru-rs/tree/0.1.8) - 2018-08-19

- Add `pop_lru` to remove least recently used element and `unbounded` constructor.

## [v0.1.7](https://github.com/jeromefroe/lru-rs/tree/0.1.7) - 2018-01-22

- Implement `Send` and `Sync` for the cache.

## [v0.1.6](https://github.com/jeromefroe/lru-rs/tree/0.1.6) - 2018-01-15

- Add `resize` method to dynamically resize the cache.

## [v0.1.5](https://github.com/jeromefroe/lru-rs/tree/0.1.5) - 2018-01-15

- Add `get_mut` method to get a mutable reference from the cache.

## [v0.1.4](https://github.com/jeromefroe/lru-rs/tree/0.1.4) - 2017-02-19

- Add function to clear the contents of the cache.

## [v0.1.3](https://github.com/jeromefroe/lru-rs/tree/0.1.3) - 2017-01-02

- Create internal hashmap with specified capacity.

## [v0.1.2](https://github.com/jeromefroe/lru-rs/tree/0.1.2) - 2017-01-02

- Add `peek` and `contains` functions.

## [v0.1.1](https://github.com/jeromefroe/lru-rs/tree/0.1.1) - 2016-12-31

- Fix documentation link in Cargo.toml.

## [v0.1.0](https://github.com/jeromefroe/lru-rs/tree/0.1.0) - 2016-12-31

- Initial release.
