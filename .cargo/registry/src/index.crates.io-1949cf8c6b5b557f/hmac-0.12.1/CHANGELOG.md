# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.12.1 (2022-02-17)
### Fixed
- Minimal versions build ([#108])

[#108]: https://github.com/RustCrypto/MACs/pull/108

## 0.12.0 (2021-12-07)
### Changed
- Bump `digest` crate dependency to v0.10 and remove `crypto-mac` ([#97])
- Use a more efficient state representation by using block-level hash API ([#97])

### Added
- `SimpleHmac` as a less constrained alternative to `Hmac` ([#97])

[#97]: https://github.com/RustCrypto/MACs/pull/97

## 0.11.0 (2021-04-29)
### Changed
- Bump `crypto-mac` crate dependency to v0.11 ([#73])

[#73]: https://github.com/RustCrypto/MACs/pull/73

## 0.10.1 (2020-10-16)
### Added
- Zulip badge ([#64])

[#64]: https://github.com/RustCrypto/MACs/pull/64

## 0.10.0 (2020-10-16)
### Changed
- Bump `crypto-mac` dependency to v0.10 ([#62])

[#62]: https://github.com/RustCrypto/MACs/pull/62

## 0.9.0 (2020-08-12)
### Changed
- Bump `crypto-mac` dependency to v0.9 ([#57])

### Added
- Implement `io::Write` ([#55])

[#55]: https://github.com/RustCrypto/MACs/pull/55
[#57]: https://github.com/RustCrypto/MACs/pull/57

## 0.8.1 (2020-06-24)
### Fixed
- Replace outdated `code` with `into_bytes` in documentation ([#50])

[#50]: https://github.com/RustCrypto/MACs/pull/50

## 0.8.0 (2020-06-09)
### Changed
- Upgrade to `digest` v0.9 crate release; MSRV 1.41 ([#45])
- Upgrade `crypto-mac` to v0.8 ([#33])
- Rename `*result*` to `finalize` ([#38])
- Upgrade to Rust 2018 edition  ([#33])

[#45]: https://github.com/RustCrypto/MACs/pull/45
[#38]: https://github.com/RustCrypto/MACs/pull/38
[#33]: https://github.com/RustCrypto/MACs/pull/33

## 0.7.1 (2019-07-11)

## 0.7.0 (2018-10-03)

## 0.6.3 (2018-08-15)

## 0.6.2 (2018-04-15)

## 0.6.1 (2018-04-05)

## 0.6.0 (2018-03-30)

## 0.5.0 (2017-11-15)

## 0.4.2 (2017-07-24)

## 0.4.1 (2017-07-24)

## 0.4.0 (2017-07-24)

## 0.3.1 (2017-06-12)

## 0.1.2 (2017-07-24)

## 0.1.1 (2017-05-14)

## 0.1.0 (2017-05-14)

## 0.0.1 (2016-10-21)
