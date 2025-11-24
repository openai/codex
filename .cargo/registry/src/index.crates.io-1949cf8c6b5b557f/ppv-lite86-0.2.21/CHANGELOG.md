# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.16]
### Added
- add [u64; 4] conversion for generic vec256, to support BLAKE on non-x86.
- impl `From` (rather than just `Into`) for conversions between `*_storage` types and arrays.
