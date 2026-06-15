//! Locates the trusted Codex Desktop resources bundled with the installed app.

use std::fs;
use std::io;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use codex_utils_absolute_path::AbsolutePathBuf;

#[cfg(target_os = "macos")]
#[path = "macos.rs"]
mod platform;
#[cfg(not(any(target_os = "macos", windows)))]
#[path = "unsupported.rs"]
mod platform;
#[cfg(windows)]
#[path = "windows.rs"]
mod platform;

// Resource override for trusted Desktop development builds and external CLI launchers.
const DESKTOP_RESOURCES_PATH_ENV_VAR: &str = "CODEX_DESKTOP_RESOURCES_PATH";

#[derive(Debug, thiserror::Error)]
pub enum DesktopInstallationError {
    /// No launcher hint was provided and no installed Codex Desktop app was found. This can
    /// happen, for example, when Desktop was uninstalled after installing a bundled plugin.
    #[error("no Codex Desktop installation was found")]
    NotFound,
    /// The platform app-discovery mechanism could not be queried successfully.
    #[error("Codex Desktop discovery failed: {0}")]
    Discovery(String),
    /// An expected app resource could not be read or canonicalized.
    #[error("Codex Desktop filesystem validation failed during {stage}: {source}")]
    Filesystem {
        stage: &'static str,
        #[source]
        source: io::Error,
    },
    /// A requested resource was absolute, contained `..`, escaped through a symlink, or had the
    /// wrong file kind, so it was not strictly contained beneath Desktop's resources directory.
    #[error("Codex Desktop resource containment failed: {0}")]
    Containment(String),
}

#[derive(Debug)]
pub struct DesktopResources {
    root: AbsolutePathBuf,
}

impl DesktopResources {
    pub fn contained_file(
        &self,
        relative_path: impl AsRef<Path>,
    ) -> Result<AbsolutePathBuf, DesktopInstallationError> {
        contained_path(&self.root, relative_path.as_ref(), ResourceKind::File)
    }

    pub fn contained_directory(
        &self,
        relative_path: impl AsRef<Path>,
    ) -> Result<AbsolutePathBuf, DesktopInstallationError> {
        contained_path(&self.root, relative_path.as_ref(), ResourceKind::Directory)
    }

    /// Uses a resources directory supplied by the trusted Desktop launcher.
    pub fn from_trusted_path(root: PathBuf) -> Result<Self, DesktopInstallationError> {
        let root = canonical(&root, "resources root")?;
        if !root.as_path().is_dir() {
            return Err(containment("expected the resources root to be a directory"));
        }
        Ok(Self { root })
    }
}

#[derive(Debug)]
pub struct VerifiedDesktopInstallation {
    app_root: AbsolutePathBuf,
    resources: DesktopResources,
}

impl VerifiedDesktopInstallation {
    pub fn app_root(&self) -> &Path {
        self.app_root.as_path()
    }

    #[cfg(any(target_os = "macos", windows, test))]
    fn from_paths(
        app_root: PathBuf,
        resources_root: PathBuf,
    ) -> Result<Self, DesktopInstallationError> {
        let app_root = canonical(&app_root, "application root")?;
        let resources = DesktopResources::from_trusted_path(resources_root)?;
        if resources.root == app_root || !resources.root.as_path().starts_with(app_root.as_path()) {
            return Err(containment(
                "resources root is not strictly below the discovered application root",
            ));
        }
        Ok(Self {
            app_root,
            resources,
        })
    }
}

/// Uses resources supplied by Desktop, otherwise discovers the installed stable app.
pub fn locate_current_or_installed_resources() -> Result<DesktopResources, DesktopInstallationError>
{
    if let Some(resources_root) = std::env::var_os(DESKTOP_RESOURCES_PATH_ENV_VAR) {
        return DesktopResources::from_trusted_path(PathBuf::from(resources_root));
    }
    discover_desktop_installation()?
        .map(|installation| installation.resources)
        .ok_or(DesktopInstallationError::NotFound)
}

/// Discovers an installed Desktop app without consulting the launcher resources hint.
pub fn discover_desktop_installation()
-> Result<Option<VerifiedDesktopInstallation>, DesktopInstallationError> {
    platform::discover()
}

/// Validates a macOS Codex app bundle at a known path using the same identity and containment
/// checks as installed-app discovery. Returns `Ok(None)` when the path is not a trusted Codex
/// Desktop installation.
#[cfg(target_os = "macos")]
pub fn validate_desktop_installation_at(
    app_root: impl AsRef<Path>,
) -> Result<Option<VerifiedDesktopInstallation>, DesktopInstallationError> {
    platform::validate_candidate(app_root.as_ref())
}

#[derive(Clone, Copy)]
enum ResourceKind {
    Directory,
    File,
}

/// Resolves a resource path beneath the trusted Desktop resources root.
///
/// Rejects empty paths and non-normal components, canonicalizes the joined path to catch symlink
/// escapes, requires the result to remain strictly below `root`, and verifies that it has the
/// requested file kind.
fn contained_path(
    root: &AbsolutePathBuf,
    relative_path: &Path,
    kind: ResourceKind,
) -> Result<AbsolutePathBuf, DesktopInstallationError> {
    if relative_path.as_os_str().is_empty()
        || relative_path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(containment(
            "resource paths must contain only normal relative components",
        ));
    }
    let path = canonical(root.join(relative_path).as_path(), "resource path")?;
    if path == *root || !path.as_path().starts_with(root.as_path()) {
        return Err(containment(
            "resource must remain strictly below the Desktop resources root",
        ));
    }
    let metadata =
        fs::metadata(path.as_path()).map_err(|source| DesktopInstallationError::Filesystem {
            stage: "resource metadata",
            source,
        })?;
    let expected_kind = match kind {
        ResourceKind::Directory => metadata.is_dir(),
        ResourceKind::File => metadata.is_file(),
    };
    if !expected_kind {
        return Err(containment(match kind {
            ResourceKind::Directory => "expected a directory",
            ResourceKind::File => "expected a regular file",
        }));
    }
    Ok(path)
}

fn canonical(
    path: &Path,
    stage: &'static str,
) -> Result<AbsolutePathBuf, DesktopInstallationError> {
    AbsolutePathBuf::from_absolute_path_checked(path)
        .and_then(|path| path.canonicalize())
        .map_err(|source| DesktopInstallationError::Filesystem { stage, source })
}

fn containment(message: impl Into<String>) -> DesktopInstallationError {
    DesktopInstallationError::Containment(message.into())
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
