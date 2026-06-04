use anyhow::Context as _;
use anyhow::Result;
use anyhow::anyhow;
use base64::Engine as _;
use codex_utils_home_dir::find_codex_home;
use rama_net::tls::ApplicationProtocol;
use rama_tls_rustls::dep::pki_types::CertificateDer;
use rama_tls_rustls::dep::pki_types::PrivateKeyDer;
use rama_tls_rustls::dep::pki_types::pem::PemObject;
use rama_tls_rustls::dep::rcgen::BasicConstraints;
use rama_tls_rustls::dep::rcgen::CertificateParams;
use rama_tls_rustls::dep::rcgen::DistinguishedName;
use rama_tls_rustls::dep::rcgen::DnType;
use rama_tls_rustls::dep::rcgen::ExtendedKeyUsagePurpose;
use rama_tls_rustls::dep::rcgen::IsCa;
use rama_tls_rustls::dep::rcgen::Issuer;
use rama_tls_rustls::dep::rcgen::KeyPair;
use rama_tls_rustls::dep::rcgen::KeyUsagePurpose;
use rama_tls_rustls::dep::rcgen::PKCS_ECDSA_P256_SHA256;
use rama_tls_rustls::dep::rcgen::SanType;
use rama_tls_rustls::dep::rustls;
use rama_tls_rustls::server::TlsAcceptorData;
use sha2::Digest as _;
use sha2::Sha256;
use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::Write;
use std::net::IpAddr;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use tracing::info;
use tracing::warn;

pub(super) struct ManagedMitmCa {
    issuer: Issuer<'static, KeyPair>,
}

impl ManagedMitmCa {
    pub(super) fn load_or_create() -> Result<Self> {
        let (ca_cert_pem, ca_key_pem) = load_or_create_ca()?;
        let ca_key = KeyPair::from_pem(&ca_key_pem).context("failed to parse CA key")?;
        let issuer: Issuer<'static, KeyPair> =
            Issuer::from_ca_cert_pem(&ca_cert_pem, ca_key).context("failed to parse CA cert")?;
        Ok(Self { issuer })
    }

    pub(super) fn tls_acceptor_data_for_host(&self, host: &str) -> Result<TlsAcceptorData> {
        let (cert_pem, key_pem) = issue_host_certificate_pem(host, &self.issuer)?;
        let cert = CertificateDer::from_pem_slice(cert_pem.as_bytes())
            .context("failed to parse host cert PEM")?;
        let key = PrivateKeyDer::from_pem_slice(key_pem.as_bytes())
            .context("failed to parse host key PEM")?;
        let mut server_config =
            rustls::ServerConfig::builder_with_protocol_versions(rustls::ALL_VERSIONS)
                .with_no_client_auth()
                .with_single_cert(vec![cert], key)
                .context("failed to build rustls server config")?;
        server_config.alpn_protocols = vec![
            ApplicationProtocol::HTTP_2.as_bytes().to_vec(),
            ApplicationProtocol::HTTP_11.as_bytes().to_vec(),
        ];

        Ok(TlsAcceptorData::from(server_config))
    }
}

fn issue_host_certificate_pem(
    host: &str,
    issuer: &Issuer<'_, KeyPair>,
) -> Result<(String, String)> {
    let mut params = if let Ok(ip) = host.parse::<IpAddr>() {
        let mut params = CertificateParams::new(Vec::new())
            .map_err(|err| anyhow!("failed to create cert params: {err}"))?;
        params.subject_alt_names.push(SanType::IpAddress(ip));
        params
    } else {
        CertificateParams::new(vec![host.to_string()])
            .map_err(|err| anyhow!("failed to create cert params: {err}"))?
    };

    params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    params.key_usages = vec![
        KeyUsagePurpose::DigitalSignature,
        KeyUsagePurpose::KeyEncipherment,
    ];

    let key_pair = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)
        .map_err(|err| anyhow!("failed to generate host key pair: {err}"))?;
    let cert = params
        .signed_by(&key_pair, issuer)
        .map_err(|err| anyhow!("failed to sign host cert: {err}"))?;

    Ok((cert.pem(), key_pair.serialize_pem()))
}

const MANAGED_MITM_CA_DIR: &str = "proxy";
const MANAGED_MITM_CA_CERT: &str = "ca.pem";
const MANAGED_MITM_CA_KEY: &str = "ca.key";
const MANAGED_MITM_CA_TRUST_BUNDLE_PREFIX: &str = "ca-bundle";
const SSL_CERT_FILE_ENV_KEY: &str = "SSL_CERT_FILE";
pub const SSL_CERT_DIR_ENV_KEY: &str = "SSL_CERT_DIR";

// Best-effort compatibility set for common child toolchains that accept a CA bundle path.
// This is intentionally curated rather than pretending to cover every TLS client.
pub const CUSTOM_CA_ENV_KEYS: [&str; 10] = [
    "CODEX_CA_CERTIFICATE",
    SSL_CERT_FILE_ENV_KEY,
    "REQUESTS_CA_BUNDLE",
    "CURL_CA_BUNDLE",
    "NODE_EXTRA_CA_CERTS",
    "GIT_SSL_CAINFO",
    "PIP_CERT",
    "BUNDLE_SSL_CA_CERT",
    "npm_config_cafile",
    "NPM_CONFIG_CAFILE",
];

/// Immutable managed MITM CA bundle path plus startup TLS env values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ManagedMitmCaTrustBundle {
    pub(crate) path: PathBuf,
    pub(crate) startup_env_values: HashMap<&'static str, String>,
    pub(crate) startup_cwd: PathBuf,
}

fn managed_ca_paths() -> Result<(PathBuf, PathBuf)> {
    let codex_home =
        find_codex_home().context("failed to resolve CODEX_HOME for managed MITM CA")?;
    let proxy_dir = codex_home.join(MANAGED_MITM_CA_DIR);
    Ok((
        proxy_dir.join(MANAGED_MITM_CA_CERT).to_path_buf(),
        proxy_dir.join(MANAGED_MITM_CA_KEY).to_path_buf(),
    ))
}

pub(crate) fn managed_ca_trust_bundle(
    env: &HashMap<&'static str, String>,
) -> Result<ManagedMitmCaTrustBundle> {
    load_or_create_ca()?;
    let (cert_path, _) = managed_ca_paths()?;
    managed_ca_trust_bundle_for_cert_path(&cert_path, env)
}

fn managed_ca_trust_bundle_for_cert_path(
    cert_path: &Path,
    env: &HashMap<&'static str, String>,
) -> Result<ManagedMitmCaTrustBundle> {
    let startup_cwd =
        std::env::current_dir().context("failed to resolve startup cwd for managed MITM CA")?;
    let startup_env_values = CUSTOM_CA_ENV_KEYS
        .into_iter()
        .chain(std::iter::once(SSL_CERT_DIR_ENV_KEY))
        .filter_map(|key| {
            env.get(key)
                .filter(|value| !value.is_empty())
                .map(|value| (key, value.clone()))
        })
        .collect();
    let trust_bundle = build_managed_ca_trust_bundle(cert_path)?;
    let path = persist_managed_ca_trust_bundle(cert_path, &trust_bundle)?;

    Ok(ManagedMitmCaTrustBundle {
        path,
        startup_env_values,
        startup_cwd,
    })
}

fn build_managed_ca_trust_bundle(managed_ca_cert_path: &Path) -> Result<String> {
    let mut trust_bundle = String::new();
    let rustls_native_certs::CertificateResult { certs, errors, .. } =
        crate::native_certs::load_platform_native_certs();
    if !errors.is_empty() {
        warn!(
            native_root_error_count = errors.len(),
            "encountered errors while loading native root certificates for MITM trust bundle"
        );
    }
    for cert in certs {
        push_certificate_pem(&mut trust_bundle, cert.as_ref());
    }
    append_pem_file(&mut trust_bundle, managed_ca_cert_path)?;
    Ok(trust_bundle)
}

fn is_current_generated_trust_bundle_path(path: &Path, managed_ca_cert_path: &Path) -> bool {
    let Some(proxy_dir) = managed_ca_cert_path.parent() else {
        return false;
    };
    let Some(file_name) = path.file_name().and_then(|file_name| file_name.to_str()) else {
        return false;
    };
    if path.parent() != Some(proxy_dir)
        || !file_name.starts_with(MANAGED_MITM_CA_TRUST_BUNDLE_PREFIX)
        || !file_name.ends_with(".pem")
    {
        return false;
    }
    let Ok(trust_bundle) = fs::read(path) else {
        return false;
    };
    let Ok(managed_ca_cert) = fs::read(managed_ca_cert_path) else {
        return false;
    };
    !managed_ca_cert.is_empty()
        && trust_bundle
            .windows(managed_ca_cert.len())
            .any(|window| window == managed_ca_cert)
}

/// Returns whether `path` points at a current Codex-generated MITM CA bundle.
pub fn is_managed_mitm_ca_trust_bundle_path(path: &str) -> bool {
    let Ok((managed_ca_cert_path, _)) = managed_ca_paths() else {
        return false;
    };
    is_current_generated_trust_bundle_path(Path::new(path), &managed_ca_cert_path)
}

fn persist_managed_ca_trust_bundle(
    managed_ca_cert_path: &Path,
    trust_bundle: &str,
) -> Result<PathBuf> {
    let proxy_dir = managed_ca_cert_path
        .parent()
        .ok_or_else(|| anyhow!("managed MITM CA cert path is missing a parent"))?;
    fs::create_dir_all(proxy_dir)
        .with_context(|| format!("failed to create {}", proxy_dir.display()))?;
    let hash = Sha256::digest(trust_bundle.as_bytes());
    let trust_bundle_path = proxy_dir.join(format!(
        "{MANAGED_MITM_CA_TRUST_BUNDLE_PREFIX}-{hash:x}.pem"
    ));
    write_atomic_create_new_or_reuse(
        &trust_bundle_path,
        trust_bundle.as_bytes(),
        /*mode*/ 0o644,
    )
    .with_context(|| {
        format!(
            "failed to persist managed MITM CA trust bundle {}",
            trust_bundle_path.display()
        )
    })?;
    Ok(trust_bundle_path)
}

fn append_pem_file(bundle: &mut String, path: &Path) -> Result<()> {
    if !bundle.ends_with('\n') {
        bundle.push('\n');
    }
    let pem = fs::read_to_string(path)
        .with_context(|| format!("failed to read CA bundle {}", path.display()))?;
    bundle.push_str(&pem);
    if !bundle.ends_with('\n') {
        bundle.push('\n');
    }
    Ok(())
}

fn push_certificate_pem(bundle: &mut String, der: &[u8]) {
    bundle.push_str("-----BEGIN CERTIFICATE-----\n");
    let encoded = base64::engine::general_purpose::STANDARD.encode(der);
    for chunk in encoded.as_bytes().chunks(64) {
        bundle.push_str(&String::from_utf8_lossy(chunk));
        bundle.push('\n');
    }
    bundle.push_str("-----END CERTIFICATE-----\n");
}

fn load_or_create_ca() -> Result<(String, String)> {
    let (cert_path, key_path) = managed_ca_paths()?;

    if cert_path.exists() || key_path.exists() {
        if !cert_path.exists() || !key_path.exists() {
            return Err(anyhow!(
                "both managed MITM CA files must exist (cert={}, key={})",
                cert_path.display(),
                key_path.display()
            ));
        }
        validate_existing_ca_key_file(&key_path)?;
        let cert_pem = fs::read_to_string(&cert_path)
            .with_context(|| format!("failed to read CA cert {}", cert_path.display()))?;
        let key_pem = fs::read_to_string(&key_path)
            .with_context(|| format!("failed to read CA key {}", key_path.display()))?;
        return Ok((cert_pem, key_pem));
    }

    if let Some(parent) = cert_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    if let Some(parent) = key_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let (cert_pem, key_pem) = generate_ca()?;
    // The CA key is a high-value secret. Create it atomically with restrictive permissions.
    // The cert can be world-readable, but we still write it atomically to avoid partial writes.
    //
    // We intentionally use create-new semantics: if a key already exists, we should not overwrite
    // it silently (that would invalidate previously-trusted cert chains).
    write_atomic_create_new(&key_path, key_pem.as_bytes(), /*mode*/ 0o600)
        .with_context(|| format!("failed to persist CA key {}", key_path.display()))?;
    if let Err(err) = write_atomic_create_new(&cert_path, cert_pem.as_bytes(), /*mode*/ 0o644)
        .with_context(|| format!("failed to persist CA cert {}", cert_path.display()))
    {
        // Avoid leaving a partially-created CA around (cert missing) if the second write fails.
        let _ = fs::remove_file(&key_path);
        return Err(err);
    }
    let cert_path = cert_path.display();
    let key_path = key_path.display();
    info!("generated MITM CA (cert_path={cert_path}, key_path={key_path})");
    Ok((cert_pem, key_pem))
}

fn generate_ca() -> Result<(String, String)> {
    let mut params = CertificateParams::default();
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    params.key_usages = vec![
        KeyUsagePurpose::KeyCertSign,
        KeyUsagePurpose::DigitalSignature,
        KeyUsagePurpose::KeyEncipherment,
    ];
    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, "network_proxy MITM CA");
    params.distinguished_name = dn;

    let key_pair = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)
        .map_err(|err| anyhow!("failed to generate CA key pair: {err}"))?;
    let cert = params
        .self_signed(&key_pair)
        .map_err(|err| anyhow!("failed to generate CA cert: {err}"))?;
    Ok((cert.pem(), key_pair.serialize_pem()))
}

fn write_atomic_create_new(path: &Path, contents: &[u8], mode: u32) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("missing parent directory"))?;

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let pid = std::process::id();
    let file_name = path.file_name().unwrap_or_default().to_string_lossy();
    let tmp_path = parent.join(format!(".{file_name}.tmp.{pid}.{nanos}"));

    let mut file = open_create_new_with_mode(&tmp_path, mode)?;
    file.write_all(contents)
        .with_context(|| format!("failed to write {}", tmp_path.display()))?;
    file.sync_all()
        .with_context(|| format!("failed to fsync {}", tmp_path.display()))?;
    drop(file);

    // Create the final file using "create-new" semantics (no overwrite). `rename` on Unix can
    // overwrite existing files, so prefer a hard-link, which fails if the destination exists.
    match fs::hard_link(&tmp_path, path) {
        Ok(()) => {
            fs::remove_file(&tmp_path)
                .with_context(|| format!("failed to remove {}", tmp_path.display()))?;
        }
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
            let _ = fs::remove_file(&tmp_path);
            return Err(anyhow!(
                "refusing to overwrite existing file {}",
                path.display()
            ));
        }
        Err(_) => {
            // Best-effort fallback for environments where hard links are not supported.
            // This is still subject to a TOCTOU race, but the typical case is a private per-user
            // config directory, where other users cannot create files anyway.
            if path.exists() {
                let _ = fs::remove_file(&tmp_path);
                return Err(anyhow!(
                    "refusing to overwrite existing file {}",
                    path.display()
                ));
            }
            fs::rename(&tmp_path, path).with_context(|| {
                format!(
                    "failed to rename {} -> {}",
                    tmp_path.display(),
                    path.display()
                )
            })?;
        }
    }

    sync_parent_dir(parent)?;

    Ok(())
}

#[cfg(not(windows))]
fn sync_parent_dir(parent: &Path) -> Result<()> {
    // Best-effort durability: ensure the directory entry is persisted too.
    let dir = File::open(parent).with_context(|| format!("failed to open {}", parent.display()))?;
    dir.sync_all()
        .with_context(|| format!("failed to fsync {}", parent.display()))
}

#[cfg(windows)]
fn sync_parent_dir(_parent: &Path) -> Result<()> {
    Ok(())
}

fn write_atomic_create_new_or_reuse(path: &Path, contents: &[u8], mode: u32) -> Result<()> {
    if fs::symlink_metadata(path)
        .ok()
        .is_some_and(|metadata| metadata.file_type().is_symlink())
    {
        return Err(anyhow!("refusing to reuse symlink {}", path.display()));
    }
    if fs::read(path).ok().as_deref() == Some(contents) {
        return Ok(());
    }
    if path.exists() {
        return Err(anyhow!(
            "refusing to reuse existing mismatched file {}",
            path.display()
        ));
    }
    match write_atomic_create_new(path, contents, mode) {
        Ok(()) => Ok(()),
        Err(_err) if fs::read(path).ok().as_deref() == Some(contents) => Ok(()),
        Err(err) => Err(err),
    }
}

#[cfg(unix)]
fn validate_existing_ca_key_file(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to stat CA key {}", path.display()))?;
    if metadata.file_type().is_symlink() {
        return Err(anyhow!(
            "refusing to use symlink for managed MITM CA key {}",
            path.display()
        ));
    }
    if !metadata.is_file() {
        return Err(anyhow!(
            "managed MITM CA key is not a regular file: {}",
            path.display()
        ));
    }

    let mode = metadata.permissions().mode() & 0o777;
    if mode & 0o077 != 0 {
        return Err(anyhow!(
            "managed MITM CA key {} must not be group/world accessible (mode={mode:o}; expected <= 600)",
            path.display()
        ));
    }

    Ok(())
}

#[cfg(not(unix))]
fn validate_existing_ca_key_file(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn open_create_new_with_mode(path: &Path, mode: u32) -> Result<File> {
    use std::os::unix::fs::OpenOptionsExt;

    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(path)
        .with_context(|| format!("failed to create {}", path.display()))
}

#[cfg(not(unix))]
fn open_create_new_with_mode(path: &Path, _mode: u32) -> Result<File> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .with_context(|| format!("failed to create {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    use pretty_assertions::assert_eq;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    #[test]
    fn current_generated_trust_bundle_path_rejects_stale_bundle() {
        let dir = tempdir().unwrap();
        let managed_ca_cert_path = dir.path().join("ca.pem");
        let trust_bundle_path = dir.path().join("ca-bundle-123.pem");
        fs::write(&managed_ca_cert_path, "managed ca\n").unwrap();
        fs::write(&trust_bundle_path, "stale managed bundle\n").unwrap();
        assert!(!is_current_generated_trust_bundle_path(
            &trust_bundle_path,
            &managed_ca_cert_path,
        ));
    }

    #[test]
    fn managed_ca_trust_bundle_records_startup_ca_env_values() {
        let dir = tempdir().unwrap();
        let managed_ca_cert_path = dir.path().join("ca.pem");
        fs::write(&managed_ca_cert_path, "managed ca\n").unwrap();
        let env = HashMap::from([
            ("SSL_CERT_FILE", "/tmp/startup-ca.pem".to_string()),
            (SSL_CERT_DIR_ENV_KEY, "/tmp/startup-certs".to_string()),
        ]);
        let trust_bundle =
            managed_ca_trust_bundle_for_cert_path(&managed_ca_cert_path, &env).unwrap();
        assert_eq!(
            trust_bundle.startup_env_values,
            HashMap::from([
                ("SSL_CERT_FILE", "/tmp/startup-ca.pem".to_string()),
                (SSL_CERT_DIR_ENV_KEY, "/tmp/startup-certs".to_string()),
            ])
        );
    }

    #[test]
    fn managed_ca_trust_bundle_does_not_append_startup_ca_override_to_baseline() {
        let dir = tempdir().unwrap();
        let managed_ca_cert_path = dir.path().join("ca.pem");
        let startup_ca_bundle_path = dir.path().join("startup-ca.pem");
        fs::write(&managed_ca_cert_path, "managed ca\n").unwrap();
        fs::write(&startup_ca_bundle_path, "startup ca\n").unwrap();
        let env = HashMap::from([(
            "SSL_CERT_FILE",
            startup_ca_bundle_path.display().to_string(),
        )]);

        let trust_bundle =
            managed_ca_trust_bundle_for_cert_path(&managed_ca_cert_path, &env).unwrap();
        let baseline_bundle = fs::read_to_string(trust_bundle.path).unwrap();

        assert!(!baseline_bundle.contains("startup ca"));
        assert!(baseline_bundle.contains("managed ca"));
    }

    #[cfg(unix)]
    #[test]
    fn validate_existing_ca_key_file_rejects_group_world_permissions() {
        let dir = tempdir().unwrap();
        let key_path = dir.path().join("ca.key");
        fs::write(&key_path, "key").unwrap();
        fs::set_permissions(&key_path, fs::Permissions::from_mode(0o644)).unwrap();

        let err = validate_existing_ca_key_file(&key_path).unwrap_err();
        assert!(
            err.to_string().contains("group/world accessible"),
            "unexpected error: {err:#}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn validate_existing_ca_key_file_rejects_symlink() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().unwrap();
        let target = dir.path().join("real.key");
        let link = dir.path().join("ca.key");
        fs::write(&target, "key").unwrap();
        symlink(&target, &link).unwrap();

        let err = validate_existing_ca_key_file(&link).unwrap_err();
        assert!(
            err.to_string().contains("symlink"),
            "unexpected error: {err:#}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn validate_existing_ca_key_file_allows_private_permissions() {
        let dir = tempdir().unwrap();
        let key_path = dir.path().join("ca.key");
        fs::write(&key_path, "key").unwrap();
        fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600)).unwrap();

        validate_existing_ca_key_file(&key_path).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn write_atomic_create_new_or_reuse_rejects_matching_symlink_target() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().unwrap();
        let target = dir.path().join("real-bundle.pem");
        let link = dir.path().join("ca-bundle.pem");
        fs::write(&target, "bundle").unwrap();
        symlink(&target, &link).unwrap();

        let err = write_atomic_create_new_or_reuse(&link, b"bundle", /*mode*/ 0o644).unwrap_err();

        assert_eq!(
            err.to_string(),
            format!("refusing to reuse symlink {}", link.display())
        );
    }
}
