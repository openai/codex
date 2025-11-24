# X11 rust bindings

[![GitHub Actions Status](https://github.com/psychon/x11rb/workflows/CI/badge.svg)](https://github.com/psychon/x11rb/actions)
[![Crate](https://img.shields.io/crates/v/x11rb.svg)](https://crates.io/crates/x11rb)
[![API](https://docs.rs/x11rb/badge.svg)](https://docs.rs/x11rb)
![Minimum rustc version](https://img.shields.io/badge/rustc-1.63+-lightgray.svg)
[![License](https://img.shields.io/crates/l/x11rb.svg)](https://github.com/psychon/x11rb#license)

Feel free to open issues for any problems or questions you might have.
A comparison with some other Rust X11 libraries is available in an [extra
document](doc/comparison.md).


## Building

This crate uses a code generator that is implemented in Rust. A copy of the
generated code is included, so you do not need to run the generator unless
you have modified the definitions or the generator itself.

The code generator uses the X11 XML description from `xcb-proto`. A copy of
xcb-proto that comes with the source code is used.

The interaction with libxcb via `XCBConnection` requires at least libxcb 1.12.


## Crate features

Most X11 extensions are feature-gated. For example, to use the shared memory
extension, the `shm` feature has to be enabled.

The `all-extensions` feature just enables all X11 extensions.

Additionally, the `allow-unsafe-code` feature enables `XCBConnection`. This uses
`libxcb` internally and allows sharing the underlying `xcb_connection_t` pointer
with other code.

The `cursor` feature enables X11 cursor support via the `cursor` module. This
module helps with loading cursors from the current cursor theme.


## Current state

The full X11 protocol is supported by this library. All extensions that are
available in `xcb-proto` can be used and even [FD
passing](x11rb/examples/shared_memory.rs) with the server is supported.

The changelog is available in a [separate file](doc/changelog.md).


## License

Licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

The subdirectory xcb-proto-1.17.0 contains a vendored copy of the
package of the same name. It is covered by the MIT license. See
[xcb-proto-1.17.0/COPYING](xcb-proto-1.17.0/COPYING) for details.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
