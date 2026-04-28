use crate::store::PluginInstallResult;
use crate::store::PluginStore;
use crate::store::PluginStoreError;
use crate::store::validate_plugin_version_segment;
use codex_login::default_client::build_reqwest_client;
use codex_plugin::PluginId;
use codex_plugin::PluginIdError;
use codex_utils_absolute_path::AbsolutePathBuf;
use flate2::read::GzDecoder;
use reqwest::StatusCode;
use std::fs;
use std::io;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

const REMOTE_PLUGIN_BUNDLE_DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(60);
const REMOTE_PLUGIN_INSTALL_STAGING_DIR: &str = "plugins/.remote-plugin-install-staging";
const TAR_BLOCK_SIZE: usize = 512;

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
    let parsed_bundle_url = url::Url::parse(&bundle_download_url).map_err(|source| {
        RemotePluginBundleInstallError::InvalidBundleDownloadUrl {
            remote_plugin_id: remote_plugin_id.to_string(),
            url: bundle_download_url.clone(),
            source,
        }
    })?;
    match parsed_bundle_url.scheme() {
        "http" | "https" => {}
        scheme => {
            return Err(
                RemotePluginBundleInstallError::UnsupportedBundleDownloadUrlScheme {
                    remote_plugin_id: remote_plugin_id.to_string(),
                    scheme: scheme.to_string(),
                },
            );
        }
    }

    Ok(ValidatedRemotePluginBundle {
        plugin_id,
        plugin_version,
        bundle_download_url,
    })
}

pub async fn download_and_install_remote_plugin_bundle(
    codex_home: PathBuf,
    bundle: ValidatedRemotePluginBundle,
) -> Result<PluginInstallResult, RemotePluginBundleInstallError> {
    let bundle_bytes = download_remote_plugin_bundle(&bundle.bundle_download_url).await?;
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

async fn download_remote_plugin_bundle(
    bundle_download_url: &str,
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
    let status = response.status();
    let body =
        response
            .bytes()
            .await
            .map_err(|source| RemotePluginBundleInstallError::DownloadBody {
                url: bundle_download_url.to_string(),
                source,
            })?;
    if !status.is_success() {
        let body = String::from_utf8_lossy(&body).to_string();
        return Err(RemotePluginBundleInstallError::DownloadStatus {
            url: bundle_download_url.to_string(),
            status,
            body,
        });
    }

    Ok(body.to_vec())
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
    fs::create_dir_all(destination).map_err(|source| {
        RemotePluginBundleInstallError::io(
            "failed to create remote plugin bundle extraction directory",
            source,
        )
    })?;

    let mut archive = GzDecoder::new(std::io::Cursor::new(bytes));
    extract_plugin_bundle_tar(&mut archive, destination)
}

fn extract_plugin_bundle_tar<R: Read>(
    archive: &mut R,
    destination: &Path,
) -> Result<(), RemotePluginBundleInstallError> {
    loop {
        let mut header = [0u8; TAR_BLOCK_SIZE];
        archive.read_exact(&mut header).map_err(|source| {
            RemotePluginBundleInstallError::io(
                "failed to read remote plugin bundle tar header",
                source,
            )
        })?;
        if header.iter().all(|byte| *byte == 0) {
            return Ok(());
        }

        let entry_name = parse_tar_path(&header)?;
        let entry_size = parse_tar_octal(&header[124..136], "size")?;
        let entry_mode = parse_tar_octal(&header[100..108], "mode")? as u32;
        let output_path = checked_tar_output_path(destination, &entry_name)?;

        match header[156] {
            b'\0' | b'0' => {
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
                let mut output = fs::File::create(&output_path).map_err(|source| {
                    RemotePluginBundleInstallError::io(
                        "failed to create remote plugin bundle file",
                        source,
                    )
                })?;
                copy_exact_bytes(archive, &mut output, entry_size)?;
                apply_tar_permissions(&output_path, entry_mode)?;
            }
            b'5' => {
                fs::create_dir_all(&output_path).map_err(|source| {
                    RemotePluginBundleInstallError::io(
                        "failed to create remote plugin bundle directory",
                        source,
                    )
                })?;
                apply_tar_permissions(&output_path, entry_mode)?;
                discard_exact_bytes(archive, entry_size)?;
            }
            b'1' | b'2' => {
                return Err(RemotePluginBundleInstallError::InvalidBundle(format!(
                    "remote plugin bundle tar entry `{entry_name}` is a link"
                )));
            }
            entry_type => {
                return Err(RemotePluginBundleInstallError::InvalidBundle(format!(
                    "remote plugin bundle tar entry `{entry_name}` has unsupported type {entry_type}"
                )));
            }
        }

        discard_tar_padding(archive, entry_size)?;
    }
}

fn parse_tar_path(header: &[u8; TAR_BLOCK_SIZE]) -> Result<String, RemotePluginBundleInstallError> {
    let name = parse_tar_string(&header[0..100]);
    let prefix = parse_tar_string(&header[345..500]);
    let path = match (prefix.as_deref(), name.as_deref()) {
        (_, None) => None,
        (Some(prefix), Some(name)) => Some(format!("{prefix}/{name}")),
        (None, Some(name)) => Some(name.to_string()),
    };
    path.filter(|path| !path.is_empty()).ok_or_else(|| {
        RemotePluginBundleInstallError::InvalidBundle(
            "remote plugin bundle tar entry has an empty path".to_string(),
        )
    })
}

fn parse_tar_string(bytes: &[u8]) -> Option<String> {
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    let value = String::from_utf8_lossy(&bytes[..end]).trim().to_string();
    (!value.is_empty()).then_some(value)
}

fn parse_tar_octal(bytes: &[u8], field: &str) -> Result<u64, RemotePluginBundleInstallError> {
    let value = String::from_utf8_lossy(bytes)
        .trim_matches(|ch| ch == '\0' || ch == ' ')
        .to_string();
    if value.is_empty() {
        return Ok(0);
    }
    u64::from_str_radix(&value, 8).map_err(|err| {
        RemotePluginBundleInstallError::InvalidBundle(format!(
            "remote plugin bundle tar entry has invalid {field}: {err}"
        ))
    })
}

fn checked_tar_output_path(
    destination: &Path,
    entry_name: &str,
) -> Result<PathBuf, RemotePluginBundleInstallError> {
    let mut output_path = destination.to_path_buf();
    for component in Path::new(entry_name).components() {
        match component {
            std::path::Component::Normal(component) => output_path.push(component),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir
            | std::path::Component::RootDir
            | std::path::Component::Prefix(_) => {
                return Err(RemotePluginBundleInstallError::InvalidBundle(format!(
                    "remote plugin bundle tar entry `{entry_name}` escapes extraction root"
                )));
            }
        }
    }
    Ok(output_path)
}

fn copy_exact_bytes<R: Read, W: io::Write>(
    reader: &mut R,
    writer: &mut W,
    bytes: u64,
) -> Result<(), RemotePluginBundleInstallError> {
    let mut remaining = bytes;
    let mut buffer = [0u8; 8192];
    while remaining > 0 {
        let chunk_len = remaining.min(buffer.len() as u64) as usize;
        reader
            .read_exact(&mut buffer[..chunk_len])
            .map_err(|source| {
                RemotePluginBundleInstallError::io(
                    "failed to read remote plugin bundle file",
                    source,
                )
            })?;
        writer.write_all(&buffer[..chunk_len]).map_err(|source| {
            RemotePluginBundleInstallError::io("failed to write remote plugin bundle file", source)
        })?;
        remaining -= chunk_len as u64;
    }
    Ok(())
}

fn discard_exact_bytes<R: Read>(
    reader: &mut R,
    bytes: u64,
) -> Result<(), RemotePluginBundleInstallError> {
    copy_exact_bytes(reader, &mut io::sink(), bytes)
}

fn discard_tar_padding<R: Read>(
    reader: &mut R,
    entry_size: u64,
) -> Result<(), RemotePluginBundleInstallError> {
    let padding =
        (TAR_BLOCK_SIZE as u64 - entry_size % TAR_BLOCK_SIZE as u64) % TAR_BLOCK_SIZE as u64;
    discard_exact_bytes(reader, padding)
}

#[cfg(unix)]
fn apply_tar_permissions(
    output_path: &Path,
    mode: u32,
) -> Result<(), RemotePluginBundleInstallError> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(output_path, fs::Permissions::from_mode(mode & 0o7777)).map_err(|source| {
        RemotePluginBundleInstallError::io(
            "failed to set remote plugin bundle file permissions",
            source,
        )
    })
}

#[cfg(not(unix))]
fn apply_tar_permissions(
    _output_path: &Path,
    _mode: u32,
) -> Result<(), RemotePluginBundleInstallError> {
    Ok(())
}

fn find_extracted_plugin_root(
    extraction_root: &Path,
) -> Result<PathBuf, RemotePluginBundleInstallError> {
    if is_standard_plugin_root(extraction_root) {
        return Ok(extraction_root.to_path_buf());
    }

    let mut candidates = Vec::new();
    for entry in fs::read_dir(extraction_root).map_err(|source| {
        RemotePluginBundleInstallError::io(
            "failed to read remote plugin bundle extraction directory",
            source,
        )
    })? {
        let entry = entry.map_err(|source| {
            RemotePluginBundleInstallError::io(
                "failed to enumerate remote plugin bundle extraction directory",
                source,
            )
        })?;
        if entry
            .file_type()
            .map(|file_type| file_type.is_dir())
            .unwrap_or(false)
            && is_standard_plugin_root(&entry.path())
        {
            candidates.push(entry.path());
        }
    }

    match candidates.as_slice() {
        [plugin_root] => Ok(plugin_root.clone()),
        [] => Err(RemotePluginBundleInstallError::InvalidBundle(
            "remote plugin bundle did not contain a standard plugin root with .codex-plugin/plugin.json".to_string(),
        )),
        _ => Err(RemotePluginBundleInstallError::InvalidBundle(
            "remote plugin bundle contained multiple standard plugin roots".to_string(),
        )),
    }
}

fn is_standard_plugin_root(path: &Path) -> bool {
    path.join(".codex-plugin/plugin.json").is_file()
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use pretty_assertions::assert_eq;
    use std::io::Write;
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
            bundle.bundle_download_url,
            "https://example.com/linear.tar.gz"
        );
    }

    #[test]
    fn validate_remote_plugin_bundle_rejects_missing_release_version() {
        let err = validate_remote_plugin_bundle(
            REMOTE_PLUGIN_ID,
            "chatgpt-global",
            "linear",
            None,
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
            None,
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
            Some("file:///tmp/linear.tar.gz"),
        )
        .expect_err("file URLs should be rejected before cloud install");

        assert!(matches!(
            err,
            RemotePluginBundleInstallError::UnsupportedBundleDownloadUrlScheme { .. }
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

        assert!(format!("{err}").contains("failed to read remote plugin bundle tar header"));
    }

    #[test]
    fn install_rejects_bundle_without_standard_plugin_root() {
        let codex_home = tempdir().expect("tempdir");
        let bundle = valid_remote_plugin_bundle();

        let err = install_remote_plugin_bundle(
            codex_home.path().to_path_buf(),
            bundle,
            tar_gz_bytes(&[TarEntry::file(
                "README.md",
                b"missing plugin manifest",
                0o644,
            )]),
        )
        .expect_err("bundle without plugin root should be rejected");

        assert!(
            format!("{err}")
                .contains("did not contain a standard plugin root with .codex-plugin/plugin.json")
        );
    }

    #[test]
    fn extraction_rejects_tar_path_traversal() {
        let destination = tempdir().expect("tempdir");
        let err = extract_plugin_bundle_tar_gz(
            &tar_gz_bytes(&[TarEntry::file(
                "../evil.txt",
                b"outside extraction root",
                0o644,
            )]),
            destination.path(),
        )
        .expect_err("tar path traversal should be rejected");

        assert!(format!("{err}").contains("escapes extraction root"));
    }

    #[cfg(unix)]
    #[test]
    fn extraction_preserves_executable_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let destination = tempdir().expect("tempdir");
        extract_plugin_bundle_tar_gz(
            &tar_gz_bytes(&[
                TarEntry::file(".codex-plugin/plugin.json", b"{\"name\":\"linear\"}", 0o644),
                TarEntry::Directory {
                    path: "bin",
                    mode: 0o755,
                },
                TarEntry::file("bin/helper", b"#!/bin/sh\n", 0o755),
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

    enum TarEntry<'a> {
        Directory {
            path: &'a str,
            mode: u32,
        },
        File {
            path: &'a str,
            contents: &'a [u8],
            mode: u32,
        },
    }

    impl<'a> TarEntry<'a> {
        fn file(path: &'a str, contents: &'a [u8], mode: u32) -> Self {
            Self::File {
                path,
                contents,
                mode,
            }
        }
    }

    fn tar_gz_bytes(entries: &[TarEntry<'_>]) -> Vec<u8> {
        let mut tar = Vec::new();
        for entry in entries {
            match entry {
                TarEntry::Directory { path, mode } => {
                    append_tar_entry(&mut tar, path, b"", *mode, b'5');
                }
                TarEntry::File {
                    path,
                    contents,
                    mode,
                } => append_tar_entry(&mut tar, path, contents, *mode, b'0'),
            }
        }
        tar.extend_from_slice(&[0u8; TAR_BLOCK_SIZE]);
        tar.extend_from_slice(&[0u8; TAR_BLOCK_SIZE]);

        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&tar).expect("write gzip");
        encoder.finish().expect("finish gzip")
    }

    fn append_tar_entry(output: &mut Vec<u8>, path: &str, contents: &[u8], mode: u32, kind: u8) {
        let mut header = [0u8; TAR_BLOCK_SIZE];
        write_tar_field(&mut header[0..100], path.as_bytes());
        write_tar_octal(&mut header[100..108], mode as u64);
        write_tar_octal(&mut header[108..116], 0);
        write_tar_octal(&mut header[116..124], 0);
        write_tar_octal(&mut header[124..136], contents.len() as u64);
        write_tar_octal(&mut header[136..148], 0);
        header[148..156].fill(b' ');
        header[156] = kind;
        write_tar_field(&mut header[257..263], b"ustar");
        write_tar_field(&mut header[263..265], b"00");
        let checksum = header.iter().map(|byte| u32::from(*byte)).sum::<u32>();
        let checksum = format!("{checksum:06o}\0 ");
        header[148..156].copy_from_slice(checksum.as_bytes());

        output.extend_from_slice(&header);
        output.extend_from_slice(contents);
        let padding = (TAR_BLOCK_SIZE - contents.len() % TAR_BLOCK_SIZE) % TAR_BLOCK_SIZE;
        output.extend(std::iter::repeat_n(0, padding));
    }

    fn write_tar_field(field: &mut [u8], value: &[u8]) {
        field[..value.len()].copy_from_slice(value);
    }

    fn write_tar_octal(field: &mut [u8], value: u64) {
        let value = format!("{value:0width$o}\0", width = field.len() - 1);
        field.copy_from_slice(value.as_bytes());
    }
}
