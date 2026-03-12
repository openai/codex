use crate::PackageManagerError;
use flate2::read::GzDecoder;
use sha2::Digest;
use sha2::Sha256;
use std::fs::File;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use tar::Archive;
use zip::ZipArchive;

/// Archive metadata for a platform entry in a release manifest.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub struct PackageReleaseArchive {
    /// Archive file name relative to the package release location.
    pub archive: String,
    /// Expected SHA-256 of the downloaded archive body.
    pub sha256: String,
    /// Archive format used by the download.
    pub format: ArchiveFormat,
    /// Expected archive length in bytes, when the manifest provides it.
    pub size_bytes: Option<u64>,
}

/// Archive formats supported by the generic extractor.
#[derive(Clone, Copy, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub enum ArchiveFormat {
    /// A `.zip` archive.
    #[serde(rename = "zip")]
    Zip,
    /// A `.tar.gz` archive.
    #[serde(rename = "tar.gz")]
    TarGz,
}

/// Detects a package root with a `manifest.json` in an extraction directory.
pub(crate) fn detect_single_package_root(
    extraction_root: &Path,
) -> Result<PathBuf, PackageManagerError> {
    let direct_manifest = extraction_root.join("manifest.json");
    if direct_manifest.exists() {
        return Ok(extraction_root.to_path_buf());
    }

    let mut directory_candidates = Vec::new();
    for entry in std::fs::read_dir(extraction_root).map_err(|source| PackageManagerError::Io {
        context: format!("failed to read {}", extraction_root.display()),
        source,
    })? {
        let entry = entry.map_err(|source| PackageManagerError::Io {
            context: format!("failed to read entry in {}", extraction_root.display()),
            source,
        })?;
        let path = entry.path();
        if path.is_dir() {
            directory_candidates.push(path);
        }
    }

    if directory_candidates.len() == 1 {
        let candidate = &directory_candidates[0];
        if candidate.join("manifest.json").exists() {
            return Ok(candidate.clone());
        }
    }

    Err(PackageManagerError::MissingPackageRoot(
        extraction_root.to_path_buf(),
    ))
}

pub(crate) fn verify_archive_size(
    bytes: &[u8],
    expected: Option<u64>,
) -> Result<(), PackageManagerError> {
    let Some(expected) = expected else {
        return Ok(());
    };
    let actual = bytes.len() as u64;
    if actual == expected {
        return Ok(());
    }
    Err(PackageManagerError::UnexpectedArchiveSize { expected, actual })
}

pub(crate) fn verify_sha256(bytes: &[u8], expected: &str) -> Result<(), PackageManagerError> {
    let actual = format!("{:x}", Sha256::digest(bytes));
    if actual == expected.to_ascii_lowercase() {
        return Ok(());
    }
    Err(PackageManagerError::ChecksumMismatch {
        expected: expected.to_string(),
        actual,
    })
}

pub(crate) fn extract_archive(
    archive_path: &Path,
    destination: &Path,
    format: ArchiveFormat,
) -> Result<(), PackageManagerError> {
    match format {
        ArchiveFormat::Zip => extract_zip_archive(archive_path, destination),
        ArchiveFormat::TarGz => extract_tar_gz_archive(archive_path, destination),
    }
}

fn extract_zip_archive(archive_path: &Path, destination: &Path) -> Result<(), PackageManagerError> {
    let file = File::open(archive_path).map_err(|source| PackageManagerError::Io {
        context: format!("failed to open {}", archive_path.display()),
        source,
    })?;
    let mut archive = ZipArchive::new(file)
        .map_err(|error| PackageManagerError::ArchiveExtraction(error.to_string()))?;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| PackageManagerError::ArchiveExtraction(error.to_string()))?;
        let Some(relative_path) = entry.enclosed_name() else {
            return Err(PackageManagerError::ArchiveExtraction(format!(
                "zip entry `{}` escapes extraction root",
                entry.name()
            )));
        };
        let output_path = destination.join(relative_path);
        if entry.is_dir() {
            std::fs::create_dir_all(&output_path).map_err(|source| PackageManagerError::Io {
                context: format!("failed to create {}", output_path.display()),
                source,
            })?;
            continue;
        }
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| PackageManagerError::Io {
                context: format!("failed to create {}", parent.display()),
                source,
            })?;
        }
        let mut output = File::create(&output_path).map_err(|source| PackageManagerError::Io {
            context: format!("failed to create {}", output_path.display()),
            source,
        })?;
        std::io::copy(&mut entry, &mut output).map_err(|source| PackageManagerError::Io {
            context: format!("failed to write {}", output_path.display()),
            source,
        })?;
        apply_zip_permissions(&entry, &output_path)?;
    }
    Ok(())
}

#[cfg(unix)]
fn apply_zip_permissions(
    entry: &zip::read::ZipFile<'_>,
    output_path: &Path,
) -> Result<(), PackageManagerError> {
    let Some(mode) = entry.unix_mode() else {
        return Ok(());
    };
    std::fs::set_permissions(output_path, std::fs::Permissions::from_mode(mode)).map_err(|source| {
        PackageManagerError::Io {
            context: format!("failed to set permissions on {}", output_path.display()),
            source,
        }
    })
}

#[cfg(not(unix))]
fn apply_zip_permissions(
    _entry: &zip::read::ZipFile<'_>,
    _output_path: &Path,
) -> Result<(), PackageManagerError> {
    Ok(())
}

fn extract_tar_gz_archive(
    archive_path: &Path,
    destination: &Path,
) -> Result<(), PackageManagerError> {
    let file = File::open(archive_path).map_err(|source| PackageManagerError::Io {
        context: format!("failed to open {}", archive_path.display()),
        source,
    })?;
    let decoder = GzDecoder::new(file);
    let mut archive = Archive::new(decoder);
    for entry in archive
        .entries()
        .map_err(|error| PackageManagerError::ArchiveExtraction(error.to_string()))?
    {
        let mut entry =
            entry.map_err(|error| PackageManagerError::ArchiveExtraction(error.to_string()))?;
        let path = entry
            .path()
            .map_err(|error| PackageManagerError::ArchiveExtraction(error.to_string()))?;
        let output_path = safe_extract_path(destination, path.as_ref())?;
        let entry_type = entry.header().entry_type();

        if entry_type.is_symlink()
            || entry_type.is_hard_link()
            || entry_type.is_block_special()
            || entry_type.is_character_special()
            || entry_type.is_fifo()
            || entry_type.is_gnu_sparse()
        {
            return Err(PackageManagerError::ArchiveExtraction(format!(
                "tar entry `{}` has unsupported type",
                path.display()
            )));
        }

        if entry_type.is_pax_global_extensions()
            || entry_type.is_pax_local_extensions()
            || entry_type.is_gnu_longname()
            || entry_type.is_gnu_longlink()
        {
            continue;
        }

        if entry_type.is_dir() {
            std::fs::create_dir_all(&output_path).map_err(|source| PackageManagerError::Io {
                context: format!("failed to create {}", output_path.display()),
                source,
            })?;
            continue;
        }

        if !entry_type.is_file() && !entry_type.is_contiguous() {
            return Err(PackageManagerError::ArchiveExtraction(format!(
                "tar entry `{}` has unsupported type",
                path.display()
            )));
        }

        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| PackageManagerError::Io {
                context: format!("failed to create {}", parent.display()),
                source,
            })?;
        }
        entry
            .unpack(&output_path)
            .map_err(|error| PackageManagerError::ArchiveExtraction(error.to_string()))?;
    }
    Ok(())
}

fn safe_extract_path(root: &Path, relative_path: &Path) -> Result<PathBuf, PackageManagerError> {
    let mut clean_relative = PathBuf::new();
    for component in relative_path.components() {
        match component {
            Component::Normal(segment) => clean_relative.push(segment),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(PackageManagerError::ArchiveExtraction(format!(
                    "entry `{}` escapes extraction root",
                    relative_path.display()
                )));
            }
        }
    }

    if clean_relative.as_os_str().is_empty() {
        return Err(PackageManagerError::ArchiveExtraction(
            "archive entry had an empty path".to_string(),
        ));
    }
    Ok(root.join(clean_relative))
}
