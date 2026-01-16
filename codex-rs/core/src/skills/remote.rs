use crate::default_client::create_client;
use sha2::Digest;
use sha2::Sha256;
use std::fs;
use std::future::Future;
use std::io::Cursor;
use std::path::Path;
use std::path::PathBuf;
use thiserror::Error;
use url::Url;
use zip::ZipArchive;

const REMOTE_CACHE_DIR_NAME: &str = ".remote";
const SOURCE_URL_MARKER_FILENAME: &str = ".codex-skill-source.url";
const SKILL_FILENAME: &str = "SKILL.md";

#[derive(Debug, Error)]
pub(crate) enum RemoteSkillError {
    #[error("invalid URL: {url}")]
    InvalidUrl { url: String },
    #[error("unsupported URL scheme: {scheme}")]
    UnsupportedScheme { scheme: String },
    #[error("request failed: {source}")]
    Request {
        #[from]
        source: reqwest::Error,
    },
    #[error("failed to extract zip: {source}")]
    Zip {
        #[from]
        source: zip::result::ZipError,
    },
    #[error("invalid zip entry: {name}")]
    InvalidZipEntry { name: String },
    #[error("io error: {source}")]
    Io {
        #[from]
        source: std::io::Error,
    },
}

pub(crate) fn remote_cache_root_dir(codex_home: &Path) -> PathBuf {
    codex_home.join("skills").join(REMOTE_CACHE_DIR_NAME)
}

pub(crate) fn ensure_remote_skill(
    cache_root: &Path,
    url: &str,
    force_reload: bool,
) -> Result<PathBuf, RemoteSkillError> {
    let parsed = Url::parse(url).map_err(|_| RemoteSkillError::InvalidUrl {
        url: url.to_string(),
    })?;
    let cache_dir = cache_root.join(url_hash(parsed.as_str()));
    if cache_dir.exists() && !force_reload {
        return Ok(cache_dir);
    }

    fs::create_dir_all(cache_root)?;
    if cache_dir.exists() {
        fs::remove_dir_all(&cache_dir)?;
    }

    let temp_dir = tempfile::Builder::new()
        .prefix("skill-download-")
        .tempdir_in(cache_root)?;
    let temp_path = temp_dir.path().to_path_buf();

    download_into(&parsed, &temp_path)?;
    fs::write(temp_path.join(SOURCE_URL_MARKER_FILENAME), parsed.as_str())?;

    let temp_path = temp_dir.keep();
    fs::rename(&temp_path, &cache_dir)?;
    Ok(cache_dir)
}

fn download_into(url: &Url, dest: &Path) -> Result<(), RemoteSkillError> {
    match url.scheme() {
        "http" | "https" => {}
        scheme => {
            return Err(RemoteSkillError::UnsupportedScheme {
                scheme: scheme.to_string(),
            });
        }
    }

    let client = create_client();
    let bytes = block_on(fetch_bytes(client, url.clone()))?;

    let path_lower = url.path().to_ascii_lowercase();
    if path_lower.ends_with(".skill") || path_lower.ends_with(".zip") {
        extract_zip(&bytes, dest)?;
    } else {
        fs::create_dir_all(dest)?;
        fs::write(dest.join(SKILL_FILENAME), bytes)?;
    }

    Ok(())
}

fn extract_zip(bytes: &[u8], dest: &Path) -> Result<(), RemoteSkillError> {
    fs::create_dir_all(dest)?;
    let reader = Cursor::new(bytes);
    let mut archive = ZipArchive::new(reader)?;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let Some(entry_path) = entry.enclosed_name() else {
            return Err(RemoteSkillError::InvalidZipEntry {
                name: entry.name().to_string(),
            });
        };
        let out_path = dest.join(entry_path);
        if entry.is_dir() {
            fs::create_dir_all(&out_path)?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut out_file = fs::File::create(&out_path)?;
        std::io::copy(&mut entry, &mut out_file)?;
    }
    Ok(())
}

async fn fetch_bytes(
    client: codex_client::CodexHttpClient,
    url: Url,
) -> Result<Vec<u8>, RemoteSkillError> {
    let response = client.get(url).send().await?.error_for_status()?;
    Ok(response.bytes().await?.to_vec())
}

fn block_on<F, T>(future: F) -> Result<T, RemoteSkillError>
where
    F: Future<Output = Result<T, RemoteSkillError>> + Send + 'static,
    T: Send + 'static,
{
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        return Ok(tokio::task::block_in_place(|| handle.block_on(future))?);
    }

    let runtime = tokio::runtime::Runtime::new()?;
    Ok(runtime.block_on(future)?)
}

fn url_hash(url: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(url.as_bytes());
    hex_encode(&hasher.finalize())
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(out, "{byte:02x}");
    }
    out
}
