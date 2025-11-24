# 0.3.3 (August 1, 2024)

### Added

- **builder,util**: add convenience methods for boxing services ([#616])
- **all**: new functions const when possible ([#760])

[#616]: https://github.com/tower-rs/tower/pull/616
[#760]: https://github.com/tower-rs/tower/pull/760

# 0.3.2 (Octpber  10, 2022)

## Added

- Implement `Layer` for tuples of up to 16 elements ([#694])

[#694]: https://github.com/tower-rs/tower/pull/694

# 0.3.1 (January 7, 2021)

### Added

- Added `layer_fn`, for constructing a `Layer` from a function taking
  a `Service` and returning a different `Service` ([#491])
- Added an implementation of `Layer` for `&Layer` ([#446])
- Multiple documentation improvements ([#487], [#490])

[#491]: https://github.com/tower-rs/tower/pull/491
[#446]: https://github.com/tower-rs/tower/pull/446
[#487]: https://github.com/tower-rs/tower/pull/487
[#490]: https://github.com/tower-rs/tower/pull/490

# 0.3.0 (November 29, 2019)

- Move layer builder from `tower-util` to tower-layer.

# 0.3.0-alpha.2 (September 30, 2019)

- Move to `futures-*-preview 0.3.0-alpha.19`
- Move to `pin-project 0.4`

# 0.3.0-alpha.1 (September 11, 2019)

- Move to `std::future`

# 0.1.0 (April 26, 2019)

- Initial release
