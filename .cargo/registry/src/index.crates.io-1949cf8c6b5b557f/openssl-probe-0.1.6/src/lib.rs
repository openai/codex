use std::env;
use std::path::{Path, PathBuf};

/// The OpenSSL environment variable to configure what certificate file to use.
pub const ENV_CERT_FILE: &'static str = "SSL_CERT_FILE";

/// The OpenSSL environment variable to configure what certificates directory to use.
pub const ENV_CERT_DIR: &'static str = "SSL_CERT_DIR";

pub struct ProbeResult {
    pub cert_file: Option<PathBuf>,
    pub cert_dir: Option<PathBuf>,
}

/// Probe the system for the directory in which CA certificates should likely be
/// found.
///
/// This will only search known system locations.
#[doc(hidden)]
#[deprecated(note = "use `candidate_cert_dirs` instead")]
pub fn find_certs_dirs() -> Vec<PathBuf> {
    candidate_cert_dirs().map(Path::to_path_buf).collect()
}

/// Probe the system for the directory in which CA certificates should likely be
/// found.
///
/// This will only search known system locations.
pub fn candidate_cert_dirs() -> impl Iterator<Item = &'static Path> {
    // see http://gagravarr.org/writing/openssl-certs/others.shtml
    [
        "/var/ssl",
        "/usr/share/ssl",
        "/usr/local/ssl",
        "/usr/local/openssl",
        "/usr/local/etc/openssl",
        "/usr/local/share",
        "/usr/lib/ssl",
        "/usr/ssl",
        "/etc/openssl",
        "/etc/pki/ca-trust/extracted/pem",
        "/etc/pki/tls",
        "/etc/ssl",
        "/etc/certs",
        "/opt/etc/ssl", // Entware
        #[cfg(target_os = "android")]
        "/data/data/com.termux/files/usr/etc/tls",
        #[cfg(target_os = "haiku")]
        "/boot/system/data/ssl",
    ]
    .iter()
    .map(Path::new)
    .filter(|p| p.exists())
}

/// Deprecated as this isn't sound, use [`init_openssl_env_vars`] instead.
#[doc(hidden)]
#[deprecated(note = "this function is not safe, use `init_openssl_env_vars` instead")]
pub fn init_ssl_cert_env_vars() {
    unsafe {
        init_openssl_env_vars();
    }
}

/// Probe for SSL certificates on the system, then configure the SSL certificate `SSL_CERT_FILE`
/// and `SSL_CERT_DIR` environment variables in this process for OpenSSL to use.
///
/// Preconfigured values in the environment variables will not be overwritten if the paths they
/// point to exist and are accessible.
///
/// # Safety
///
/// This function is not safe because it mutates the process's environment
/// variables which is generally not safe. See the [documentation in libstd][doc]
/// for information about why setting environment variables is not safe.
///
/// If possible use the [`probe`] function and directly configure OpenSSL
/// methods instead of relying on environment variables.
///
/// [doc]: https://doc.rust-lang.org/stable/std/env/fn.set_var.html#safety
pub unsafe fn init_openssl_env_vars() {
    try_init_openssl_env_vars();
}

/// Deprecated as this isn't sound, use [`try_init_openssl_env_vars`] instead.
#[doc(hidden)]
#[deprecated(note = "use try_init_openssl_env_vars instead, this function is not safe")]
pub fn try_init_ssl_cert_env_vars() -> bool {
    unsafe { try_init_openssl_env_vars() }
}

/// Probe for SSL certificates on the system, then configure the SSL certificate `SSL_CERT_FILE`
/// and `SSL_CERT_DIR` environment variables in this process for OpenSSL to use.
///
/// Preconfigured values in the environment variables will not be overwritten if the paths they
/// point to exist and are accessible.
///
/// Returns `true` if any certificate file or directory was found while probing.
/// Combine this with `has_ssl_cert_env_vars()` to check whether previously configured environment
/// variables are valid.
///
/// # Safety
///
/// This function is not safe because it mutates the process's environment
/// variables which is generally not safe. See the [documentation in libstd][doc]
/// for information about why setting environment variables is not safe.
///
/// If possible use the [`probe`] function and directly configure OpenSSL
/// methods instead of relying on environment variables.
///
/// [doc]: https://doc.rust-lang.org/stable/std/env/fn.set_var.html#safety
pub unsafe fn try_init_openssl_env_vars() -> bool {
    let ProbeResult {
        cert_file,
        cert_dir,
    } = probe();
    // we won't be overwriting existing env variables because if they're valid probe() will have
    // returned them unchanged
    if let Some(path) = &cert_file {
        unsafe {
            put(ENV_CERT_FILE, path);
        }
    }
    if let Some(path) = &cert_dir {
        unsafe {
            put(ENV_CERT_DIR, path);
        }
    }

    unsafe fn put(var: &str, path: &Path) {
        // Avoid calling `setenv` if the variable already has the same contents. This avoids a
        // crash when called from out of perl <5.38 (Debian Bookworm is at 5.36), as old versions
        // of perl tend to manipulate the `environ` pointer directly.
        if env::var_os(var).as_deref() != Some(path.as_os_str()) {
            unsafe {
                env::set_var(var, path);
            }
        }
    }

    cert_file.is_some() || cert_dir.is_some()
}

/// Check whether the OpenSSL `SSL_CERT_FILE` and/or `SSL_CERT_DIR` environment variable is
/// configured in this process with an existing file or directory.
///
/// That being the case would indicate that certificates will be found successfully by OpenSSL.
///
/// Returns `true` if either variable is set to an existing file or directory.
pub fn has_ssl_cert_env_vars() -> bool {
    let probe = probe_from_env();
    probe.cert_file.is_some() || probe.cert_dir.is_some()
}

fn probe_from_env() -> ProbeResult {
    let var = |name| env::var_os(name).map(PathBuf::from).filter(|p| p.exists());
    ProbeResult {
        cert_file: var(ENV_CERT_FILE),
        cert_dir: var(ENV_CERT_DIR),
    }
}

/// Probe the current system for the "cert file" and "cert dir" variables that
/// OpenSSL typically requires.
///
/// The probe result is returned as a [`ProbeResult`] structure here.
pub fn probe() -> ProbeResult {
    let mut result = probe_from_env();
    for certs_dir in candidate_cert_dirs() {
        // cert.pem looks to be an openssl 1.0.1 thing, while
        // certs/ca-certificates.crt appears to be a 0.9.8 thing
        let cert_filenames = [
            "cert.pem",
            "certs.pem",
            "ca-bundle.pem",
            "cacert.pem",
            "ca-certificates.crt",
            "certs/ca-certificates.crt",
            "certs/ca-root-nss.crt",
            "certs/ca-bundle.crt",
            "CARootCertificates.pem",
            "tls-ca-bundle.pem",
        ];
        if result.cert_file.is_none() {
            result.cert_file = cert_filenames
                .iter()
                .map(|fname| certs_dir.join(fname))
                .find(|p| p.exists());
        }
        if result.cert_dir.is_none() {
            let cert_dir = certs_dir.join("certs");
            if cert_dir.exists() {
                result.cert_dir = Some(cert_dir);
            }
        }
        if result.cert_file.is_some() && result.cert_dir.is_some() {
            break;
        }
    }
    result
}
