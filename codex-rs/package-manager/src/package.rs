use crate::PackageManagerError;
use crate::PackagePlatform;
use crate::PackageReleaseArchive;
use crate::archive::detect_single_package_root;
use serde::de::DeserializeOwned;
use std::path::Path;
use std::path::PathBuf;
use url::Url;

/// Describes how a specific package is located, validated, and loaded.
///
/// Implementations should treat this trait as the package manager contract:
///
/// - [`Self::install_dir`] should resolve to a directory unique to the package version and
///   platform so concurrent versions never overwrite each other.
/// - [`Self::load_installed`] should fully validate whatever "installed" means for the package,
///   because cache resolution trusts a successful load as a valid install.
/// - The default [`Self::detect_extracted_root`] implementation expects the extracted archive to
///   contain a `manifest.json` at the package root or a single top-level directory that does.
pub trait ManagedPackage: Clone {
    /// Error type surfaced by package-specific loading and validation.
    type Error: From<PackageManagerError>;

    /// The fully loaded package instance returned to callers.
    type Installed: Clone;

    /// The decoded release manifest fetched from the remote source.
    type ReleaseManifest: DeserializeOwned;

    /// Returns the default cache root relative to Codex home.
    fn default_cache_root_relative(&self) -> &str;

    /// Returns the requested package version.
    fn version(&self) -> &str;

    /// Returns the manifest URL for the requested version.
    fn manifest_url(&self) -> Result<Url, PackageManagerError>;

    /// Returns the archive download URL for a platform-specific manifest entry.
    fn archive_url(&self, archive: &PackageReleaseArchive) -> Result<Url, PackageManagerError>;

    /// Returns the version string stored in the fetched release manifest.
    fn release_version<'a>(&self, manifest: &'a Self::ReleaseManifest) -> &'a str;

    /// Selects the archive to download for the current platform.
    fn platform_archive(
        &self,
        manifest: &Self::ReleaseManifest,
        platform: PackagePlatform,
    ) -> Result<PackageReleaseArchive, Self::Error>;

    /// Returns the final install directory for the package version and platform.
    fn install_dir(&self, cache_root: &Path, platform: PackagePlatform) -> PathBuf;

    /// Returns the version string encoded in a fully loaded installed package.
    fn installed_version<'a>(&self, package: &'a Self::Installed) -> &'a str;

    /// Loads and validates an installed package from disk.
    fn load_installed(
        &self,
        root_dir: PathBuf,
        platform: PackagePlatform,
    ) -> Result<Self::Installed, Self::Error>;

    /// Resolves the extracted package root before the staged install is promoted.
    fn detect_extracted_root(&self, extraction_root: &Path) -> Result<PathBuf, Self::Error> {
        detect_single_package_root(extraction_root).map_err(Self::Error::from)
    }
}
