# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.1.4 (2025-02-21)
### Fixed
- Return output length from `InOutBufReserved::get_out_len` instead of input length ([#1150])

[#1150]: https://github.com/RustCrypto/utils/pull/1150

## 0.1.3 (2022-03-31)
### Fixed
- MIRI error in `From` impl for `InOutBuf` ([#755])

[#755]: https://github.com/RustCrypto/utils/pull/755

## 0.1.2 (2022-02-10)
### Changed
- Use borrow instead of consuming in `InOutBufReserved::get_*_len()` methods ([#734])

[#734]: https://github.com/RustCrypto/utils/pull/734

## 0.1.1 (2022-02-10)
### Fixed
- Fix doc build on docs.rs by optionally enabling the `doc_cfg` feature ([#733])

[#733]: https://github.com/RustCrypto/utils/pull/733

## 0.1.0 (2022-02-10)
- Initial release ([#675])

[#675]: https://github.com/RustCrypto/utils/pull/675
