use crate::plugin_bundle_archive::PluginBundleUnpackError;
use crate::plugin_bundle_archive::unpack_plugin_bundle_tar_gz;
use crate::remote::REMOTE_GLOBAL_MARKETPLACE_NAME;
use crate::remote::RemotePluginServiceConfig;
use crate::store::PluginInstallResult;
use crate::store::PluginStore;
use crate::store::PluginStoreError;
use crate::store::error_context_sub_error_type;
use crate::store::validate_plugin_version_segment;
use codex_http_client::HttpResponse;
use codex_http_client::RouteAwareRequestError;
use codex_plugin::PluginId;
use codex_plugin::PluginIdError;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_plugins::find_plugin_manifest_path;
use http::Method;
use http::StatusCode;
use serde_json::Value as JsonValue;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
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
    remote_plugin_id: String,
    app_manifest: Option<JsonValue>,
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
        source: RouteAwareRequestError,
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
        source: codex_http_client::HttpError,
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

    pub fn sub_error_type(&self) -> Option<String> {
        match self {
            Self::Io { context, .. } => Some(error_context_sub_error_type(context)),
            Self::Store(err) => err.sub_error_type(),
            Self::MissingReleaseVersion { .. }
            | Self::InvalidReleaseVersion { .. }
            | Self::MissingBundleDownloadUrl { .. }
            | Self::InvalidBundleDownloadUrl { .. }
            | Self::UnsupportedBundleDownloadUrlScheme { .. }
            | Self::InvalidPluginId { .. }
            | Self::DownloadRequest { .. }
            | Self::DownloadStatus { .. }
            | Self::DownloadBody { .. }
            | Self::DownloadTooLarge { .. }
            | Self::UnsupportedBundleDownloadFinalUrl { .. }
            | Self::ExtractedBundleTooLarge { .. }
            | Self::InvalidBundle(_) => None,
        }
    }
}

pub fn validate_remote_plugin_bundle(
    remote_plugin_id: &str,
    remote_marketplace_name: &str,
    plugin_name: &str,
    release_version: Option<&str>,
    bundle_download_url: Option<&str>,
    app_manifest: Option<JsonValue>,
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
        remote_plugin_id: remote_plugin_id.to_string(),
        app_manifest,
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
    config: &RemotePluginServiceConfig,
    codex_home: PathBuf,
    bundle: ValidatedRemotePluginBundle,
) -> Result<PluginInstallResult, RemotePluginBundleInstallError> {
    let bundle_bytes = download_remote_plugin_bundle_with_limit(
        config,
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

pub(crate) async fn download_and_extract_remote_plugin_bundle_to_path(
    config: &RemotePluginServiceConfig,
    bundle: ValidatedRemotePluginBundle,
    destination: AbsolutePathBuf,
) -> Result<AbsolutePathBuf, RemotePluginBundleInstallError> {
    let bundle_bytes = download_remote_plugin_bundle_with_limit(
        config,
        &bundle.bundle_download_url,
        /*max_bytes*/ REMOTE_PLUGIN_BUNDLE_MAX_DOWNLOAD_BYTES,
    )
    .await?;
    tokio::task::spawn_blocking(move || {
        extract_remote_plugin_bundle_to_path(bundle, bundle_bytes, destination)
    })
    .await
    .map_err(|err| {
        RemotePluginBundleInstallError::InvalidBundle(format!(
            "failed to join remote plugin bundle extraction task: {err}"
        ))
    })?
}

async fn download_remote_plugin_bundle_with_limit(
    config: &RemotePluginServiceConfig,
    bundle_download_url: &str,
    max_bytes: u64,
) -> Result<Vec<u8>, RemotePluginBundleInstallError> {
    let response = config
        .http_request(Method::GET, bundle_download_url)
        .timeout(REMOTE_PLUGIN_BUNDLE_DOWNLOAD_TIMEOUT)
        .send()
        .await
        .map_err(|source| RemotePluginBundleInstallError::DownloadRequest {
            url: bundle_download_url.to_string(),
            source,
        })?;

    let final_url = response.url().clone();
    // The shared client has already followed redirects here. Reject an unsupported final scheme
    // before caching a backend-issued bundle.
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
        let mut response = response;
        let mut body = Vec::new();
        let mut body_truncated = false;
        let mut body_read_error = None;
        loop {
            let chunk = match response.chunk().await {
                Ok(Some(chunk)) => chunk,
                Ok(None) => break,
                Err(source) => {
                    body_read_error = Some(source);
                    break;
                }
            };
            let remaining = REMOTE_PLUGIN_BUNDLE_ERROR_BODY_MAX_BYTES as usize - body.len();
            if chunk.len() > remaining {
                body.extend_from_slice(&chunk[..remaining]);
                body_truncated = true;
                break;
            }
            body.extend_from_slice(&chunk);
        }

        let mut body = String::from_utf8_lossy(&body).into_owned();
        if body_truncated {
            body.push_str(&format!(
                "\n[response body truncated after {REMOTE_PLUGIN_BUNDLE_ERROR_BODY_MAX_BYTES} bytes]"
            ));
        }
        if let Some(source) = body_read_error {
            body.push_str(&format!("\n[failed to read response body: {source}]"));
        }
        return Err(RemotePluginBundleInstallError::DownloadStatus { url, status, body });
    }

    read_response_body_with_limit(response, &url, max_bytes).await
}

async fn read_response_body_with_limit(
    mut response: HttpResponse,
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
    prepare_extracted_remote_plugin_root(&plugin_root, &bundle)?;
    let plugin_root = AbsolutePathBuf::try_from(plugin_root).map_err(|err| {
        RemotePluginBundleInstallError::InvalidBundle(format!(
            "failed to resolve extracted remote plugin bundle root: {err}"
        ))
    })?;

    let store = PluginStore::try_new(codex_home)?;
    let remote_plugin_id = bundle.remote_plugin_id;
    let result = store
        .install_with_version(plugin_root, bundle.plugin_id, bundle.plugin_version)
        .map_err(RemotePluginBundleInstallError::from)?;
    store.write_remote_plugin_id(&result.plugin_id, &remote_plugin_id)?;
    Ok(result)
}

fn extract_remote_plugin_bundle_to_path(
    bundle: ValidatedRemotePluginBundle,
    bundle_bytes: Vec<u8>,
    destination: AbsolutePathBuf,
) -> Result<AbsolutePathBuf, RemotePluginBundleInstallError> {
    if destination.as_path().exists() {
        return Err(RemotePluginBundleInstallError::InvalidBundle(format!(
            "plugin checkout destination already exists: {}",
            destination.display()
        )));
    }

    let parent = destination.as_path().parent().ok_or_else(|| {
        RemotePluginBundleInstallError::InvalidBundle(format!(
            "plugin checkout destination has no parent: {}",
            destination.display()
        ))
    })?;
    fs::create_dir_all(parent).map_err(|source| {
        RemotePluginBundleInstallError::io("failed to create plugin checkout directory", source)
    })?;

    let extract_dir = tempfile::Builder::new()
        .prefix("remote-plugin-checkout-")
        .tempdir_in(parent)
        .map_err(|source| {
            RemotePluginBundleInstallError::io(
                "failed to create remote plugin bundle extraction directory",
                source,
            )
        })?;

    extract_plugin_bundle_tar_gz(&bundle_bytes, extract_dir.path())?;
    let plugin_root = find_extracted_plugin_root(extract_dir.path())?;
    let manifest = crate::manifest::load_plugin_manifest(&plugin_root).ok_or_else(|| {
        RemotePluginBundleInstallError::InvalidBundle(
            "remote plugin bundle did not contain a valid plugin.json".to_string(),
        )
    })?;
    if manifest.name != bundle.plugin_id.plugin_name {
        return Err(RemotePluginBundleInstallError::InvalidBundle(format!(
            "plugin.json name `{}` does not match remote plugin name `{}`",
            manifest.name, bundle.plugin_id.plugin_name
        )));
    }

    let staged_path = extract_dir.keep();
    fs::rename(&staged_path, destination.as_path()).map_err(|source| {
        RemotePluginBundleInstallError::io(
            "failed to activate checked out plugin directory",
            source,
        )
    })?;

    Ok(destination)
}

fn prepare_extracted_remote_plugin_root(
    plugin_root: &Path,
    bundle: &ValidatedRemotePluginBundle,
) -> Result<(), RemotePluginBundleInstallError> {
    if bundle.plugin_id.marketplace_name != REMOTE_GLOBAL_MARKETPLACE_NAME {
        return Ok(());
    }

    overwrite_plugin_manifest_version(plugin_root, &bundle.plugin_version)?;
    if let Some(app_manifest) = &bundle.app_manifest {
        overwrite_plugin_app_manifest(plugin_root, app_manifest)?;
    }
    Ok(())
}

fn overwrite_plugin_manifest_version(
    plugin_root: &Path,
    plugin_version: &str,
) -> Result<(), RemotePluginBundleInstallError> {
    let manifest_path = find_plugin_manifest_path(plugin_root).ok_or_else(|| {
        RemotePluginBundleInstallError::InvalidBundle(
            "remote plugin bundle did not contain a valid plugin.json".to_string(),
        )
    })?;
    let contents = fs::read_to_string(&manifest_path).map_err(|source| {
        RemotePluginBundleInstallError::io("failed to read remote plugin manifest", source)
    })?;
    let mut manifest: JsonValue = serde_json::from_str(&contents).map_err(|err| {
        RemotePluginBundleInstallError::InvalidBundle(format!(
            "failed to parse remote plugin manifest: {err}"
        ))
    })?;
    let Some(manifest_object) = manifest.as_object_mut() else {
        return Err(RemotePluginBundleInstallError::InvalidBundle(
            "remote plugin manifest must be a JSON object".to_string(),
        ));
    };
    manifest_object.insert(
        "version".to_string(),
        JsonValue::String(plugin_version.to_string()),
    );
    write_json_file(
        &manifest_path,
        &manifest,
        "failed to write remote plugin manifest",
    )
}

fn overwrite_plugin_app_manifest(
    plugin_root: &Path,
    app_manifest: &JsonValue,
) -> Result<(), RemotePluginBundleInstallError> {
    let app_manifest_path = crate::manifest::load_plugin_manifest(plugin_root)
        .and_then(|manifest| manifest.paths.apps.map(|path| path.to_path_buf()))
        .unwrap_or_else(|| plugin_root.join(".app.json"));
    write_json_file(
        &app_manifest_path,
        app_manifest,
        "failed to write remote plugin app manifest",
    )
}

fn write_json_file(
    path: &Path,
    value: &JsonValue,
    context: &'static str,
) -> Result<(), RemotePluginBundleInstallError> {
    let parent = path.parent().ok_or_else(|| {
        RemotePluginBundleInstallError::InvalidBundle(format!(
            "remote plugin output path has no parent: {}",
            path.display()
        ))
    })?;
    fs::create_dir_all(parent)
        .map_err(|source| RemotePluginBundleInstallError::io(context, source))?;
    let mut contents = serde_json::to_vec_pretty(value).map_err(|err| {
        RemotePluginBundleInstallError::InvalidBundle(format!(
            "failed to serialize remote plugin JSON override: {err}"
        ))
    })?;
    contents.push(b'\n');
    fs::write(path, contents).map_err(|source| RemotePluginBundleInstallError::io(context, source))
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
    unpack_plugin_bundle_tar_gz(bytes, destination, max_total_bytes).map_err(|err| match err {
        PluginBundleUnpackError::ExtractedBundleTooLarge { bytes, max_bytes } => {
            RemotePluginBundleInstallError::ExtractedBundleTooLarge { bytes, max_bytes }
        }
        PluginBundleUnpackError::Io { context, source } => {
            RemotePluginBundleInstallError::io(context, source)
        }
        PluginBundleUnpackError::InvalidBundle(message) => {
            RemotePluginBundleInstallError::InvalidBundle(message)
        }
    })
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
    use crate::test_support::recorded_http_client_urls;
    use crate::test_support::recording_remote_plugin_service_config;
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use pretty_assertions::assert_eq;
    use std::io::Write;
    use tempfile::tempdir;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    const REMOTE_PLUGIN_ID: &str = "plugins~Plugin_00000000000000000000000000000000";

    #[test]
    fn validate_remote_plugin_bundle_uses_detail_name_for_local_plugin_id() {
        let bundle = validate_remote_plugin_bundle(
            REMOTE_PLUGIN_ID,
            "openai-curated-remote",
            "linear",
            Some("1.2.3"),
            Some("https://example.com/linear.tar.gz"),
            /*app_manifest*/ None,
        )
        .expect("valid install plan");

        assert_eq!(bundle.plugin_id.plugin_name, "linear");
        assert_eq!(bundle.plugin_id.marketplace_name, "openai-curated-remote");
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
            "openai-curated-remote",
            "linear",
            /*release_version*/ None,
            Some("https://example.com/linear.tar.gz"),
            /*app_manifest*/ None,
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
            "openai-curated-remote",
            "linear",
            Some("../1.2.3"),
            Some("https://example.com/linear.tar.gz"),
            /*app_manifest*/ None,
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
            "openai-curated-remote",
            "linear",
            Some("1.2.3"),
            /*bundle_download_url*/ None,
            /*app_manifest*/ None,
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
            "openai-curated-remote",
            "linear",
            Some("1.2.3"),
            Some("http://example.com/linear.tar.gz"),
            /*app_manifest*/ None,
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

    #[tokio::test]
    async fn bundle_download_routes_the_backend_supplied_url() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/signed/plugin-bundle"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"bundle"))
            .expect(1)
            .mount(&server)
            .await;
        let (config, selected_urls) =
            recording_remote_plugin_service_config(format!("{}/backend-api", server.uri()));
        let download_url = format!("{}/signed/plugin-bundle?sig=signed-token", server.uri());

        let err =
            download_remote_plugin_bundle_with_limit(&config, &download_url, /*max_bytes*/ 64)
                .await
                .expect_err("plain HTTP final URL should remain unsupported");

        assert!(matches!(
            err,
            RemotePluginBundleInstallError::UnsupportedBundleDownloadFinalUrl { .. }
        ));
        assert_eq!(
            recorded_http_client_urls(&selected_urls),
            vec![download_url]
        );
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

        assert!(format!("{err}").contains("failed to read plugin bundle tar"));
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
    fn install_persists_remote_plugin_install_metadata() {
        let codex_home = tempdir().expect("tempdir");
        let bundle = valid_remote_plugin_bundle();

        let result = install_remote_plugin_bundle(
            codex_home.path().to_path_buf(),
            bundle,
            tar_gz_bytes(&[(
                ".codex-plugin/plugin.json",
                br#"{"name":"linear","version":"1.2.3"}"#,
                /*mode*/ 0o644,
            )]),
        )
        .expect("install bundle");
        let store = PluginStore::new(codex_home.path().to_path_buf());

        assert_eq!(
            store.remote_plugin_id(&result.plugin_id).unwrap(),
            Some(REMOTE_PLUGIN_ID.to_string())
        );
        let metadata_path = store
            .plugin_base_root(&result.plugin_id)
            .join(".codex-remote-plugin-install.json");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(
                &std::fs::read_to_string(metadata_path.as_path())
                    .expect("read remote plugin install metadata")
            )
            .expect("parse remote plugin install metadata"),
            serde_json::json!({
                "schema_version": 1,
                "remote_plugin_id": REMOTE_PLUGIN_ID,
            })
        );
    }

    #[test]
    fn install_preserves_non_global_bundle_manifest_metadata() {
        let codex_home = tempdir().expect("tempdir");
        let bundle = validate_remote_plugin_bundle(
            REMOTE_PLUGIN_ID,
            "workspace-shared-with-me",
            "linear",
            Some("backend-version"),
            Some("https://example.com/linear.tar.gz"),
            Some(serde_json::json!({
                "apps": {
                    "remote": {
                        "id": "remote-app"
                    }
                }
            })),
        )
        .expect("valid install plan");

        let result = install_remote_plugin_bundle(
            codex_home.path().to_path_buf(),
            bundle,
            tar_gz_bytes(&[
                (
                    ".codex-plugin/plugin.json",
                    br#"{"name":"linear","version":"bundle-version"}"#,
                    /*mode*/ 0o644,
                ),
                (
                    ".app.json",
                    br#"{"apps":{"bundled":{"id":"bundled-app"}}}"#,
                    /*mode*/ 0o644,
                ),
            ]),
        )
        .expect("install bundle");

        assert_eq!(result.plugin_version, "backend-version");
        let installed_manifest: JsonValue = serde_json::from_str(
            &std::fs::read_to_string(
                result
                    .installed_path
                    .join(".codex-plugin/plugin.json")
                    .as_path(),
            )
            .expect("read installed plugin manifest"),
        )
        .expect("parse installed plugin manifest");
        assert_eq!(
            installed_manifest,
            serde_json::json!({
                "name": "linear",
                "version": "bundle-version",
            })
        );
        let installed_app_manifest: JsonValue = serde_json::from_str(
            &std::fs::read_to_string(result.installed_path.join(".app.json").as_path())
                .expect("read installed app manifest"),
        )
        .expect("parse installed app manifest");
        assert_eq!(
            installed_app_manifest,
            serde_json::json!({
                "apps": {
                    "bundled": {
                        "id": "bundled-app",
                    },
                },
            })
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
        let err = extract_plugin_bundle_tar_gz(
            &tar_gz_bytes_with_raw_path("../evil.txt", b"evil", /*mode*/ 0o644),
            destination.path(),
        )
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
    fn extraction_supports_gnu_long_name_entries() {
        let destination = tempdir().expect("tempdir");
        let long_path = format!("{}/file.txt", ["segment"; 40].join("/"));

        extract_plugin_bundle_tar_gz(
            &tar_gz_bytes(&[(long_path.as_str(), b"long", /*mode*/ 0o644)]),
            destination.path(),
        )
        .expect("extract bundle with GNU long name entry");

        assert_eq!(
            std::fs::read(destination.path().join(long_path)).expect("read extracted file"),
            b"long"
        );
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
            "openai-curated-remote",
            "linear",
            Some("1.2.3"),
            Some("https://example.com/linear.tar.gz"),
            /*app_manifest*/ None,
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

    fn tar_gz_bytes_with_raw_path(path: &str, contents: &[u8], mode: u32) -> Vec<u8> {
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Regular);
        header.set_size(contents.len() as u64);
        header.set_mode(mode);
        header.as_mut_bytes()[..path.len()].copy_from_slice(path.as_bytes());
        header.set_cksum();

        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder
            .write_all(header.as_bytes())
            .expect("write tar header");
        encoder.write_all(contents).expect("write tar contents");
        let padding = (512 - (contents.len() % 512)) % 512;
        encoder
            .write_all(&vec![0; padding])
            .expect("write tar padding");
        encoder.write_all(&[0; 1024]).expect("write tar terminator");
        encoder.finish().expect("finish gzip")
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
