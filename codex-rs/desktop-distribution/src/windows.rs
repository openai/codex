use std::path::Path;
use std::path::PathBuf;

use windows::ApplicationModel::Package;
use windows::ApplicationModel::PackageSignatureKind;
use windows::Management::Deployment::PackageManager;
use windows::Management::Deployment::PackageTypes;
use windows::Win32::Foundation::APPMODEL_ERROR_NO_PACKAGE;
use windows::Win32::Foundation::ERROR_INSUFFICIENT_BUFFER;
use windows::Win32::Storage::Packaging::Appx::GetCurrentPackageFullName;
use windows::Win32::System::WinRT::RO_INIT_MULTITHREADED;
use windows::Win32::System::WinRT::RoInitialize;
use windows::Win32::System::WinRT::RoUninitialize;
use windows::core::HSTRING;
use windows::core::PWSTR;

use crate::DesktopDistributionError;

const STORE_PUBLISHER: &str = "CN=50BDFD77-8903-4850-9FFE-6E8522F64D5B";
const DIRECT_PUBLISHER: &str =
    "CN=\"OpenAI OpCo, LLC\", O=\"OpenAI OpCo, LLC\", L=San Francisco, S=California, C=US";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExpectedSignatureKind {
    Developer,
    Store,
}

#[derive(Debug, Clone)]
pub(crate) struct PlatformIdentity {
    name: String,
    publisher: String,
    signature_kind: ExpectedSignatureKind,
}

pub(crate) struct PlatformDistribution {
    pub app_root: PathBuf,
    pub resources_relative_path: PathBuf,
    pub identity: PlatformIdentity,
}

#[derive(Clone, Copy)]
struct ExpectedPackage {
    name: &'static str,
    publisher: &'static str,
    signature_kind: ExpectedSignatureKind,
}

const EXPECTED_PACKAGES: &[ExpectedPackage] = &[
    ExpectedPackage {
        name: "OpenAI.Codex",
        publisher: STORE_PUBLISHER,
        signature_kind: ExpectedSignatureKind::Store,
    },
    ExpectedPackage {
        name: "OpenAI.CodexBeta",
        publisher: STORE_PUBLISHER,
        signature_kind: ExpectedSignatureKind::Store,
    },
    ExpectedPackage {
        name: "OpenAI.CodexAlpha",
        publisher: DIRECT_PUBLISHER,
        signature_kind: ExpectedSignatureKind::Developer,
    },
    ExpectedPackage {
        name: "OpenAI.CodexNightly",
        publisher: DIRECT_PUBLISHER,
        signature_kind: ExpectedSignatureKind::Developer,
    },
];

pub(crate) fn verify_hint(hint: &Path) -> Result<PlatformDistribution, DesktopDistributionError> {
    let hint = hint.to_path_buf();
    run_in_mta(move || {
        let canonical_hint = std::fs::canonicalize(&hint).map_err(|source| {
            DesktopDistributionError::Filesystem {
                stage: "Windows app hint",
                source,
            }
        })?;
        if let Some(distribution) = verified_distribution_containing_path(&canonical_hint)? {
            return Ok(distribution);
        }
        Err(DesktopDistributionError::Verification {
            stage: "Windows app hint",
            message: "hint is not contained by an authenticated Codex MSIX package".to_string(),
        })
    })
}

pub(crate) fn discover() -> Result<PlatformDistribution, DesktopDistributionError> {
    run_in_mta(|| {
        first_available_or_first_error(EXPECTED_PACKAGES.iter().copied().map(verified_packages))?
            .ok_or(DesktopDistributionError::NotFound)
    })
}

pub(crate) fn current_process_distribution()
-> Result<Option<PlatformDistribution>, DesktopDistributionError> {
    let current_exe =
        std::env::current_exe().map_err(|source| DesktopDistributionError::Filesystem {
            stage: "Windows current executable",
            source,
        })?;
    run_in_mta(move || {
        let canonical_exe = std::fs::canonicalize(current_exe).map_err(|source| {
            DesktopDistributionError::Filesystem {
                stage: "Windows current executable",
                source,
            }
        })?;
        if let Some(distribution) = verified_distribution_containing_path(&canonical_exe)? {
            return Ok(Some(distribution));
        }
        if !current_process_has_package_identity()? {
            return Ok(None);
        }

        let package = Package::Current().map_err(windows_error("Windows current package"))?;
        let id = package
            .Id()
            .map_err(windows_error("Windows current package identity"))?;
        let name = id
            .Name()
            .map_err(windows_error("Windows current package identity"))?
            .to_string_lossy();
        let publisher = id
            .Publisher()
            .map_err(windows_error("Windows current package identity"))?
            .to_string_lossy();
        let Some(expected) = EXPECTED_PACKAGES
            .iter()
            .copied()
            .find(|expected| expected.name == name)
        else {
            return Ok(None);
        };
        if expected.publisher != publisher {
            return Err(DesktopDistributionError::Verification {
                stage: "Windows current package identity",
                message: "current package claims a Codex name with the wrong publisher".to_string(),
            });
        }
        let distribution = verify_package(&package, expected)?;
        let canonical_root = std::fs::canonicalize(&distribution.app_root).map_err(|source| {
            DesktopDistributionError::Filesystem {
                stage: "Windows installed location",
                source,
            }
        })?;
        if !canonical_exe.starts_with(canonical_root) {
            return Err(DesktopDistributionError::Verification {
                stage: "Windows current package containment",
                message: "current executable is outside its authenticated package location"
                    .to_string(),
            });
        }
        Ok(Some(distribution))
    })
}

pub(crate) fn reverify(
    identity: &PlatformIdentity,
    app_root: &Path,
) -> Result<(), DesktopDistributionError> {
    let identity = identity.clone();
    let app_root = app_root.to_path_buf();
    run_in_mta(move || {
        let expected = EXPECTED_PACKAGES
            .iter()
            .copied()
            .find(|expected| {
                expected.name == identity.name
                    && expected.publisher == identity.publisher
                    && expected.signature_kind == identity.signature_kind
            })
            .ok_or_else(|| DesktopDistributionError::Verification {
                stage: "Windows package reverification",
                message: "package identity is not in the Codex allowlist".to_string(),
            })?;
        let canonical_root = std::fs::canonicalize(app_root).map_err(|source| {
            DesktopDistributionError::Filesystem {
                stage: "Windows installed location",
                source,
            }
        })?;
        if verified_packages(expected)?
            .into_iter()
            .any(|distribution| {
                std::fs::canonicalize(distribution.app_root)
                    .is_ok_and(|root| root == canonical_root)
            })
        {
            Ok(())
        } else {
            Err(DesktopDistributionError::Verification {
                stage: "Windows package reverification",
                message: "the authenticated package is no longer healthy at its installed location"
                    .to_string(),
            })
        }
    })
}

fn verified_packages(
    expected: ExpectedPackage,
) -> Result<Vec<PlatformDistribution>, DesktopDistributionError> {
    collect_verified_or_first_error(
        queried_packages(expected)?
            .iter()
            .map(|package| verify_package(package, expected)),
    )
}

fn collect_verified_or_first_error<T, E>(
    candidates: impl IntoIterator<Item = Result<T, E>>,
) -> Result<Vec<T>, E> {
    let mut verified = Vec::new();
    let mut first_error = None;
    for candidate in candidates {
        match candidate {
            Ok(value) => verified.push(value),
            Err(error) => {
                first_error.get_or_insert(error);
            }
        }
    }
    if verified.is_empty()
        && let Some(error) = first_error
    {
        return Err(error);
    }
    Ok(verified)
}

fn first_available_or_first_error<T, E>(
    candidate_groups: impl IntoIterator<Item = Result<Vec<T>, E>>,
) -> Result<Option<T>, E> {
    let mut first_error = None;
    for group in candidate_groups {
        match group {
            Ok(candidates) => {
                if let Some(candidate) = candidates.into_iter().next() {
                    return Ok(Some(candidate));
                }
            }
            Err(error) => {
                first_error.get_or_insert(error);
            }
        }
    }
    match first_error {
        Some(error) => Err(error),
        None => Ok(None),
    }
}

fn queried_packages(expected: ExpectedPackage) -> Result<Vec<Package>, DesktopDistributionError> {
    let manager = PackageManager::new().map_err(windows_error("Windows package manager"))?;
    let packages = manager
        .FindPackagesByUserSecurityIdNamePublisherWithPackageTypes(
            &HSTRING::new(),
            &HSTRING::from(expected.name),
            &HSTRING::from(expected.publisher),
            PackageTypes::Main,
        )
        .map_err(windows_error("Windows package query"))?;
    let iterator = packages
        .First()
        .map_err(windows_error("Windows package iteration"))?;
    let mut packages = Vec::new();
    while iterator
        .HasCurrent()
        .map_err(windows_error("Windows package iteration"))?
    {
        packages.push(
            iterator
                .Current()
                .map_err(windows_error("Windows package iteration"))?,
        );
        iterator
            .MoveNext()
            .map_err(windows_error("Windows package iteration"))?;
    }
    Ok(packages)
}

fn verified_distribution_containing_path(
    canonical_path: &Path,
) -> Result<Option<PlatformDistribution>, DesktopDistributionError> {
    let candidate_groups = EXPECTED_PACKAGES.iter().copied().map(|expected| {
        queried_packages(expected).map(|packages| {
            packages
                .into_iter()
                .map(move |package| (package, expected))
                .collect()
        })
    });
    first_verified_containing_path(
        canonical_path,
        candidate_groups,
        |(package, _expected)| {
            let app_root = installed_location_path(package)?;
            std::fs::canonicalize(&app_root).map_err(|source| {
                DesktopDistributionError::Filesystem {
                    stage: "Windows installed location",
                    source,
                }
            })
        },
        |(package, expected)| verify_package(package, *expected),
    )
}

fn first_verified_containing_path<C, D, E>(
    canonical_path: &Path,
    candidate_groups: impl IntoIterator<Item = Result<Vec<C>, E>>,
    mut canonical_root: impl FnMut(&C) -> Result<PathBuf, E>,
    mut verify: impl FnMut(&C) -> Result<D, E>,
) -> Result<Option<D>, E> {
    for group in candidate_groups {
        let Ok(candidates) = group else {
            // PackageManager can retain stale registrations. Until containment is established,
            // an error is not evidence about the target path and must not hide later channels.
            continue;
        };
        for candidate in candidates {
            let Ok(root) = canonical_root(&candidate) else {
                continue;
            };
            if canonical_path.starts_with(root) {
                // Once a package contains the target, its verification failure is authoritative.
                return verify(&candidate).map(Some);
            }
        }
    }
    Ok(None)
}

fn current_process_has_package_identity() -> Result<bool, DesktopDistributionError> {
    let mut length = 0;
    let result = unsafe { GetCurrentPackageFullName(&mut length, PWSTR::null()) };
    if result == APPMODEL_ERROR_NO_PACKAGE {
        return Ok(false);
    }
    if result == ERROR_INSUFFICIENT_BUFFER {
        return Ok(true);
    }
    Err(DesktopDistributionError::Verification {
        stage: "Windows current package identity",
        message: format!(
            "GetCurrentPackageFullName returned unexpected status {}",
            result.0
        ),
    })
}

fn verify_package(
    package: &Package,
    expected: ExpectedPackage,
) -> Result<PlatformDistribution, DesktopDistributionError> {
    let id = package
        .Id()
        .map_err(windows_error("Windows package identity"))?;
    if id
        .Name()
        .map_err(windows_error("Windows package identity"))?
        .to_string_lossy()
        != expected.name
        || id
            .Publisher()
            .map_err(windows_error("Windows package identity"))?
            .to_string_lossy()
            != expected.publisher
        || !id
            .ResourceId()
            .map_err(windows_error("Windows package identity"))?
            .is_empty()
    {
        return Err(DesktopDistributionError::Verification {
            stage: "Windows package identity",
            message: "queried package identity did not exactly match the expected Codex package"
                .to_string(),
        });
    }
    if package
        .IsFramework()
        .map_err(windows_error("Windows package type"))?
        || package
            .IsResourcePackage()
            .map_err(windows_error("Windows package type"))?
        || package
            .IsBundle()
            .map_err(windows_error("Windows package type"))?
        || package
            .IsDevelopmentMode()
            .map_err(windows_error("Windows package type"))?
        || package
            .IsOptional()
            .map_err(windows_error("Windows package type"))?
    {
        return Err(DesktopDistributionError::Verification {
            stage: "Windows package type",
            message: "Codex package has an ineligible package type or development-mode status"
                .to_string(),
        });
    }
    let signature_kind = package
        .SignatureKind()
        .map_err(windows_error("Windows package signature"))?;
    let signature_matches = match expected.signature_kind {
        ExpectedSignatureKind::Developer => signature_kind == PackageSignatureKind::Developer,
        ExpectedSignatureKind::Store => signature_kind == PackageSignatureKind::Store,
    };
    if !signature_matches {
        return Err(DesktopDistributionError::Verification {
            stage: "Windows package signature",
            message: "Codex package signature kind did not match the expected distribution channel"
                .to_string(),
        });
    }
    if !package
        .Status()
        .and_then(|status| status.VerifyIsOK())
        .map_err(windows_error("Windows package status"))?
    {
        return Err(DesktopDistributionError::Verification {
            stage: "Windows package status",
            message: "Codex package status is not healthy".to_string(),
        });
    }
    if !package
        .VerifyContentIntegrityAsync()
        .and_then(|operation| operation.get())
        .map_err(windows_error("Windows package content integrity"))?
    {
        return Err(DesktopDistributionError::Verification {
            stage: "Windows package content integrity",
            message: "Codex package content integrity verification failed".to_string(),
        });
    }
    let app_root = installed_location_path(package)?;
    Ok(PlatformDistribution {
        app_root,
        resources_relative_path: PathBuf::from("app/resources"),
        identity: PlatformIdentity {
            name: expected.name.to_string(),
            publisher: expected.publisher.to_string(),
            signature_kind: expected.signature_kind,
        },
    })
}

fn installed_location_path(package: &Package) -> Result<PathBuf, DesktopDistributionError> {
    Ok(PathBuf::from(
        package
            .InstalledLocation()
            .and_then(|location| location.Path())
            .map_err(windows_error("Windows installed location"))?
            .to_string_lossy(),
    ))
}

fn run_in_mta<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, DesktopDistributionError> + Send + 'static,
) -> Result<T, DesktopDistributionError> {
    std::thread::spawn(move || {
        unsafe { RoInitialize(RO_INIT_MULTITHREADED) }
            .map_err(windows_error("Windows Runtime initialization"))?;
        struct Apartment;
        impl Drop for Apartment {
            fn drop(&mut self) {
                unsafe { RoUninitialize() };
            }
        }
        let _apartment = Apartment;
        work()
    })
    .join()
    .map_err(|_| DesktopDistributionError::Verification {
        stage: "Windows package verification",
        message: "package verification thread panicked".to_string(),
    })?
}

fn windows_error(
    stage: &'static str,
) -> impl FnOnce(windows::core::Error) -> DesktopDistributionError {
    move |error| DesktopDistributionError::Verification {
        stage,
        message: error.to_string(),
    }
}

#[cfg(test)]
#[path = "windows_tests.rs"]
mod tests;
