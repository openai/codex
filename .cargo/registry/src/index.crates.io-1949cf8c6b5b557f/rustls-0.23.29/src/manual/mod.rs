/*!

This documentation primarily aims to explain design decisions taken in rustls.

It does this from a few aspects: how rustls attempts to avoid construction errors
that occurred in other TLS libraries, how rustls attempts to avoid past TLS
protocol vulnerabilities, and assorted advice for achieving common tasks with rustls.
*/
#![allow(non_snake_case)]

/// This section discusses vulnerabilities in other TLS implementations, theorising their
/// root cause and how we aim to avoid them in rustls.
#[path = "implvulns.rs"]
pub mod _01_impl_vulnerabilities;

/// This section discusses vulnerabilities and design errors in the TLS protocol.
#[path = "tlsvulns.rs"]
pub mod _02_tls_vulnerabilities;

/// This section collects together goal-oriented documentation.
#[path = "howto.rs"]
pub mod _03_howto;

/// This section documents rustls itself: what protocol features are and are not implemented.
#[path = "features.rs"]
pub mod _04_features;

/// This section provides rationale for the defaults in rustls.
#[path = "defaults.rs"]
pub mod _05_defaults;

/// This section provides guidance on using rustls with FIPS-approved cryptography.
#[path = "fips.rs"]
pub mod _06_fips;
