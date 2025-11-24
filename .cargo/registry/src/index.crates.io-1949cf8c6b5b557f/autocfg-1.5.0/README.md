autocfg
=======

[![autocfg crate](https://img.shields.io/crates/v/autocfg.svg)](https://crates.io/crates/autocfg)
[![autocfg documentation](https://docs.rs/autocfg/badge.svg)](https://docs.rs/autocfg)
![minimum rustc 1.0](https://img.shields.io/badge/rustc-1.0+-red.svg)
![build status](https://github.com/cuviper/autocfg/workflows/CI/badge.svg)

A Rust library for build scripts to automatically configure code based on
compiler support.  Code snippets are dynamically tested to see if the `rustc`
will accept them, rather than hard-coding specific version support.


## Usage

Add this to your `Cargo.toml`:

```toml
[build-dependencies]
autocfg = "1"
```

Then use it in your `build.rs` script to detect compiler features.  For
example, to test for 128-bit integer support, it might look like:

```rust
extern crate autocfg;

fn main() {
    let ac = autocfg::new();
    ac.emit_has_type("i128");

    // (optional) We don't need to rerun for anything external.
    autocfg::rerun_path("build.rs");
}
```

If the type test succeeds, this will write a `cargo:rustc-cfg=has_i128` line
for Cargo, which translates to Rust arguments `--cfg has_i128`.  Then in the
rest of your Rust code, you can add `#[cfg(has_i128)]` conditions on code that
should only be used when the compiler supports it.


## Release Notes

- 1.5.0 (2025-06-17)

  - Add `edition` and `set_edition` to control the Rust edition used in probes.
  - Remove probe result files so they don't pollute the output directory.

- 1.4.0 (2024-09-26)

  - Add `emit_possibility` for Rust 1.80's [checked cfgs], and call that
    automatically for methods that conditionally `emit`, by @Techcable.

[checked cfgs]: https://blog.rust-lang.org/2024/05/06/check-cfg.html

- 1.3.0 (2024-05-03)

  - Add `probe_raw` for direct control of the code that will be test-compiled.
  - Use wrappers when querying the `rustc` version information too.

- 1.2.0 (2024-03-25)

  - Add `no_std` and `set_no_std` to control the use of `#![no_std]` in probes.
  - Use `RUSTC_WRAPPER` and `RUSTC_WORKSPACE_WRAPPER` when they are set.

- 1.1.0 (2022-02-07)
  - Use `CARGO_ENCODED_RUSTFLAGS` when it is set.

- 1.0.1 (2020-08-20)
  - Apply `RUSTFLAGS` for more `--target` scenarios, by @adamreichold.

- 1.0.0 (2020-01-08)
  - 🎉 Release 1.0! 🎉 (no breaking changes)
  - Add `probe_expression` and `emit_expression_cfg` to test arbitrary expressions.
  - Add `probe_constant` and `emit_constant_cfg` to test arbitrary constant expressions.

- 0.1.7 (2019-10-20)
  - Apply `RUSTFLAGS` when probing `$TARGET != $HOST`, mainly for sysroot, by @roblabla.

- 0.1.6 (2019-08-19)
  - Add `probe`/`emit_sysroot_crate`, by @leo60228.

- 0.1.5 (2019-07-16)
  - Mask some warnings from newer rustc.

- 0.1.4 (2019-05-22)
  - Relax `std`/`no_std` probing to a warning instead of an error.
  - Improve `rustc` bootstrap compatibility.

- 0.1.3 (2019-05-21)
  - Auto-detects if `#![no_std]` is needed for the `$TARGET`.

- 0.1.2 (2019-01-16)
  - Add `rerun_env(ENV)` to print `cargo:rerun-if-env-changed=ENV`.
  - Add `rerun_path(PATH)` to print `cargo:rerun-if-changed=PATH`.


## Minimum Rust version policy

This crate's minimum supported `rustc` version is `1.0.0`.  Compatibility is
its entire reason for existence, so this crate will be extremely conservative
about raising this requirement.  If this is ever deemed necessary, it will be
treated as a major breaking change for semver purposes.


## License

This project is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   https://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   https://opensource.org/licenses/MIT)

at your option.
