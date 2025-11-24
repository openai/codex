![Logo](https://raw.githubusercontent.com/rustls/rustls/main/admin/rustls-logo-web.png)

**rustls-native-certs** allows [rustls](https://github.com/rustls/rustls) to use the
platform's native certificate store when operating as a TLS client.

> [!IMPORTANT]
> Instead of this crate, we suggest using [rustls-platform-verifier](https://github.com/rustls/rustls-platform-verifier),
> which provides a more robust solution with a simpler API. This crate is still maintained,
> but mostly for use inside the platform verifier on platforms where no other
> solution is available. For more context, see
> [deployment considerations](https://github.com/rustls/rustls-platform-verifier?tab=readme-ov-file#deployment-considerations).

# Status
rustls-native-certs is mature and widely used.

If you'd like to help out, please see [CONTRIBUTING.md](CONTRIBUTING.md).

[![rustls](https://github.com/rustls/rustls-native-certs/actions/workflows/rust.yml/badge.svg)](https://github.com/rustls/rustls-native-certs/actions/workflows/rust.yml)
[![Documentation](https://docs.rs/rustls-native-certs/badge.svg)](https://docs.rs/rustls-native-certs)

Release notes can be found [on GitHub](https://github.com/rustls/rustls-native-certs/releases).

# API

This library exposes a single function with this signature:

```rust
pub fn load_native_certs() -> Result<Vec<pki_types::CertificateDer<'static>>, std::io::Error>
```

On success, this returns a `Vec<pki_types::CertificateDer<'static>>` loaded with a
snapshot of the root certificates found on this platform.  This
function fails in a platform-specific way, expressed in a `std::io::Error`.

This function can be expensive: on some platforms it involves loading
and parsing a ~300KB disk file.  It's therefore prudent to call
this sparingly.

# Platform support

This is supported on Windows, macOS and Linux:

- On all platforms, the `SSL_CERT_FILE` environment variable is checked first.
  If that's set, certificates are loaded from the path specified by that variable,
  or an error is returned if certificates cannot be loaded from the given path.
  If it's not set, then the platform-specific certificate source is used.
- On Windows, certificates are loaded from the system certificate store.
  The [`schannel`](https://github.com/steffengy/schannel-rs) crate is used to access
  the Windows certificate store APIs.
- On macOS, certificates are loaded from the keychain.
  The user, admin and system trust settings are merged together as documented
  by Apple.  The [`security-framework`](https://github.com/kornelski/rust-security-framework)
  crate is used to access the keystore APIs.
- On Linux and other UNIX-like operating systems, the
  [`openssl-probe`](https://github.com/alexcrichton/openssl-probe) crate is used to discover
  the filename of the system CA bundle.

# Worked example

See [`examples/google.rs`](examples/google.rs).

# License

rustls-native-certs is distributed under the following three licenses:

- Apache License version 2.0.
- MIT license.
- ISC license.

These are included as LICENSE-APACHE, LICENSE-MIT and LICENSE-ISC
respectively.  You may use this software under the terms of any
of these licenses, at your option.
