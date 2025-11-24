<a name="v0.1.7"></a>
### v0.1.7 (2023-10-04)


#### Bug Fixes

*   index out of bounds in `get()` and `get_owned()` (#88) ([fdbc930f](https://github.com/hawkw/sharded-slab/commit/fdbc930fb14b0f6f8b77cd6efdad5a1bdf8d3c04))
* **unique_iter:**  prevent panics if a slab is empty (#88) ([bd599e0b](https://github.com/hawkw/sharded-slab/commit/bd599e0b2a60a953f25f27ba1fa86682150e05c2), closes [#73](https://github.com/hawkw/sharded-slab/issues/73))



<a name="0.1.6"></a>
## 0.1.6 (2023-09-27)


#### Features

*   publicly export `UniqueIter` (#87) ([e4d6482d](https://github.com/hawkw/sharded-slab/commit/e4d6482db05d5767b47eae1b0217faad30f2ebd5), closes [#77](https://github.com/hawkw/sharded-slab/issues/77))

#### Bug Fixes

*   use a smaller `CustomConfig` for 32-bit tests (#84) ([828ffff9](https://github.com/hawkw/sharded-slab/commit/828ffff9f82cfc41ed66b4743563c4dddc97c1ce), closes [#82](https://github.com/hawkw/sharded-slab/issues/82))



<a name="0.1.5"></a>
## 0.1.5 (2023-08-28)


#### Bug Fixes

* **Slab:**  invalid generation in case of custom config (#80) ([ca090279](https://github.com/hawkw/sharded-slab/commit/ca09027944812d024676029a3dde62d27ef22015))



<a name="0.1.4"></a>
### 0.1.4 (2021-10-12)


#### Features

*   emit a nicer panic when thread count overflows `MAX_SHARDS` (#64) ([f1ed058a](https://github.com/hawkw/sharded-slab/commit/f1ed058a3ee296eff033fc0fb88f62a8b2f83f10))



<a name="0.1.3"></a>
### 0.1.3 (2021-08-02)


#### Bug Fixes

*   set up MSRV in CI (#61) ([dfcc9080](https://github.com/hawkw/sharded-slab/commit/dfcc9080a62d08e359f298a9ffb0f275928b83e4), closes [#60](https://github.com/hawkw/sharded-slab/issues/60))
* **tests:**  duplicate `hint` mod defs with loom ([0ce3fd91](https://github.com/hawkw/sharded-slab/commit/0ce3fd91feac8b4edb4f1ece6aebfc4ba4e50026))



<a name="0.1.2"></a>
### 0.1.2 (2021-08-01)


#### Bug Fixes

*   make debug assertions drop safe ([26d35a69](https://github.com/hawkw/sharded-slab/commit/26d35a695c9e5d7c62ab07cc5e66a0c6f8b6eade))

#### Features

*   improve panics on thread ID bit exhaustion ([9ecb8e61](https://github.com/hawkw/sharded-slab/commit/9ecb8e614f107f68b5c6ba770342ae72af1cd07b))



<a name="0.1.1"></a>
## 0.1.1 (2021-1-4)


#### Bug Fixes

* change `loom` to an optional dependency ([9bd442b5](https://github.com/hawkw/sharded-slab/commit/9bd442b57bc56153a67d7325144ebcf303e0fe98))

<a name="0.1.0"></a>
## 0.1.0 (2020-10-20)


#### Bug Fixes

*   fix `remove` and `clear` returning true when the key is stale ([b52d38b2](https://github.com/hawkw/sharded-slab/commit/b52d38b2d2d3edc3a59d3dba6b75095bbd864266))

#### Breaking Changes

* **Pool:**  change `Pool::create` to return a mutable guard (#48) ([778065ea](https://github.com/hawkw/sharded-slab/commit/778065ead83523e0a9d951fbd19bb37fda3cc280), closes [#41](https://github.com/hawkw/sharded-slab/issues/41), [#16](https://github.com/hawkw/sharded-slab/issues/16))
* **Slab:**  rename `Guard` to `Entry` for consistency ([425ad398](https://github.com/hawkw/sharded-slab/commit/425ad39805ee818dc6b332286006bc92c8beab38))

#### Features

*   add missing `Debug` impls ([71a8883f](https://github.com/hawkw/sharded-slab/commit/71a8883ff4fd861b95e81840cb5dca167657fe36))
* **Pool:**
  *  add `Pool::create_owned` and `OwnedRefMut` ([f7774ae0](https://github.com/hawkw/sharded-slab/commit/f7774ae0c5be99340f1e7941bde62f7044f4b4d8))
  *  add `Arc<Pool>::get_owned` and `OwnedRef` ([3e566d91](https://github.com/hawkw/sharded-slab/commit/3e566d91e1bc8cc4630a8635ad24b321ec047fe7), closes [#29](https://github.com/hawkw/sharded-slab/issues/29))
  *  change `Pool::create` to return a mutable guard (#48) ([778065ea](https://github.com/hawkw/sharded-slab/commit/778065ead83523e0a9d951fbd19bb37fda3cc280), closes [#41](https://github.com/hawkw/sharded-slab/issues/41), [#16](https://github.com/hawkw/sharded-slab/issues/16))
* **Slab:**
  *  add `Arc<Slab>::get_owned` and `OwnedEntry` ([53a970a2](https://github.com/hawkw/sharded-slab/commit/53a970a2298c30c1afd9578268c79ccd44afba05), closes [#29](https://github.com/hawkw/sharded-slab/issues/29))
  *  rename `Guard` to `Entry` for consistency ([425ad398](https://github.com/hawkw/sharded-slab/commit/425ad39805ee818dc6b332286006bc92c8beab38))
  *  add `slab`-style `VacantEntry` API ([6776590a](https://github.com/hawkw/sharded-slab/commit/6776590adeda7bf4a117fb233fc09cfa64d77ced), closes [#16](https://github.com/hawkw/sharded-slab/issues/16))

#### Performance

*   allocate shard metadata lazily (#45) ([e543a06d](https://github.com/hawkw/sharded-slab/commit/e543a06d7474b3ff92df2cdb4a4571032135ff8d))



<a name="0.0.9"></a>
### 0.0.9 (2020-04-03)


#### Features

* **Config:**  validate concurrent refs ([9b32af58](9b32af58), closes [#21](21))
* **Pool:**
  *  add `fmt::Debug` impl for `Pool` ([ffa5c7a0](ffa5c7a0))
  *  add `Default` impl for `Pool` ([d2399365](d2399365))
  *  add a sharded object pool for reusing heap allocations (#19) ([89734508](89734508), closes [#2](2), [#15](15))
* **Slab::take:**  add exponential backoff when spinning ([6b743a27](6b743a27))

#### Bug Fixes

*   incorrect wrapping when overflowing maximum ref count ([aea693f3](aea693f3), closes [#22](22))



<a name="0.0.8"></a>
### 0.0.8 (2020-01-31)


#### Bug Fixes

*   `remove` not adding slots to free lists ([dfdd7aee](dfdd7aee))



<a name="0.0.7"></a>
### 0.0.7 (2019-12-06)


#### Bug Fixes

* **Config:**  compensate for 0 being a valid TID ([b601f5d9](b601f5d9))
* **DefaultConfig:**
  *  const overflow on 32-bit ([74d42dd1](74d42dd1), closes [#10](10))
  *  wasted bit patterns on 64-bit ([8cf33f66](8cf33f66))



<a name="0.0.6"></a>
## 0.0.6 (2019-11-08)


#### Features

* **Guard:**  expose `key` method #8 ([748bf39b](748bf39b))



<a name="0.0.5"></a>
## 0.0.5 (2019-10-31)


#### Performance

*   consolidate per-slot state into one AtomicUsize (#6) ([f1146d33](f1146d33))

#### Features

*   add Default impl for Slab ([61bb3316](61bb3316))



<a name="0.0.4"></a>
## 0.0.4 (2019-21-30)


#### Features

*   prevent items from being removed while concurrently accessed ([872c81d1](872c81d1))
*   added `Slab::remove` method that marks an item to be removed when the last thread
    accessing it finishes ([872c81d1](872c81d1))

#### Bug Fixes

*   nicer handling of races in remove ([475d9a06](475d9a06))

#### Breaking Changes

*   renamed `Slab::remove` to `Slab::take` ([872c81d1](872c81d1))
*   `Slab::get` now returns a `Guard` type ([872c81d1](872c81d1))


<a name="0.0.3"></a>
## 0.0.3 (2019-07-30)


#### Bug Fixes

*   split local/remote to fix false sharing & potential races ([69f95fb0](69f95fb0))
*   set next pointer _before_ head ([cc7a0bf1](cc7a0bf1))

#### Breaking Changes

*   removed potentially racy `Slab::len` and `Slab::capacity` methods ([27af7d6c](27af7d6c))

<a name="0.0.2"></a>
## 0.0.2 (2019-03-30)


#### Bug Fixes

*   fix compilation failure in release mode ([617031da](617031da))


<a name="0.0.1"></a>
## 0.0.1 (2019-02-30)

- Initial release
