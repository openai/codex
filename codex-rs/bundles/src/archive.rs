use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use codex_client::build_reqwest_client_with_custom_ca;
use sha2::Digest;
use sha2::Sha256;
use std::path::Component;
use std::path::Path;
use tokio::fs;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

pub(crate) async fn download_file(url: &str, destination: &Path) -> Result<()> {
    let client = build_reqwest_client_with_custom_ca(reqwest::Client::builder())
        .context("failed to build HTTP client")?;
    let mut response = client
        .get(url)
        .header("User-Agent", "codex-bundles-installer")
        .send()
        .await
        .with_context(|| format!("failed to download runtime archive {url}"))?;
    if !response.status().is_success() {
        bail!(
            "failed to download runtime archive {url} ({} {})",
            response.status().as_u16(),
            response.status().canonical_reason().unwrap_or("unknown")
        );
    }
    let mut file = fs::File::create(destination)
        .await
        .with_context(|| format!("failed to create {}", destination.display()))?;
    while let Some(chunk) = response
        .chunk()
        .await
        .with_context(|| format!("failed reading runtime archive {url}"))?
    {
        file.write_all(&chunk)
            .await
            .with_context(|| format!("failed writing {}", destination.display()))?;
    }
    file.flush()
        .await
        .with_context(|| format!("failed flushing {}", destination.display()))
}

pub(crate) async fn verify_archive_checksum(
    archive_path: &Path,
    expected_sha256: &str,
    source_url: &str,
) -> Result<()> {
    let actual_sha256 = compute_sha256(archive_path).await?;
    if actual_sha256 != expected_sha256.to_ascii_lowercase() {
        bail!(
            "checksum mismatch for `{source_url}`: expected {expected_sha256}, got {actual_sha256}"
        );
    }
    Ok(())
}

pub(crate) async fn compute_sha256(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)
        .await
        .with_context(|| format!("failed to open {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let bytes_read = file
            .read(&mut buffer)
            .await
            .with_context(|| format!("failed to read {}", path.display()))?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub(crate) async fn list_tar_entries(archive_path: &Path) -> Result<Vec<String>> {
    let output = Command::new("tar")
        .arg("-tf")
        .arg(archive_path)
        .output()
        .await
        .context("failed to run tar to list runtime archive")?;
    if !output.status.success() {
        bail!(
            "failed to list runtime archive: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(ToOwned::to_owned)
        .collect())
}

pub fn assert_archive_entries_stay_within_directory(entries: &[String]) -> Result<()> {
    for entry in entries {
        let path = Path::new(entry);
        if path.is_absolute()
            || path
                .components()
                .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
        {
            bail!("archive entry `{entry}` would extract outside target");
        }
    }
    Ok(())
}

pub(crate) async fn extract_tar_archive(archive_path: &Path, extract_dir: &Path) -> Result<()> {
    let output = Command::new("tar")
        .arg("-xJf")
        .arg(archive_path)
        .arg("-C")
        .arg(extract_dir)
        .output()
        .await
        .context("failed to run tar to extract runtime archive")?;
    if !output.status.success() {
        bail!(
            "failed to extract runtime archive: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

pub(crate) fn validate_sha256(value: &str) -> Result<()> {
    if value.len() == 64 && value.chars().all(|ch| ch.is_ascii_hexdigit()) {
        Ok(())
    } else {
        bail!("expected a sha256 hex digest, got `{value}`")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_archive_traversal_entries() {
        let err = assert_archive_entries_stay_within_directory(&[
            "codex-primary-runtime/runtime.json".to_string(),
            "../escape".to_string(),
        ])
        .expect_err("traversal rejected");
        assert!(err.to_string().contains("outside target"));
    }
}
