//! Custom CA handling for login HTTP clients.
//!
//! Login flows are the only place this crate constructs ad hoc outbound HTTP clients, so this
//! module centralizes the trust-store behavior that those clients must share. Enterprise networks
//! often terminate TLS with an internal root CA, which means system roots alone cannot validate
//! the OAuth and device-code endpoints that the login flows call.
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
//! starts a login flow.
//!
//! The tests in this module therefore split on that boundary:
//!
//! - unit tests cover pure env-selection logic without constructing a real client
//! - subprocess tests in `tests/ca_env.rs` cover real client construction, because that path is
//!   not hermetic in macOS sandboxed runs and must also scrub inherited CA environment variables

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

const CODEX_CA_CERT_ENV: &str = "CODEX_CA_CERTIFICATE";
const SSL_CERT_FILE_ENV: &str = "SSL_CERT_FILE";
const CA_CERT_HINT: &str = "If you set CODEX_CA_CERTIFICATE or SSL_CERT_FILE, ensure it points to a PEM file containing one or more CERTIFICATE blocks, or unset it to use system roots.";
type PemSection = (SectionKind, Vec<u8>);

/// Describes why the login HTTP client could not be constructed.
///
/// This boundary is more specific than `io::Error`: login can fail because the configured CA file
/// could not be read, could not be parsed as certificates, contained certs that `reqwest` refused
/// to register, or because the final client builder failed. The rest of the login crate still
/// speaks `io::Error`, so callers that do not care about the distinction can rely on the
/// `From<BuildLoginHttpClientError> for io::Error` conversion.
#[derive(Debug, Error)]
pub enum BuildLoginHttpClientError {
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
        "Failed to build login HTTP client while using CA bundle from {} ({}): {source}",
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
    #[error("Failed to build login HTTP client while using system root certificates: {0}")]
    BuildClientWithSystemRoots(#[source] reqwest::Error),
}

impl From<BuildLoginHttpClientError> for io::Error {
    fn from(error: BuildLoginHttpClientError) -> Self {
        match error {
            BuildLoginHttpClientError::ReadCaFile { ref source, .. } => {
                io::Error::new(source.kind(), error)
            }
            BuildLoginHttpClientError::InvalidCaFile { .. }
            | BuildLoginHttpClientError::RegisterCertificate { .. } => {
                io::Error::new(io::ErrorKind::InvalidData, error)
            }
            BuildLoginHttpClientError::BuildClientWithCustomCa { .. }
            | BuildLoginHttpClientError::BuildClientWithSystemRoots(_) => io::Error::other(error),
        }
    }
}

/// Builds the HTTP client used by login and device-code flows.
///
/// Callers should use this instead of constructing a raw `reqwest::Client` so every login entry
/// point honors the same CA override behavior. A caller that bypasses this helper can silently
/// regress enterprise login setups that rely on `CODEX_CA_CERTIFICATE` or `SSL_CERT_FILE`.
///
/// # Errors
///
/// Returns a [`BuildLoginHttpClientError`] when the configured CA file is unreadable, malformed,
/// or contains a certificate block that `reqwest` cannot register as a root.
pub fn build_login_http_client() -> Result<reqwest::Client, BuildLoginHttpClientError> {
    build_login_http_client_with_env(&ProcessEnv)
}

/// Builds a login HTTP client using an injected environment source.
///
/// This exists so unit tests can exercise precedence and PEM-handling behavior deterministically.
/// Production code should call [`build_login_http_client`] instead of supplying its own
/// environment adapter, otherwise the tests and the real process environment can drift apart.
/// This function is also the place where module responsibilities come together: it selects the CA
/// bundle, delegates file parsing to [`ConfiguredCaBundle::load_certificates`], logs the unusual-but-supported PEM shapes,
/// and finally registers each parsed certificate with the `reqwest` builder.
fn build_login_http_client_with_env(
    env_source: &dyn EnvSource,
) -> Result<reqwest::Client, BuildLoginHttpClientError> {
    let mut builder = reqwest::Client::builder();
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
                        "failed to register login CA certificate"
                    );
                    return Err(BuildLoginHttpClientError::RegisterCertificate {
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
                Err(BuildLoginHttpClientError::BuildClientWithCustomCa {
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
            Err(BuildLoginHttpClientError::BuildClientWithSystemRoots(
                source,
            ))
        }
    }
}

/// Abstracts environment access so tests can cover precedence rules without mutating process-wide
/// variables.
trait EnvSource {
    /// Returns the environment variable value for `key`, if this source considers it set.
    ///
    /// Implementations should return `None` for absent values and may also collapse unreadable
    /// process-environment states into `None`, because the login CA logic treats both cases as
    /// "no override configured". Callers build precedence and empty-string handling on top of this
    /// method, so implementations should not trim or normalize the returned string.
    fn var(&self, key: &str) -> Option<String>;

    /// Returns a non-empty environment variable value interpreted as a filesystem path.
    ///
    /// Empty strings are treated as unset because login uses presence here as a boolean "custom CA
    /// override requested" signal. This keeps the precedence logic from treating `VAR=""` as an
    /// attempt to open the current working directory or some other platform-specific oddity once it
    /// is converted into a path.
    fn non_empty_path(&self, key: &str) -> Option<PathBuf> {
        self.var(key)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    }

    /// Returns the configured CA bundle and which environment variable selected it.
    ///
    /// `CODEX_CA_CERTIFICATE` wins over `SSL_CERT_FILE` because it is the login-specific override.
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

/// Reads login CA configuration from the real process environment.
///
/// This is the production `EnvSource` implementation used by
/// [`build_login_http_client`]. Tests substitute in-memory env maps so they can
/// exercise precedence and empty-value behavior without mutating process-global
/// variables.
struct ProcessEnv;

impl EnvSource for ProcessEnv {
    fn var(&self, key: &str) -> Option<String> {
        env::var(key).ok()
    }
}

/// Identifies the CA bundle selected for login and the policy decision that selected it.
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
    fn load_certificates(&self) -> Result<Vec<CertificateDer<'static>>, BuildLoginHttpClientError> {
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

    /// Loads every certificate block from a PEM file intended for login CA overrides.
    ///
    /// This accepts a few common real-world variants so login behaves like other CA-aware tooling:
    /// leading comments are preserved, `TRUSTED CERTIFICATE` labels are normalized to standard
    /// certificate labels, and embedded CRLs are ignored.
    ///
    /// # Errors
    ///
    /// Returns `InvalidData` when the file cannot be interpreted as one or more certificates, and
    /// preserves the filesystem error kind when the file itself cannot be read.
    fn parse_certificates(
        &self,
    ) -> Result<Vec<CertificateDer<'static>>, BuildLoginHttpClientError> {
        let pem_data = self.read_pem_data()?;
        let normalized_pem = NormalizedPem::from_pem_data(self.source_env, &self.path, &pem_data);

        let mut certificates = Vec::new();
        let mut logged_crl_presence = false;
        // Use the mixed-section parser from `rustls-pki-types` so CRLs can be identified and
        // skipped explicitly instead of being removed with ad hoc text rewriting.
        for section_result in normalized_pem.sections() {
            let (section_kind, der) = match section_result {
                Ok(section) => section,
                Err(error) => return Err(self.pem_parse_error(&error)),
            };
            match section_kind {
                SectionKind::Certificate => {
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
    /// the higher-level login surfaces still benefit from distinguishing "not found" from other I/O
    /// failures. This helper keeps both pieces together.
    fn read_pem_data(&self) -> Result<Vec<u8>, BuildLoginHttpClientError> {
        fs::read(&self.path).map_err(|source| BuildLoginHttpClientError::ReadCaFile {
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
    fn pem_parse_error(&self, error: &pem::Error) -> BuildLoginHttpClientError {
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
    fn invalid_ca_file(&self, detail: impl std::fmt::Display) -> BuildLoginHttpClientError {
        BuildLoginHttpClientError::InvalidCaFile {
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
    /// Login only needs certificate DER bytes to seed `reqwest`'s root store, but operators may
    /// point it at CA files that came from OpenSSL tooling rather than from a minimal certificate
    /// bundle. OpenSSL's `TRUSTED CERTIFICATE` form is one such variant: it is still certificate
    /// material, but it uses a different PEM label and may carry auxiliary trust metadata that
    /// this crate does not consume. This constructor rewrites only the PEM labels so the mixed-
    /// section parser can keep treating the file as certificate input.
    ///
    /// See also:
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

    // Keep this module limited to pure precedence logic. Building a real reqwest client here is
    // not hermetic on macOS sandboxed test runs because client construction can consult platform
    // networking configuration and panic before the test asserts anything. The real client-building
    // cases live in `tests/ca_env.rs`, which exercises them in a subprocess with explicit env.
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
