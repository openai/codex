//! Built-in pet asset acquisition and cache ownership.
//!
//! Unlike custom pets, built-in pets are not checked into the TUI package as
//! local spritesheets. The TUI resolves them from the public Codex pets CDN on
//! first use, verifies that the downloaded file has the expected spritesheet
//! geometry, and installs it into a versioned cache under CODEX_HOME.
//!
//! This module deliberately stops at "a validated spritesheet exists at this
//! path". Higher layers remain responsible for deciding when downloads are
//! allowed, when previews should block on them, and when a successfully loaded
//! built-in pet is safe to persist to config.

use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use codex_http_client::RouteAwareClientPool;
use url::Url;
use uuid::Uuid;

use super::catalog;

const PET_PACK_VERSION: &str = "v1";
const PET_PACK_DIR: &str = "cache/tui-pets";
const PET_CDN_BASE_URL: &str = "https://persistent.oaistatic.com/codex/pets/v1";
const PET_DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(60);
const PET_MAX_DOWNLOAD_BYTES: u64 = 4 * 1024 * 1024;

pub(crate) fn builtin_spritesheet_path(codex_home: &Path, file: &str) -> PathBuf {
    pack_dir(codex_home).join("assets").join(file)
}

/// Ensure that a built-in pet's spritesheet is present and structurally valid.
///
/// The cache key is the CDN-facing filename, so updating a built-in pet means
/// publishing a new versioned filename rather than mutating an existing one in
/// place. If a cached file is missing or invalid, this downloads a fresh copy,
/// validates the decoded image dimensions, and installs it atomically. Callers
/// should treat any error here as "the asset is unavailable", not as a partial
/// install they can safely ignore.
pub(crate) async fn ensure_builtin_pet(
    codex_home: &Path,
    pet: catalog::BuiltinPet,
    http_client: &RouteAwareClientPool,
) -> Result<()> {
    let destination = builtin_spritesheet_path(codex_home, pet.spritesheet_file);
    let cache_destination = destination.clone();
    let cache_valid = tokio::task::spawn_blocking(move || {
        validate_cached_spritesheet(&cache_destination).is_ok()
    })
    .await
    .context("join pet spritesheet cache validation task")?;
    if cache_valid {
        return Ok(());
    }

    let url = builtin_pet_url(pet)?;
    let bytes = download_bytes_with_limit(http_client, &url, PET_MAX_DOWNLOAD_BYTES).await?;
    tokio::task::spawn_blocking(move || {
        let parent = destination
            .parent()
            .context("pet spritesheet path should include an assets directory")?;
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;

        let staging = destination.with_file_name(format!(
            ".{}.download-{}.webp",
            pet.spritesheet_file,
            Uuid::new_v4()
        ));
        fs::write(&staging, &bytes).with_context(|| format!("write {}", staging.display()))?;
        if let Err(err) = validate_cached_spritesheet(&staging) {
            let _ = fs::remove_file(&staging);
            return Err(err);
        }

        if install_downloaded_spritesheet(&staging, &destination).is_ok() {
            return Ok(());
        }

        if validate_cached_spritesheet(&destination).is_ok() {
            let _ = fs::remove_file(&staging);
            return Ok(());
        }

        if destination.exists() {
            fs::remove_file(&destination)
                .with_context(|| format!("remove {}", destination.display()))?;
        }
        install_downloaded_spritesheet(&staging, &destination)
    })
    .await
    .context("join pet spritesheet install task")?
}

fn builtin_pet_url(pet: catalog::BuiltinPet) -> Result<String> {
    let url = format!("{PET_CDN_BASE_URL}/{}", pet.spritesheet_file);
    validate_download_url(&url)?;
    Ok(url)
}

fn pack_dir(codex_home: &Path) -> PathBuf {
    codex_home.join(PET_PACK_DIR).join(PET_PACK_VERSION)
}

async fn download_bytes_with_limit(
    http_client: &RouteAwareClientPool,
    url: &str,
    max_bytes: u64,
) -> Result<Vec<u8>> {
    validate_download_url(url)?;
    let mut response = http_client
        .get(url)
        .timeout(PET_DOWNLOAD_TIMEOUT)
        .send()
        .await
        .with_context(|| format!("download pet asset from {url}"))?
        .error_for_status()
        .with_context(|| format!("download pet asset from {url}"))?;
    validate_download_url(response.url().as_str())?;

    if response.content_length().is_some_and(|len| len > max_bytes) {
        bail!("pet asset download from {url} exceeded {max_bytes} bytes");
    }

    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .with_context(|| format!("read pet asset download from {url}"))?
    {
        append_download_chunk(&mut bytes, &chunk, max_bytes, url)?;
    }
    Ok(bytes)
}

fn append_download_chunk(
    bytes: &mut Vec<u8>,
    chunk: &[u8],
    max_bytes: u64,
    url: &str,
) -> Result<()> {
    if (bytes.len() as u64).saturating_add(chunk.len() as u64) > max_bytes {
        bail!("pet asset download from {url} exceeded {max_bytes} bytes");
    }
    bytes.extend_from_slice(chunk);
    Ok(())
}

fn install_downloaded_spritesheet(staging: &Path, destination: &Path) -> Result<()> {
    fs::rename(staging, destination).with_context(|| format!("install {}", destination.display()))
}

fn validate_download_url(value: &str) -> Result<()> {
    let url = Url::parse(value).with_context(|| format!("parse pet asset download URL {value}"))?;
    if url.scheme() != "https" {
        bail!("unsupported pet asset download URL scheme {}", url.scheme());
    }
    Ok(())
}

fn validate_cached_spritesheet(path: &Path) -> Result<()> {
    let (width, height) =
        image::image_dimensions(path).with_context(|| format!("read {}", path.display()))?;
    if width != catalog::SPRITESHEET_WIDTH || height != catalog::SPRITESHEET_HEIGHT {
        bail!(
            "invalid pet spritesheet dimensions for {}: expected {}x{}, got {}x{}",
            path.display(),
            catalog::SPRITESHEET_WIDTH,
            catalog::SPRITESHEET_HEIGHT,
            width,
            height
        );
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn write_test_pack(codex_home: &Path) {
    let assets_dir = pack_dir(codex_home).join("assets");
    fs::create_dir_all(&assets_dir).unwrap();
    for pet in catalog::BUILTIN_PETS {
        let path = assets_dir.join(pet.spritesheet_file);
        catalog::write_test_spritesheet(&path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn builtin_pet_url_uses_public_cdn_path() {
        let pet = catalog::builtin_pet("dewey").unwrap();

        let url = builtin_pet_url(pet).unwrap();

        assert_eq!(
            url,
            "https://persistent.oaistatic.com/codex/pets/v1/dewey-spritesheet-v4.webp"
        );
    }

    #[test]
    fn oversized_download_chunk_is_rejected() {
        let url = "https://example.com/pet.webp";
        let mut bytes = Vec::new();

        append_download_chunk(&mut bytes, b"1234", /*max_bytes*/ 8, url).unwrap();
        let error = append_download_chunk(&mut bytes, b"56789", /*max_bytes*/ 8, url)
            .expect_err("chunk should exceed the download limit");

        assert_eq!(
            error.to_string(),
            "pet asset download from https://example.com/pet.webp exceeded 8 bytes"
        );
        assert_eq!(bytes, b"1234");
    }

    #[test]
    fn write_test_pack_installs_all_builtins() {
        let dir = tempfile::tempdir().unwrap();

        write_test_pack(dir.path());

        for pet in catalog::BUILTIN_PETS {
            let path = builtin_spritesheet_path(dir.path(), pet.spritesheet_file);
            assert!(path.is_file());
            validate_cached_spritesheet(&path).unwrap();
        }
    }
}
