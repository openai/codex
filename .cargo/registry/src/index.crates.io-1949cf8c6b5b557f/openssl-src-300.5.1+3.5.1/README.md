# openssl-src

This crate contains the logic to build OpenSSL and is intended to be consumed by
the `openssl-sys` crate. You likely in theory aren't interacting with this too
much!

## Versioning

This crate follows the latest minor and patch versions for each maintained major version,
according to the [OpenSSL release strategy](https://www.openssl.org/policies/releasestrat.html).
It has no specific support for LTS versions.

The crate versions follow the `X.Y.Z+B` pattern:
* The major version `X` is the upstream OpenSSL API/ABI compatibility version:
  * `300` for 3.Y.Z
* The minor `Y` and patch `Z` versions are incremented when making changes
  to the crate, either OpenSSL update or internal changes.
* `B` contains the full upstream OpenSSL version, like `1.1.1k` or `3.0.7`.
  Note that this field is actually ignored in comparisons and only there for
  documentation.

## Windows MSVC Assembly

Building OpenSSL for `windows-msvc` targets, users can choose whether to enable
assembly language routines, which requires [nasm](https://www.nasm.us/).  
The build process will automatically detect whether `nasm.exe` is installed in
PATH. If found, the assembly language routines will be enabled (in other words,
the `no-asm` option will NOT be configured).

You can manipulate this behavior by setting the `OPENSSL_RUST_USE_NASM` environment
variable:
* `1`: Force enable the assembly language routines. (panic if `nasm.exe` is not
available.)
* `0`: Force disable the assembly language routines even if the `nasm.exe` can be
found in PATH.
* not set: Let the build process automatically detect whether `nasm.exe` is
installed. If found, enable. If not, disable.

However, this environment variable does NOT take effects on non-windows platforms.

## License

This project is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in openssl-src by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
