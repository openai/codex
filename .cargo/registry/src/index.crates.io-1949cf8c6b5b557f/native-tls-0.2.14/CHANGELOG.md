# Change Log

## [Unreleased]

## [v0.2.12]

### Fixed

* Stopped using a deprecated openssl-probe API.

## [v0.2.11]

### Fixed

* Removed an unused build dependency.

## [v0.2.10]

### Fixed

* Fixed the build for iOS.

## [v0.2.9]

### Added

* Added `Identity::from_pkcs8`.

## [v0.2.8]

### Fixed

* Fixed an off by one error in the schannel backend's handling of max_protocol_version.

## [v0.2.7]

### Added

* Added support for ALPN in client APIs flagged under the `alpn` Cargo feature.

## [v0.2.6]

### Fixed

* Fixed compilation on iOS.

## [v0.2.5]

### Added

* Added `TlsConnectorBuilder::disable_built_in_roots` to only trust root certificates explicitly
    added to the builder.

### Updated

* Updated security-framework to 2.0.

## [v0.2.4]

### Added

* Added a `Clone` implementation for `Identity`.

### Updated

* Updated security-framework to 0.4.

## [v0.2.3]

### Fixed

* Adding an already-trusted certificate to the root certificate set no longer triggers an error
    with OpenSSL.

### Updated

* Updated security-framework to 0.3.

## [v0.2.2]

### Fixed

* Failure to load a root certificate on Android now logs a message rather than producing an error.
* Fixed ordering of the certificate chain in the OpenSSL backend.

## [v0.2.1]

### Added

* The `vendored` Cargo feature will cause the crate to compile and statically link to a vendored
    copy of OpenSSL on platforms that use that backend.

## [v0.2.0]

### Added

* The `openssl_probe` crate is now used with the OpenSSL backend so that trusted root certificates
    will automatically be detected when statically linking to OpenSSL.
* Root certificates are now automatically loaded from the Android trust root.
* Added `Certificate::to_der` to serialize an X509 certificate to DER.
* Added `TlsConnectorBuilder::danger_accept_invalid_certs` to disable certificate verification.
* Added `TlsAcceptor::new` and `TlsConnector::new` to easily create an acceptor/connector with
    default settings.
* Added `TlsStream::peer_certificate` to obtain the peer's leaf certificate.
* Added `TlsStream::tls_server_end_point` to retrieve RFC 5929 tls-server-end-point channel binding
    data.

### Changed

* Upgraded to `openssl` 0.10 and `security-framework` 0.2.
* `Pkcs12` has been renamed to `Identity`, and `Pkcs12::from_der` has been renamed to
    `Identity::from_pkcs12`.
* `HandshakeError::Interrupted` has been renamed to `HandshakeError::WouldBlock`.
* `TlsConnectorBuilder` and `TlsAcceptorBuilder` are now "traditional"-style builders. Their methods
    are now infallible and return `&mut Self` to allow them to be chained together.
* `supported_protocols` has been replaced by `min_protocol_version` and `max_protocol_version` on
    `TlsConnectorBuilder` and `TlsAcceptorBuilder`.
* SNI and hostname verification are now configured separately via `TlsConnectorBuilder::use_sni` and
    `TlsConnectorBuilder::danger_accept_invalid_hostnames`. They replace the
    `TlsConnector::danger_connect_without_providing_domain_for_certificate_verification_and_server_name_indication`
    method, which has been removed.

### Removed

* The backend-specific extension traits have been removed. We want to avoid exposing the specific
    version of the backend library in the public API to provide more flexibility.

## Older

Look at the [release tags] for information about older releases.

[Unreleased]: https://github.com/sfackler/rust-native-tls/compare/v0.2.11...master
[v0.2.11]: https://github.com/sfackler/rust-native-tls/compare/v0.2.10...v0.2.11
[v0.2.10]: https://github.com/sfackler/rust-native-tls/compare/v0.2.9...v0.2.10
[v0.2.9]: https://github.com/sfackler/rust-native-tls/compare/v0.2.8...v0.2.9
[v0.2.8]: https://github.com/sfackler/rust-native-tls/compare/v0.2.7...v0.2.8
[v0.2.7]: https://github.com/sfackler/rust-native-tls/compare/v0.2.6...v0.2.7
[v0.2.6]: https://github.com/sfackler/rust-native-tls/compare/v0.2.5...v0.2.6
[v0.2.5]: https://github.com/sfackler/rust-native-tls/compare/v0.2.4...v0.2.5
[v0.2.4]: https://github.com/sfackler/rust-native-tls/compare/v0.2.3...v0.2.4
[v0.2.3]: https://github.com/sfackler/rust-native-tls/compare/v0.2.2...v0.2.3
[v0.2.2]: https://github.com/sfackler/rust-native-tls/compare/v0.2.1...v0.2.2
[v0.2.1]: https://github.com/sfackler/rust-native-tls/compare/v0.2.0...v0.2.1
[v0.2.0]: https://github.com/sfackler/rust-native-tls/compare/v0.1.5...v0.2.0
[release tags]: https://github.com/sfackler/rust-native-tls/releases
