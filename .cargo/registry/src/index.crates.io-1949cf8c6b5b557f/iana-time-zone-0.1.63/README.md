# iana-time-zone - get the IANA time zone for the current system

[![Crates.io](https://img.shields.io/crates/v/iana-time-zone.svg)](https://crates.io/crates/iana-time-zone)
[![Documentation](https://docs.rs/iana-time-zone/badge.svg)](https://docs.rs/iana-time-zone/)
[![Crate License](https://img.shields.io/crates/l/iana-time-zone.svg)](https://crates.io/crates/iana-time-zone)
[![build](https://github.com/strawlab/iana-time-zone/actions/workflows/rust.yml/badge.svg)](https://github.com/strawlab/iana-time-zone/actions?query=branch%3Amain)

This small utility crate gets the IANA time zone for the current system. This is
also known as the [tz database], tzdata, the zoneinfo database, and the Olson
database.

[tz database]: https://en.wikipedia.org/wiki/Tz_database

Example:

```rust
// Get the current time zone as a string.
let tz_str = iana_time_zone::get_timezone()?;
println!("The current time zone is: {}", tz_str);
```

You can test this is working on your platform with:

```
cargo run --example get_timezone
```

## Minimum supported rust version policy

This crate has a minimum supported rust version (MSRV) of 1.62.0 for [Tier 1]
platforms.

[tier 1]: https://doc.rust-lang.org/1.62.0/rustc/platform-support.html

Updates to the MSRV are sometimes necessary due to the MSRV of dependencies.
MSRV updates will not be indicated as a breaking change to the semver version.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or
  <http://opensource.org/licenses/MIT>)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
