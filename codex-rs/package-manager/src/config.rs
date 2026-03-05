use crate::ManagedPackage;
use std::path::Path;
use std::path::PathBuf;

/// Immutable configuration for a [`crate::PackageManager`] instance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackageManagerConfig<P> {
    codex_home: PathBuf,
    package: P,
    cache_root: Option<PathBuf>,
}

impl<P> PackageManagerConfig<P> {
    /// Creates a config rooted at the provided Codex home directory.
    pub fn new(codex_home: PathBuf, package: P) -> Self {
        Self {
            codex_home,
            package,
            cache_root: None,
        }
    }

    /// Overrides the package cache root instead of deriving it from `codex_home`.
    pub fn with_cache_root(mut self, cache_root: PathBuf) -> Self {
        self.cache_root = Some(cache_root);
        self
    }

    /// Returns the owning Codex home directory.
    pub fn codex_home(&self) -> &Path {
        &self.codex_home
    }

    /// Returns the package description used by the manager.
    pub fn package(&self) -> &P {
        &self.package
    }
}

impl<P: ManagedPackage> PackageManagerConfig<P> {
    /// Returns the effective cache root for the package.
    pub fn cache_root(&self) -> PathBuf {
        self.cache_root.clone().unwrap_or_else(|| {
            self.codex_home.join(
                self.package
                    .default_cache_root_relative()
                    .replace('/', std::path::MAIN_SEPARATOR_STR),
            )
        })
    }
}
