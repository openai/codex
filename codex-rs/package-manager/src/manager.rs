use crate::ManagedPackage;
use crate::PackageManagerConfig;
use crate::PackageManagerError;
use crate::PackagePlatform;
use crate::archive::extract_archive;
use crate::archive::verify_archive_size;
use crate::archive::verify_sha256;
use fd_lock::RwLock as FileRwLock;
use reqwest::Client;
use std::fs::OpenOptions;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use tempfile::tempdir_in;
use tokio::fs;
use tokio::time::sleep;
use url::Url;

const INSTALL_LOCK_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Fetches and installs a versioned package into a shared cache directory.
#[derive(Clone, Debug)]
pub struct PackageManager<P> {
    client: Client,
    config: PackageManagerConfig<P>,
}

impl<P> PackageManager<P> {
    /// Creates a manager with a default `reqwest` client.
    pub fn new(config: PackageManagerConfig<P>) -> Self {
        Self {
            client: Client::new(),
            config,
        }
    }

    /// Creates a manager with a caller-provided HTTP client.
    pub fn with_client(config: PackageManagerConfig<P>, client: Client) -> Self {
        Self { client, config }
    }
}

impl<P: ManagedPackage> PackageManager<P> {
    /// Resolves a valid cached install for the current platform, if one exists.
    pub async fn resolve_cached(&self) -> Result<Option<P::Installed>, P::Error> {
        let platform = PackagePlatform::detect_current().map_err(P::Error::from)?;
        let install_dir = self
            .config
            .package
            .install_dir(&self.config.cache_root(), platform);
        self.resolve_cached_at(platform, install_dir).await
    }

    /// Ensures the requested package is installed for the current platform.
    pub async fn ensure_installed(&self) -> Result<P::Installed, P::Error> {
        // Fast path: most calls should resolve an already validated cache entry
        // without touching the network or the install lock.
        if let Some(package) = self.resolve_cached().await? {
            return Ok(package);
        }

        let platform = PackagePlatform::detect_current().map_err(P::Error::from)?;
        let cache_root = self.config.cache_root();
        let install_dir = self.config.package.install_dir(&cache_root, platform);
        if let Some(package) = self
            .resolve_cached_at(platform, install_dir.clone())
            .await?
        {
            return Ok(package);
        }

        if let Some(parent) = install_dir.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|source| PackageManagerError::Io {
                    context: format!("failed to create {}", parent.display()),
                    source,
                })
                .map_err(P::Error::from)?;
        }

        let lock_path = install_dir.with_extension("lock");
        let lock_file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(|source| PackageManagerError::Io {
                context: format!("failed to open {}", lock_path.display()),
                source,
            })
            .map_err(P::Error::from)?;
        let mut install_lock = FileRwLock::new(lock_file);
        let _install_guard = loop {
            match install_lock.try_write() {
                Ok(guard) => break guard,
                Err(source) if source.kind() == std::io::ErrorKind::WouldBlock => {
                    sleep(INSTALL_LOCK_POLL_INTERVAL).await;
                }
                Err(source) => {
                    return Err(PackageManagerError::Io {
                        context: format!("failed to lock {}", lock_path.display()),
                        source,
                    }
                    .into());
                }
            }
        };

        // Another process may have finished the install while we were waiting
        // on the lock, so re-check before doing any download or extraction work.
        if let Some(package) = self
            .resolve_cached_at(platform, install_dir.clone())
            .await?
        {
            return Ok(package);
        }

        let manifest = self.fetch_release_manifest().await?;
        if self.config.package.release_version(&manifest) != self.config.package.version() {
            return Err(PackageManagerError::UnexpectedPackageVersion {
                expected: self.config.package.version().to_string(),
                actual: self.config.package.release_version(&manifest).to_string(),
            }
            .into());
        }

        fs::create_dir_all(&cache_root)
            .await
            .map_err(|source| PackageManagerError::Io {
                context: format!("failed to create {}", cache_root.display()),
                source,
            })
            .map_err(P::Error::from)?;
        let staging_root = cache_root.join(".staging");
        fs::create_dir_all(&staging_root)
            .await
            .map_err(|source| PackageManagerError::Io {
                context: format!("failed to create {}", staging_root.display()),
                source,
            })
            .map_err(P::Error::from)?;

        // Everything below happens in a disposable staging area until the
        // extracted package has passed package-specific validation.
        let platform_archive = self.config.package.platform_archive(&manifest, platform)?;
        let archive_url = self
            .config
            .package
            .archive_url(&platform_archive)
            .map_err(P::Error::from)?;
        let archive_bytes = self.download_bytes(&archive_url).await?;
        verify_archive_size(&archive_bytes, platform_archive.size_bytes).map_err(P::Error::from)?;
        verify_sha256(&archive_bytes, &platform_archive.sha256).map_err(P::Error::from)?;

        let staging_dir = tempdir_in(&staging_root)
            .map_err(|source| PackageManagerError::Io {
                context: format!(
                    "failed to create staging directory in {}",
                    staging_root.display()
                ),
                source,
            })
            .map_err(P::Error::from)?;
        let archive_path = staging_dir.path().join(&platform_archive.archive);
        fs::write(&archive_path, &archive_bytes)
            .await
            .map_err(|source| PackageManagerError::Io {
                context: format!("failed to write {}", archive_path.display()),
                source,
            })
            .map_err(P::Error::from)?;
        let extraction_root = staging_dir.path().join("extract");
        fs::create_dir_all(&extraction_root)
            .await
            .map_err(|source| PackageManagerError::Io {
                context: format!("failed to create {}", extraction_root.display()),
                source,
            })
            .map_err(P::Error::from)?;

        extract_archive(&archive_path, &extraction_root, platform_archive.format)
            .map_err(P::Error::from)?;
        let extracted_root = self
            .config
            .package
            .detect_extracted_root(&extraction_root)?;
        let package = self
            .config
            .package
            .load_installed(extracted_root.clone(), platform)?;
        if self.config.package.installed_version(&package) != self.config.package.version() {
            return Err(PackageManagerError::UnexpectedPackageVersion {
                expected: self.config.package.version().to_string(),
                actual: self.config.package.installed_version(&package).to_string(),
            }
            .into());
        }

        if let Some(parent) = install_dir.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|source| PackageManagerError::Io {
                    context: format!("failed to create {}", parent.display()),
                    source,
                })
                .map_err(P::Error::from)?;
        }

        // Promotion is intentionally two-phase: move the old install aside,
        // then attempt an atomic rename of the staged tree into place.
        // If promotion fails, restore the previous install before returning.
        let replaced_install_dir = quarantine_existing_install(&install_dir)
            .await
            .map_err(P::Error::from)?;
        let promotion = promote_staged_install(&extracted_root, &install_dir).await;
        if let Err(error) = promotion {
            // If another process won the race after we staged our copy, prefer
            // the now-installed cache entry and clean up our quarantined copy.
            if matches!(
                &error,
                PackageManagerError::Io { source, .. }
                    if matches!(
                        source.kind(),
                        std::io::ErrorKind::AlreadyExists
                            | std::io::ErrorKind::DirectoryNotEmpty
                    )
            ) && let Some(package) = self
                .resolve_cached_at(platform, install_dir.clone())
                .await?
            {
                if let Some(replaced_install_dir) = replaced_install_dir {
                    let _ = fs::remove_dir_all(replaced_install_dir).await;
                }
                return Ok(package);
            }

            restore_quarantined_install(&install_dir, replaced_install_dir.as_deref(), &error)
                .await
                .map_err(P::Error::from)?;
            return Err(error.into());
        }

        // Validate from the final install path before deleting the quarantined
        // previous install. Some packages may only fully validate once the
        // promoted tree is in place at its real cache location.
        let package = match self
            .config
            .package
            .load_installed(install_dir.clone(), platform)
        {
            Ok(package) => package,
            Err(error) => {
                if let Some(replaced_install_dir) = replaced_install_dir.as_deref() {
                    // Final validation failed after promotion, so discard the
                    // broken install and restore the last known-good copy.
                    if fs::try_exists(&install_dir)
                        .await
                        .map_err(|source| PackageManagerError::Io {
                            context: format!("failed to read {}", install_dir.display()),
                            source,
                        })
                        .map_err(P::Error::from)?
                    {
                        fs::remove_dir_all(&install_dir)
                            .await
                            .map_err(|source| PackageManagerError::Io {
                                context: format!(
                                    "failed to remove invalid install {} after final validation failed",
                                    install_dir.display()
                                ),
                                source,
                            })
                            .map_err(P::Error::from)?;
                    }
                    fs::rename(replaced_install_dir, &install_dir)
                        .await
                        .map_err(|source| PackageManagerError::Io {
                            context: format!(
                                "failed to restore {} from {} after final validation failed",
                                install_dir.display(),
                                replaced_install_dir.display()
                            ),
                            source,
                        })
                        .map_err(P::Error::from)?;
                }
                return Err(error);
            }
        };

        if let Some(replaced_install_dir) = replaced_install_dir {
            let _ = fs::remove_dir_all(replaced_install_dir).await;
        }

        Ok(package)
    }

    async fn resolve_cached_at(
        &self,
        platform: PackagePlatform,
        install_dir: PathBuf,
    ) -> Result<Option<P::Installed>, P::Error> {
        if !fs::try_exists(&install_dir)
            .await
            .map_err(|source| PackageManagerError::Io {
                context: format!("failed to read {}", install_dir.display()),
                source,
            })
            .map_err(P::Error::from)?
        {
            return Ok(None);
        }

        let package = match self.config.package.load_installed(install_dir, platform) {
            Ok(package) => package,
            Err(_) => return Ok(None),
        };
        if self.config.package.installed_version(&package) != self.config.package.version() {
            return Ok(None);
        }
        Ok(Some(package))
    }

    async fn fetch_release_manifest(&self) -> Result<P::ReleaseManifest, P::Error> {
        let manifest_url = self.config.package.manifest_url().map_err(P::Error::from)?;
        let response = self
            .client
            .get(manifest_url.clone())
            .send()
            .await
            .map_err(|source| PackageManagerError::Http {
                context: format!("failed to fetch {manifest_url}"),
                source,
            })
            .map_err(P::Error::from)?
            .error_for_status()
            .map_err(|source| PackageManagerError::Http {
                context: format!("manifest request failed for {manifest_url}"),
                source,
            })
            .map_err(P::Error::from)?;

        response
            .json::<P::ReleaseManifest>()
            .await
            .map_err(|source| PackageManagerError::Http {
                context: format!("failed to decode manifest from {manifest_url}"),
                source,
            })
            .map_err(P::Error::from)
    }

    async fn download_bytes(&self, url: &Url) -> Result<Vec<u8>, P::Error> {
        let response = self
            .client
            .get(url.clone())
            .send()
            .await
            .map_err(|source| PackageManagerError::Http {
                context: format!("failed to download {url}"),
                source,
            })
            .map_err(P::Error::from)?
            .error_for_status()
            .map_err(|source| PackageManagerError::Http {
                context: format!("archive request failed for {url}"),
                source,
            })
            .map_err(P::Error::from)?;
        let bytes = response
            .bytes()
            .await
            .map_err(|source| PackageManagerError::Http {
                context: format!("failed to read response body for {url}"),
                source,
            })
            .map_err(P::Error::from)?;
        Ok(bytes.to_vec())
    }
}

pub(crate) async fn quarantine_existing_install(
    install_dir: &Path,
) -> Result<Option<PathBuf>, PackageManagerError> {
    if !fs::try_exists(install_dir)
        .await
        .map_err(|source| PackageManagerError::Io {
            context: format!("failed to read {}", install_dir.display()),
            source,
        })?
    {
        return Ok(None);
    }

    let install_name = install_dir.file_name().ok_or_else(|| {
        PackageManagerError::ArchiveExtraction(format!(
            "install path `{}` has no terminal component",
            install_dir.display()
        ))
    })?;
    let install_name = install_name.to_string_lossy();
    let mut suffix = 0u32;
    loop {
        let quarantined_path = install_dir.with_file_name(format!(
            ".{install_name}.replaced-{}-{suffix}",
            std::process::id()
        ));
        match fs::rename(install_dir, &quarantined_path).await {
            Ok(()) => return Ok(Some(quarantined_path)),
            Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => {
                suffix += 1;
            }
            Err(source) => {
                return Err(PackageManagerError::Io {
                    context: format!(
                        "failed to quarantine {} to {}",
                        install_dir.display(),
                        quarantined_path.display()
                    ),
                    source,
                });
            }
        }
    }
}

pub(crate) async fn promote_staged_install(
    extracted_root: &Path,
    install_dir: &Path,
) -> Result<(), PackageManagerError> {
    fs::rename(extracted_root, install_dir)
        .await
        .map_err(|source| PackageManagerError::Io {
            context: format!(
                "failed to move {} to {}",
                extracted_root.display(),
                install_dir.display()
            ),
            source,
        })
}

pub(crate) async fn restore_quarantined_install(
    install_dir: &Path,
    quarantined_install_dir: Option<&Path>,
    promotion_error: &PackageManagerError,
) -> Result<(), PackageManagerError> {
    let Some(quarantined_install_dir) = quarantined_install_dir else {
        return Ok(());
    };

    fs::rename(quarantined_install_dir, install_dir)
        .await
        .map_err(|source| PackageManagerError::Io {
            context: format!(
                "{promotion_error}; failed to restore {} from {}",
                install_dir.display(),
                quarantined_install_dir.display()
            ),
            source,
        })
}
