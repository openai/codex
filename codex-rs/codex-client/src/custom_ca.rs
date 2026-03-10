//! Custom CA handling for Codex reqwest clients.
//!
//! Codex constructs outbound reqwest clients in a few crates, but they all need the same trust-
//! store policy when enterprise proxies or gateways intercept TLS. This module centralizes that
//! policy so callers can start from an ordinary `reqwest::ClientBuilder`, layer in custom CA
//! support, and either get back a configured client or a user-facing error that explains how to
//! fix a misconfigured CA bundle.
//!
//! The module intentionally has a narrow responsibility:
//!
//! - read CA material from `CODEX_CA_CERTIFICATE`, falling back to `SSL_CERT_FILE`
//! - normalize PEM variants that show up in real deployments, including OpenSSL-style
//!   `TRUSTED CERTIFICATE` labels and bundles that also contain CRLs
//! - return user-facing errors that explain how to fix misconfigured CA files
//!
//! It does not validate certificate chains or perform a handshake in tests. Its contract is
//! narrower: produce a `reqwest::Client` whose root store contains every parseable certificate
//! block from the configured PEM bundle, or fail early with a precise error before the caller
//! starts network traffic.
//!
//! In this module's test setup, a hermetic test is one whose result depends only on the CA file
//! and environment variables that the test chose for itself. That matters here because the normal
//! reqwest client-construction path is not hermetic enough for environment-sensitive tests:
//!
//! - on macOS seatbelt runs, `reqwest::Client::builder().build()` can panic inside
//!   `system-configuration` while probing platform proxy settings, which means the process can die
//!   before the custom-CA code reports success or a structured error. That matters in practice
//!   because Codex itself commonly runs spawned test processes under seatbelt, so this is not just
//!   a hypothetical CI edge case.
//! - child processes inherit CA-related environment variables by default, which lets developer
//!   shell state or CI configuration affect a test unless the test scrubs those variables first
//!
//! The tests in this crate therefore stay split across two layers:
//!
//! - unit tests in this module cover env-selection logic without constructing a real client
//! - subprocess integration tests under `tests/` cover real client construction through
//!   [`build_reqwest_client_for_subprocess_tests`], which disables reqwest proxy autodetection so
//!   the tests can observe custom-CA success and failure directly
//! - those subprocess tests also scrub inherited CA environment variables before launch so their
//!   result depends only on the test fixtures and env vars set by the test itself

use std::env;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use rustls_pki_types::CertificateDer;
use rustls_pki_types::pem::PemObject;
use rustls_pki_types::pem::SectionKind;
use rustls_pki_types::pem::{self};
use thiserror::Error;
use tracing::info;
use tracing::warn;

pub const CODEX_CA_CERT_ENV: &str = "CODEX_CA_CERTIFICATE";
pub const SSL_CERT_FILE_ENV: &str = "SSL_CERT_FILE";
const CA_CERT_HINT: &str = "If you set CODEX_CA_CERTIFICATE or SSL_CERT_FILE, ensure it points to a PEM file containing one or more CERTIFICATE blocks, or unset it to use system roots.";
type PemSection = (SectionKind, Vec<u8>);

/// Describes why a reqwest client with custom CA support could not be constructed.
///
/// This boundary is more specific than `io::Error`: a client can fail because the configured CA
/// file could not be read, could not be parsed as certificates, contained certs that `reqwest`
/// refused to register, or because the final client builder failed. Callers that do not care
/// about the distinction can rely on the `From<BuildReqwestClientError> for io::Error`
/// conversion.
#[derive(Debug, Error)]
pub enum BuildReqwestClientError {
    /// Reading the selected CA file from disk failed before any PEM parsing could happen.
    #[error(
        "Failed to read CA certificate file {} selected by {}: {source}. {hint}",
        path.display(),
        source_env,
        hint = CA_CERT_HINT
    )]
    ReadCaFile {
        source_env: &'static str,
        path: PathBuf,
        source: io::Error,
    },

    /// The selected CA file was readable, but did not produce usable certificate material.
    #[error(
        "Failed to load CA certificates from {} selected by {}: {detail}. {hint}",
        path.display(),
        source_env,
        hint = CA_CERT_HINT
    )]
    InvalidCaFile {
        source_env: &'static str,
        path: PathBuf,
        detail: String,
    },

    /// One parsed certificate block could not be registered with the reqwest client builder.
    #[error(
        "Failed to parse certificate #{certificate_index} from {} selected by {}: {source}. {hint}",
        path.display(),
        source_env,
        hint = CA_CERT_HINT
    )]
    RegisterCertificate {
        source_env: &'static str,
        path: PathBuf,
        certificate_index: usize,
        source: reqwest::Error,
    },

    /// Reqwest rejected the final client configuration after a custom CA bundle was loaded.
    #[error(
        "Failed to build HTTP client while using CA bundle from {} ({}): {source}",
        source_env,
        path.display()
    )]
    BuildClientWithCustomCa {
        source_env: &'static str,
        path: PathBuf,
        #[source]
        source: reqwest::Error,
    },

    /// Reqwest rejected the final client configuration while using only system roots.
    #[error("Failed to build HTTP client while using system root certificates: {0}")]
    BuildClientWithSystemRoots(#[source] reqwest::Error),
}

impl From<BuildReqwestClientError> for io::Error {
    fn from(error: BuildReqwestClientError) -> Self {
        match error {
            BuildReqwestClientError::ReadCaFile { ref source, .. } => {
                io::Error::new(source.kind(), error)
            }
            BuildReqwestClientError::InvalidCaFile { .. }
            | BuildReqwestClientError::RegisterCertificate { .. } => {
                io::Error::new(io::ErrorKind::InvalidData, error)
            }
            BuildReqwestClientError::BuildClientWithCustomCa { .. }
            | BuildReqwestClientError::BuildClientWithSystemRoots(_) => io::Error::other(error),
        }
    }
}

/// Builds a reqwest client that honors Codex custom CA environment variables.
///
/// Callers supply the baseline builder configuration they need, and this helper layers in custom
/// CA handling before finally constructing the client. `CODEX_CA_CERTIFICATE` takes precedence
/// over `SSL_CERT_FILE`, and empty values for either are treated as unset so callers do not
/// accidentally turn `VAR=""` into a bogus path lookup.
///
/// Callers that build a raw `reqwest::Client` directly bypass this policy entirely. That is an
/// easy mistake to make when adding a new outbound Codex HTTP path, and the resulting bug only
/// shows up in environments where a proxy or gateway requires a custom root CA.
///
/// # Errors
///
/// Returns a [`BuildReqwestClientError`] when the configured CA file is unreadable, malformed, or
/// contains a certificate block that `reqwest` cannot register as a root.
pub fn build_reqwest_client_with_custom_ca(
    builder: reqwest::ClientBuilder,
) -> Result<reqwest::Client, BuildReqwestClientError> {
    build_reqwest_client_with_env(&ProcessEnv, builder)
}

/// Builds a reqwest client for spawned subprocess tests that exercise CA behavior.
///
/// This is the test-only client-construction path used by the subprocess coverage in `tests/`.
/// The module-level docs explain the hermeticity problem in full; this helper only addresses the
/// reqwest proxy-discovery panic side of that problem by disabling proxy autodetection. The tests
/// still scrub inherited CA environment variables themselves. Normal production callers should use
/// [`build_reqwest_client_with_custom_ca`] so test-only proxy behavior does not leak into
/// ordinary client construction.
pub fn build_reqwest_client_for_subprocess_tests(
    builder: reqwest::ClientBuilder,
) -> Result<reqwest::Client, BuildReqwestClientError> {
    build_reqwest_client_with_env(&ProcessEnv, builder.no_proxy())
}

/// Builds a reqwest client using an injected environment source and reqwest builder.
///
/// This exists so tests can exercise precedence behavior deterministically without mutating the
/// real process environment. It selects the CA bundle, delegates file parsing to
/// [`ConfiguredCaBundle::load_certificates`], preserves the caller's chosen `reqwest` builder
/// configuration, and finally registers each parsed certificate with that builder.
fn build_reqwest_client_with_env(
    env_source: &dyn EnvSource,
    mut builder: reqwest::ClientBuilder,
) -> Result<reqwest::Client, BuildReqwestClientError> {
    if let Some(bundle) = env_source.configured_ca_bundle() {
        let certificates = bundle.load_certificates()?;

        for (idx, cert) in certificates.iter().enumerate() {
            let certificate = match reqwest::Certificate::from_der(cert.as_ref()) {
                Ok(certificate) => certificate,
                Err(source) => {
                    warn!(
                        source_env = bundle.source_env,
                        ca_path = %bundle.path.display(),
                        certificate_index = idx + 1,
                        error = %source,
                        "failed to register CA certificate"
                    );
                    return Err(BuildReqwestClientError::RegisterCertificate {
                        source_env: bundle.source_env,
                        path: bundle.path.clone(),
                        certificate_index: idx + 1,
                        source,
                    });
                }
            };
            builder = builder.add_root_certificate(certificate);
        }
        return match builder.build() {
            Ok(client) => Ok(client),
            Err(source) => {
                warn!(
                    source_env = bundle.source_env,
                    ca_path = %bundle.path.display(),
                    error = %source,
                    "failed to build client after loading custom CA bundle"
                );
                Err(BuildReqwestClientError::BuildClientWithCustomCa {
                    source_env: bundle.source_env,
                    path: bundle.path.clone(),
                    source,
                })
            }
        };
    }

    info!(
        codex_ca_certificate_configured = false,
        ssl_cert_file_configured = false,
        "using system root certificates because no CA override environment variable was selected"
    );

    match builder.build() {
        Ok(client) => Ok(client),
        Err(source) => {
            warn!(
                error = %source,
                "failed to build client while using system root certificates"
            );
            Err(BuildReqwestClientError::BuildClientWithSystemRoots(source))
        }
    }
}

/// Abstracts environment access so tests can cover precedence rules without mutating process-wide
/// variables.
trait EnvSource {
    /// Returns the environment variable value for `key`, if this source considers it set.
    ///
    /// Implementations should return `None` for absent values and may also collapse unreadable
    /// process-environment states into `None`, because the custom CA logic treats both cases as
    /// "no override configured". Callers build precedence and empty-string handling on top of this
    /// method, so implementations should not trim or normalize the returned string.
    fn var(&self, key: &str) -> Option<String>;

    /// Returns a non-empty environment variable value interpreted as a filesystem path.
    ///
    /// Empty strings are treated as unset because presence here acts as a boolean "custom CA
    /// override requested" signal. This keeps the precedence logic from treating `VAR=""` as an
    /// attempt to open the current working directory or some other platform-specific oddity once
    /// it is converted into a path.
    fn non_empty_path(&self, key: &str) -> Option<PathBuf> {
        self.var(key)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    }

    /// Returns the configured CA bundle and which environment variable selected it.
    ///
    /// `CODEX_CA_CERTIFICATE` wins over `SSL_CERT_FILE` because it is the Codex-specific override.
    /// Keeping the winning variable name with the path lets later logging explain not only which
    /// file was used but also why that file was chosen.
    fn configured_ca_bundle(&self) -> Option<ConfiguredCaBundle> {
        self.non_empty_path(CODEX_CA_CERT_ENV)
            .map(|path| ConfiguredCaBundle {
                source_env: CODEX_CA_CERT_ENV,
                path,
            })
            .or_else(|| {
                self.non_empty_path(SSL_CERT_FILE_ENV)
                    .map(|path| ConfiguredCaBundle {
                        source_env: SSL_CERT_FILE_ENV,
                        path,
                    })
            })
    }
}

/// Reads CA configuration from the real process environment.
///
/// This is the production `EnvSource` implementation used by
/// [`build_reqwest_client_with_custom_ca`]. Tests substitute in-memory env maps so they can
/// exercise precedence and empty-value behavior without mutating process-global variables.
struct ProcessEnv;

impl EnvSource for ProcessEnv {
    fn var(&self, key: &str) -> Option<String> {
        env::var(key).ok()
    }
}

/// Identifies the CA bundle selected for a client and the policy decision that selected it.
///
/// This is the concrete output of the environment-precedence logic. Callers use `source_env` for
/// logging and diagnostics, while `path` is the bundle that will actually be loaded.
struct ConfiguredCaBundle {
    /// The environment variable that won the precedence check for this bundle.
    source_env: &'static str,
    /// The filesystem path that should be read as PEM certificate input.
    path: PathBuf,
}

impl ConfiguredCaBundle {
    /// Loads certificates from this selected CA bundle.
    ///
    /// The bundle already represents the output of environment-precedence selection, so this is
    /// the natural point where the file-loading phase begins. The method owns the high-level
    /// success/failure logs for that phase and keeps the source env and path together for lower-
    /// level parsing and error shaping.
    fn load_certificates(&self) -> Result<Vec<CertificateDer<'static>>, BuildReqwestClientError> {
        match self.parse_certificates() {
            Ok(certificates) => {
                info!(
                    source_env = self.source_env,
                    ca_path = %self.path.display(),
                    certificate_count = certificates.len(),
                    "loaded certificates from custom CA bundle"
                );
                Ok(certificates)
            }
            Err(error) => {
                warn!(
                    source_env = self.source_env,
                    ca_path = %self.path.display(),
                    error = %error,
                    "failed to load custom CA bundle"
                );
                Err(error)
            }
        }
    }

    /// Loads every certificate block from a PEM file intended for Codex CA overrides.
    ///
    /// This accepts a few common real-world variants so Codex behaves like other CA-aware tooling:
    /// leading comments are preserved, `TRUSTED CERTIFICATE` labels are normalized to standard
    /// certificate labels, and embedded CRLs are ignored when they are well-formed enough for the
    /// section iterator to classify them.
    fn parse_certificates(&self) -> Result<Vec<CertificateDer<'static>>, BuildReqwestClientError> {
        let pem_data = self.read_pem_data()?;
        let normalized_pem = NormalizedPem::from_pem_data(self.source_env, &self.path, &pem_data);

        let mut certificates = Vec::new();
        let mut logged_crl_presence = false;
        for section_result in normalized_pem.sections() {
            // Known limitation: if `rustls-pki-types` fails while parsing a malformed CRL section,
            // that error is reported here before we can classify the block as ignorable. A bundle
            // containing valid certificates plus a malformed `X509 CRL` therefore still fails to
            // load today, even though well-formed CRLs are ignored.
            let (section_kind, der) = match section_result {
                Ok(section) => section,
                Err(error) => return Err(self.pem_parse_error(&error)),
            };
            match section_kind {
                SectionKind::Certificate => {
                    // Standard CERTIFICATE blocks already decode to the exact DER bytes reqwest
                    // wants. Only OpenSSL TRUSTED CERTIFICATE blocks need trimming to drop any
                    // trailing X509_AUX trust metadata before registration.
                    let cert_der = normalized_pem.certificate_der(&der).ok_or_else(|| {
                        self.invalid_ca_file(
                            "failed to extract certificate data from TRUSTED CERTIFICATE: invalid DER length",
                        )
                    })?;
                    certificates.push(CertificateDer::from(cert_der.to_vec()));
                }
                SectionKind::Crl => {
                    if !logged_crl_presence {
                        info!(
                            source_env = self.source_env,
                            ca_path = %self.path.display(),
                            "ignoring X509 CRL entries found in custom CA bundle"
                        );
                        logged_crl_presence = true;
                    }
                }
                _ => {}
            }
        }

        if certificates.is_empty() {
            return Err(self.pem_parse_error(&pem::Error::NoItemsFound));
        }

        Ok(certificates)
    }

    /// Reads the CA bundle bytes while preserving the original filesystem error kind.
    ///
    /// The caller wants a user-facing error that includes the bundle path and remediation hint, but
    /// higher-level surfaces still benefit from distinguishing "not found" from other I/O
    /// failures. This helper keeps both pieces together.
    fn read_pem_data(&self) -> Result<Vec<u8>, BuildReqwestClientError> {
        fs::read(&self.path).map_err(|source| BuildReqwestClientError::ReadCaFile {
            source_env: self.source_env,
            path: self.path.clone(),
            source,
        })
    }

    /// Rewrites PEM parsing failures into user-facing configuration errors.
    ///
    /// The underlying parser knows whether the file was empty, malformed, or contained unsupported
    /// PEM content, but callers need a message that also points them back to the relevant
    /// environment variables and the expected remediation.
    fn pem_parse_error(&self, error: &pem::Error) -> BuildReqwestClientError {
        let detail = match error {
            pem::Error::NoItemsFound => "no certificates found in PEM file".to_string(),
            _ => format!("failed to parse PEM file: {error}"),
        };

        self.invalid_ca_file(detail)
    }

    /// Creates an invalid-CA error tied to this file path.
    ///
    /// Most parse-time failures in this module eventually collapse to "the configured CA bundle is
    /// not usable", but the detailed reason still matters for operator debugging. Centralizing that
    /// formatting keeps the path and hint text consistent across the different parser branches.
    fn invalid_ca_file(&self, detail: impl std::fmt::Display) -> BuildReqwestClientError {
        BuildReqwestClientError::InvalidCaFile {
            source_env: self.source_env,
            path: self.path.clone(),
            detail: detail.to_string(),
        }
    }
}

/// The PEM text shape after OpenSSL compatibility normalization.
///
/// `Standard` means the input already used ordinary PEM certificate labels. `TrustedCertificate`
/// means the input used OpenSSL's `TRUSTED CERTIFICATE` labels, so callers must also be prepared
/// to trim trailing `X509_AUX` bytes from decoded certificate sections.
enum NormalizedPem {
    /// PEM contents that already used ordinary `CERTIFICATE` labels.
    Standard(String),
    /// PEM contents rewritten from OpenSSL `TRUSTED CERTIFICATE` labels to `CERTIFICATE`.
    TrustedCertificate(String),
}

impl NormalizedPem {
    /// Normalizes PEM text from a CA bundle into the label shape this module expects.
    ///
    /// Codex only needs certificate DER bytes to seed `reqwest`'s root store, but operators may
    /// point it at CA files that came from OpenSSL tooling rather than from a minimal certificate
    /// bundle. OpenSSL's `TRUSTED CERTIFICATE` form is one such variant: it is still certificate
    /// material, but it uses a different PEM label and may carry auxiliary trust metadata that
    /// this crate does not consume. This constructor rewrites only the PEM labels so the mixed-
    /// section parser can keep treating the file as certificate input. The rustls ecosystem does
    /// not currently accept `TRUSTED CERTIFICATE` as a standard certificate label upstream, so
    /// this remains a local compatibility shim rather than behavior delegated to
    /// `rustls-pki-types`.
    ///
    /// See also:
    /// - rustls/pemfile issue #52, closed as not planned, documenting that
    ///   `BEGIN TRUSTED CERTIFICATE` blocks are ignored upstream:
    ///   <https://github.com/rustls/pemfile/issues/52>
    /// - OpenSSL `x509 -trustout`, which emits `TRUSTED CERTIFICATE` PEM blocks:
    ///   <https://docs.openssl.org/master/man1/openssl-x509/>
    /// - OpenSSL PEM readers, which document that plain `PEM_read_bio_X509()` discards auxiliary
    ///   trust settings:
    ///   <https://docs.openssl.org/master/man3/PEM_read_bio_PrivateKey/>
    /// - `openssl s_server`, a real OpenSSL-based server/test tool that operates in this
    ///   ecosystem:
    ///   <https://docs.openssl.org/master/man1/openssl-s_server/>
    fn from_pem_data(source_env: &'static str, path: &Path, pem_data: &[u8]) -> Self {
        let pem = String::from_utf8_lossy(pem_data);
        if pem.contains("TRUSTED CERTIFICATE") {
            info!(
                source_env,
                ca_path = %path.display(),
                "normalizing OpenSSL TRUSTED CERTIFICATE labels in custom CA bundle"
            );
            Self::TrustedCertificate(
                pem.replace("BEGIN TRUSTED CERTIFICATE", "BEGIN CERTIFICATE")
                    .replace("END TRUSTED CERTIFICATE", "END CERTIFICATE"),
            )
        } else {
            Self::Standard(pem.into_owned())
        }
    }

    /// Returns the normalized PEM contents regardless of the label shape that produced them.
    fn contents(&self) -> &str {
        match self {
            Self::Standard(contents) | Self::TrustedCertificate(contents) => contents,
        }
    }

    /// Iterates over every recognized PEM section in this normalized PEM text.
    ///
    /// `rustls-pki-types` exposes mixed-section parsing through a `PemObject` implementation on the
    /// `(SectionKind, Vec<u8>)` tuple. Keeping that type-directed API here lets callers iterate in
    /// terms of normalized sections rather than trait plumbing.
    fn sections(&self) -> impl Iterator<Item = Result<PemSection, pem::Error>> + '_ {
        PemSection::pem_slice_iter(self.contents().as_bytes())
    }

    /// Returns the certificate DER bytes for one parsed PEM certificate section.
    ///
    /// Standard PEM certificates already decode to the exact DER bytes `reqwest` wants. OpenSSL
    /// `TRUSTED CERTIFICATE` sections may append `X509_AUX` bytes after the certificate, so those
    /// sections need to be trimmed down to their first DER object before registration.
    fn certificate_der<'a>(&self, der: &'a [u8]) -> Option<&'a [u8]> {
        match self {
            Self::Standard(_) => Some(der),
            Self::TrustedCertificate(_) => first_der_item(der),
        }
    }
}

/// Returns the first DER-encoded ASN.1 object in `der`, ignoring any trailing OpenSSL metadata.
///
/// A PEM `CERTIFICATE` block usually decodes to exactly one DER blob: the certificate itself.
/// OpenSSL's `TRUSTED CERTIFICATE` variant is different. It starts with that same certificate
/// blob, but may append extra `X509_AUX` bytes after it to describe OpenSSL-specific trust
/// settings. `reqwest::Certificate::from_der` only understands the certificate object, not those
/// trailing OpenSSL extensions.
///
/// This helper therefore asks a narrower question than "is this a valid certificate?": where does
/// the first top-level DER object end? If that boundary can be found, the caller keeps only that
/// prefix and discards the trailing trust metadata. If it cannot be found, the input is treated as
/// malformed CA data.
fn first_der_item(der: &[u8]) -> Option<&[u8]> {
    der_item_length(der).map(|length| &der[..length])
}

/// Returns the byte length of the first DER item in `der`.
///
/// DER is a binary encoding for ASN.1 objects. Each object begins with:
///
/// - a tag byte describing what kind of object follows
/// - one or more length bytes describing how many content bytes belong to that object
/// - the content bytes themselves
///
/// For this module, the important fact is that a certificate is stored as one complete top-level
/// DER object. Once we know that object's declared length, we know exactly where the certificate
/// ends and where any trailing OpenSSL `X509_AUX` data begins.
///
/// This helper intentionally parses only that outer length field. It does not validate the inner
/// certificate structure, the meaning of the tag, or every nested ASN.1 value. That narrower scope
/// is deliberate: the caller only needs a safe slice boundary for the leading certificate object
/// before handing those bytes to `reqwest`, which performs the real certificate parsing.
///
/// The implementation supports the DER length forms needed here:
///
/// - short form, where the length is stored directly in the second byte
/// - long form, where the second byte says how many following bytes make up the length value
///
/// Indefinite lengths are rejected because DER does not permit them, and any declared length that
/// would run past the end of the input is treated as malformed.
fn der_item_length(der: &[u8]) -> Option<usize> {
    let &length_octet = der.get(1)?;
    if length_octet & 0x80 == 0 {
        return Some(2 + usize::from(length_octet)).filter(|length| *length <= der.len());
    }

    let length_octets = usize::from(length_octet & 0x7f);
    if length_octets == 0 {
        return None;
    }

    let length_start = 2usize;
    let length_end = length_start.checked_add(length_octets)?;
    let length_bytes = der.get(length_start..length_end)?;
    let mut content_length = 0usize;
    for &byte in length_bytes {
        content_length = content_length
            .checked_mul(256)?
            .checked_add(usize::from(byte))?;
    }

    length_end
        .checked_add(content_length)
        .filter(|length| *length <= der.len())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use pretty_assertions::assert_eq;

    use super::CODEX_CA_CERT_ENV;
    use super::EnvSource;
    use super::SSL_CERT_FILE_ENV;

    struct MapEnv {
        values: HashMap<String, String>,
    }

    impl EnvSource for MapEnv {
        fn var(&self, key: &str) -> Option<String> {
            self.values.get(key).cloned()
        }
    }

    fn map_env(pairs: &[(&str, &str)]) -> MapEnv {
        MapEnv {
            values: pairs
                .iter()
                .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
                .collect(),
        }
    }

    #[test]
    fn ca_path_prefers_codex_env() {
        let env = map_env(&[
            (CODEX_CA_CERT_ENV, "/tmp/codex.pem"),
            (SSL_CERT_FILE_ENV, "/tmp/fallback.pem"),
        ]);

        assert_eq!(
            env.configured_ca_bundle().map(|bundle| bundle.path),
            Some(PathBuf::from("/tmp/codex.pem"))
        );
    }

    #[test]
    fn ca_path_falls_back_to_ssl_cert_file() {
        let env = map_env(&[(SSL_CERT_FILE_ENV, "/tmp/fallback.pem")]);

        assert_eq!(
            env.configured_ca_bundle().map(|bundle| bundle.path),
            Some(PathBuf::from("/tmp/fallback.pem"))
        );
    }

    #[test]
    fn ca_path_ignores_empty_values() {
        let env = map_env(&[
            (CODEX_CA_CERT_ENV, ""),
            (SSL_CERT_FILE_ENV, "/tmp/fallback.pem"),
        ]);

        assert_eq!(
            env.configured_ca_bundle().map(|bundle| bundle.path),
            Some(PathBuf::from("/tmp/fallback.pem"))
        );
    }
}
