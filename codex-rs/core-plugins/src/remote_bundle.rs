use crate::store::PluginInstallResult;
use crate::store::PluginStore;
use crate::store::PluginStoreError;
use crate::store::validate_plugin_version_segment;
use codex_login::default_client::build_reqwest_client;
use codex_plugin::PluginId;
use codex_plugin::PluginIdError;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_plugins::find_plugin_manifest_path;
use flate2::read::GzDecoder;
use reqwest::Response;
use reqwest::StatusCode;
use std::fs;
use std::io;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use tar::Archive;
use url::Host;
use url::Url;

const REMOTE_PLUGIN_BUNDLE_DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(60);
const REMOTE_PLUGIN_BUNDLE_MAX_DOWNLOAD_BYTES: u64 = 50 * 1024 * 1024;
const REMOTE_PLUGIN_BUNDLE_ERROR_BODY_MAX_BYTES: u64 = 8 * 1024;
const REMOTE_PLUGIN_BUNDLE_MAX_EXTRACTED_BYTES: u64 = 250 * 1024 * 1024;
const REMOTE_PLUGIN_INSTALL_STAGING_DIR: &str = "plugins/.remote-plugin-install-staging";
#[cfg(debug_assertions)]
const TEST_ALLOW_LOOPBACK_HTTP_REMOTE_PLUGIN_BUNDLES_ENV: &str =
    "CODEX_TEST_ALLOW_HTTP_REMOTE_PLUGIN_BUNDLE_DOWNLOADS";

#[derive(Debug, Clone)]
pub struct ValidatedRemotePluginBundle {
    pub plugin_id: PluginId,
    pub plugin_version: String,
    bundle_download_url: String,
}

#[derive(Debug, thiserror::Error)]
pub enum RemotePluginBundleInstallError {
    #[error("backend did not return a release version for remote plugin `{remote_plugin_id}`")]
    MissingReleaseVersion { remote_plugin_id: String },

    #[error(
        "backend returned an invalid release version for remote plugin `{remote_plugin_id}`: {message}"
    )]
    InvalidReleaseVersion {
        remote_plugin_id: String,
        message: String,
    },

    #[error("backend did not return a download URL for remote plugin `{remote_plugin_id}`")]
    MissingBundleDownloadUrl { remote_plugin_id: String },

    #[error(
        "backend returned an invalid download URL for remote plugin `{remote_plugin_id}`: {url}"
    )]
    InvalidBundleDownloadUrl {
        remote_plugin_id: String,
        url: String,
        #[source]
        source: url::ParseError,
    },

    #[error(
        "backend returned an unsupported download URL scheme for remote plugin `{remote_plugin_id}`: {scheme}"
    )]
    UnsupportedBundleDownloadUrlScheme {
        remote_plugin_id: String,
        scheme: String,
    },

    #[error(
        "backend returned an invalid local plugin id for remote plugin `{remote_plugin_id}`: {source}"
    )]
    InvalidPluginId {
        remote_plugin_id: String,
        #[source]
        source: PluginIdError,
    },

    #[error("failed to send remote plugin bundle download request to {url}: {source}")]
    DownloadRequest {
        url: String,
        #[source]
        source: reqwest::Error,
    },

    #[error("remote plugin bundle download from {url} failed with status {status}: {body}")]
    DownloadStatus {
        url: String,
        status: StatusCode,
        body: String,
    },

    #[error("failed to read remote plugin bundle download response from {url}: {source}")]
    DownloadBody {
        url: String,
        #[source]
        source: reqwest::Error,
    },

    #[error("remote plugin bundle download from {url} exceeded maximum size of {max_bytes} bytes")]
    DownloadTooLarge { url: String, max_bytes: u64 },

    #[error("remote plugin bundle download from {url} redirected to unsupported URL {final_url}")]
    UnsupportedBundleDownloadFinalUrl { url: String, final_url: String },

    #[error(
        "remote plugin bundle extracted size would be {bytes} bytes, exceeding the maximum total size of {max_bytes} bytes"
    )]
    ExtractedBundleTooLarge { bytes: u64, max_bytes: u64 },

    #[error("{context}: {source}")]
    Io {
        context: &'static str,
        #[source]
        source: io::Error,
    },

    #[error("{0}")]
    InvalidBundle(String),

    #[error("{0}")]
    Store(#[from] PluginStoreError),
}

impl RemotePluginBundleInstallError {
    fn io(context: &'static str, source: io::Error) -> Self {
        Self::Io { context, source }
    }
}

pub fn validate_remote_plugin_bundle(
    remote_plugin_id: &str,
    remote_marketplace_name: &str,
    plugin_name: &str,
    release_version: Option<&str>,
    bundle_download_url: Option<&str>,
) -> Result<ValidatedRemotePluginBundle, RemotePluginBundleInstallError> {
    let plugin_id = PluginId::new(plugin_name.to_string(), remote_marketplace_name.to_string())
        .map_err(|source| RemotePluginBundleInstallError::InvalidPluginId {
            remote_plugin_id: remote_plugin_id.to_string(),
            source,
        })?;
    let plugin_version = release_version
        .map(str::trim)
        .filter(|version| !version.is_empty())
        .ok_or_else(|| RemotePluginBundleInstallError::MissingReleaseVersion {
            remote_plugin_id: remote_plugin_id.to_string(),
        })?
        .to_string();
    validate_plugin_version_segment(&plugin_version).map_err(|message| {
        RemotePluginBundleInstallError::InvalidReleaseVersion {
            remote_plugin_id: remote_plugin_id.to_string(),
            message,
        }
    })?;
    let bundle_download_url = bundle_download_url
        .map(str::trim)
        .filter(|url| !url.is_empty())
        .ok_or_else(
            || RemotePluginBundleInstallError::MissingBundleDownloadUrl {
                remote_plugin_id: remote_plugin_id.to_string(),
            },
        )?
        .to_string();
    let parsed_bundle_url = Url::parse(&bundle_download_url).map_err(|source| {
        RemotePluginBundleInstallError::InvalidBundleDownloadUrl {
            remote_plugin_id: remote_plugin_id.to_string(),
            url: bundle_download_url.clone(),
            source,
        }
    })?;
    if !is_allowed_bundle_download_url(
        &parsed_bundle_url,
        allow_test_loopback_http_bundle_downloads(),
    ) {
        return Err(
            RemotePluginBundleInstallError::UnsupportedBundleDownloadUrlScheme {
                remote_plugin_id: remote_plugin_id.to_string(),
                scheme: parsed_bundle_url.scheme().to_string(),
            },
        );
    }

    Ok(ValidatedRemotePluginBundle {
        plugin_id,
        plugin_version,
        bundle_download_url,
    })
}

fn allow_test_loopback_http_bundle_downloads() -> bool {
    #[cfg(debug_assertions)]
    {
        if let Ok(value) = std::env::var(TEST_ALLOW_LOOPBACK_HTTP_REMOTE_PLUGIN_BUNDLES_ENV) {
            return value == "1";
        }
    }

    false
}

fn is_allowed_bundle_download_url(url: &Url, allow_loopback_http: bool) -> bool {
    match url.scheme() {
        "https" => true,
        "http" => allow_loopback_http && is_loopback_url(url),
        _ => false,
    }
}

fn is_loopback_url(url: &Url) -> bool {
    match url.host() {
        Some(Host::Ipv4(addr)) => addr.is_loopback(),
        Some(Host::Ipv6(addr)) => addr.is_loopback(),
        Some(Host::Domain(host)) => host.eq_ignore_ascii_case("localhost"),
        None => false,
    }
}

pub async fn download_and_install_remote_plugin_bundle(
    codex_home: PathBuf,
    bundle: ValidatedRemotePluginBundle,
) -> Result<PluginInstallResult, RemotePluginBundleInstallError> {
    let bundle_bytes = download_remote_plugin_bundle_with_limit(
        &bundle.bundle_download_url,
        /*max_bytes*/ REMOTE_PLUGIN_BUNDLE_MAX_DOWNLOAD_BYTES,
    )
    .await?;
    tokio::task::spawn_blocking(move || {
        install_remote_plugin_bundle(codex_home, bundle, bundle_bytes)
    })
    .await
    .map_err(|err| {
        RemotePluginBundleInstallError::InvalidBundle(format!(
            "failed to join remote plugin bundle install task: {err}"
        ))
    })?
}

async fn download_remote_plugin_bundle_with_limit(
    bundle_download_url: &str,
    max_bytes: u64,
) -> Result<Vec<u8>, RemotePluginBundleInstallError> {
    let client = build_reqwest_client();
    let response = client
        .get(bundle_download_url)
        .timeout(REMOTE_PLUGIN_BUNDLE_DOWNLOAD_TIMEOUT)
        .send()
        .await
        .map_err(|source| RemotePluginBundleInstallError::DownloadRequest {
            url: bundle_download_url.to_string(),
            source,
        })?;

    let final_url = response.url().clone();
    // reqwest may already have followed redirects here. For backend-issued bundle URLs, keep the
    // shared client policy and fail unsupported final schemes before caching.
    if !is_allowed_bundle_download_url(&final_url, allow_test_loopback_http_bundle_downloads()) {
        return Err(
            RemotePluginBundleInstallError::UnsupportedBundleDownloadFinalUrl {
                url: bundle_download_url.to_string(),
                final_url: final_url.to_string(),
            },
        );
    }

    let url = final_url.to_string();
    let status = response.status();
    if !status.is_success() {
        let body = read_response_body_with_limit(
            response,
            &url,
            /*max_bytes*/ REMOTE_PLUGIN_BUNDLE_ERROR_BODY_MAX_BYTES,
        )
        .await?;
        let body = String::from_utf8_lossy(&body).to_string();
        return Err(RemotePluginBundleInstallError::DownloadStatus { url, status, body });
    }

    read_response_body_with_limit(response, &url, max_bytes).await
}

async fn read_response_body_with_limit(
    mut response: Response,
    url: &str,
    max_bytes: u64,
) -> Result<Vec<u8>, RemotePluginBundleInstallError> {
    if let Some(content_length) = response.content_length() {
        enforce_download_size_limit(url, content_length, max_bytes)?;
    }

    let mut body = Vec::new();
    while let Some(chunk) =
        response
            .chunk()
            .await
            .map_err(|source| RemotePluginBundleInstallError::DownloadBody {
                url: url.to_string(),
                source,
            })?
    {
        let next_len = body.len() as u64 + chunk.len() as u64;
        enforce_download_size_limit(url, next_len, max_bytes)?;
        body.extend_from_slice(&chunk);
    }

    Ok(body)
}

fn enforce_download_size_limit(
    url: &str,
    bytes: u64,
    max_bytes: u64,
) -> Result<(), RemotePluginBundleInstallError> {
    if bytes > max_bytes {
        return Err(RemotePluginBundleInstallError::DownloadTooLarge {
            url: url.to_string(),
            max_bytes,
        });
    }
    Ok(())
}

fn install_remote_plugin_bundle(
    codex_home: PathBuf,
    bundle: ValidatedRemotePluginBundle,
    bundle_bytes: Vec<u8>,
) -> Result<PluginInstallResult, RemotePluginBundleInstallError> {
    let staging_root = codex_home.join(REMOTE_PLUGIN_INSTALL_STAGING_DIR);
    fs::create_dir_all(&staging_root).map_err(|source| {
        RemotePluginBundleInstallError::io(
            "failed to create remote plugin bundle staging directory",
            source,
        )
    })?;
    let extract_dir = tempfile::Builder::new()
        .prefix("remote-plugin-bundle-")
        .tempdir_in(&staging_root)
        .map_err(|source| {
            RemotePluginBundleInstallError::io(
                "failed to create remote plugin bundle extraction directory",
                source,
            )
        })?;

    extract_plugin_bundle_tar_gz(&bundle_bytes, extract_dir.path())?;
    let plugin_root = find_extracted_plugin_root(extract_dir.path())?;
    let plugin_root = AbsolutePathBuf::try_from(plugin_root).map_err(|err| {
        RemotePluginBundleInstallError::InvalidBundle(format!(
            "failed to resolve extracted remote plugin bundle root: {err}"
        ))
    })?;

    let store = PluginStore::try_new(codex_home)?;
    store
        .install_with_version(plugin_root, bundle.plugin_id, bundle.plugin_version)
        .map_err(RemotePluginBundleInstallError::from)
}

fn extract_plugin_bundle_tar_gz(
    bytes: &[u8],
    destination: &Path,
) -> Result<(), RemotePluginBundleInstallError> {
    extract_plugin_bundle_tar_gz_with_limits(
        bytes,
        destination,
        REMOTE_PLUGIN_BUNDLE_MAX_EXTRACTED_BYTES,
    )
}

fn extract_plugin_bundle_tar_gz_with_limits(
    bytes: &[u8],
    destination: &Path,
    max_total_bytes: u64,
) -> Result<(), RemotePluginBundleInstallError> {
    fs::create_dir_all(destination).map_err(|source| {
        RemotePluginBundleInstallError::io(
            "failed to create remote plugin bundle extraction directory",
            source,
        )
    })?;

    let archive = GzDecoder::new(std::io::Cursor::new(bytes));
    let mut archive = Archive::new(archive);
    extract_plugin_bundle_tar(&mut archive, destination, max_total_bytes)
}

fn extract_plugin_bundle_tar<R: Read>(
    archive: &mut Archive<R>,
    destination: &Path,
    max_total_bytes: u64,
) -> Result<(), RemotePluginBundleInstallError> {
    let mut extracted_bytes = 0u64;
    let entries = archive.entries().map_err(|source| {
        RemotePluginBundleInstallError::io("failed to read remote plugin bundle tar", source)
    })?;
    let entries = entries.raw(true);
    for entry in entries {
        let mut entry = entry.map_err(|source| {
            RemotePluginBundleInstallError::io(
                "failed to read remote plugin bundle tar entry",
                source,
            )
        })?;
        let entry_type = entry.header().entry_type();
        let entry_size = entry.size();
        let entry_path = entry.path().map_err(|source| {
            RemotePluginBundleInstallError::io(
                "failed to read remote plugin bundle tar entry path",
                source,
            )
        })?;
        let entry_path = entry_path.into_owned();
        let output_path = checked_tar_output_path(destination, &entry_path)?;

        if entry_type.is_dir() {
            fs::create_dir_all(&output_path).map_err(|source| {
                RemotePluginBundleInstallError::io(
                    "failed to create remote plugin bundle directory",
                    source,
                )
            })?;
            continue;
        }

        if entry_type.is_file() {
            enforce_total_extracted_size(entry_size, &mut extracted_bytes, max_total_bytes)?;
            let Some(parent) = output_path.parent() else {
                return Err(RemotePluginBundleInstallError::InvalidBundle(format!(
                    "remote plugin bundle output path has no parent: {}",
                    output_path.display()
                )));
            };
            fs::create_dir_all(parent).map_err(|source| {
                RemotePluginBundleInstallError::io(
                    "failed to create remote plugin bundle directory",
                    source,
                )
            })?;
            entry.unpack(&output_path).map_err(|source| {
                RemotePluginBundleInstallError::io(
                    "failed to unpack remote plugin bundle entry",
                    source,
                )
            })?;
            continue;
        }

        if entry_type.is_hard_link() || entry_type.is_symlink() {
            return Err(RemotePluginBundleInstallError::InvalidBundle(format!(
                "remote plugin bundle tar entry `{}` is a link",
                entry_path.display()
            )));
        }

        return Err(RemotePluginBundleInstallError::InvalidBundle(format!(
            "remote plugin bundle tar entry `{}` has unsupported type {:?}",
            entry_path.display(),
            entry_type
        )));
    }

    Ok(())
}

fn checked_tar_output_path(
    destination: &Path,
    entry_name: &Path,
) -> Result<PathBuf, RemotePluginBundleInstallError> {
    let mut output_path = destination.to_path_buf();
    let mut has_component = false;
    for component in entry_name.components() {
        match component {
            std::path::Component::Normal(component) => {
                has_component = true;
                output_path.push(component);
            }
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir
            | std::path::Component::RootDir
            | std::path::Component::Prefix(_) => {
                return Err(RemotePluginBundleInstallError::InvalidBundle(format!(
                    "remote plugin bundle tar entry `{}` escapes extraction root",
                    entry_name.display()
                )));
            }
        }
    }
    if !has_component {
        return Err(RemotePluginBundleInstallError::InvalidBundle(
            "remote plugin bundle tar entry has an empty path".to_string(),
        ));
    }
    Ok(output_path)
}

fn enforce_total_extracted_size(
    entry_size: u64,
    extracted_bytes: &mut u64,
    max_total_bytes: u64,
) -> Result<(), RemotePluginBundleInstallError> {
    let next_total = extracted_bytes.checked_add(entry_size).ok_or(
        RemotePluginBundleInstallError::ExtractedBundleTooLarge {
            bytes: u64::MAX,
            max_bytes: max_total_bytes,
        },
    )?;
    if next_total > max_total_bytes {
        return Err(RemotePluginBundleInstallError::ExtractedBundleTooLarge {
            bytes: next_total,
            max_bytes: max_total_bytes,
        });
    }
    *extracted_bytes = next_total;
    Ok(())
}

fn find_extracted_plugin_root(
    extraction_root: &Path,
) -> Result<PathBuf, RemotePluginBundleInstallError> {
    if is_standard_plugin_root(extraction_root) {
        return Ok(extraction_root.to_path_buf());
    }

    Err(RemotePluginBundleInstallError::InvalidBundle(
        "remote plugin bundle did not contain a standard plugin root with plugin.json".to_string(),
    ))
}

fn is_standard_plugin_root(path: &Path) -> bool {
    find_plugin_manifest_path(path).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;

    const REMOTE_PLUGIN_ID: &str = "plugins~Plugin_00000000000000000000000000000000";

    #[test]
    fn validate_remote_plugin_bundle_uses_detail_name_for_local_plugin_id() {
        let bundle = validate_remote_plugin_bundle(
            REMOTE_PLUGIN_ID,
            "chatgpt-global",
            "linear",
            Some("1.2.3"),
            Some("https://example.com/linear.tar.gz"),
        )
        .expect("valid install plan");

        assert_eq!(bundle.plugin_id.plugin_name, "linear");
        assert_eq!(bundle.plugin_id.marketplace_name, "chatgpt-global");
        assert_eq!(bundle.plugin_version, "1.2.3");
        assert_eq!(
            bundle.bundle_download_url.as_str(),
            "https://example.com/linear.tar.gz"
        );
    }

    #[test]
    fn validate_remote_plugin_bundle_rejects_missing_release_version() {
        let err = validate_remote_plugin_bundle(
            REMOTE_PLUGIN_ID,
            "chatgpt-global",
            "linear",
            /*release_version*/ None,
            Some("https://example.com/linear.tar.gz"),
        )
        .expect_err("missing release version should be rejected");

        assert!(matches!(
            err,
            RemotePluginBundleInstallError::MissingReleaseVersion { .. }
        ));
    }

    #[test]
    fn validate_remote_plugin_bundle_rejects_invalid_release_version() {
        let err = validate_remote_plugin_bundle(
            REMOTE_PLUGIN_ID,
            "chatgpt-global",
            "linear",
            Some("../1.2.3"),
            Some("https://example.com/linear.tar.gz"),
        )
        .expect_err("invalid release version should be rejected");

        assert!(matches!(
            err,
            RemotePluginBundleInstallError::InvalidReleaseVersion { .. }
        ));
    }

    #[test]
    fn validate_remote_plugin_bundle_rejects_missing_download_url() {
        let err = validate_remote_plugin_bundle(
            REMOTE_PLUGIN_ID,
            "chatgpt-global",
            "linear",
            Some("1.2.3"),
            /*bundle_download_url*/ None,
        )
        .expect_err("missing bundle download URL should be rejected");

        assert!(matches!(
            err,
            RemotePluginBundleInstallError::MissingBundleDownloadUrl { .. }
        ));
    }

    #[test]
    fn validate_remote_plugin_bundle_rejects_unsupported_download_url_scheme() {
        let err = validate_remote_plugin_bundle(
            REMOTE_PLUGIN_ID,
            "chatgpt-global",
            "linear",
            Some("1.2.3"),
            Some("http://example.com/linear.tar.gz"),
        )
        .expect_err("plain HTTP URLs should be rejected before cloud install");

        assert!(matches!(
            err,
            RemotePluginBundleInstallError::UnsupportedBundleDownloadUrlScheme { .. }
        ));
    }

    #[test]
    fn download_size_limit_rejects_oversized_bundle() {
        let err = enforce_download_size_limit(
            "https://example.com/linear.tar.gz",
            /*bytes*/ 5,
            /*max_bytes*/ 4,
        )
        .expect_err("oversized bundle download should fail");

        assert!(matches!(
            err,
            RemotePluginBundleInstallError::DownloadTooLarge { .. }
        ));
    }

    #[test]
    fn install_rejects_invalid_tar_gz_bundle() {
        let codex_home = tempdir().expect("tempdir");
        let bundle = valid_remote_plugin_bundle();

        let err = install_remote_plugin_bundle(
            codex_home.path().to_path_buf(),
            bundle,
            b"not a tar.gz".to_vec(),
        )
        .expect_err("invalid tar.gz should be rejected");

        assert!(format!("{err}").contains("failed to read remote plugin bundle tar"));
    }

    #[test]
    fn install_rejects_bundle_without_standard_plugin_root() {
        let codex_home = tempdir().expect("tempdir");
        let bundle = valid_remote_plugin_bundle();

        let err = install_remote_plugin_bundle(
            codex_home.path().to_path_buf(),
            bundle,
            tar_gz_bytes(&[("README.md", b"missing plugin manifest", /*mode*/ 0o644)]),
        )
        .expect_err("bundle without plugin root should be rejected");

        assert!(
            format!("{err}").contains("did not contain a standard plugin root with plugin.json")
        );
    }

    #[test]
    fn find_extracted_plugin_root_uses_local_manifest_discovery() {
        let extraction_root = tempdir().expect("tempdir");
        std::fs::create_dir_all(extraction_root.path().join(".codex-plugin"))
            .expect("create manifest dir");
        std::fs::write(
            extraction_root.path().join(".codex-plugin/plugin.json"),
            r#"{"name":"linear"}"#,
        )
        .expect("write manifest");

        assert_eq!(
            find_extracted_plugin_root(extraction_root.path()).expect("plugin root"),
            extraction_root.path()
        );
    }

    #[test]
    fn find_extracted_plugin_root_rejects_nested_plugin_root() {
        let extraction_root = tempdir().expect("tempdir");
        let plugin_root = extraction_root.path().join("linear");
        std::fs::create_dir_all(plugin_root.join(".codex-plugin")).expect("create manifest dir");
        std::fs::write(
            plugin_root.join(".codex-plugin/plugin.json"),
            r#"{"name":"linear"}"#,
        )
        .expect("write manifest");

        let err = find_extracted_plugin_root(extraction_root.path())
            .expect_err("nested plugin root should be rejected");

        assert!(
            format!("{err}").contains("did not contain a standard plugin root with plugin.json")
        );
    }

    #[test]
    fn extraction_rejects_tar_path_traversal() {
        let destination = tempdir().expect("tempdir");
        let err = checked_tar_output_path(destination.path(), Path::new("../evil.txt"))
            .expect_err("tar path traversal should be rejected");

        assert!(format!("{err}").contains("escapes extraction root"));
    }

    #[test]
    fn extraction_rejects_total_size_over_limit() {
        let destination = tempdir().expect("tempdir");
        let err = extract_plugin_bundle_tar_gz_with_limits(
            &tar_gz_bytes(&[
                ("a.txt", b"1234", /*mode*/ 0o644),
                ("b.txt", b"5678", /*mode*/ 0o644),
            ]),
            destination.path(),
            /*max_total_bytes*/ 6,
        )
        .expect_err("oversized extracted bundle should be rejected");

        assert!(matches!(
            err,
            RemotePluginBundleInstallError::ExtractedBundleTooLarge { .. }
        ));
    }

    #[test]
    fn extraction_rejects_pax_metadata_entries() {
        let destination = tempdir().expect("tempdir");
        let err = extract_plugin_bundle_tar_gz(
            &tar_gz_bytes_with_entry_type(
                tar::EntryType::XHeader,
                "PaxHeaders.0/linear",
                b"18 path=linear\n",
                /*mode*/ 0o644,
            ),
            destination.path(),
        )
        .expect_err("pax metadata entries should be rejected");

        assert!(format!("{err}").contains("unsupported type"));
    }

    #[cfg(unix)]
    #[test]
    fn extraction_preserves_executable_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let destination = tempdir().expect("tempdir");
        extract_plugin_bundle_tar_gz(
            &tar_gz_bytes(&[
                (
                    ".codex-plugin/plugin.json",
                    b"{\"name\":\"linear\"}",
                    /*mode*/ 0o644,
                ),
                ("bin/helper", b"#!/bin/sh\n", /*mode*/ 0o755),
            ]),
            destination.path(),
        )
        .expect("extract bundle");

        let mode = std::fs::metadata(destination.path().join("bin/helper"))
            .expect("helper metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o755);
    }

    fn valid_remote_plugin_bundle() -> ValidatedRemotePluginBundle {
        validate_remote_plugin_bundle(
            REMOTE_PLUGIN_ID,
            "chatgpt-global",
            "linear",
            Some("1.2.3"),
            Some("https://example.com/linear.tar.gz"),
        )
        .expect("valid install plan")
    }

    fn tar_gz_bytes(entries: &[(&str, &[u8], u32)]) -> Vec<u8> {
        let encoder = GzEncoder::new(Vec::new(), Compression::default());
        let mut tar = tar::Builder::new(encoder);
        for (path, contents, mode) in entries {
            append_tar_entry(&mut tar, tar::EntryType::Regular, path, contents, *mode);
        }
        finish_tar_gz(tar)
    }

    fn tar_gz_bytes_with_entry_type(
        entry_type: tar::EntryType,
        path: &str,
        contents: &[u8],
        mode: u32,
    ) -> Vec<u8> {
        let encoder = GzEncoder::new(Vec::new(), Compression::default());
        let mut tar = tar::Builder::new(encoder);
        append_tar_entry(&mut tar, entry_type, path, contents, mode);
        finish_tar_gz(tar)
    }

    fn append_tar_entry<W: std::io::Write>(
        tar: &mut tar::Builder<W>,
        entry_type: tar::EntryType,
        path: &str,
        contents: &[u8],
        mode: u32,
    ) {
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(entry_type);
        header.set_size(contents.len() as u64);
        header.set_mode(mode);
        header.set_cksum();
        if let Err(error) = tar.append_data(&mut header, path, contents) {
            panic!("failed to append tar test data: {error}");
        }
    }

    fn finish_tar_gz(tar: tar::Builder<GzEncoder<Vec<u8>>>) -> Vec<u8> {
        let encoder = match tar.into_inner() {
            Ok(encoder) => encoder,
            Err(error) => panic!("failed to finish tar test data: {error}"),
        };
        match encoder.finish() {
            Ok(bytes) => bytes,
            Err(error) => panic!("failed to finish gzip test data: {error}"),
        }
    }
}
