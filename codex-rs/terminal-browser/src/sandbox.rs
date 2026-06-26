use std::collections::HashMap;
use std::ffi::OsString;
use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSpecialPath;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_sandboxing::SandboxCommand;
use codex_sandboxing::SandboxDirectSpawnTransformRequest;
use codex_sandboxing::SandboxManager;
use codex_sandboxing::SandboxTransformRequest;
use codex_sandboxing::SandboxType;
use codex_sandboxing::SandboxablePreference;
use codex_sandboxing::WindowsSandboxProxySettingsMode;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;

use crate::network::BrowserNetworkPolicy;

/// Host-local inputs needed to wrap Carbonyl in the Codex platform sandbox.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BrowserLaunchContext {
    /// Codex data directory used to store optional named browser profiles.
    pub codex_home: Option<AbsolutePathBuf>,
    /// Workspace whose identity scopes optional named browser profiles.
    pub workspace_root: Option<AbsolutePathBuf>,
    /// Absolute path to the Codex Linux sandbox helper, when running on Linux.
    pub codex_linux_sandbox_exe: Option<AbsolutePathBuf>,
    /// Whether to request the deprecated Landlock implementation instead of bubblewrap.
    pub use_legacy_landlock: bool,
}

pub(crate) struct PreparedBrowserLaunch {
    pub(crate) program: String,
    pub(crate) args: Vec<String>,
    pub(crate) cwd: AbsolutePathBuf,
    pub(crate) env: HashMap<String, String>,
    pub(crate) arg0: Option<String>,
}

pub(crate) fn prepare_browser_launch(
    binary: &Path,
    args: Vec<String>,
    browser_root: &AbsolutePathBuf,
    profile_root: &AbsolutePathBuf,
    env: HashMap<String, String>,
    network_policy: &BrowserNetworkPolicy,
    context: &BrowserLaunchContext,
) -> Result<PreparedBrowserLaunch> {
    if matches!(network_policy, BrowserNetworkPolicy::ManagedProxy { .. }) {
        anyhow::bail!(
            "managed terminal-browser networking is not yet supported because Carbonyl needs a loopback CDP listener that the current sandbox cannot permit without bypassing the managed proxy"
        );
    }

    let binary = AbsolutePathBuf::from_absolute_path(binary)
        .context("resolve Carbonyl executable path")?
        .canonicalize()
        .context("canonicalize Carbonyl executable")?;
    let binary_root = binary
        .parent()
        .context("Carbonyl executable has no parent directory")?;
    ensure_isolated_binary_root(&binary_root, browser_root, profile_root, context)?;
    let file_system_policy =
        browser_file_system_policy(browser_root, profile_root, &binary_root, &env)?;
    let network_sandbox_policy = match network_policy {
        BrowserNetworkPolicy::Disabled | BrowserNetworkPolicy::ManagedProxy { .. } => {
            NetworkSandboxPolicy::Restricted
        }
        BrowserNetworkPolicy::Direct => NetworkSandboxPolicy::Enabled,
    };
    let permissions =
        PermissionProfile::from_runtime_permissions(&file_system_policy, network_sandbox_policy);
    let enforce_managed_network = false;
    let manager = SandboxManager::new();
    let sandbox = manager.select_initial(
        &file_system_policy,
        network_sandbox_policy,
        SandboxablePreference::Require,
        WindowsSandboxLevel::Disabled,
        enforce_managed_network,
    );
    anyhow::ensure!(
        sandbox != SandboxType::None,
        "terminal browser sandbox is unavailable on this platform"
    );
    let browser_root_uri = PathUri::from_abs_path(browser_root);
    let mut request = manager
        .transform_for_direct_spawn(SandboxDirectSpawnTransformRequest {
            transform: SandboxTransformRequest {
                command: SandboxCommand {
                    program: OsString::from(binary.as_os_str()),
                    args,
                    cwd: browser_root_uri.clone(),
                    env,
                    managed_network: None,
                    additional_permissions: None,
                },
                permissions: &permissions,
                sandbox,
                enforce_managed_network,
                environment_id: None,
                network: None,
                sandbox_policy_cwd: &browser_root_uri,
                codex_linux_sandbox_exe: context
                    .codex_linux_sandbox_exe
                    .as_ref()
                    .map(AbsolutePathBuf::as_path),
                use_legacy_landlock: context.use_legacy_landlock,
                windows_sandbox_level: WindowsSandboxLevel::Disabled,
                windows_sandbox_private_desktop: false,
            },
            workspace_roots: &[],
            windows_sandbox_proxy_settings_mode: WindowsSandboxProxySettingsMode::Reconcile,
        })
        .context("prepare Carbonyl sandbox")?;
    #[cfg(target_os = "macos")]
    if request.sandbox == SandboxType::MacosSeatbelt {
        anyhow::ensure!(
            request.command.get(1).map(String::as_str) == Some("-p"),
            "Carbonyl sandbox returned an unexpected Seatbelt command"
        );
        let profile = request
            .command
            .get_mut(2)
            .context("Carbonyl sandbox omitted the Seatbelt profile")?;
        // Chromium's browser process registers a PID-scoped rendezvous service that its children
        // look up to exchange Mach ports. Keep both grants specific to Carbonyl's fixed service
        // name; the shared Codex Seatbelt policy intentionally does not permit arbitrary Mach IPC.
        profile.push_str(
            r#"
(allow mach-register mach-lookup
  (global-name-prefix "org.chromium.Chromium.MachPortRendezvousServer."))"#,
        );
    }
    let (program, args) = request
        .command
        .split_first()
        .context("Carbonyl sandbox returned an empty command")?;
    let cwd = request
        .cwd
        .to_abs_path()
        .context("resolve sandboxed Carbonyl working directory")?;
    Ok(PreparedBrowserLaunch {
        program: program.clone(),
        args: args.to_vec(),
        cwd,
        env: request.env,
        arg0: request.arg0,
    })
}

fn ensure_isolated_binary_root(
    binary_root: &AbsolutePathBuf,
    browser_root: &AbsolutePathBuf,
    profile_root: &AbsolutePathBuf,
    context: &BrowserLaunchContext,
) -> Result<()> {
    for protected_root in [
        Some(browser_root),
        Some(profile_root),
        context.codex_home.as_ref(),
        context.workspace_root.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        let canonical_root = protected_root
            .canonicalize()
            .unwrap_or_else(|_| protected_root.clone());
        if binary_root.as_path().starts_with(canonical_root.as_path())
            || canonical_root.as_path().starts_with(binary_root.as_path())
        {
            anyhow::bail!(
                "Carbonyl must be installed in a dedicated bundle outside the workspace, Codex home, and browser data directories"
            );
        }
    }
    Ok(())
}

fn browser_file_system_policy(
    browser_root: &AbsolutePathBuf,
    profile_root: &AbsolutePathBuf,
    binary_root: &AbsolutePathBuf,
    env: &HashMap<String, String>,
) -> Result<FileSystemSandboxPolicy> {
    let mut entries = vec![
        FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Minimal,
            },
            access: FileSystemAccessMode::Read,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: binary_root.clone(),
            },
            access: FileSystemAccessMode::Read,
        },
        FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: browser_root.clone(),
            },
            access: FileSystemAccessMode::Write,
        },
    ];
    #[cfg(target_os = "macos")]
    for (path, label) in [
        (
            "/System/Library/CoreServices/SystemAppearance.bundle",
            "macOS system appearance bundle",
        ),
        (
            "/System/Library/CoreServices/SystemVersion.bundle",
            "macOS system version bundle",
        ),
        ("/System/Library/Fonts", "macOS system fonts"),
    ] {
        entries.push(FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: AbsolutePathBuf::from_absolute_path(path)
                    .with_context(|| format!("resolve the {label}"))?,
            },
            access: FileSystemAccessMode::Read,
        });
    }
    if profile_root != browser_root && !profile_root.as_path().starts_with(browser_root.as_path()) {
        entries.push(FileSystemSandboxEntry {
            path: FileSystemPath::Path {
                path: profile_root.clone(),
            },
            access: FileSystemAccessMode::Write,
        });
    }
    for key in ["SSL_CERT_FILE", "SSL_CERT_DIR"] {
        let Some(path) = env.get(key) else {
            continue;
        };
        let path = AbsolutePathBuf::from_absolute_path_checked(path)
            .with_context(|| format!("resolve {key} for the Carbonyl sandbox"))?;
        entries.push(FileSystemSandboxEntry {
            path: FileSystemPath::Path { path },
            access: FileSystemAccessMode::Read,
        });
    }
    Ok(FileSystemSandboxPolicy::restricted(entries))
}

#[cfg(test)]
#[path = "sandbox_tests.rs"]
mod tests;
