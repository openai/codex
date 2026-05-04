use super::*;
use codex_login::CodexAuth;
use codex_login::default_client::build_reqwest_client;
use codex_utils_absolute_path::AbsolutePathBuf;
use flate2::Compression;
use flate2::write::GzEncoder;
use reqwest::RequestBuilder;
use reqwest::StatusCode;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io;
use std::io::Write;
use std::path::Path;
use tracing::warn;

mod local_paths;

const REMOTE_PLUGIN_SHARE_MAX_ARCHIVE_BYTES: usize = 50 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemotePluginShareSaveResult {
    pub remote_plugin_id: String,
    pub share_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct RemoteWorkspacePluginUploadUrlRequest<'a> {
    filename: &'a str,
    mime_type: &'a str,
    size_bytes: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    plugin_id: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct RemoteWorkspacePluginUploadUrlResponse {
    file_id: String,
    upload_url: String,
    etag: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct RemoteWorkspacePluginCreateRequest {
    file_id: String,
    etag: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct RemoteWorkspacePluginCreateResponse {
    plugin_id: String,
    share_url: Option<String>,
}

pub async fn save_remote_plugin_share(
    config: &RemotePluginServiceConfig,
    auth: Option<&CodexAuth>,
    codex_home: &Path,
    plugin_path: &AbsolutePathBuf,
    remote_plugin_id: Option<&str>,
) -> Result<RemotePluginShareSaveResult, RemotePluginCatalogError> {
    let auth = ensure_chatgpt_auth(auth)?;
    let plugin_path_for_archive = plugin_path.as_path().to_path_buf();
    let (filename, archive_bytes) = tokio::task::spawn_blocking(move || {
        let filename = archive_filename(&plugin_path_for_archive)?;
        let archive_bytes = archive_plugin_for_upload(&plugin_path_for_archive)?;
        Ok::<_, RemotePluginCatalogError>((filename, archive_bytes))
    })
    .await
    .map_err(RemotePluginCatalogError::ArchiveJoin)??;
    let upload = create_workspace_plugin_upload(
        config,
        auth,
        &filename,
        archive_bytes.len(),
        remote_plugin_id,
    )
    .await?;
    let etag = upload
        .etag
        .ok_or(RemotePluginCatalogError::MissingUploadEtag)?;
    put_workspace_plugin_upload(&upload.upload_url, archive_bytes).await?;
    let response = finalize_workspace_plugin_upload(
        config,
        auth,
        remote_plugin_id,
        RemoteWorkspacePluginCreateRequest {
            file_id: upload.file_id,
            etag,
        },
    )
    .await?;
    if response.plugin_id.is_empty() {
        return Err(RemotePluginCatalogError::UnexpectedResponse(
            "workspace plugin create response did not include a plugin id".to_string(),
        ));
    }

    if let Err(err) = local_paths::record_plugin_share_local_path(
        codex_home,
        &response.plugin_id,
        plugin_path.clone(),
    ) {
        warn!(
            remote_plugin_id = %response.plugin_id,
            "failed to record plugin share local path mapping: {err}"
        );
    }

    Ok(RemotePluginShareSaveResult {
        remote_plugin_id: response.plugin_id,
        share_url: response.share_url,
    })
}

pub async fn list_remote_plugin_shares(
    config: &RemotePluginServiceConfig,
    auth: Option<&CodexAuth>,
    codex_home: &Path,
) -> Result<Vec<RemotePluginShareSummary>, RemotePluginCatalogError> {
    let auth = ensure_chatgpt_auth(auth)?;
    let created_plugins = fetch_created_workspace_plugins(config, auth).await?;
    if created_plugins.is_empty() {
        return Ok(Vec::new());
    }

    let installed_by_id =
        fetch_installed_plugins_for_scope(config, auth, RemotePluginScope::Workspace)
            .await?
            .into_iter()
            .map(|plugin| (plugin.plugin.id.clone(), plugin))
            .collect::<BTreeMap<_, _>>();
    let local_plugin_paths =
        local_paths::load_plugin_share_local_paths(codex_home).unwrap_or_else(|err| {
            warn!("failed to load plugin share local path mapping: {err}");
            BTreeMap::new()
        });

    Ok(created_plugins
        .into_iter()
        .map(|plugin| {
            let summary = build_remote_plugin_summary(&plugin, installed_by_id.get(&plugin.id));
            let local_plugin_path = local_plugin_paths.get(&plugin.id).cloned();
            RemotePluginShareSummary {
                summary,
                share_url: plugin.share_url,
                local_plugin_path,
            }
        })
        .collect())
}

pub async fn delete_remote_plugin_share(
    config: &RemotePluginServiceConfig,
    auth: Option<&CodexAuth>,
    codex_home: &Path,
    remote_plugin_id: &str,
) -> Result<(), RemotePluginCatalogError> {
    let auth = ensure_chatgpt_auth(auth)?;
    let base_url = config.chatgpt_base_url.trim_end_matches('/');
    let url = format!("{base_url}/public/plugins/workspace/{remote_plugin_id}");
    let client = build_reqwest_client();
    let request = authenticated_request(client.delete(&url), auth)?;
    send_and_expect_status(request, &url, &[StatusCode::NO_CONTENT]).await?;
    if let Err(err) = local_paths::remove_plugin_share_local_path(codex_home, remote_plugin_id) {
        warn!(
            remote_plugin_id = %remote_plugin_id,
            "failed to remove plugin share local path mapping: {err}"
        );
    }
    Ok(())
}

async fn fetch_created_workspace_plugins(
    config: &RemotePluginServiceConfig,
    auth: &CodexAuth,
) -> Result<Vec<RemotePluginDirectoryItem>, RemotePluginCatalogError> {
    let mut plugins = Vec::new();
    let mut page_token = None;
    loop {
        let response =
            get_created_workspace_plugins_page(config, auth, page_token.as_deref()).await?;
        plugins.extend(response.plugins);
        let Some(next_page_token) = response.pagination.next_page_token else {
            break;
        };
        page_token = Some(next_page_token);
    }
    Ok(plugins)
}

async fn get_created_workspace_plugins_page(
    config: &RemotePluginServiceConfig,
    auth: &CodexAuth,
    page_token: Option<&str>,
) -> Result<RemotePluginListResponse, RemotePluginCatalogError> {
    let base_url = config.chatgpt_base_url.trim_end_matches('/');
    let url = format!("{base_url}/ps/plugins/workspace/created");
    let client = build_reqwest_client();
    let mut request = authenticated_request(client.get(&url), auth)?;
    request = request.query(&[("limit", REMOTE_PLUGIN_LIST_PAGE_LIMIT)]);
    if let Some(page_token) = page_token {
        request = request.query(&[("pageToken", page_token)]);
    }
    send_and_decode(request, &url).await
}

async fn create_workspace_plugin_upload(
    config: &RemotePluginServiceConfig,
    auth: &CodexAuth,
    filename: &str,
    size_bytes: usize,
    remote_plugin_id: Option<&str>,
) -> Result<RemoteWorkspacePluginUploadUrlResponse, RemotePluginCatalogError> {
    let base_url = config.chatgpt_base_url.trim_end_matches('/');
    let url = format!("{base_url}/public/plugins/workspace/upload-url");
    let client = build_reqwest_client();
    let request = authenticated_request(client.post(&url), auth)?.json(
        &RemoteWorkspacePluginUploadUrlRequest {
            filename,
            mime_type: "application/gzip",
            size_bytes,
            plugin_id: remote_plugin_id,
        },
    );
    send_and_decode(request, &url).await
}

async fn put_workspace_plugin_upload(
    upload_url: &str,
    archive_bytes: Vec<u8>,
) -> Result<(), RemotePluginCatalogError> {
    let client = build_reqwest_client();
    let request = client
        .put(upload_url)
        .timeout(REMOTE_PLUGIN_CATALOG_TIMEOUT)
        .header("x-ms-blob-type", "BlockBlob")
        .header("Content-Type", "application/gzip")
        .body(archive_bytes);
    let response = request
        .send()
        .await
        .map_err(|source| RemotePluginCatalogError::Request {
            url: "workspace plugin upload URL".to_string(),
            source,
        })?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if ![StatusCode::OK, StatusCode::CREATED].contains(&status) {
        return Err(RemotePluginCatalogError::UnexpectedStatus {
            url: "workspace plugin upload URL".to_string(),
            status,
            body,
        });
    }
    Ok(())
}

async fn finalize_workspace_plugin_upload(
    config: &RemotePluginServiceConfig,
    auth: &CodexAuth,
    remote_plugin_id: Option<&str>,
    body: RemoteWorkspacePluginCreateRequest,
) -> Result<RemoteWorkspacePluginCreateResponse, RemotePluginCatalogError> {
    let base_url = config.chatgpt_base_url.trim_end_matches('/');
    let url = if let Some(remote_plugin_id) = remote_plugin_id {
        format!("{base_url}/public/plugins/workspace/{remote_plugin_id}")
    } else {
        format!("{base_url}/public/plugins/workspace")
    };
    let client = build_reqwest_client();
    let request = authenticated_request(client.post(&url), auth)?.json(&body);
    send_and_decode(request, &url).await
}

fn archive_filename(plugin_path: &Path) -> Result<String, RemotePluginCatalogError> {
    let plugin_name = plugin_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| RemotePluginCatalogError::InvalidPluginPath {
            path: plugin_path.to_path_buf(),
            reason: "plugin path must end in a valid UTF-8 directory name".to_string(),
        })?;
    Ok(format!("{plugin_name}.tar.gz"))
}

fn archive_plugin_for_upload(plugin_path: &Path) -> Result<Vec<u8>, RemotePluginCatalogError> {
    archive_plugin_for_upload_with_limit(plugin_path, REMOTE_PLUGIN_SHARE_MAX_ARCHIVE_BYTES)
}

fn archive_plugin_for_upload_with_limit(
    plugin_path: &Path,
    max_bytes: usize,
) -> Result<Vec<u8>, RemotePluginCatalogError> {
    if !plugin_path.is_dir() {
        return Err(RemotePluginCatalogError::InvalidPluginPath {
            path: plugin_path.to_path_buf(),
            reason: "expected a plugin directory".to_string(),
        });
    }
    if !plugin_path.join(".codex-plugin/plugin.json").is_file() {
        return Err(RemotePluginCatalogError::InvalidPluginPath {
            path: plugin_path.to_path_buf(),
            reason: "missing .codex-plugin/plugin.json".to_string(),
        });
    }

    let encoder = GzEncoder::new(SizeLimitedBuffer::new(max_bytes), Compression::default());
    let mut archive = tar::Builder::new(encoder);
    append_plugin_tree(&mut archive, plugin_path, plugin_path)
        .map_err(|source| archive_error(plugin_path, source))?;
    let encoder = archive
        .into_inner()
        .map_err(|source| archive_error(plugin_path, source))?;
    encoder
        .finish()
        .map(SizeLimitedBuffer::into_inner)
        .map_err(|source| archive_error(plugin_path, source))
}

fn append_plugin_tree<W: Write>(
    archive: &mut tar::Builder<W>,
    plugin_root: &Path,
    current: &Path,
) -> io::Result<()> {
    let mut entries = fs::read_dir(current)?.collect::<Result<Vec<_>, io::Error>>()?;
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let file_type = entry.file_type()?;
        let relative_path = path.strip_prefix(plugin_root).map_err(|err| {
            io::Error::other(format!(
                "failed to compute plugin archive path for `{}`: {err}",
                path.display()
            ))
        })?;
        if file_type.is_dir() {
            archive.append_dir(relative_path, &path)?;
            append_plugin_tree(archive, plugin_root, &path)?;
        } else if file_type.is_file() {
            archive.append_path_with_name(&path, relative_path)?;
        } else {
            return Err(io::Error::other(format!(
                "unsupported plugin archive entry type: {}",
                path.display()
            )));
        }
    }
    Ok(())
}

fn archive_error(plugin_path: &Path, source: io::Error) -> RemotePluginCatalogError {
    if let Some(limit) = source
        .get_ref()
        .and_then(|err| err.downcast_ref::<ArchiveSizeLimitExceeded>())
    {
        return RemotePluginCatalogError::ArchiveTooLarge {
            bytes: limit.bytes,
            max_bytes: limit.max_bytes,
        };
    }

    RemotePluginCatalogError::Archive {
        path: plugin_path.to_path_buf(),
        source,
    }
}

struct SizeLimitedBuffer {
    bytes: Vec<u8>,
    max_bytes: usize,
}

impl SizeLimitedBuffer {
    fn new(max_bytes: usize) -> Self {
        Self {
            bytes: Vec::new(),
            max_bytes,
        }
    }

    fn into_inner(self) -> Vec<u8> {
        self.bytes
    }
}

impl Write for SizeLimitedBuffer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let next_len = self.bytes.len().checked_add(buf.len()).ok_or_else(|| {
            io::Error::other(ArchiveSizeLimitExceeded {
                bytes: usize::MAX,
                max_bytes: self.max_bytes,
            })
        })?;
        if next_len > self.max_bytes {
            return Err(io::Error::other(ArchiveSizeLimitExceeded {
                bytes: next_len,
                max_bytes: self.max_bytes,
            }));
        }

        self.bytes.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Debug)]
struct ArchiveSizeLimitExceeded {
    bytes: usize,
    max_bytes: usize,
}

impl fmt::Display for ArchiveSizeLimitExceeded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "archive would be {} bytes, exceeding maximum size of {} bytes",
            self.bytes, self.max_bytes
        )
    }
}

impl std::error::Error for ArchiveSizeLimitExceeded {}

async fn send_and_expect_status(
    request: RequestBuilder,
    url_for_error: &str,
    expected_statuses: &[StatusCode],
) -> Result<(), RemotePluginCatalogError> {
    let response = request
        .send()
        .await
        .map_err(|source| RemotePluginCatalogError::Request {
            url: url_for_error.to_string(),
            source,
        })?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !expected_statuses.contains(&status) {
        return Err(RemotePluginCatalogError::UnexpectedStatus {
            url: url_for_error.to_string(),
            status,
            body,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests;
