# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.1] - 2021-06-09
- add getters corresponding to existing setters: `get_seed`, `get_stream` (#1124)
- add serde support, gated by the `serde1` feature (#1124)
- ensure expected layout via `repr(transparent)` (#1120)

## [0.3.0] - 2020-12-08
- Bump `rand_core` version to 0.6.0
- Bump MSRV to 1.36 (#1011)
- Remove usage of deprecated feature "simd" of `ppv-lite86` (#979), then revert
  this change (#1023) since SIMD is only enabled by default from `ppv-lite86 v0.2.10`
- impl PartialEq+Eq for ChaChaXRng and ChaChaXCore (#979)
- Fix panic on block counter wrap that was occurring in debug builds (#980)

## [0.2.2] - 2020-03-09
- Integrate `c2-chacha`, reducing dependency count (#931)
- Add CryptoRng to ChaChaXCore (#944)

## [0.2.1] - 2019-07-22
- Force enable the `simd` feature of `c2-chacha` (#845)

## [0.2.0] - 2019-06-06
- Rewrite based on the much faster `c2-chacha` crate (#789)

## [0.1.1] - 2019-01-04
- Disable `i128` and `u128` if the `target_os` is `emscripten` (#671: work-around Emscripten limitation)
- Update readme and doc links

## [0.1.0] - 2018-10-17
- Pulled out of the Rand crate
