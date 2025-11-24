# Rust bindings to *nix APIs

[![Cirrus Build Status](https://api.cirrus-ci.com/github/nix-rust/nix.svg)](https://cirrus-ci.com/github/nix-rust/nix)
[![crates.io](https://img.shields.io/crates/v/nix.svg)](https://crates.io/crates/nix)
[![maintenance-status](https://img.shields.io/badge/maintenance-looking--for--maintainer-orange.svg)](https://github.com/nix-rust/nix/issues/2132)

[Documentation (Releases)](https://docs.rs/nix/)

Nix seeks to provide friendly bindings to various *nix platform APIs (Linux, Darwin,
...). The goal is to not provide a 100% unified interface, but to unify
what can be while still providing platform specific APIs.

For many system APIs, Nix provides a safe alternative to the unsafe APIs
exposed by the [libc crate](https://github.com/rust-lang/libc).  This is done by
wrapping the libc functionality with types/abstractions that enforce legal/safe
usage.


As an example of what Nix provides, examine the differences between what is
exposed by libc and nix for the
[gethostname](https://man7.org/linux/man-pages/man2/gethostname.2.html) system
call:

```rust,ignore
// libc api (unsafe, requires handling return code/errno)
pub unsafe extern fn gethostname(name: *mut c_char, len: size_t) -> c_int;

// nix api (returns a nix::Result<OsString>)
pub fn gethostname() -> Result<OsString>;
```

## Supported Platforms

nix target support consists of three tiers. While nix attempts to support all
platforms supported by [libc](https://github.com/rust-lang/libc), only some
platforms are actively supported due to either technical or manpower
limitations. Support for platforms is split into three tiers:

  * Tier 1 - Builds and tests for this target are run in CI. Failures of either
             block the inclusion of new code.
  * Tier 2 - Builds for this target are run in CI. Failures during the build
             blocks the inclusion of new code. Tests may be run, but failures
             in tests don't block the inclusion of new code.
  * Tier 3 - Builds for this target are run in CI. Failures during the build
             *do not* necessarily block the inclusion of new code.  That is, at
             our discretion a Tier 3 target may be dropped at any time, if it
             would otherwise block development.

Platforms not listed are supported on a best-effort basis, relying on our users
to report any problems.

The following targets are supported by `nix`:

<table>
 <tr>
  <th>Tier 1</th>
  <th>Tier 2</th>
  <th>Tier 3</th>
 </tr>
 <tr>
  <td>
   <ul>
    <li>aarch64-apple-darwin</li>
    <li>aarch64-unknown-linux-gnu</li>
    <li>arm-unknown-linux-gnueabi</li>
    <li>armv7-unknown-linux-gnueabihf</li>
    <li>i686-unknown-freebsd</li>
    <li>i686-unknown-linux-gnu</li>
    <li>i686-unknown-linux-musl</li>
    <li>mips-unknown-linux-gnu</li>
    <li>mips64-unknown-linux-gnuabi64</li>
    <li>mips64el-unknown-linux-gnuabi64</li>
    <li>mipsel-unknown-linux-gnu</li>
    <li>powerpc64le-unknown-linux-gnu</li>
    <li>x86_64-unknown-freebsd</li>
    <li>x86_64-unknown-linux-gnu</li>
    <li>x86_64-unknown-linux-musl</li>
   </ul>
  </td>
  <td>
   <ul>
    <li>aarch64-apple-ios</li>
    <li>aarch64-linux-android</li>
    <li>arm-linux-androideabi</li>
    <li>arm-unknown-linux-musleabi</li>
    <li>armv7-linux-androideabi</li>
    <li>i686-linux-android</li>
    <li>s390x-unknown-linux-gnu</li>
    <li>x86_64-linux-android</li>
    <li>x86_64-unknown-illumos</li>
    <li>x86_64-unknown-netbsd</li>
   </td>
   <td>
    <li>armv7-unknown-linux-uclibceabihf</li>
    <li>powerpc64-unknown-linux-gnu</li>
    <li>x86_64-fuchsia</li>
    <li>x86_64-unknown-dragonfly</li>
    <li>x86_64-unknown-haiku</li>
    <li>x86_64-unknown-linux-gnux32</li>
    <li>x86_64-unknown-openbsd</li>
    <li>x86_64-unknown-redox</li>
    <li>i686-unknown-hurd-gnu</li>
   </td>
  </tr>
</table>

## Minimum Supported Rust Version (MSRV)

nix is supported on Rust 1.69 and higher.  Its MSRV will not be
changed in the future without bumping the major or minor version.

## Contributing

Contributions are very welcome.  Please See [CONTRIBUTING](CONTRIBUTING.md) for
additional details.

Feel free to join us in [the nix-rust/nix](https://gitter.im/nix-rust/nix) channel on Gitter to
discuss `nix` development.

## License

Nix is licensed under the MIT license.  See [LICENSE](LICENSE) for more details.
