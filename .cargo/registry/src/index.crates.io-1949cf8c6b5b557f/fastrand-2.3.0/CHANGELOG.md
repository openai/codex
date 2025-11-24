# Version 2.3.0

- Accept `IntoIterator` in `choose_multiple` functions instead of just `Iterator`. (#92)

# Version 2.2.0

- Expose missing `fill` method for the global RNG. (#90)

# Version 2.1.1

- Remove support for 128-bit targets, as they are not supported by rustc yet. (#87)

# Version 2.1.0

- Change the RNG algorithm and the way that the seed is computed. This will cause
  the algorithm to emit different constants for different seeds, hence the minor
  SemVer change.
  - Update to the final WyRand v4.2 constants for better entropy. (#82)
  - Remove an unnecessary seed modification. (#73)

# Version 2.0.2

- Slight restructuring of the `with_seed` function. (#79)

# Version 2.0.1

- Clarify documentation for the `fork()` method. (#62)
- Mention `fastrand-contrib` in documentation. (#70)

# Version 2.0.0

- **Breaking:** Remove interior mutability from `Rng`. (#47)
- Add a `fork()` method. (#49)
- Add a `no_std` mode. (#50)
- Add an iterator selection function. (#51)
- Add a `choose_multiple()` function for sampling several elements from an iterator. (#55)
- Use the `getrandom` crate for seeding on WebAssembly targets if the `js` feature is enabled. (#60)

# Version 1.9.0

- Add `Rng::fill()` (#35, #43)
- Add `#[must_use]` to `Rng::with_seed()` (#46)

# Version 1.8.0

- Add `get_seed()` and `Rng::get_seed()` (#33)

# Version 1.7.0

- Add `char()` and `Rng::char()` (#25)

# Version 1.6.0

- Implement `PartialEq` and `Eq` for `Rng` (#23)

# Version 1.5.0

- Switch to Wyrand (#14)

# Version 1.4.1

- Fix bug when generating a signed integer within a range (#16)

# Version 1.4.0

- Add wasm support.

# Version 1.3.5

- Reword docs.
- Add `Rng::with_seed()`.

# Version 1.3.4

- Implement `Clone` for `Rng`.

# Version 1.3.3

- Forbid unsafe code.

# Version 1.3.2

- Support older Rust versions.

# Version 1.3.1

- Tweak Cargo keywords.

# Version 1.3.0

- Add `f32()` and `f64()`.
- Add `lowercase()`, `uppercase()`, `alphabetic()`, and `digit()`.

# Version 1.2.4

- Switch to PCG XSH RR 64/32.
- Fix a bug in `gen_mod_u128`.
- Fix bias in ranges.

# Version 1.2.3

- Support Rust 1.32.0

# Version 1.2.2

- Use `std::$t::MAX` rather than `$t::MAX` to support older Rust versions.

# Version 1.2.1

- Inline all functions.

# Version 1.2.0

- Add `Rng` struct.

# Version 1.1.0

- Switch to PCG implementation.
- Add `alphanumeric()`.
- Add `seed()`.

# Version 1.0.0

- Initial version
