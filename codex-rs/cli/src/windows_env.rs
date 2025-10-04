#[cfg(target_os = "windows")]
use std::collections::HashMap;
#[cfg(target_os = "windows")]
use std::env;
#[cfg(target_os = "windows")]
use std::path::PathBuf;
#[cfg(target_os = "windows")]
use std::process::Command;

#[cfg(target_os = "windows")]
const REQUIRED_ENV_VARS: &[&str] = &["INCLUDE", "LIB", "LIBPATH", "VSINSTALLDIR", "WindowsSdkDir"];

#[cfg(target_os = "windows")]
const BOOTSTRAP_ENV_VARS: &[&str] = &[
    "COMSPEC",
    "SYSTEMROOT",
    "SYSTEMDRIVE",
    "PROGRAMFILES",
    "PROGRAMFILES(X86)",
    "PROGRAMDATA",
    "LOCALAPPDATA",
    "APPDATA",
    "USERPROFILE",
    "HOMEDRIVE",
    "HOMEPATH",
    "TEMP",
    "TMP",
    "POWERSHELL",
    "PWSH",
    "PATH",
    "PATHEXT",
    "USERNAME",
    "USERDOMAIN",
    "INCLUDE",
    "LIB",
    "LIBPATH",
    "VSINSTALLDIR",
    "VCINSTALLDIR",
    "VCToolsInstallDir",
    "VCToolsVersion",
    "VCToolsRedistDir",
    "VisualStudioVersion",
    "VS170COMNTOOLS",
    "DevEnvDir",
    "VCIDEInstallDir",
    "WindowsSdkDir",
    "WindowsSDKVersion",
    "WindowsSDKLibVersion",
    "WindowsSdkBinPath",
    "WindowsSdkVerBinPath",
    "WindowsLibPath",
    "UCRTVersion",
    "UniversalCRTSdkDir",
    "ExtensionSdkDir",
    "FrameworkDir",
    "FrameworkDir64",
    "FrameworkVersion",
    "FrameworkVersion64",
];

#[cfg(target_os = "windows")]
const BOOTSTRAP_ENV_PREFIXES: &[&str] = &["VSCMD_"];

#[cfg(target_os = "windows")]
const SKIP_BOOTSTRAP_ENV: &str = "CODEX_SKIP_VISUAL_STUDIO_ENV";

#[cfg(target_os = "windows")]
pub fn bootstrap_visual_studio_env_if_available() {
    if env::var_os(SKIP_BOOTSTRAP_ENV).is_some() {
        return;
    }

    if !REQUIRED_ENV_VARS.iter().any(is_missing_or_empty) {
        return;
    }

    if let Err(err) = try_bootstrap_visual_studio_env() {
        // Silently ignore failures in release builds; developers can opt-in by
        // setting RUST_LOG=debug to see this message.
        #[cfg(debug_assertions)]
        eprintln!("codex: failed to load Visual Studio environment: {err:?}");
    }
}

#[cfg(not(target_os = "windows"))]
pub fn bootstrap_visual_studio_env_if_available() {}

#[cfg(target_os = "windows")]
fn is_missing_or_empty(var: &&str) -> bool {
    match env::var_os(var) {
        Some(val) if !val.is_empty() => false,
        _ => true,
    }
}

#[cfg(target_os = "windows")]
fn try_bootstrap_visual_studio_env() -> anyhow::Result<()> {
    let vswhere = find_vswhere().ok_or_else(|| anyhow::anyhow!("vswhere.exe not found"))?;

    let output = Command::new(&vswhere)
        .args([
            "-latest",
            "-requires",
            "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
            "-property",
            "installationPath",
        ])
        .output()
        .map_err(|err| anyhow::anyhow!("failed to run vswhere.exe: {err}"))?;

    if !output.status.success() {
        anyhow::bail!("vswhere.exe exited with status {:?}", output.status.code());
    }

    let installation_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if installation_path.is_empty() {
        anyhow::bail!("vswhere.exe reported an empty installationPath");
    }

    let install_dir = PathBuf::from(installation_path);
    let vcvarsall = install_dir.join("VC\\Auxiliary\\Build\\vcvarsall.bat");
    if !vcvarsall.exists() {
        anyhow::bail!("vcvarsall.bat not found at {}", vcvarsall.display());
    }

    let mut command = Command::new("cmd");
    let script = format!("\"{}\" x64 && set", vcvarsall.display());
    let output = command
        .arg("/C")
        .arg(script)
        .current_dir(&install_dir)
        .env("VSCMD_SKIP_SENDTELEMETRY", "1")
        .env("VSCMD_START_DIR", ".")
        .output()
        .map_err(|err| anyhow::anyhow!("failed to run vcvarsall.bat: {err}"))?;

    if !output.status.success() {
        anyhow::bail!(
            "vcvarsall.bat exited with status {:?}",
            output.status.code()
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut exported = HashMap::new();
    for line in stdout.lines() {
        if let Some((key, value)) = line.split_once('=')
            && !key.is_empty()
        {
            exported.insert(key.to_string(), value.trim().to_string());
        }
    }

    if exported.is_empty() {
        anyhow::bail!("vcvarsall.bat did not emit any environment variables");
    }

    let mut applied = false;
    for key in BOOTSTRAP_ENV_VARS {
        if let Some(value) = exported.get(*key) {
            unsafe {
                env::set_var(key, value);
            }
            applied = true;
        }
    }

    for (key, value) in &exported {
        if BOOTSTRAP_ENV_PREFIXES
            .iter()
            .any(|prefix| key.starts_with(prefix))
        {
            unsafe {
                env::set_var(key, value);
            }
            applied = true;
        }
    }

    if applied {
        unsafe {
            env::set_var("CODEX_VS_ENV_BOOTSTRAPPED", "1");
        }
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn find_vswhere() -> Option<PathBuf> {
    if let Some(path) = env::var_os("VSWHERE").map(PathBuf::from)
        && path.is_file()
    {
        return Some(path);
    }

    let common_locations = [
        env::var_os("ProgramFiles(x86)").map(PathBuf::from),
        env::var_os("ProgramFiles").map(PathBuf::from),
    ];

    for base in common_locations.into_iter().flatten() {
        let candidate = base
            .join("Microsoft Visual Studio")
            .join("Installer")
            .join("vswhere.exe");
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    None
}
