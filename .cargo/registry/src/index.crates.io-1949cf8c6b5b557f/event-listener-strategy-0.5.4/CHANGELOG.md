# Version 0.5.4

- Add `portable-atomic` feature that exposes `event-listener`'s underlying `portable-atomic` feature. (#27)

# Version 0.5.3

- Add `loom` feature that exposes `event-listener`'s underlying `loom` feature. (#24)

# Version 0.5.2

- Re-export the `event-listener` crate. (#20)

# Version 0.5.1

- Fix the `repository` field in `Cargo.toml` to point to the correct repository. (#17)

# Version 0.5.0

- **Breaking:** Bump `event-listener` to v5.0.0. (#12)
- Bump MSRV to 1.60. (#14)
- Make `NonBlocking` `Send` and `Sync`. (#15)

# Version 0.4.0

- **Breaking:** Bump `event-listener` to v4.0.0. (#10)

# Version 0.3.0

- **Breaking:** Remove an unneeded lifetime from the public API. (#6)

# Version 0.2.0

- **Breaking:** Add support for WASM targets by disabling `wait()` on them. (#3)

# Version 0.1.0

- Initial version
