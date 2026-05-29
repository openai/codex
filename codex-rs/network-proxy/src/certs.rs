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
use std::ffi::OsString;
use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::Read as _;
use std::io::Write;
use std::net::IpAddr;
use std::path::Path;
use std::path::PathBuf;
use std::sync::LazyLock;
use std::sync::Mutex;
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
const MAX_EXTERNAL_CA_BUNDLE_BYTES: u64 = 1024 * 1024;
const CODEX_CA_CERTIFICATE_ENV_KEY: &str = "CODEX_CA_CERTIFICATE";
static NATIVE_CERT_ENV_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

// Best-effort compatibility set for common child toolchains that accept a CA bundle path.
// This is intentionally curated rather than pretending to cover every TLS client.
pub(crate) const CUSTOM_CA_ENV_KEYS: [&str; 10] = [
    CODEX_CA_CERTIFICATE_ENV_KEY,
    "SSL_CERT_FILE",
    "REQUESTS_CA_BUNDLE",
    "CURL_CA_BUNDLE",
    "NODE_EXTRA_CA_CERTS",
    "GIT_SSL_CAINFO",
    "PIP_CERT",
    "BUNDLE_SSL_CA_CERT",
    "npm_config_cafile",
    "NPM_CONFIG_CAFILE",
];

/// Immutable managed MITM CA bundle paths keyed by child TLS env variable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ManagedMitmCaTrustBundles {
    pub(crate) managed_ca_cert_path: PathBuf,
    pub(crate) default_path: PathBuf,
}

impl ManagedMitmCaTrustBundles {
    pub(crate) fn apply_to_env(&self, env: &mut HashMap<String, String>) {
        let has_non_codex_ca_override = CUSTOM_CA_ENV_KEYS.iter().any(|key| {
            *key != CODEX_CA_CERTIFICATE_ENV_KEY
                && env.get(*key).is_some_and(|value| !value.is_empty())
        });
        for key in CUSTOM_CA_ENV_KEYS {
            if env.get(key).is_some_and(|value| !value.is_empty()) {
                continue;
            }
            if key == CODEX_CA_CERTIFICATE_ENV_KEY && has_non_codex_ca_override {
                continue;
            }
            env.insert(
                key.to_string(),
                self.default_path.to_string_lossy().into_owned(),
            );
        }
    }

    pub(crate) fn prepare_child_env<F>(
        &self,
        env: &mut HashMap<String, String>,
        cwd: &Path,
        can_read_path: F,
    ) -> Vec<PathBuf>
    where
        F: Fn(&Path) -> bool,
    {
        self.apply_to_env(env);
        for key in CUSTOM_CA_ENV_KEYS {
            let Some(value) = env.get(key).filter(|value| !value.is_empty()) else {
                continue;
            };
            if self
                .managed_path_for_env_value(Some(value.as_str()))
                .is_some()
            {
                continue;
            }

            let custom_ca_path = resolve_ca_bundle_path(value, cwd);
            if !can_read_path(&custom_ca_path) {
                continue;
            }
            let custom_ca_path = match custom_ca_path.canonicalize() {
                Ok(custom_ca_path) => custom_ca_path,
                Err(err) => {
                    warn!(
                        path = %custom_ca_path.display(),
                        "failed to resolve command-scoped CA bundle; preserving original override: {err}"
                    );
                    continue;
                }
            };
            if !can_read_path(&custom_ca_path) {
                continue;
            }

            let trust_bundle = match build_managed_ca_trust_bundle_for_required_path(
                self.managed_ca_cert_path.as_path(),
                &custom_ca_path,
            ) {
                Ok(trust_bundle) => trust_bundle,
                Err(err) => {
                    warn!(
                        path = %custom_ca_path.display(),
                        "failed to append command-scoped CA bundle; preserving original override: {err}"
                    );
                    continue;
                }
            };
            let trust_bundle_path = match persist_managed_ca_trust_bundle(
                self.managed_ca_cert_path.as_path(),
                &trust_bundle,
            ) {
                Ok(trust_bundle_path) => trust_bundle_path,
                Err(err) => {
                    warn!(
                        path = %custom_ca_path.display(),
                        "failed to persist command-scoped CA bundle; preserving original override: {err}"
                    );
                    continue;
                }
            };
            env.insert(
                key.to_string(),
                trust_bundle_path.to_string_lossy().into_owned(),
            );
        }

        self.bundle_paths_for_env(env)
    }

    fn bundle_paths_for_env(&self, env: &HashMap<String, String>) -> Vec<PathBuf> {
        let mut paths = CUSTOM_CA_ENV_KEYS
            .iter()
            .filter_map(|key| {
                env.get(*key)
                    .and_then(|value| self.managed_path_for_env_value(Some(value)))
            })
            .collect::<Vec<_>>();
        paths.sort();
        paths.dedup();
        paths
    }

    fn managed_path_for_env_value(&self, value: Option<&str>) -> Option<PathBuf> {
        let Some(value) = value.filter(|value| !value.is_empty()) else {
            return Some(self.default_path.clone());
        };
        let path = PathBuf::from(value);
        self.is_current_generated_trust_bundle_path(&path)
            .then_some(path)
    }

    fn is_generated_trust_bundle_path(&self, path: &Path) -> bool {
        self.default_path.parent().is_some_and(|proxy_dir| {
            let Some(file_name) = path.file_name().and_then(|file_name| file_name.to_str()) else {
                return false;
            };
            path.parent() == Some(proxy_dir)
                && file_name.starts_with(MANAGED_MITM_CA_TRUST_BUNDLE_PREFIX)
                && file_name.ends_with(".pem")
        })
    }

    fn is_current_generated_trust_bundle_path(&self, path: &Path) -> bool {
        if !self.is_generated_trust_bundle_path(path) {
            return false;
        }
        match generated_trust_bundle_contains_managed_ca(path, &self.managed_ca_cert_path) {
            Ok(is_current) => is_current,
            Err(err) => {
                warn!(
                    path = %path.display(),
                    "failed to validate generated CA bundle; rebuilding it: {err}"
                );
                false
            }
        }
    }
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

pub(crate) fn managed_ca_trust_bundles(
    env: &HashMap<String, String>,
    cwd: &Path,
) -> Result<ManagedMitmCaTrustBundles> {
    let (cert_path, _) = managed_ca_paths()?;
    managed_ca_trust_bundles_for_cert_path(&cert_path, env, cwd)
}

fn managed_ca_trust_bundles_for_cert_path(
    cert_path: &Path,
    env: &HashMap<String, String>,
    cwd: &Path,
) -> Result<ManagedMitmCaTrustBundles> {
    let inherited_ssl_cert_file = env.get("SSL_CERT_FILE").map(String::as_str);
    let current_ssl_cert_file =
        current_generated_trust_bundle_path(inherited_ssl_cert_file, cert_path, cwd)?;
    let stale_generated_ssl_cert_file = inherited_ssl_cert_file.is_some_and(|value| {
        current_ssl_cert_file.is_none()
            && is_generated_trust_bundle_path(&resolve_ca_bundle_path(value, cwd), cert_path)
    });
    let default_path = current_ssl_cert_file.map_or_else(
        || {
            let native_root_source = if stale_generated_ssl_cert_file {
                NativeRootSource::IgnoreStaleGeneratedSslCertFile
            } else {
                NativeRootSource::ProcessEnvironment
            };
            let trust_bundle =
                build_default_managed_ca_trust_bundle(cert_path, native_root_source)?;
            persist_managed_ca_trust_bundle(cert_path, &trust_bundle)
        },
        Ok,
    )?;

    Ok(ManagedMitmCaTrustBundles {
        managed_ca_cert_path: cert_path.to_path_buf(),
        default_path,
    })
}

#[derive(Clone, Copy)]
enum NativeRootSource {
    ProcessEnvironment,
    IgnoreStaleGeneratedSslCertFile,
}

fn build_default_managed_ca_trust_bundle(
    managed_ca_cert_path: &Path,
    native_root_source: NativeRootSource,
) -> Result<String> {
    let mut trust_bundle = String::new();
    let rustls_native_certs::CertificateResult { certs, errors, .. } =
        load_native_certs(native_root_source);
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

fn load_native_certs(
    native_root_source: NativeRootSource,
) -> rustls_native_certs::CertificateResult {
    match native_root_source {
        NativeRootSource::ProcessEnvironment => rustls_native_certs::load_native_certs(),
        NativeRootSource::IgnoreStaleGeneratedSslCertFile => {
            let _env_lock = NATIVE_CERT_ENV_LOCK
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let _ssl_cert_file = RemovedEnvVar::remove("SSL_CERT_FILE");
            rustls_native_certs::load_native_certs()
        }
    }
}

struct RemovedEnvVar {
    key: &'static str,
    value: Option<OsString>,
}

impl RemovedEnvVar {
    fn remove(key: &'static str) -> Self {
        let value = std::env::var_os(key);
        unsafe {
            std::env::remove_var(key);
        }
        Self { key, value }
    }
}

impl Drop for RemovedEnvVar {
    fn drop(&mut self) {
        if let Some(value) = self.value.as_ref() {
            unsafe {
                std::env::set_var(self.key, value);
            }
        }
    }
}

fn build_managed_ca_trust_bundle_for_required_path(
    managed_ca_cert_path: &Path,
    custom_ca_path: &Path,
) -> Result<String> {
    let mut trust_bundle = String::new();
    if custom_ca_path != managed_ca_cert_path {
        append_bounded_pem_file(
            &mut trust_bundle,
            custom_ca_path,
            "command-scoped CA bundle",
        )?;
    }
    append_pem_file(&mut trust_bundle, managed_ca_cert_path)?;
    Ok(trust_bundle)
}

fn current_generated_trust_bundle_path(
    value: Option<&str>,
    managed_ca_cert_path: &Path,
    cwd: &Path,
) -> Result<Option<PathBuf>> {
    let Some(path) = value.map(|value| resolve_ca_bundle_path(value, cwd)) else {
        return Ok(None);
    };
    if !is_generated_trust_bundle_path(&path, managed_ca_cert_path) {
        return Ok(None);
    }
    if !generated_trust_bundle_contains_managed_ca(&path, managed_ca_cert_path)? {
        return Ok(None);
    }

    Ok(Some(path))
}

fn is_generated_trust_bundle_path(path: &Path, managed_ca_cert_path: &Path) -> bool {
    let Some(proxy_dir) = managed_ca_cert_path.parent() else {
        return false;
    };
    let Some(file_name) = path.file_name().and_then(|file_name| file_name.to_str()) else {
        return false;
    };
    path.parent() == Some(proxy_dir)
        && file_name.starts_with(MANAGED_MITM_CA_TRUST_BUNDLE_PREFIX)
        && file_name.ends_with(".pem")
}

pub(crate) fn is_managed_mitm_ca_trust_bundle_path(path: &Path) -> bool {
    let Some(parent_name) = path.parent().and_then(Path::file_name) else {
        return false;
    };
    let Some(file_name) = path.file_name().and_then(|file_name| file_name.to_str()) else {
        return false;
    };

    parent_name == std::ffi::OsStr::new(MANAGED_MITM_CA_DIR)
        && file_name.starts_with(MANAGED_MITM_CA_TRUST_BUNDLE_PREFIX)
        && file_name.ends_with(".pem")
}

fn generated_trust_bundle_contains_managed_ca(
    trust_bundle_path: &Path,
    managed_ca_cert_path: &Path,
) -> Result<bool> {
    let managed_ca = fs::read_to_string(managed_ca_cert_path).with_context(|| {
        format!(
            "failed to read CA bundle {}",
            managed_ca_cert_path.display()
        )
    })?;
    let trust_bundle = match read_bounded_pem_file(trust_bundle_path, "managed CA trust bundle") {
        Ok(trust_bundle) => trust_bundle,
        Err(err) => {
            warn!(
                path = %trust_bundle_path.display(),
                "failed to validate inherited generated CA bundle; rebuilding it: {err}"
            );
            return Ok(false);
        }
    };

    Ok(trust_bundle.contains(&managed_ca))
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

fn resolve_ca_bundle_path(path: &str, cwd: &Path) -> PathBuf {
    let path = Path::new(path);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

fn append_pem_file(bundle: &mut String, path: &Path) -> Result<()> {
    let pem = fs::read_to_string(path)
        .with_context(|| format!("failed to read CA bundle {}", path.display()))?;
    append_pem_contents(bundle, &pem);
    Ok(())
}

fn append_bounded_pem_file(bundle: &mut String, path: &Path, kind: &str) -> Result<()> {
    let pem = read_bounded_pem_file(path, kind)?;
    append_pem_contents(bundle, &pem);
    Ok(())
}

fn read_bounded_pem_file(path: &Path, kind: &str) -> Result<String> {
    let metadata = fs::metadata(path)
        .with_context(|| format!("failed to inspect {kind} {}", path.display()))?;
    if !metadata.is_file() {
        return Err(anyhow!("{kind} must be a regular file: {}", path.display()));
    }
    if metadata.len() > MAX_EXTERNAL_CA_BUNDLE_BYTES {
        return Err(anyhow!(
            "{kind} exceeds {} bytes: {}",
            MAX_EXTERNAL_CA_BUNDLE_BYTES,
            path.display()
        ));
    }

    let mut pem = String::new();
    File::open(path)
        .with_context(|| format!("failed to open {kind} {}", path.display()))?
        .take(MAX_EXTERNAL_CA_BUNDLE_BYTES + 1)
        .read_to_string(&mut pem)
        .with_context(|| format!("failed to read {kind} {}", path.display()))?;
    if pem.len() as u64 > MAX_EXTERNAL_CA_BUNDLE_BYTES {
        return Err(anyhow!(
            "{kind} exceeds {} bytes: {}",
            MAX_EXTERNAL_CA_BUNDLE_BYTES,
            path.display()
        ));
    }

    Ok(pem)
}

fn append_pem_contents(bundle: &mut String, pem: &str) {
    if !bundle.ends_with('\n') {
        bundle.push('\n');
    }
    bundle.push_str(pem);
    if !bundle.ends_with('\n') {
        bundle.push('\n');
    }
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

    // Best-effort durability: ensure the directory entry is persisted too.
    let dir = File::open(parent).with_context(|| format!("failed to open {}", parent.display()))?;
    dir.sync_all()
        .with_context(|| format!("failed to fsync {}", parent.display()))?;

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
    fn managed_ca_trust_bundles_rebuild_stale_generated_default_bundle() {
        let dir = tempdir().unwrap();
        let managed_ca_cert_path = dir.path().join("ca.pem");
        let stale_trust_bundle_path = dir.path().join("ca-bundle-stale.pem");
        fs::write(&managed_ca_cert_path, "managed ca\n").unwrap();
        fs::write(&stale_trust_bundle_path, "stale managed bundle\n").unwrap();
        let env = HashMap::from([(
            "SSL_CERT_FILE".to_string(),
            stale_trust_bundle_path.display().to_string(),
        )]);

        let bundles =
            managed_ca_trust_bundles_for_cert_path(&managed_ca_cert_path, &env, dir.path())
                .unwrap();
        let default_bundle = fs::read_to_string(&bundles.default_path).unwrap();

        assert_ne!(bundles.default_path, stale_trust_bundle_path);
        assert!(default_bundle.contains("managed ca"));
    }

    #[test]
    fn managed_ca_trust_bundles_keep_distinct_client_roots_separate() {
        let dir = tempdir().unwrap();
        let managed_ca_cert_path = dir.path().join("ca.pem");
        let requests_bundle_path = dir.path().join("requests.pem");
        let curl_bundle_path = dir.path().join("curl.pem");
        fs::write(&managed_ca_cert_path, "managed ca\n").unwrap();
        fs::write(&requests_bundle_path, "requests ca\n").unwrap();
        fs::write(&curl_bundle_path, "curl ca\n").unwrap();
        let env = HashMap::from([
            (
                "REQUESTS_CA_BUNDLE".to_string(),
                requests_bundle_path.display().to_string(),
            ),
            (
                "CURL_CA_BUNDLE".to_string(),
                curl_bundle_path.display().to_string(),
            ),
        ]);

        let bundles =
            managed_ca_trust_bundles_for_cert_path(&managed_ca_cert_path, &env, dir.path())
                .unwrap();
        let mut child_env = env;
        bundles.prepare_child_env(&mut child_env, dir.path(), |_| true);
        let requests_bundle =
            fs::read_to_string(child_env.get("REQUESTS_CA_BUNDLE").unwrap()).unwrap();
        let curl_bundle = fs::read_to_string(child_env.get("CURL_CA_BUNDLE").unwrap()).unwrap();

        assert!(requests_bundle.contains("requests ca"));
        assert!(!requests_bundle.contains("curl ca"));
        assert!(curl_bundle.contains("curl ca"));
        assert!(!curl_bundle.contains("requests ca"));
    }

    #[test]
    fn managed_ca_trust_bundles_keep_single_replacement_root_separate_from_default() {
        let dir = tempdir().unwrap();
        let managed_ca_cert_path = dir.path().join("ca.pem");
        let requests_bundle_path = dir.path().join("requests.pem");
        fs::write(&managed_ca_cert_path, "managed ca\n").unwrap();
        fs::write(&requests_bundle_path, "requests ca\n").unwrap();
        let env = HashMap::from([(
            "REQUESTS_CA_BUNDLE".to_string(),
            requests_bundle_path.display().to_string(),
        )]);

        let bundles =
            managed_ca_trust_bundles_for_cert_path(&managed_ca_cert_path, &env, dir.path())
                .unwrap();
        let mut child_env = env;
        bundles.prepare_child_env(&mut child_env, dir.path(), |_| true);
        let requests_bundle =
            fs::read_to_string(child_env.get("REQUESTS_CA_BUNDLE").unwrap()).unwrap();

        assert!(requests_bundle.contains("requests ca"));
        assert!(requests_bundle.contains("managed ca"));
        assert!(!requests_bundle.contains("-----BEGIN CERTIFICATE-----"));
    }

    #[test]
    fn managed_ca_trust_bundles_prepare_inherited_relative_override_against_child_cwd() {
        let dir = tempdir().unwrap();
        let startup_cwd = dir.path().join("startup");
        let child_cwd = dir.path().join("child");
        let managed_ca_cert_path = dir.path().join("ca.pem");
        let inherited_bundle_path = child_cwd.join("certs/inherited.pem");
        fs::create_dir_all(&startup_cwd).unwrap();
        fs::create_dir_all(inherited_bundle_path.parent().unwrap()).unwrap();
        fs::write(&managed_ca_cert_path, "managed ca\n").unwrap();
        fs::write(&inherited_bundle_path, "inherited ca\n").unwrap();
        let inherited_bundle_path = "certs/inherited.pem".to_string();
        let env = HashMap::from([(
            "REQUESTS_CA_BUNDLE".to_string(),
            inherited_bundle_path.clone(),
        )]);
        let bundles =
            managed_ca_trust_bundles_for_cert_path(&managed_ca_cert_path, &env, &startup_cwd)
                .unwrap();
        let mut child_env =
            HashMap::from([("REQUESTS_CA_BUNDLE".to_string(), inherited_bundle_path)]);

        bundles.prepare_child_env(&mut child_env, &child_cwd, |_| true);
        let requests_bundle =
            fs::read_to_string(child_env.get("REQUESTS_CA_BUNDLE").unwrap()).unwrap();

        assert!(requests_bundle.contains("inherited ca"));
        assert!(requests_bundle.contains("managed ca"));
    }

    #[test]
    fn managed_ca_trust_bundles_prepare_readable_command_scoped_override() {
        let dir = tempdir().unwrap();
        let managed_ca_cert_path = dir.path().join("ca.pem");
        fs::write(&managed_ca_cert_path, "managed ca\n").unwrap();
        let inherited_bundle_path = dir.path().join("inherited.pem");
        fs::write(&inherited_bundle_path, "inherited ca\n").unwrap();
        let command_bundle_path = dir.path().join("command.pem");
        fs::write(&command_bundle_path, "command ca\n").unwrap();
        let env = HashMap::from([(
            "REQUESTS_CA_BUNDLE".to_string(),
            inherited_bundle_path.display().to_string(),
        )]);
        let bundles =
            managed_ca_trust_bundles_for_cert_path(&managed_ca_cert_path, &env, dir.path())
                .unwrap();
        let mut child_env = HashMap::from([(
            "REQUESTS_CA_BUNDLE".to_string(),
            command_bundle_path.display().to_string(),
        )]);

        let bundle_paths = bundles.prepare_child_env(&mut child_env, dir.path(), |_| true);
        let requests_bundle_path = child_env.get("REQUESTS_CA_BUNDLE").unwrap();
        let requests_bundle = fs::read_to_string(requests_bundle_path).unwrap();

        assert!(requests_bundle.contains("command ca"));
        assert!(requests_bundle.contains("managed ca"));
        assert!(!requests_bundle.contains("inherited ca"));
        assert!(bundle_paths.contains(&PathBuf::from(requests_bundle_path)));
    }

    #[test]
    fn managed_ca_trust_bundles_preserve_unreadable_inherited_override() {
        let dir = tempdir().unwrap();
        let managed_ca_cert_path = dir.path().join("ca.pem");
        fs::write(&managed_ca_cert_path, "managed ca\n").unwrap();
        let inherited_bundle_path = dir.path().join("inherited.pem");
        fs::write(&inherited_bundle_path, "inherited ca\n").unwrap();
        let inherited_bundle_path = inherited_bundle_path.display().to_string();
        let env = HashMap::from([(
            "REQUESTS_CA_BUNDLE".to_string(),
            inherited_bundle_path.clone(),
        )]);
        let bundles =
            managed_ca_trust_bundles_for_cert_path(&managed_ca_cert_path, &env, dir.path())
                .unwrap();
        let mut child_env = env;

        let bundle_paths = bundles.prepare_child_env(&mut child_env, dir.path(), |_| false);

        assert_eq!(
            child_env.get("REQUESTS_CA_BUNDLE"),
            Some(&inherited_bundle_path)
        );
        assert!(!bundle_paths.contains(&PathBuf::from(inherited_bundle_path)));
    }

    #[test]
    fn managed_ca_trust_bundles_preserve_unreadable_command_scoped_override() {
        let dir = tempdir().unwrap();
        let managed_ca_cert_path = dir.path().join("ca.pem");
        fs::write(&managed_ca_cert_path, "managed ca\n").unwrap();
        let command_bundle_path = dir.path().join("command.pem");
        fs::write(&command_bundle_path, "command ca\n").unwrap();
        let bundles = managed_ca_trust_bundles_for_cert_path(
            &managed_ca_cert_path,
            &HashMap::new(),
            dir.path(),
        )
        .unwrap();
        let command_bundle_path = command_bundle_path.display().to_string();
        let mut child_env = HashMap::from([(
            "REQUESTS_CA_BUNDLE".to_string(),
            command_bundle_path.clone(),
        )]);

        let bundle_paths = bundles.prepare_child_env(&mut child_env, dir.path(), |_| false);

        assert_eq!(
            child_env.get("REQUESTS_CA_BUNDLE"),
            Some(&command_bundle_path)
        );
        assert!(!bundle_paths.contains(&PathBuf::from(command_bundle_path)));
    }

    #[test]
    fn managed_ca_trust_bundles_preserve_ssl_cert_file_precedence_for_nested_codex() {
        let dir = tempdir().unwrap();
        let managed_ca_cert_path = dir.path().join("ca.pem");
        fs::write(&managed_ca_cert_path, "managed ca\n").unwrap();
        let ssl_cert_file_path = dir.path().join("corp.pem");
        fs::write(&ssl_cert_file_path, "corp ca\n").unwrap();
        let bundles = managed_ca_trust_bundles_for_cert_path(
            &managed_ca_cert_path,
            &HashMap::new(),
            dir.path(),
        )
        .unwrap();
        let mut child_env = HashMap::from([(
            "SSL_CERT_FILE".to_string(),
            ssl_cert_file_path.display().to_string(),
        )]);

        bundles.prepare_child_env(&mut child_env, dir.path(), |_| true);

        assert!(child_env.contains_key("SSL_CERT_FILE"));
        assert!(!child_env.contains_key(CODEX_CA_CERTIFICATE_ENV_KEY));
    }

    #[cfg(unix)]
    #[test]
    fn managed_ca_trust_bundles_preserve_symlinked_command_scoped_override_to_unreadable_target() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().unwrap();
        let managed_ca_cert_path = dir.path().join("ca.pem");
        fs::write(&managed_ca_cert_path, "managed ca\n").unwrap();
        let command_bundle_path = dir.path().join("command.pem");
        fs::write(&command_bundle_path, "command ca\n").unwrap();
        let symlinked_bundle_path = dir.path().join("symlinked.pem");
        symlink(&command_bundle_path, &symlinked_bundle_path).unwrap();
        let canonical_command_bundle_path = command_bundle_path.canonicalize().unwrap();
        let bundles = managed_ca_trust_bundles_for_cert_path(
            &managed_ca_cert_path,
            &HashMap::new(),
            dir.path(),
        )
        .unwrap();
        let symlinked_bundle_path = symlinked_bundle_path.display().to_string();
        let mut child_env = HashMap::from([(
            "REQUESTS_CA_BUNDLE".to_string(),
            symlinked_bundle_path.clone(),
        )]);

        let bundle_paths = bundles.prepare_child_env(&mut child_env, dir.path(), |path| {
            path != canonical_command_bundle_path
        });

        assert_eq!(
            child_env.get("REQUESTS_CA_BUNDLE"),
            Some(&symlinked_bundle_path)
        );
        assert!(!bundle_paths.contains(&PathBuf::from(symlinked_bundle_path)));
    }

    #[test]
    fn managed_ca_trust_bundles_preserve_oversized_command_scoped_override() {
        let dir = tempdir().unwrap();
        let managed_ca_cert_path = dir.path().join("ca.pem");
        fs::write(&managed_ca_cert_path, "managed ca\n").unwrap();
        let command_bundle_path = dir.path().join("command.pem");
        fs::write(
            &command_bundle_path,
            vec![b'a'; MAX_EXTERNAL_CA_BUNDLE_BYTES as usize + 1],
        )
        .unwrap();
        let bundles = managed_ca_trust_bundles_for_cert_path(
            &managed_ca_cert_path,
            &HashMap::new(),
            dir.path(),
        )
        .unwrap();
        let command_bundle_path = command_bundle_path.display().to_string();
        let mut child_env = HashMap::from([(
            "REQUESTS_CA_BUNDLE".to_string(),
            command_bundle_path.clone(),
        )]);

        let bundle_paths = bundles.prepare_child_env(&mut child_env, dir.path(), |_| true);

        assert_eq!(
            child_env.get("REQUESTS_CA_BUNDLE"),
            Some(&command_bundle_path)
        );
        assert!(!bundle_paths.contains(&PathBuf::from(command_bundle_path)));
    }

    #[test]
    fn managed_ca_trust_bundles_preserve_non_file_command_scoped_override() {
        let dir = tempdir().unwrap();
        let managed_ca_cert_path = dir.path().join("ca.pem");
        fs::write(&managed_ca_cert_path, "managed ca\n").unwrap();
        let command_bundle_path = dir.path().join("command-dir");
        fs::create_dir(&command_bundle_path).unwrap();
        let bundles = managed_ca_trust_bundles_for_cert_path(
            &managed_ca_cert_path,
            &HashMap::new(),
            dir.path(),
        )
        .unwrap();
        let command_bundle_path = command_bundle_path.display().to_string();
        let mut child_env = HashMap::from([(
            "REQUESTS_CA_BUNDLE".to_string(),
            command_bundle_path.clone(),
        )]);

        let bundle_paths = bundles.prepare_child_env(&mut child_env, dir.path(), |_| true);

        assert_eq!(
            child_env.get("REQUESTS_CA_BUNDLE"),
            Some(&command_bundle_path)
        );
        assert!(!bundle_paths.contains(&PathBuf::from(command_bundle_path)));
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
