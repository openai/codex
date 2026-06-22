#[cfg(any(target_os = "macos", windows))]
use rama_tls_rustls::dep::pki_types::CertificateDer;
use rustls_native_certs::CertificateResult;
#[cfg(any(target_os = "macos", windows))]
use rustls_native_certs::Error;
#[cfg(any(target_os = "macos", windows))]
use rustls_native_certs::ErrorKind;

// `rustls_native_certs::load_native_certs()` first consults SSL_CERT_FILE and
// SSL_CERT_DIR. Load platform roots directly so a startup custom CA can be
// layered onto the managed bundle without replacing the platform trust store.
#[cfg(all(unix, not(target_os = "macos")))]
pub(crate) fn load_platform_native_certs() -> CertificateResult {
    let mut result =
        rustls_native_certs::load_certs_from_paths(platform_cert_file().as_deref(), None);
    for cert_dir in platform_cert_dirs() {
        extend_certificate_result(
            &mut result,
            rustls_native_certs::load_certs_from_paths(None, Some(&cert_dir)),
        );
    }
    dedupe_certs(&mut result);
    result
}

#[cfg(target_os = "macos")]
pub(crate) fn load_platform_native_certs() -> CertificateResult {
    use security_framework::trust_settings::Domain;
    use security_framework::trust_settings::TrustSettings;
    use security_framework::trust_settings::TrustSettingsForCertificate;
    use std::collections::BTreeMap;

    let mut result = CertificateResult::default();
    let mut all_certs = BTreeMap::new();
    for domain in &[Domain::User, Domain::Admin, Domain::System] {
        let ts = TrustSettings::new(*domain);
        let iter = match ts.iter() {
            Ok(iter) => iter,
            Err(err) => {
                result.errors.push(Error {
                    context: match domain {
                        Domain::User => "failed to load user trust settings",
                        Domain::Admin => "failed to load admin trust settings",
                        Domain::System => "failed to load system trust settings",
                    },
                    kind: ErrorKind::Os(err.into()),
                });
                continue;
            }
        };

        for cert in iter {
            let der = cert.to_der();
            let trusted = match ts.tls_trust_settings_for_certificate(&cert) {
                Ok(trusted) => trusted.unwrap_or(TrustSettingsForCertificate::TrustRoot),
                Err(err) => {
                    result.errors.push(Error {
                        context: "certificate not trusted",
                        kind: ErrorKind::Os(err.into()),
                    });
                    continue;
                }
            };
            all_certs.entry(der).or_insert(trusted);
        }
    }

    for (der, trusted) in all_certs {
        use TrustSettingsForCertificate::*;

        if let TrustRoot | TrustAsRoot = trusted {
            result.certs.push(CertificateDer::from(der));
        }
    }
    result
}

#[cfg(windows)]
pub(crate) fn load_platform_native_certs() -> CertificateResult {
    use schannel::cert_store::CertStore;

    let mut result = CertificateResult::default();
    let current_user_store = match CertStore::open_current_user("ROOT") {
        Ok(store) => store,
        Err(err) => {
            result.errors.push(Error {
                context: "failed to open current user certificate store",
                kind: ErrorKind::Os(err.into()),
            });
            return result;
        }
    };

    for cert in current_user_store.certs() {
        let valid_uses = match cert.valid_uses() {
            Ok(valid_uses) => valid_uses,
            Err(err) => {
                result.errors.push(Error {
                    context: "failed to inspect certificate valid uses",
                    kind: ErrorKind::Os(err.into()),
                });
                continue;
            }
        };
        let is_time_valid = match cert.is_time_valid() {
            Ok(is_time_valid) => is_time_valid,
            Err(err) => {
                result.errors.push(Error {
                    context: "failed to inspect certificate time validity",
                    kind: ErrorKind::Os(err.into()),
                });
                continue;
            }
        };
        if usable_for_rustls(valid_uses) && is_time_valid {
            result
                .certs
                .push(CertificateDer::from(cert.to_der().to_vec()));
        }
    }
    result
}

#[cfg(not(any(all(unix, not(target_os = "macos")), target_os = "macos", windows)))]
pub(crate) fn load_platform_native_certs() -> CertificateResult {
    rustls_native_certs::load_native_certs()
}

#[cfg(all(unix, not(target_os = "macos")))]
fn extend_certificate_result(result: &mut CertificateResult, extra: CertificateResult) {
    result.certs.extend(extra.certs);
    result.errors.extend(extra.errors);
}

#[cfg(all(unix, not(target_os = "macos")))]
fn dedupe_certs(result: &mut CertificateResult) {
    result.certs.sort_unstable_by(|a, b| a.cmp(b));
    result.certs.dedup();
}

#[cfg(all(unix, not(target_os = "macos")))]
fn platform_cert_file() -> Option<std::path::PathBuf> {
    PLATFORM_CERTIFICATE_FILE_NAMES
        .iter()
        .map(std::path::Path::new)
        .find(|path| path.exists())
        .map(std::path::Path::to_path_buf)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn platform_cert_dirs() -> impl Iterator<Item = std::path::PathBuf> {
    PLATFORM_CERTIFICATE_DIRS
        .iter()
        .map(std::path::Path::new)
        .filter(|path| path.exists())
        .map(std::path::Path::to_path_buf)
}

#[cfg(all(unix, not(target_os = "macos"), target_os = "linux"))]
const PLATFORM_CERTIFICATE_DIRS: &[&str] = &[
    "/etc/ssl/certs",
    "/etc/pki/tls/certs",
    "/etc/security/certificates",
];

#[cfg(all(unix, not(target_os = "macos"), target_os = "freebsd"))]
const PLATFORM_CERTIFICATE_DIRS: &[&str] = &["/etc/ssl/certs", "/usr/local/share/certs"];

#[cfg(all(
    unix,
    not(target_os = "macos"),
    any(target_os = "illumos", target_os = "solaris")
))]
const PLATFORM_CERTIFICATE_DIRS: &[&str] = &["/etc/certs/CA"];

#[cfg(all(unix, not(target_os = "macos"), target_os = "netbsd"))]
const PLATFORM_CERTIFICATE_DIRS: &[&str] = &["/etc/openssl/certs"];

#[cfg(all(unix, not(target_os = "macos"), target_os = "aix"))]
const PLATFORM_CERTIFICATE_DIRS: &[&str] = &["/var/ssl/certs"];

#[cfg(all(
    unix,
    not(target_os = "macos"),
    not(any(
        target_os = "linux",
        target_os = "freebsd",
        target_os = "illumos",
        target_os = "solaris",
        target_os = "netbsd",
        target_os = "aix"
    ))
))]
const PLATFORM_CERTIFICATE_DIRS: &[&str] = &["/etc/ssl/certs"];

#[cfg(all(unix, not(target_os = "macos"), target_os = "linux"))]
const PLATFORM_CERTIFICATE_FILE_NAMES: &[&str] = &[
    "/etc/ssl/certs/ca-certificates.crt",
    "/etc/pki/ca-trust/extracted/pem/tls-ca-bundle.pem",
    "/etc/pki/tls/certs/ca-bundle.crt",
    "/etc/ssl/ca-bundle.pem",
    "/etc/pki/tls/cacert.pem",
    "/etc/ssl/cert.pem",
    "/opt/etc/ssl/certs/ca-certificates.crt",
    "/etc/ssl/certs/cacert.pem",
];

#[cfg(all(unix, not(target_os = "macos"), target_os = "freebsd"))]
const PLATFORM_CERTIFICATE_FILE_NAMES: &[&str] = &["/usr/local/etc/ssl/cert.pem"];

#[cfg(all(unix, not(target_os = "macos"), target_os = "dragonfly"))]
const PLATFORM_CERTIFICATE_FILE_NAMES: &[&str] = &["/usr/local/share/certs/ca-root-nss.crt"];

#[cfg(all(unix, not(target_os = "macos"), target_os = "netbsd"))]
const PLATFORM_CERTIFICATE_FILE_NAMES: &[&str] = &["/etc/openssl/certs/ca-certificates.crt"];

#[cfg(all(unix, not(target_os = "macos"), target_os = "openbsd"))]
const PLATFORM_CERTIFICATE_FILE_NAMES: &[&str] = &["/etc/ssl/cert.pem"];

#[cfg(all(unix, not(target_os = "macos"), target_os = "solaris"))]
const PLATFORM_CERTIFICATE_FILE_NAMES: &[&str] = &["/etc/certs/ca-certificates.crt"];

#[cfg(all(unix, not(target_os = "macos"), target_os = "illumos"))]
const PLATFORM_CERTIFICATE_FILE_NAMES: &[&str] =
    &["/etc/ssl/cacert.pem", "/etc/certs/ca-certificates.crt"];

#[cfg(all(unix, not(target_os = "macos"), target_os = "android"))]
const PLATFORM_CERTIFICATE_FILE_NAMES: &[&str] =
    &["/data/data/com.termux/files/usr/etc/tls/cert.pem"];

#[cfg(all(unix, not(target_os = "macos"), target_os = "haiku"))]
const PLATFORM_CERTIFICATE_FILE_NAMES: &[&str] = &["/boot/system/data/ssl/CARootCertificates.pem"];

#[cfg(all(
    unix,
    not(target_os = "macos"),
    not(any(
        target_os = "linux",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "solaris",
        target_os = "illumos",
        target_os = "android",
        target_os = "haiku",
    ))
))]
const PLATFORM_CERTIFICATE_FILE_NAMES: &[&str] = &["/etc/ssl/certs/ca-certificates.crt"];

#[cfg(windows)]
fn usable_for_rustls(uses: schannel::cert_context::ValidUses) -> bool {
    match uses {
        schannel::cert_context::ValidUses::All => true,
        schannel::cert_context::ValidUses::Oids(strs) => strs.iter().any(|x| x == PKIX_SERVER_AUTH),
    }
}

#[cfg(windows)]
const PKIX_SERVER_AUTH: &str = "1.3.6.1.5.5.7.3.1";
