//! Locate and authenticate an installed Codex Desktop distribution.

use codex_utils_absolute_path::AbsolutePathBuf;
use std::fs;
use std::io;
use std::path::Component;
use std::path::Path;

#[cfg(target_os = "macos")]
#[path = "macos.rs"]
mod platform;
#[cfg(not(any(target_os = "macos", windows)))]
#[path = "unsupported.rs"]
mod platform;
#[cfg(windows)]
#[path = "windows.rs"]
mod platform;

#[cfg(test)]
mod tests;

#[derive(Debug, thiserror::Error)]
pub enum DesktopDistributionError {
    #[error("Codex Desktop distribution discovery is unsupported on this platform")]
    Unsupported,
    #[error("no authenticated Codex Desktop distribution was found")]
    NotFound,
    #[error("Codex Desktop verification failed during {stage}: {message}")]
    Verification {
        stage: &'static str,
        message: String,
    },
    #[error("Codex Desktop filesystem validation failed during {stage}: {source}")]
    Filesystem {
        stage: &'static str,
        #[source]
        source: io::Error,
    },
    #[error("Codex Desktop resource containment failed: {message}")]
    Containment { message: String },
}

impl DesktopDistributionError {
    pub fn stage(&self) -> &'static str {
        match self {
            Self::Unsupported => "platform",
            Self::NotFound => "discovery",
            Self::Verification { stage, .. } | Self::Filesystem { stage, .. } => stage,
            Self::Containment { .. } => "containment",
        }
    }
}

#[derive(Debug, Clone)]
pub struct VerifiedDesktopDistribution {
    app_root: AbsolutePathBuf,
    resources_root: AbsolutePathBuf,
    identity: platform::PlatformIdentity,
}

impl VerifiedDesktopDistribution {
    pub fn app_root(&self) -> &AbsolutePathBuf {
        &self.app_root
    }

    pub fn resources_root(&self) -> &AbsolutePathBuf {
        &self.resources_root
    }

    pub fn contained_file(
        &self,
        relative_path: impl AsRef<Path>,
    ) -> Result<AbsolutePathBuf, DesktopDistributionError> {
        self.contained_resource(relative_path.as_ref(), ResourceKind::File)
    }

    pub fn contained_directory(
        &self,
        relative_path: impl AsRef<Path>,
    ) -> Result<AbsolutePathBuf, DesktopDistributionError> {
        self.contained_resource(relative_path.as_ref(), ResourceKind::Directory)
    }

    pub fn reverify(&self) -> Result<(), DesktopDistributionError> {
        platform::reverify(&self.identity, self.app_root.as_path())
    }

    #[cfg(target_os = "macos")]
    pub fn authenticate_spawned_executable(
        &self,
        pid: i32,
        expected_executable: &Path,
        expected_identifier: &str,
    ) -> Result<(), DesktopDistributionError> {
        let relative_path = expected_executable
            .strip_prefix(self.resources_root.as_path())
            .map_err(|_| DesktopDistributionError::Containment {
                message: "spawned executable escaped the authenticated resources root".to_string(),
            })?;
        let current_executable = self.contained_file(relative_path)?;
        if current_executable.as_path() != expected_executable {
            return Err(DesktopDistributionError::Containment {
                message: "spawned executable path changed after verification".to_string(),
            });
        }
        self.reverify()?;
        platform::authenticate_spawned_executable(
            pid,
            current_executable.as_path(),
            expected_identifier,
        )
    }

    fn from_platform(
        distribution: platform::PlatformDistribution,
    ) -> Result<Self, DesktopDistributionError> {
        reject_link_or_reparse(distribution.app_root.as_path(), "distribution root")?;
        let app_root = canonical_absolute(distribution.app_root.as_path(), "distribution root")?;
        let resources_root = contained_existing_path(
            &app_root,
            distribution.resources_relative_path.as_path(),
            ResourceKind::Directory,
        )?;
        Ok(Self {
            app_root,
            resources_root,
            identity: distribution.identity,
        })
    }

    fn contained_resource(
        &self,
        relative_path: &Path,
        kind: ResourceKind,
    ) -> Result<AbsolutePathBuf, DesktopDistributionError> {
        contained_existing_path(&self.resources_root, relative_path, kind)
    }
}

pub fn verify_enclosing_distribution(
    hint: &Path,
) -> Result<VerifiedDesktopDistribution, DesktopDistributionError> {
    VerifiedDesktopDistribution::from_platform(platform::verify_hint(hint)?)
}

pub fn discover_installed_distribution()
-> Result<VerifiedDesktopDistribution, DesktopDistributionError> {
    VerifiedDesktopDistribution::from_platform(platform::discover()?)
}

pub fn locate_current_or_installed_distribution()
-> Result<VerifiedDesktopDistribution, DesktopDistributionError> {
    if let Some(distribution) = platform::current_process_distribution()? {
        return VerifiedDesktopDistribution::from_platform(distribution);
    }
    discover_installed_distribution()
}

#[derive(Clone, Copy)]
enum ResourceKind {
    Directory,
    File,
}

fn contained_existing_path(
    root: &AbsolutePathBuf,
    relative_path: &Path,
    kind: ResourceKind,
) -> Result<AbsolutePathBuf, DesktopDistributionError> {
    let components = relative_components(relative_path)?;
    let mut candidate = root.as_path().to_path_buf();
    for component in components {
        candidate.push(component);
        reject_link_or_reparse(&candidate, "resource path")?;
    }
    let canonical = canonical_absolute(&candidate, "resource path")?;
    if canonical.as_path() == root.as_path() || !canonical.as_path().starts_with(root.as_path()) {
        return Err(DesktopDistributionError::Containment {
            message: "resolved path must remain strictly below the authenticated root".to_string(),
        });
    }
    let metadata = fs::metadata(canonical.as_path()).map_err(|source| {
        DesktopDistributionError::Filesystem {
            stage: "resource metadata",
            source,
        }
    })?;
    let valid_kind = match kind {
        ResourceKind::Directory => metadata.is_dir(),
        ResourceKind::File => metadata.is_file(),
    };
    if !valid_kind {
        return Err(DesktopDistributionError::Containment {
            message: match kind {
                ResourceKind::Directory => "expected an authenticated directory".to_string(),
                ResourceKind::File => "expected an authenticated regular file".to_string(),
            },
        });
    }
    Ok(canonical)
}

fn relative_components(path: &Path) -> Result<Vec<&std::ffi::OsStr>, DesktopDistributionError> {
    let components = path
        .components()
        .map(|component| match component {
            Component::Normal(component) => Ok(component),
            Component::CurDir
            | Component::ParentDir
            | Component::RootDir
            | Component::Prefix(_) => Err(DesktopDistributionError::Containment {
                message: "resource paths must contain only normal relative components".to_string(),
            }),
        })
        .collect::<Result<Vec<_>, _>>()?;
    if components.is_empty() {
        return Err(DesktopDistributionError::Containment {
            message: "resource path must not be empty".to_string(),
        });
    }
    Ok(components)
}

fn canonical_absolute(
    path: &Path,
    stage: &'static str,
) -> Result<AbsolutePathBuf, DesktopDistributionError> {
    let canonical = fs::canonicalize(path)
        .map_err(|source| DesktopDistributionError::Filesystem { stage, source })?;
    AbsolutePathBuf::try_from(canonical).map_err(|err| DesktopDistributionError::Containment {
        message: format!("canonical path was not absolute: {err}"),
    })
}

fn reject_link_or_reparse(
    path: &Path,
    stage: &'static str,
) -> Result<(), DesktopDistributionError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|source| DesktopDistributionError::Filesystem { stage, source })?;
    if metadata.file_type().is_symlink() || is_windows_reparse_point(&metadata) {
        return Err(DesktopDistributionError::Containment {
            message: "authenticated roots and resources must not traverse links or reparse points"
                .to_string(),
        });
    }
    Ok(())
}

#[cfg(windows)]
fn is_windows_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_windows_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}
