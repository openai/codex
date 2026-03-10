mod archive;
mod config;
mod error;
mod manager;
mod package;
mod platform;

#[cfg(test)]
mod tests;

pub use archive::ArchiveFormat;
pub use archive::PackageReleaseArchive;
pub use config::PackageManagerConfig;
pub use error::PackageManagerError;
pub use manager::PackageManager;
pub use package::ManagedPackage;
pub use platform::PackagePlatform;
