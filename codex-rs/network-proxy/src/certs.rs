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
use std::collections::HashSet;
use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::Write;
use std::net::IpAddr;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::Mutex;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use tracing::info;
use tracing::warn;

pub(super) struct ManagedMitmCa {
    issuer: Issuer<'static, KeyPair>,
    certificate_path: PathBuf,
    _artifact_lease: File,
}

static MANAGED_MITM_CAS: LazyLock<Mutex<HashMap<PathBuf, Arc<ManagedMitmCa>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

impl ManagedMitmCa {
    pub(super) fn load_or_create() -> Result<Arc<Self>> {
        let proxy_dir = managed_ca_dir()?;
        let mut managed_cas = MANAGED_MITM_CAS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(ca) = managed_cas.get(&proxy_dir) {
            return Ok(ca.clone());
        }

        let ca = Arc::new(Self::create(&proxy_dir)?);
        managed_cas.insert(proxy_dir, ca.clone());
        Ok(ca)
    }

    fn create(proxy_dir: &Path) -> Result<Self> {
        fs::create_dir_all(proxy_dir)
            .with_context(|| format!("failed to create {}", proxy_dir.display()))?;

        let (certificate_pem, private_key) = generate_ca()?;
        let artifact_lock = match lock_managed_ca_artifacts(proxy_dir) {
            Ok(lock) => Some(lock),
            Err(err) => {
                warn!("failed to lock managed MITM CA artifacts; skipping pruning: {err}");
                None
            }
        };
        let certificate_path = persist_managed_ca_certificate(proxy_dir, &certificate_pem)?;
        let issuer = Issuer::from_ca_cert_pem(&certificate_pem, private_key)
            .context("failed to parse managed MITM CA certificate")?;
        let artifact_lease = lock_managed_ca_certificate(&certificate_path)?;
        if artifact_lock.is_some() {
            prune_managed_ca_artifacts(proxy_dir);
        }
        info!(
            cert_path = %certificate_path.display(),
            "generated process-local MITM CA"
        );
        Ok(Self {
            issuer,
            certificate_path,
            _artifact_lease: artifact_lease,
        })
    }

    fn certificate_path(&self) -> &Path {
        &self.certificate_path
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
const MANAGED_MITM_CA_ARTIFACT_LOCK: &str = ".artifacts.lock";
const MANAGED_MITM_CA_CERT_PREFIX: &str = "ca";
const MANAGED_MITM_CA_TRUST_BUNDLE_PREFIX: &str = "ca-bundle";
pub(crate) const SSL_CERT_DIR_ENV_KEY: &str = "SSL_CERT_DIR";

// Best-effort compatibility set for common child toolchains that accept a CA bundle path.
// This is intentionally curated rather than pretending to cover every TLS client.
pub const CUSTOM_CA_ENV_KEYS: [&str; 11] = [
    "CODEX_CA_CERTIFICATE",
    "SSL_CERT_FILE",
    "REQUESTS_CA_BUNDLE",
    "CURL_CA_BUNDLE",
    "NODE_EXTRA_CA_CERTS",
    "GIT_SSL_CAINFO",
    "CARGO_HTTP_CAINFO",
    "PIP_CERT",
    "BUNDLE_SSL_CA_CERT",
    "npm_config_cafile",
    "NPM_CONFIG_CAFILE",
];

pub(crate) fn ca_env_from_process() -> HashMap<&'static str, String> {
    CUSTOM_CA_ENV_KEYS
        .into_iter()
        .chain([SSL_CERT_DIR_ENV_KEY])
        .filter_map(|key| std::env::var(key).ok().map(|value| (key, value)))
        .collect()
}

/// Immutable managed MITM CA bundle path plus startup TLS env values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ManagedMitmCaTrustBundle {
    pub(crate) path: PathBuf,
    pub(crate) startup_env_values: HashMap<&'static str, String>,
}

fn managed_ca_dir() -> Result<PathBuf> {
    let codex_home =
        find_codex_home().context("failed to resolve CODEX_HOME for managed MITM CA")?;
    Ok(codex_home.join(MANAGED_MITM_CA_DIR).to_path_buf())
}

pub(crate) fn managed_ca_trust_bundle(
    env: &HashMap<&'static str, String>,
) -> Result<ManagedMitmCaTrustBundle> {
    let ca = ManagedMitmCa::load_or_create()?;
    managed_ca_trust_bundle_for_cert_path(ca.certificate_path(), env)
}

fn managed_ca_trust_bundle_for_cert_path(
    cert_path: &Path,
    env: &HashMap<&'static str, String>,
) -> Result<ManagedMitmCaTrustBundle> {
    let startup_env_values = startup_ca_file_env_values(env);
    let startup_cert_dir = env
        .get(SSL_CERT_DIR_ENV_KEY)
        .filter(|value| !value.is_empty())
        .map(String::as_str);
    let trust_bundle =
        build_managed_ca_trust_bundle(cert_path, &startup_env_values, startup_cert_dir)?;
    let path = persist_managed_ca_trust_bundle(cert_path, &trust_bundle)?;

    Ok(ManagedMitmCaTrustBundle {
        path,
        startup_env_values,
    })
}

pub(crate) fn upstream_tls_root_store(
    env: &HashMap<&'static str, String>,
) -> Result<Arc<rustls::RootCertStore>> {
    let ca = ManagedMitmCa::load_or_create()?;
    upstream_tls_root_store_for_cert_path(ca.certificate_path(), env)
}

pub(crate) fn upstream_tls_root_store_for_cert_path(
    managed_ca_cert_path: &Path,
    env: &HashMap<&'static str, String>,
) -> Result<Arc<rustls::RootCertStore>> {
    let startup_env_values = startup_ca_file_env_values(env);
    let startup_cert_dir = env
        .get(SSL_CERT_DIR_ENV_KEY)
        .filter(|value| !value.is_empty())
        .map(String::as_str);
    let certificates = load_platform_and_startup_root_certificates(
        managed_ca_cert_path,
        &startup_env_values,
        startup_cert_dir,
    )?;
    let mut roots = rustls::RootCertStore::empty();
    let (_, ignored) = roots.add_parsable_certificates(certificates);
    if ignored > 0 {
        warn!(
            ignored_root_count = ignored,
            "ignored invalid platform or startup roots for MITM upstream TLS"
        );
    }
    Ok(Arc::new(roots))
}

fn startup_ca_file_env_values(
    env: &HashMap<&'static str, String>,
) -> HashMap<&'static str, String> {
    CUSTOM_CA_ENV_KEYS
        .into_iter()
        .filter_map(|key| {
            env.get(key)
                .filter(|value| !value.is_empty())
                .map(|value| (key, value.clone()))
        })
        .collect()
}

fn build_managed_ca_trust_bundle(
    managed_ca_cert_path: &Path,
    startup_env_values: &HashMap<&'static str, String>,
    startup_cert_dir: Option<&str>,
) -> Result<String> {
    let mut trust_bundle = String::new();
    for cert in load_platform_and_startup_root_certificates(
        managed_ca_cert_path,
        startup_env_values,
        startup_cert_dir,
    )? {
        push_certificate_pem(&mut trust_bundle, cert.as_ref());
    }
    append_pem_file(&mut trust_bundle, managed_ca_cert_path)?;
    Ok(trust_bundle)
}

fn load_platform_and_startup_root_certificates(
    managed_ca_cert_path: &Path,
    startup_env_values: &HashMap<&'static str, String>,
    startup_cert_dir: Option<&str>,
) -> Result<Vec<CertificateDer<'static>>> {
    let managed_ca_cert = fs::read(managed_ca_cert_path).with_context(|| {
        format!(
            "failed to read managed MITM CA certificate: {}",
            managed_ca_cert_path.display()
        )
    })?;
    let managed_ca_cert = CertificateDer::from_pem_slice(&managed_ca_cert)
        .context("failed to parse managed MITM CA certificate")?;
    let rustls_native_certs::CertificateResult { certs, errors, .. } =
        crate::native_certs::load_platform_native_certs();
    if !errors.is_empty() {
        warn!(
            native_root_error_count = errors.len(),
            "encountered errors while loading native root certificates for MITM trust bundle"
        );
    }
    let mut certificates = certs;
    let mut appended_startup_paths = HashSet::new();
    for path in CUSTOM_CA_ENV_KEYS
        .into_iter()
        .filter_map(|key| startup_env_values.get(key))
        .map(PathBuf::from)
    {
        if path != managed_ca_cert_path
            && !is_current_generated_trust_bundle_path(&path, managed_ca_cert_path)
            && appended_startup_paths.insert(path.clone())
        {
            certificates.extend(read_ca_certificates(&path)?);
        }
    }
    if let Some(startup_cert_dir) = startup_cert_dir {
        for path in std::env::split_paths(startup_cert_dir) {
            if appended_startup_paths.insert(path.clone()) {
                certificates.extend(load_ca_directory_certificates(&path));
            }
        }
    }
    let mut seen = HashSet::new();
    certificates.retain(|cert| cert != &managed_ca_cert && seen.insert(cert.as_ref().to_vec()));
    Ok(certificates)
}

fn read_ca_certificates(path: &Path) -> Result<Vec<CertificateDer<'static>>> {
    let pem = fs::read(path)
        .with_context(|| format!("failed to read startup CA bundle: {}", path.display()))?;
    let pem = String::from_utf8_lossy(&pem);
    let contains_trusted_certificates = pem.contains("TRUSTED CERTIFICATE");
    let normalized_pem = pem
        .replace("BEGIN TRUSTED CERTIFICATE", "BEGIN CERTIFICATE")
        .replace("END TRUSTED CERTIFICATE", "END CERTIFICATE");
    let certs = CertificateDer::pem_slice_iter(normalized_pem.as_bytes())
        .collect::<std::result::Result<Vec<_>, _>>()
        .with_context(|| format!("failed to parse startup CA bundle: {}", path.display()))?;
    if certs.is_empty() {
        return Err(anyhow!(
            "startup CA bundle contained no certificates: {}",
            path.display()
        ));
    }
    certs
        .into_iter()
        .map(|cert| {
            let cert = if contains_trusted_certificates {
                first_der_item(cert.as_ref()).ok_or_else(|| {
                    anyhow!(
                        "startup CA bundle contained an invalid trusted certificate: {}",
                        path.display()
                    )
                })?
            } else {
                cert.as_ref()
            };
            Ok(CertificateDer::from(cert.to_vec()))
        })
        .collect()
}

fn load_ca_directory_certificates(path: &Path) -> Vec<CertificateDer<'static>> {
    let rustls_native_certs::CertificateResult { certs, errors, .. } =
        rustls_native_certs::load_certs_from_paths(None, Some(path));
    if !errors.is_empty() {
        warn!(
            ca_path = %path.display(),
            ca_error_count = errors.len(),
            "encountered errors while loading startup CA directory"
        );
    }
    certs
}

fn first_der_item(der: &[u8]) -> Option<&[u8]> {
    der_item_length(der).map(|length| &der[..length])
}

fn der_item_length(der: &[u8]) -> Option<usize> {
    let &length_octet = der.get(1)?;
    if length_octet & 0x80 == 0 {
        return Some(2 + usize::from(length_octet)).filter(|length| *length <= der.len());
    }

    let length_octets = usize::from(length_octet & 0x7f);
    if length_octets == 0 {
        return None;
    }

    let length_end = 2usize.checked_add(length_octets)?;
    let mut content_length = 0usize;
    for &byte in der.get(2..length_end)? {
        content_length = content_length
            .checked_mul(256)?
            .checked_add(usize::from(byte))?;
    }
    length_end
        .checked_add(content_length)
        .filter(|length| *length <= der.len())
}

fn is_current_generated_trust_bundle_path(path: &Path, managed_ca_cert_path: &Path) -> bool {
    let Some(proxy_dir) = managed_ca_cert_path.parent() else {
        return false;
    };
    if is_generated_trust_bundle_path(path, proxy_dir) {
        return true;
    }
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

fn is_generated_trust_bundle_path(path: &Path, proxy_dir: &Path) -> bool {
    is_generated_managed_ca_artifact_path(path, proxy_dir, MANAGED_MITM_CA_TRUST_BUNDLE_PREFIX)
}

fn is_generated_managed_ca_artifact_path(path: &Path, proxy_dir: &Path, prefix: &str) -> bool {
    let Some(file_name) = path.file_name().and_then(|file_name| file_name.to_str()) else {
        return false;
    };
    let Some(expected_hash) = file_name
        .strip_prefix(prefix)
        .and_then(|suffix| suffix.strip_prefix('-'))
        .and_then(|suffix| suffix.strip_suffix(".pem"))
    else {
        return false;
    };
    if path.parent() != Some(proxy_dir)
        || expected_hash.len() != 64
        || !expected_hash.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return false;
    }
    let Ok(trust_bundle) = fs::read(path) else {
        return false;
    };
    format!("{:x}", Sha256::digest(trust_bundle)) == expected_hash
}

/// Returns whether `path` points at a current Codex-generated MITM CA bundle.
pub fn is_managed_mitm_ca_trust_bundle_path(path: &str) -> bool {
    let Ok(proxy_dir) = managed_ca_dir() else {
        return false;
    };
    is_generated_trust_bundle_path(Path::new(path), &proxy_dir)
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

fn persist_managed_ca_certificate(proxy_dir: &Path, cert_pem: &str) -> Result<PathBuf> {
    let hash = Sha256::digest(cert_pem.as_bytes());
    let cert_path = proxy_dir.join(format!("{MANAGED_MITM_CA_CERT_PREFIX}-{hash:x}.pem"));
    write_atomic_create_new_or_reuse(&cert_path, cert_pem.as_bytes(), /*mode*/ 0o644)
        .with_context(|| {
            format!(
                "failed to persist managed MITM CA certificate {}",
                cert_path.display()
            )
        })?;
    Ok(cert_path)
}

fn lock_managed_ca_certificate(certificate_path: &Path) -> Result<File> {
    let lock_path = managed_ca_certificate_lock_path(certificate_path)
        .ok_or_else(|| anyhow!("managed MITM CA certificate path is missing a file name"))?;
    let file = open_managed_ca_lock(&lock_path)?;
    file.lock_shared()
        .with_context(|| format!("failed to lock {}", lock_path.display()))?;
    Ok(file)
}

fn lock_managed_ca_artifacts(proxy_dir: &Path) -> Result<File> {
    let lock_path = proxy_dir.join(MANAGED_MITM_CA_ARTIFACT_LOCK);
    let file = open_managed_ca_lock(&lock_path)?;
    file.lock()
        .with_context(|| format!("failed to lock {}", lock_path.display()))?;
    Ok(file)
}

fn managed_ca_certificate_lock_path(certificate_path: &Path) -> Option<PathBuf> {
    let file_name = certificate_path.file_name()?.to_string_lossy();
    Some(certificate_path.with_file_name(format!(".{file_name}.lock")))
}

fn open_managed_ca_lock(path: &Path) -> Result<File> {
    if fs::symlink_metadata(path)
        .ok()
        .is_some_and(|metadata| metadata.file_type().is_symlink())
    {
        return Err(anyhow!(
            "refusing to use symlink lock file {}",
            path.display()
        ));
    }

    #[cfg(unix)]
    use std::os::unix::fs::OpenOptionsExt;

    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    options.mode(0o600);
    options
        .open(path)
        .with_context(|| format!("failed to open {}", path.display()))
}

fn prune_managed_ca_artifacts(proxy_dir: &Path) {
    for certificate_path in
        generated_managed_ca_artifact_paths(proxy_dir, MANAGED_MITM_CA_CERT_PREFIX)
    {
        remove_inactive_managed_ca_certificate(&certificate_path);
    }

    let remaining_certificates =
        generated_managed_ca_artifact_paths(proxy_dir, MANAGED_MITM_CA_CERT_PREFIX)
            .into_iter()
            .filter_map(|path| fs::read(path).ok())
            .filter(|certificate| !certificate.is_empty())
            .collect::<Vec<_>>();
    let bundle_paths =
        generated_managed_ca_artifact_paths(proxy_dir, MANAGED_MITM_CA_TRUST_BUNDLE_PREFIX);
    for bundle_path in bundle_paths {
        let Ok(contents) = fs::read(&bundle_path) else {
            continue;
        };
        if remaining_certificates.iter().any(|certificate| {
            contents
                .windows(certificate.len())
                .any(|window| window == certificate)
        }) {
            continue;
        }
        if let Err(err) = fs::remove_file(&bundle_path)
            && err.kind() != std::io::ErrorKind::NotFound
        {
            warn!(
                path = %bundle_path.display(),
                "failed to prune stale managed MITM CA trust bundle: {err}"
            );
        }
    }
}

fn generated_managed_ca_artifact_paths(proxy_dir: &Path, prefix: &str) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(proxy_dir) else {
        return Vec::new();
    };
    entries
        .filter_map(std::result::Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            if !is_generated_managed_ca_artifact_path(&path, proxy_dir, prefix) {
                return None;
            }
            Some(path)
        })
        .collect()
}

fn remove_inactive_managed_ca_certificate(certificate_path: &Path) {
    let Some(lock_path) = managed_ca_certificate_lock_path(certificate_path) else {
        return;
    };
    let Ok(lock_file) = open_managed_ca_lock(&lock_path) else {
        return;
    };
    match lock_file.try_lock() {
        Ok(()) => {}
        Err(std::fs::TryLockError::WouldBlock) => return,
        Err(err) => {
            warn!(
                path = %lock_path.display(),
                "failed to inspect managed MITM CA artifact lease: {err}"
            );
            return;
        }
    }

    let removed = match fs::remove_file(certificate_path) {
        Ok(()) => true,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => true,
        Err(err) => {
            warn!(
                path = %certificate_path.display(),
                "failed to prune stale managed MITM CA certificate: {err}"
            );
            false
        }
    };
    drop(lock_file);
    if removed
        && let Err(err) = fs::remove_file(&lock_path)
        && err.kind() != std::io::ErrorKind::NotFound
    {
        warn!(
            path = %lock_path.display(),
            "failed to prune stale managed MITM CA artifact lease: {err}"
        );
    }
}

fn generate_ca() -> Result<(String, KeyPair)> {
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
    Ok((cert.pem(), key_pair))
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

    use codex_utils_rustls_provider::ensure_rustls_crypto_provider;
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;

    #[test]
    fn managed_ca_private_key_is_not_persisted() {
        ensure_rustls_crypto_provider();
        let dir = tempdir().unwrap();
        let ca = ManagedMitmCa::create(dir.path()).unwrap();
        ca.tls_acceptor_data_for_host("example.com").unwrap();
        let mut persisted_files = fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        persisted_files.sort();
        let mut expected_files = vec![
            ca.certificate_path().to_path_buf(),
            managed_ca_certificate_lock_path(ca.certificate_path()).unwrap(),
            dir.path().join(MANAGED_MITM_CA_ARTIFACT_LOCK),
        ];
        expected_files.sort();

        assert_eq!(persisted_files, expected_files);
        assert_eq!(
            fs::read(managed_ca_certificate_lock_path(ca.certificate_path()).unwrap()).unwrap(),
            Vec::<u8>::new()
        );
    }

    #[test]
    fn managed_ca_artifact_pruning_preserves_only_active_certificates() {
        let dir = tempdir().unwrap();
        let mut artifacts = Vec::new();
        let mut active_lease = None;
        for index in 0..3 {
            let certificate = format!("certificate {index}\n");
            let certificate_path =
                persist_managed_ca_certificate(dir.path(), &certificate).unwrap();
            let lease = lock_managed_ca_certificate(&certificate_path).unwrap();
            if index == 0 {
                active_lease = Some(lease);
            } else {
                drop(lease);
            }
            let bundle_path = persist_managed_ca_trust_bundle(
                &certificate_path,
                &format!("roots\n{certificate}"),
            )
            .unwrap();
            artifacts.push((certificate_path, bundle_path));
        }
        let unrelated_path = dir.path().join("ca-user.pem");
        fs::write(&unrelated_path, "user managed").unwrap();

        prune_managed_ca_artifacts(dir.path());

        let remaining_certificate_count =
            generated_managed_ca_artifact_paths(dir.path(), MANAGED_MITM_CA_CERT_PREFIX).len();
        assert_eq!(remaining_certificate_count, 1);
        assert!(artifacts[0].0.exists());
        assert!(artifacts[0].1.exists());
        assert!(!artifacts[1].0.exists());
        assert!(!artifacts[1].1.exists());
        assert!(!artifacts[2].0.exists());
        assert!(!artifacts[2].1.exists());
        assert!(unrelated_path.exists());

        drop(active_lease.take());
        prune_managed_ca_artifacts(dir.path());

        let remaining_certificates =
            generated_managed_ca_artifact_paths(dir.path(), MANAGED_MITM_CA_CERT_PREFIX);
        assert!(remaining_certificates.is_empty());
        assert!(!artifacts[0].0.exists());
        assert!(!artifacts[0].1.exists());
    }

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
    fn generated_trust_bundle_path_requires_matching_content_hash() {
        let dir = tempdir().unwrap();
        let managed_ca_cert_path = dir.path().join("ca.pem");
        let trust_bundle_path =
            persist_managed_ca_trust_bundle(&managed_ca_cert_path, "trusted roots").unwrap();

        assert!(is_generated_trust_bundle_path(
            &trust_bundle_path,
            dir.path()
        ));
        fs::write(&trust_bundle_path, "tampered roots").unwrap();
        assert!(!is_generated_trust_bundle_path(
            &trust_bundle_path,
            dir.path()
        ));
    }

    #[test]
    fn managed_ca_trust_bundle_appends_startup_file_and_directory_certificates() {
        let dir = tempdir().unwrap();
        let managed_ca_cert_path = dir.path().join("ca.pem");
        let startup_ca_bundle_path = dir.path().join("startup-ca.pem");
        let startup_ca_dir = dir.path().join("startup-certs");
        let (managed_ca_cert, _) = generate_ca().unwrap();
        let (startup_ca_cert, startup_ca_key) = generate_ca().unwrap();
        let startup_ca_key = startup_ca_key.serialize_pem();
        let (directory_ca_cert, _) = generate_ca().unwrap();
        let mut trusted_ca_der = CertificateDer::from_pem_slice(startup_ca_cert.as_bytes())
            .unwrap()
            .as_ref()
            .to_vec();
        trusted_ca_der.extend_from_slice(&[0x30, 0x00]);
        let mut trusted_ca_cert = String::new();
        push_certificate_pem(&mut trusted_ca_cert, &trusted_ca_der);
        let trusted_ca_cert = trusted_ca_cert.replace("CERTIFICATE", "TRUSTED CERTIFICATE");
        fs::write(&managed_ca_cert_path, &managed_ca_cert).unwrap();
        fs::write(
            &startup_ca_bundle_path,
            format!("{trusted_ca_cert}{startup_ca_key}"),
        )
        .unwrap();
        fs::create_dir(&startup_ca_dir).unwrap();
        fs::write(startup_ca_dir.join("directory-ca.pem"), &directory_ca_cert).unwrap();
        let startup_ca_bundle_path = startup_ca_bundle_path.display().to_string();
        let env = HashMap::from([
            ("SSL_CERT_FILE", startup_ca_bundle_path.clone()),
            (SSL_CERT_DIR_ENV_KEY, startup_ca_dir.display().to_string()),
        ]);

        let trust_bundle =
            managed_ca_trust_bundle_for_cert_path(&managed_ca_cert_path, &env).unwrap();
        assert_eq!(
            trust_bundle.startup_env_values,
            HashMap::from([("SSL_CERT_FILE", startup_ca_bundle_path)])
        );
        let baseline_bundle = fs::read_to_string(&trust_bundle.path).unwrap();
        let baseline_certs = CertificateDer::pem_slice_iter(baseline_bundle.as_bytes())
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        let expected_certs = [&startup_ca_cert, &directory_ca_cert, &managed_ca_cert]
            .map(|cert| CertificateDer::from_pem_slice(cert.as_bytes()).unwrap());

        assert!(
            expected_certs
                .iter()
                .all(|cert| baseline_certs.contains(cert))
        );
        assert!(!baseline_bundle.contains(&startup_ca_key));
        assert!(!baseline_bundle.contains("TRUSTED CERTIFICATE"));
    }

    #[test]
    fn managed_ca_trust_bundle_skips_inherited_current_bundle() {
        let dir = tempdir().unwrap();
        let managed_ca_cert_path = dir.path().join("ca.pem");
        let inherited_bundle_path = dir.path().join("ca-bundle-parent.pem");
        let (managed_ca_cert, _) = generate_ca().unwrap();
        fs::write(&managed_ca_cert_path, &managed_ca_cert).unwrap();
        fs::write(
            &inherited_bundle_path,
            format!("parent roots\n{managed_ca_cert}"),
        )
        .unwrap();
        let env = HashMap::from([(
            "REQUESTS_CA_BUNDLE",
            inherited_bundle_path.display().to_string(),
        )]);

        let trust_bundle =
            managed_ca_trust_bundle_for_cert_path(&managed_ca_cert_path, &env).unwrap();
        let baseline_bundle = fs::read_to_string(&trust_bundle.path).unwrap();

        assert_eq!(baseline_bundle.matches(&managed_ca_cert).count(), 1);
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
