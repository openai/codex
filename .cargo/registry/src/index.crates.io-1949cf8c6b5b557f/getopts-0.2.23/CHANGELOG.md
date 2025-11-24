# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.23](https://github.com/rust-lang/getopts/compare/v0.2.22...v0.2.23) - 2025-06-09

### Other

- Add caching
- Remove redundant configuration from Cargo.toml
- Bump unicode-width to 0.2.0
- Update the MSRV to 1.66 and edition to 2021

## [0.2.22](https://github.com/rust-lang/getopts/compare/v0.2.21...v0.2.22) - 2025-06-05

### Other

- Add a check for formatting, apply `cargo fmt`
- Add a release job
- Document and start testing the MSRV
- Test on more platforms, deny warnings
- Eliminate `html_root_url`
- Update version number in html_root_url
- Use SPDX license format
- Fix compiler warning in documentation example
- Merge pull request #100 from zdenek-crha/parse_args_end_position
- Merge pull request #103 from zdenek-crha/better_usage_examples
- Add usage examples for methods that add option config
- Update outdated top level documentation
- Add triagebot configuration
- remove deprecated Error::description
- Update documentation of opt_present() and other functions that might panic
- Updated tests for opts_str() and opts_str_first() to check order of processing
- Add opts_present_any() and opts_str_first() interface functions
- Parse options without names vector
