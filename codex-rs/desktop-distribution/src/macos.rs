use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use security_framework::os::macos::code_signing::Flags;
use security_framework::os::macos::code_signing::GuestAttributes;
use security_framework::os::macos::code_signing::SecCode;
use security_framework::os::macos::code_signing::SecRequirement;

use crate::DesktopDistributionError;

const OPENAI_TEAM_ID: &str = "2DC432GLL2";
const CODEX_BUNDLE_IDENTIFIERS: &[&str] = &[
    "com.openai.codex",
    "com.openai.codex.agent",
    "com.openai.codex.dev",
    "com.openai.codex.nightly",
    "com.openai.codex.alpha",
    "com.openai.codex.beta",
];
const PRODUCT_NAMES: &[&str] = &[
    "Codex",
    "Codex (Beta)",
    "Codex (Alpha)",
    "Codex (Nightly)",
    "Codex (Agent)",
    "Codex (Dev)",
];

#[derive(Debug, Clone)]
pub(crate) struct PlatformIdentity {
    identifier: String,
}

pub(crate) struct PlatformDistribution {
    pub app_root: PathBuf,
    pub resources_relative_path: PathBuf,
    pub identity: PlatformIdentity,
}

pub(crate) fn verify_hint(hint: &Path) -> Result<PlatformDistribution, DesktopDistributionError> {
    let canonical_hint =
        std::fs::canonicalize(hint).map_err(|source| DesktopDistributionError::Filesystem {
            stage: "macOS app hint",
            source,
        })?;
    let app_root =
        enclosing_app(&canonical_hint).ok_or_else(|| DesktopDistributionError::Verification {
            stage: "app hint",
            message: "hint is not enclosed by a macOS application bundle".to_string(),
        })?;
    let identity = verify_app(&app_root, /*expected_identifier*/ None)?;
    Ok(distribution(app_root, identity))
}

pub(crate) fn discover() -> Result<PlatformDistribution, DesktopDistributionError> {
    let mut found_candidate = false;
    for candidate in installed_app_candidates() {
        if !candidate.is_dir() {
            continue;
        }
        found_candidate = true;
        if let Ok(identity) = verify_app(&candidate, /*expected_identifier*/ None) {
            return Ok(distribution(candidate, identity));
        }
    }
    if found_candidate {
        Err(DesktopDistributionError::Verification {
            stage: "installed app discovery",
            message: "installed Codex candidates failed identity or sealed-resource validation"
                .to_string(),
        })
    } else {
        Err(DesktopDistributionError::NotFound)
    }
}

pub(crate) fn current_process_distribution()
-> Result<Option<PlatformDistribution>, DesktopDistributionError> {
    let Ok(current_exe) = std::env::current_exe() else {
        return Ok(None);
    };
    let Some(app_root) = enclosing_app(&current_exe) else {
        return Ok(None);
    };
    if !app_claims_codex_identity(&app_root)? {
        return Ok(None);
    }
    let identity = verify_app(&app_root, /*expected_identifier*/ None)?;
    Ok(Some(distribution(app_root, identity)))
}

pub(crate) fn reverify(
    identity: &PlatformIdentity,
    app_root: &Path,
) -> Result<(), DesktopDistributionError> {
    // This detects static mutation and ordinary update races for the path-backed app bundle. It
    // is not a transaction against an already-running hostile same-UID process racing a
    // user-writable installation; enforcing that stronger boundary requires an immutable install
    // root or an OS-authenticated launch broker. Hook execution separately authenticates the live,
    // suspended child before it can run user-space code.
    verify_app(app_root, Some(&identity.identifier)).map(|_| ())
}

pub(crate) fn authenticate_spawned_executable(
    pid: i32,
    expected_executable: &Path,
    expected_identifier: &str,
) -> Result<(), DesktopDistributionError> {
    let mut attributes = GuestAttributes::new();
    attributes.set_pid(pid);
    let code =
        SecCode::copy_guest_with_attribues(None, &attributes, Flags::NONE).map_err(|error| {
            DesktopDistributionError::Verification {
                stage: "macOS spawned code identity",
                message: format!("failed to resolve suspended process code: {error}"),
            }
        })?;
    let requirement: SecRequirement = format!(
        "anchor apple generic and certificate 1[field.1.2.840.113635.100.6.2.6] exists and certificate leaf[field.1.2.840.113635.100.6.1.13] exists and certificate leaf[subject.OU] = \"{OPENAI_TEAM_ID}\" and identifier \"{expected_identifier}\""
    )
    .parse()
    .map_err(|error| DesktopDistributionError::Verification {
        stage: "macOS spawned code requirement",
        message: format!("failed to build spawned code requirement: {error}"),
    })?;
    code.check_validity(
        Flags::STRICT_VALIDATE | Flags::NO_NETWORK_ACCESS,
        &requirement,
    )
    .map_err(|error| DesktopDistributionError::Verification {
        stage: "macOS spawned code signature",
        message: format!("suspended process failed dynamic code validation: {error}"),
    })?;

    let code_path = code
        .path(Flags::NONE)
        .map_err(|error| DesktopDistributionError::Verification {
            stage: "macOS spawned code path",
            message: format!("failed to resolve suspended process path: {error}"),
        })?
        .to_path()
        .ok_or_else(|| DesktopDistributionError::Verification {
            stage: "macOS spawned code path",
            message: "suspended process path was not a filesystem path".to_string(),
        })?;
    let code_path = std::fs::canonicalize(code_path).map_err(|source| {
        DesktopDistributionError::Filesystem {
            stage: "macOS spawned code path",
            source,
        }
    })?;
    let expected_executable = std::fs::canonicalize(expected_executable).map_err(|source| {
        DesktopDistributionError::Filesystem {
            stage: "macOS expected executable path",
            source,
        }
    })?;
    let expected_code_root = enclosing_app(&expected_executable).ok_or_else(|| {
        DesktopDistributionError::Verification {
            stage: "macOS spawned code path",
            message: "expected executable is not enclosed by a nested application bundle"
                .to_string(),
        }
    })?;
    if code_path != expected_executable && code_path != expected_code_root {
        return Err(DesktopDistributionError::Verification {
            stage: "macOS spawned code path",
            message: "suspended process does not match the expected bundled executable".to_string(),
        });
    }
    Ok(())
}

fn distribution(app_root: PathBuf, identity: PlatformIdentity) -> PlatformDistribution {
    PlatformDistribution {
        app_root,
        resources_relative_path: PathBuf::from("Contents/Resources"),
        identity,
    }
}

fn enclosing_app(path: &Path) -> Option<PathBuf> {
    path.ancestors()
        .find(|ancestor| {
            ancestor
                .extension()
                .is_some_and(|extension| extension == "app")
        })
        .map(Path::to_path_buf)
}

fn installed_app_candidates() -> Vec<PathBuf> {
    let application_dirs = [
        Some(PathBuf::from("/Applications")),
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Applications")),
    ];
    application_dirs
        .into_iter()
        .flatten()
        .flat_map(|directory| {
            PRODUCT_NAMES
                .iter()
                .map(move |name| directory.join(format!("{name}.app")))
        })
        .collect()
}

fn app_claims_codex_identity(app_root: &Path) -> Result<bool, DesktopDistributionError> {
    if displayed_identifier(app_root)?
        .is_some_and(|identifier| CODEX_BUNDLE_IDENTIFIERS.contains(&identifier.as_str()))
    {
        return Ok(true);
    }

    Ok(app_root
        .file_stem()
        .and_then(|name| name.to_str())
        .is_some_and(|name| PRODUCT_NAMES.contains(&name)))
}

fn displayed_identifier(app_root: &Path) -> Result<Option<String>, DesktopDistributionError> {
    Ok(displayed_codesign_details(app_root)?
        .and_then(|details| codesign_detail(&details, "Identifier=").map(str::to_string)))
}

fn displayed_codesign_details(app_root: &Path) -> Result<Option<String>, DesktopDistributionError> {
    let details = Command::new("/usr/bin/codesign")
        .args(["--display", "--verbose=4"])
        .arg(app_root)
        .output()
        .map_err(|source| DesktopDistributionError::Filesystem {
            stage: "macOS code signature identity",
            source,
        })?;
    if !details.status.success() {
        return Ok(None);
    }
    Ok(Some(String::from_utf8_lossy(&details.stderr).into_owned()))
}

fn verify_app(
    app_root: &Path,
    expected_identifier: Option<&str>,
) -> Result<PlatformIdentity, DesktopDistributionError> {
    let identifiers = expected_identifier.map_or_else(
        || {
            CODEX_BUNDLE_IDENTIFIERS
                .iter()
                .map(|identifier| format!("identifier \"{identifier}\""))
                .collect::<Vec<_>>()
                .join(" or ")
        },
        |identifier| format!("identifier \"{identifier}\""),
    );
    let requirement = format!(
        "=anchor apple generic and certificate 1[field.1.2.840.113635.100.6.2.6] exists and certificate leaf[field.1.2.840.113635.100.6.1.13] exists and certificate leaf[subject.OU] = \"{OPENAI_TEAM_ID}\" and ({identifiers})"
    );
    let output = Command::new("/usr/bin/codesign")
        .args(["--verify", "--deep", "--strict", "--verbose=2", "-R"])
        .arg(requirement)
        .arg(app_root)
        .output()
        .map_err(|source| DesktopDistributionError::Filesystem {
            stage: "macOS code signature invocation",
            source,
        })?;
    if !output.status.success() {
        return Err(DesktopDistributionError::Verification {
            stage: "macOS code signature and sealed resources",
            message: format!("codesign rejected the application ({})", output.status),
        });
    }
    let details = displayed_codesign_details(app_root)?.ok_or_else(|| {
        DesktopDistributionError::Verification {
            stage: "macOS code signature identity",
            message: "codesign output did not contain a bundle identifier".to_string(),
        }
    })?;
    let identifier = codesign_detail(&details, "Identifier=").ok_or_else(|| {
        DesktopDistributionError::Verification {
            stage: "macOS code signature identity",
            message: "codesign output did not contain a bundle identifier".to_string(),
        }
    })?;
    let team_identifier = codesign_detail(&details, "TeamIdentifier=").ok_or_else(|| {
        DesktopDistributionError::Verification {
            stage: "macOS code signature identity",
            message: "codesign output did not contain a team identifier".to_string(),
        }
    })?;
    if !CODEX_BUNDLE_IDENTIFIERS.contains(&identifier) || team_identifier != OPENAI_TEAM_ID {
        return Err(DesktopDistributionError::Verification {
            stage: "macOS code signature identity",
            message: "application identity is not allowlisted for Codex Desktop".to_string(),
        });
    }
    if expected_identifier.is_some_and(|expected| expected != identifier) {
        return Err(DesktopDistributionError::Verification {
            stage: "macOS code signature identity",
            message: "application identity changed after initial verification".to_string(),
        });
    }
    Ok(PlatformIdentity {
        identifier: identifier.to_string(),
    })
}

fn codesign_detail<'a>(details: &'a str, prefix: &str) -> Option<&'a str> {
    details
        .lines()
        .find_map(|line| line.strip_prefix(prefix))
        .map(str::trim)
}
