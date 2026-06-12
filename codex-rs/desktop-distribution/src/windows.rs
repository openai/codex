use std::ffi::CStr;
use std::ffi::c_void;
use std::mem::size_of;
use std::path::Path;
use std::path::PathBuf;
use std::ptr;

use sha2::Digest as _;
use sha2::Sha256;
use windows::ApplicationModel::Package;
use windows::ApplicationModel::PackageSignatureKind;
use windows::Management::Deployment::PackageManager;
use windows::Management::Deployment::PackageTypes;
use windows::Win32::Foundation::APPMODEL_ERROR_NO_PACKAGE;
use windows::Win32::Foundation::ERROR_INSUFFICIENT_BUFFER;
use windows::Win32::Foundation::HWND;
use windows::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows::Win32::Security::Cryptography::CERT_CHAIN_CACHE_ONLY_URL_RETRIEVAL;
use windows::Win32::Security::Cryptography::CERT_CHAIN_CONTEXT;
use windows::Win32::Security::Cryptography::CERT_CHAIN_DISABLE_AIA;
use windows::Win32::Security::Cryptography::CERT_CHAIN_DISABLE_AUTH_ROOT_AUTO_UPDATE;
use windows::Win32::Security::Cryptography::CERT_CHAIN_PARA;
use windows::Win32::Security::Cryptography::CERT_CONTEXT;
use windows::Win32::Security::Cryptography::CERT_FIND_EXT_ONLY_ENHKEY_USAGE_FLAG;
use windows::Win32::Security::Cryptography::CERT_FIND_SUBJECT_CERT;
use windows::Win32::Security::Cryptography::CERT_NAME_STR_REVERSE_FLAG;
use windows::Win32::Security::Cryptography::CERT_QUERY_CONTENT_FLAG_PKCS7_SIGNED;
use windows::Win32::Security::Cryptography::CERT_QUERY_FORMAT_FLAG_BINARY;
use windows::Win32::Security::Cryptography::CERT_QUERY_OBJECT_BLOB;
use windows::Win32::Security::Cryptography::CERT_STRING_TYPE;
use windows::Win32::Security::Cryptography::CERT_TRUST_IS_NOT_TIME_VALID;
use windows::Win32::Security::Cryptography::CERT_X500_NAME_STR;
use windows::Win32::Security::Cryptography::CMSG_SIGNER_COUNT_PARAM;
use windows::Win32::Security::Cryptography::CMSG_SIGNER_INFO;
use windows::Win32::Security::Cryptography::CMSG_SIGNER_INFO_PARAM;
use windows::Win32::Security::Cryptography::CRYPT_INTEGER_BLOB;
use windows::Win32::Security::Cryptography::CertCloseStore;
use windows::Win32::Security::Cryptography::CertCompareCertificateName;
use windows::Win32::Security::Cryptography::CertFindCertificateInStore;
use windows::Win32::Security::Cryptography::CertFreeCertificateChain;
use windows::Win32::Security::Cryptography::CertFreeCertificateContext;
use windows::Win32::Security::Cryptography::CertGetCertificateChain;
use windows::Win32::Security::Cryptography::CertGetEnhancedKeyUsage;
use windows::Win32::Security::Cryptography::CertStrToNameW;
use windows::Win32::Security::Cryptography::CryptMsgClose;
use windows::Win32::Security::Cryptography::CryptMsgGetParam;
use windows::Win32::Security::Cryptography::CryptQueryObject;
use windows::Win32::Security::Cryptography::HCERTCHAINENGINE;
use windows::Win32::Security::Cryptography::HCERTSTORE;
use windows::Win32::Security::Cryptography::PKCS_7_ASN_ENCODING;
use windows::Win32::Security::Cryptography::X509_ASN_ENCODING;
use windows::Win32::Security::WinTrust::WINTRUST_ACTION_GENERIC_VERIFY_V2;
use windows::Win32::Security::WinTrust::WINTRUST_BLOB_INFO;
use windows::Win32::Security::WinTrust::WINTRUST_DATA;
use windows::Win32::Security::WinTrust::WINTRUST_DATA_0;
use windows::Win32::Security::WinTrust::WTD_CACHE_ONLY_URL_RETRIEVAL;
use windows::Win32::Security::WinTrust::WTD_CHOICE_BLOB;
use windows::Win32::Security::WinTrust::WTD_REVOCATION_CHECK_NONE;
use windows::Win32::Security::WinTrust::WTD_REVOKE_NONE;
use windows::Win32::Security::WinTrust::WTD_STATEACTION_CLOSE;
use windows::Win32::Security::WinTrust::WTD_STATEACTION_VERIFY;
use windows::Win32::Security::WinTrust::WTD_UI_NONE;
use windows::Win32::Security::WinTrust::WinVerifyTrust;
use windows::Win32::Storage::Packaging::Appx::GetCurrentPackageFullName;
use windows::Win32::System::WinRT::RO_INIT_MULTITHREADED;
use windows::Win32::System::WinRT::RoInitialize;
use windows::Win32::System::WinRT::RoUninitialize;
use windows::core::HSTRING;
use windows::core::PCWSTR;
use windows::core::PWSTR;

use crate::DesktopDistributionError;
use crate::reject_link_or_reparse;

const STORE_PUBLISHER: &str = "CN=50BDFD77-8903-4850-9FFE-6E8522F64D5B";
const DIRECT_PUBLISHER: &str =
    "CN=\"OpenAI OpCo, LLC\", O=\"OpenAI OpCo, LLC\", L=San Francisco, S=California, C=US";
// Azure Artifact Signing rotates its leaf certificates every few days. This EKU identifies the
// OpenAI certificate profile across those rotations; the pinned Microsoft identity-verification
// root below prevents a locally trusted same-subject certificate from copying the OID.
const DIRECT_SIGNING_PROFILE_OID: &str =
    "1.3.6.1.4.1.311.97.34411380.685553541.718322805.135574919";
const ARTIFACT_SIGNING_PUBLIC_TRUST_OID: &str = "1.3.6.1.4.1.311.97.1.0";
const CODE_SIGNING_OID: &str = "1.3.6.1.5.5.7.3.3";
const DIRECT_SIGNING_ROOT_CERT_SHA256: [u8; 32] = [
    0x53, 0x67, 0xf2, 0x0c, 0x7a, 0xde, 0x0e, 0x2b, 0xca, 0x79, 0x09, 0x15, 0x05, 0x6d, 0x08, 0x6b,
    0x72, 0x0c, 0x33, 0xc1, 0xfa, 0x2a, 0x26, 0x61, 0xac, 0xf7, 0x87, 0xe3, 0x29, 0x2e, 0x12, 0x70,
];
const APPX_SIGNATURE_MAGIC: &[u8; 4] = b"PKCX";
const MAX_APPX_SIGNATURE_SIZE: u64 = 1024 * 1024;
const APPX_P7X_SUBJECT_GUID: windows::core::GUID =
    windows::core::GUID::from_u128(0x5598cff1_68db_4340_b57f_1cacf88c9a51);
// Windows defines HCCE_LOCAL_MACHINE as the pseudo-handle value 1. The windows crate does not
// expose that macro, so keep the equivalent typed constant local to the verifier.
const LOCAL_MACHINE_CHAIN_ENGINE: HCERTCHAINENGINE = HCERTCHAINENGINE(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExpectedSignatureKind {
    Developer,
    Store,
}

#[derive(Debug, Clone)]
pub(crate) struct PlatformIdentity {
    name: String,
    publisher: String,
    package_family_name: String,
    package_full_name: String,
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
                distribution.identity.package_family_name == identity.package_family_name
                    && distribution.identity.package_full_name == identity.package_full_name
                    && std::fs::canonicalize(distribution.app_root)
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
    let package_family_name = id
        .FamilyName()
        .map_err(windows_error("Windows package identity"))?
        .to_string_lossy();
    if package_family_name.is_empty() {
        return Err(DesktopDistributionError::Verification {
            stage: "Windows package identity",
            message: "Codex package has an empty package family name".to_string(),
        });
    }
    let package_full_name = id
        .FullName()
        .map_err(windows_error("Windows package identity"))?
        .to_string_lossy();
    if package_full_name.is_empty() {
        return Err(DesktopDistributionError::Verification {
            stage: "Windows package identity",
            message: "Codex package has an empty package full name".to_string(),
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
    if expected.signature_kind == ExpectedSignatureKind::Developer {
        verify_direct_signing_identity(&app_root)?;
    }
    Ok(PlatformDistribution {
        app_root,
        resources_relative_path: PathBuf::from("app/resources"),
        identity: PlatformIdentity {
            name: expected.name.to_string(),
            publisher: expected.publisher.to_string(),
            package_family_name,
            package_full_name,
            signature_kind: expected.signature_kind,
        },
    })
}

pub(crate) fn package_family_name(identity: &PlatformIdentity) -> &str {
    &identity.package_family_name
}

pub(crate) fn package_full_name(identity: &PlatformIdentity) -> &str {
    &identity.package_full_name
}

fn verify_direct_signing_identity(app_root: &Path) -> Result<(), DesktopDistributionError> {
    reject_link_or_reparse(app_root, "Windows package signing identity")?;
    let canonical_root =
        std::fs::canonicalize(app_root).map_err(|source| DesktopDistributionError::Filesystem {
            stage: "Windows package signing identity",
            source,
        })?;
    let signature_path = app_root.join("AppxSignature.p7x");
    reject_link_or_reparse(&signature_path, "Windows package signing identity")?;
    let signature_path = std::fs::canonicalize(signature_path).map_err(|source| {
        DesktopDistributionError::Filesystem {
            stage: "Windows package signing identity",
            source,
        }
    })?;
    if signature_path == canonical_root || !signature_path.starts_with(&canonical_root) {
        return Err(signing_identity_error(
            "package signature escaped the authenticated installed location",
        ));
    }
    let metadata = std::fs::metadata(&signature_path).map_err(|source| {
        DesktopDistributionError::Filesystem {
            stage: "Windows package signing identity",
            source,
        }
    })?;
    if !metadata.is_file() || metadata.len() > MAX_APPX_SIGNATURE_SIZE {
        return Err(signing_identity_error(
            "package signature is missing, not a regular file, or unexpectedly large",
        ));
    }
    let signature =
        std::fs::read(signature_path).map_err(|source| DesktopDistributionError::Filesystem {
            stage: "Windows package signing identity",
            source,
        })?;
    let Some(pkcs7) = signature.strip_prefix(APPX_SIGNATURE_MAGIC) else {
        return Err(signing_identity_error(
            "package signature has an invalid AppX signature header",
        ));
    };
    verify_appx_authenticode(&signature)?;
    verify_direct_pkcs7_signer(pkcs7)
}

fn verify_appx_authenticode(signature: &[u8]) -> Result<(), DesktopDistributionError> {
    let cb_mem_object = u32::try_from(signature.len())
        .map_err(|_| signing_identity_error("package signature is too large"))?;
    let mut blob = WINTRUST_BLOB_INFO {
        cbStruct: size_of::<WINTRUST_BLOB_INFO>() as u32,
        gSubject: APPX_P7X_SUBJECT_GUID,
        cbMemObject: cb_mem_object,
        pbMemObject: signature.as_ptr().cast_mut(),
        ..Default::default()
    };
    let mut trust_data = WINTRUST_DATA {
        cbStruct: size_of::<WINTRUST_DATA>() as u32,
        dwUIChoice: WTD_UI_NONE,
        fdwRevocationChecks: WTD_REVOKE_NONE,
        dwUnionChoice: WTD_CHOICE_BLOB,
        Anonymous: WINTRUST_DATA_0 { pBlob: &mut blob },
        dwStateAction: WTD_STATEACTION_VERIFY,
        dwProvFlags: WTD_CACHE_ONLY_URL_RETRIEVAL | WTD_REVOCATION_CHECK_NONE,
        ..Default::default()
    };
    let mut action = WINTRUST_ACTION_GENERIC_VERIFY_V2;
    let window = HWND(INVALID_HANDLE_VALUE.0);
    let verify_status = unsafe {
        WinVerifyTrust(
            window,
            &mut action,
            (&mut trust_data as *mut WINTRUST_DATA).cast(),
        )
    };
    trust_data.dwStateAction = WTD_STATEACTION_CLOSE;
    let close_status = unsafe {
        WinVerifyTrust(
            window,
            &mut action,
            (&mut trust_data as *mut WINTRUST_DATA).cast(),
        )
    };
    if verify_status != 0 {
        return Err(signing_identity_error(format!(
            "WinVerifyTrust rejected the package signature with status 0x{:08x}",
            verify_status as u32
        )));
    }
    if close_status != 0 {
        return Err(signing_identity_error(format!(
            "WinVerifyTrust failed to close package signature state with status 0x{:08x}",
            close_status as u32
        )));
    }
    Ok(())
}

fn verify_direct_pkcs7_signer(pkcs7: &[u8]) -> Result<(), DesktopDistributionError> {
    let cb_data = u32::try_from(pkcs7.len())
        .map_err(|_| signing_identity_error("package signature is too large"))?;
    let blob = CRYPT_INTEGER_BLOB {
        cbData: cb_data,
        pbData: pkcs7.as_ptr().cast_mut(),
    };
    let mut store = HCERTSTORE::default();
    let mut message = ptr::null_mut();
    let query_result = unsafe {
        CryptQueryObject(
            CERT_QUERY_OBJECT_BLOB,
            (&blob as *const CRYPT_INTEGER_BLOB).cast(),
            CERT_QUERY_CONTENT_FLAG_PKCS7_SIGNED,
            CERT_QUERY_FORMAT_FLAG_BINARY,
            0,
            None,
            None,
            None,
            Some(&mut store),
            Some(&mut message),
            None,
        )
    };
    let query = CryptQueryHandles { store, message };
    query_result.map_err(windows_error("Windows package signing identity"))?;
    if query.store.is_invalid() || query.message.is_null() {
        return Err(signing_identity_error(
            "package signature did not yield a certificate store and signed message",
        ));
    }

    let signer_count = crypt_message_u32(query.message, CMSG_SIGNER_COUNT_PARAM)?;
    if signer_count != 1 {
        return Err(signing_identity_error(
            "package signature must contain exactly one primary signer",
        ));
    }
    let signer_info = crypt_message_buffer(query.message, CMSG_SIGNER_INFO_PARAM)?;
    let signer_info = unsafe { &*(signer_info.as_ptr().cast::<CMSG_SIGNER_INFO>()) };
    let mut signer_match = windows::Win32::Security::Cryptography::CERT_INFO::default();
    signer_match.Issuer = signer_info.Issuer;
    signer_match.SerialNumber = signer_info.SerialNumber;
    let signer = unsafe {
        CertFindCertificateInStore(
            query.store,
            X509_ASN_ENCODING | PKCS_7_ASN_ENCODING,
            0,
            CERT_FIND_SUBJECT_CERT,
            Some(
                (&signer_match as *const windows::Win32::Security::Cryptography::CERT_INFO).cast(),
            ),
            None,
        )
    };
    if signer.is_null() {
        return Err(signing_identity_error(
            "package signature signer certificate was not found",
        ));
    }
    let signer = CertificateContext(signer);
    if !certificate_subject_matches(signer.0, DIRECT_PUBLISHER)? {
        return Err(signing_identity_error(
            "package signer subject does not match the expected OpenAI publisher",
        ));
    }
    if !certificate_has_ekus(
        signer.0,
        &[
            CODE_SIGNING_OID,
            ARTIFACT_SIGNING_PUBLIC_TRUST_OID,
            DIRECT_SIGNING_PROFILE_OID,
        ],
    )? {
        return Err(signing_identity_error(
            "package signer does not use the expected OpenAI Trusted Signing identity",
        ));
    }

    let mut chain_parameters = CERT_CHAIN_PARA {
        cbSize: size_of::<CERT_CHAIN_PARA>() as u32,
        ..Default::default()
    };
    let mut chain = ptr::null_mut();
    unsafe {
        CertGetCertificateChain(
            LOCAL_MACHINE_CHAIN_ENGINE,
            signer.0,
            None,
            query.store,
            &mut chain_parameters,
            CERT_CHAIN_CACHE_ONLY_URL_RETRIEVAL
                | CERT_CHAIN_DISABLE_AIA
                | CERT_CHAIN_DISABLE_AUTH_ROOT_AUTO_UPDATE,
            None,
            &mut chain,
        )
    }
    .map_err(windows_error("Windows package signing identity"))?;
    if chain.is_null() {
        return Err(signing_identity_error(
            "package signer certificate chain was not available",
        ));
    }
    let chain = CertificateChain(chain);
    if !chain_has_expected_root(chain.0)? {
        return Err(signing_identity_error(
            "package signer is not chained to the pinned Microsoft identity-verification root",
        ));
    }
    Ok(())
}

fn crypt_message_u32(
    message: *const c_void,
    parameter: u32,
) -> Result<u32, DesktopDistributionError> {
    let mut value = 0_u32;
    let mut size = size_of::<u32>() as u32;
    unsafe {
        CryptMsgGetParam(
            message,
            parameter,
            0,
            Some((&mut value as *mut u32).cast()),
            &mut size,
        )
    }
    .map_err(windows_error("Windows package signing identity"))?;
    if size != size_of::<u32>() as u32 {
        return Err(signing_identity_error(
            "package signature returned malformed signer metadata",
        ));
    }
    Ok(value)
}

fn crypt_message_buffer(
    message: *const c_void,
    parameter: u32,
) -> Result<Vec<usize>, DesktopDistributionError> {
    let mut size = 0_u32;
    unsafe { CryptMsgGetParam(message, parameter, 0, None, &mut size) }
        .map_err(windows_error("Windows package signing identity"))?;
    if size < size_of::<CMSG_SIGNER_INFO>() as u32 || u64::from(size) > MAX_APPX_SIGNATURE_SIZE {
        return Err(signing_identity_error(
            "package signature returned malformed signer metadata",
        ));
    }
    let mut buffer = aligned_buffer(size);
    unsafe {
        CryptMsgGetParam(
            message,
            parameter,
            0,
            Some(buffer.as_mut_ptr().cast()),
            &mut size,
        )
    }
    .map_err(windows_error("Windows package signing identity"))?;
    Ok(buffer)
}

fn certificate_has_ekus(
    certificate: *const CERT_CONTEXT,
    expected_oids: &[&str],
) -> Result<bool, DesktopDistributionError> {
    let mut size = 0_u32;
    unsafe {
        CertGetEnhancedKeyUsage(
            certificate,
            CERT_FIND_EXT_ONLY_ENHKEY_USAGE_FLAG.0,
            None,
            &mut size,
        )
    }
    .map_err(windows_error("Windows package signing identity"))?;
    if size < size_of::<windows::Win32::Security::Cryptography::CTL_USAGE>() as u32
        || u64::from(size) > MAX_APPX_SIGNATURE_SIZE
    {
        return Err(signing_identity_error(
            "package signer returned malformed enhanced key usage metadata",
        ));
    }
    let mut buffer = aligned_buffer(size);
    unsafe {
        CertGetEnhancedKeyUsage(
            certificate,
            CERT_FIND_EXT_ONLY_ENHKEY_USAGE_FLAG.0,
            Some(buffer.as_mut_ptr().cast()),
            &mut size,
        )
    }
    .map_err(windows_error("Windows package signing identity"))?;
    let usages = unsafe {
        &*(buffer
            .as_ptr()
            .cast::<windows::Win32::Security::Cryptography::CTL_USAGE>())
    };
    if usages.cUsageIdentifier > 64
        || (usages.cUsageIdentifier != 0 && usages.rgpszUsageIdentifier.is_null())
    {
        return Err(signing_identity_error(
            "package signer returned malformed enhanced key usage metadata",
        ));
    }
    let mut found = vec![false; expected_oids.len()];
    for index in 0..usages.cUsageIdentifier as usize {
        let identifier = unsafe { *usages.rgpszUsageIdentifier.add(index) };
        if identifier.0.is_null() {
            return Err(signing_identity_error(
                "package signer returned malformed enhanced key usage metadata",
            ));
        }
        let identifier = unsafe { CStr::from_ptr(identifier.0.cast()) };
        for (expected_index, expected_oid) in expected_oids.iter().enumerate() {
            if identifier.to_bytes() == expected_oid.as_bytes() {
                found[expected_index] = true;
            }
        }
    }
    Ok(found.into_iter().all(|present| present))
}

fn certificate_subject_matches(
    certificate: *const CERT_CONTEXT,
    expected_subject: &str,
) -> Result<bool, DesktopDistributionError> {
    let expected_subject = expected_subject
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let string_type = CERT_STRING_TYPE(CERT_X500_NAME_STR.0 | CERT_NAME_STR_REVERSE_FLAG);
    let mut encoded_size = 0_u32;
    unsafe {
        CertStrToNameW(
            X509_ASN_ENCODING,
            PCWSTR(expected_subject.as_ptr()),
            string_type,
            None,
            None,
            &mut encoded_size,
            None,
        )
    }
    .map_err(windows_error("Windows package signing identity"))?;
    if encoded_size == 0 || u64::from(encoded_size) > MAX_APPX_SIGNATURE_SIZE {
        return Err(signing_identity_error(
            "expected publisher subject could not be encoded",
        ));
    }
    let mut encoded = vec![0_u8; encoded_size as usize];
    unsafe {
        CertStrToNameW(
            X509_ASN_ENCODING,
            PCWSTR(expected_subject.as_ptr()),
            string_type,
            None,
            Some(encoded.as_mut_ptr()),
            &mut encoded_size,
            None,
        )
    }
    .map_err(windows_error("Windows package signing identity"))?;
    let expected_name = CRYPT_INTEGER_BLOB {
        cbData: encoded_size,
        pbData: encoded.as_mut_ptr(),
    };
    let certificate = unsafe { &*certificate };
    if certificate.pCertInfo.is_null() {
        return Err(signing_identity_error(
            "package signer returned malformed subject metadata",
        ));
    }
    Ok(unsafe {
        CertCompareCertificateName(
            X509_ASN_ENCODING,
            &expected_name,
            &(*certificate.pCertInfo).Subject,
        )
        .as_bool()
    })
}

fn chain_has_expected_root(
    chain: *const CERT_CHAIN_CONTEXT,
) -> Result<bool, DesktopDistributionError> {
    let chain = unsafe { &*chain };
    if chain.cChain == 0 || chain.rgpChain.is_null() || chain.cChain > 8 {
        return Err(signing_identity_error(
            "package signer returned a malformed certificate chain",
        ));
    }
    if chain.TrustStatus.dwErrorStatus & !CERT_TRUST_IS_NOT_TIME_VALID != 0 {
        return Ok(false);
    }
    for index in 0..chain.cChain as usize {
        let simple = unsafe { *chain.rgpChain.add(index) };
        if simple.is_null() {
            return Err(signing_identity_error(
                "package signer returned a malformed certificate chain",
            ));
        }
        let simple = unsafe { &*simple };
        if simple.TrustStatus.dwErrorStatus & !CERT_TRUST_IS_NOT_TIME_VALID != 0
            || simple.cElement < 2
            || simple.cElement > 8
            || simple.rgpElement.is_null()
        {
            continue;
        }
        let root_element = unsafe { *simple.rgpElement.add(simple.cElement as usize - 1) };
        if root_element.is_null() {
            continue;
        }
        let root = unsafe { (*root_element).pCertContext };
        if root.is_null() {
            continue;
        }
        let root = unsafe { &*root };
        if root.pbCertEncoded.is_null() || root.cbCertEncoded == 0 {
            continue;
        }
        let encoded =
            unsafe { std::slice::from_raw_parts(root.pbCertEncoded, root.cbCertEncoded as usize) };
        let fingerprint: [u8; 32] = Sha256::digest(encoded).into();
        if fingerprint == DIRECT_SIGNING_ROOT_CERT_SHA256 {
            return Ok(true);
        }
    }
    Ok(false)
}

fn aligned_buffer(size: u32) -> Vec<usize> {
    let words = (size as usize).div_ceil(size_of::<usize>());
    vec![0; words]
}

fn signing_identity_error(message: impl Into<String>) -> DesktopDistributionError {
    DesktopDistributionError::Verification {
        stage: "Windows package signing identity",
        message: message.into(),
    }
}

struct CryptQueryHandles {
    store: HCERTSTORE,
    message: *mut c_void,
}

impl Drop for CryptQueryHandles {
    fn drop(&mut self) {
        unsafe {
            if !self.message.is_null() {
                let _ = CryptMsgClose(Some(self.message));
            }
            if !self.store.is_invalid() {
                let _ = CertCloseStore(self.store, 0);
            }
        }
    }
}

struct CertificateContext(*mut CERT_CONTEXT);

impl Drop for CertificateContext {
    fn drop(&mut self) {
        unsafe {
            let _ = CertFreeCertificateContext(Some(self.0));
        }
    }
}

struct CertificateChain(*mut CERT_CHAIN_CONTEXT);

impl Drop for CertificateChain {
    fn drop(&mut self) {
        unsafe { CertFreeCertificateChain(self.0) };
    }
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
