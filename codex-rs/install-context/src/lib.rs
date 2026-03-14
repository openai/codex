use std::path::Path;
use std::path::PathBuf;
use std::sync::OnceLock;

use serde::Deserialize;

const METADATA_FILENAME: &str = "metadata.toml";
static INSTALL_CONTEXT: OnceLock<InstallContext> = OnceLock::new();

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativePlatform {
    Unix,
    Windows,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InstallContext {
    Native {
        /// The native release directory that contains `codex`, `rg`, and
        /// `metadata.toml`, for example
        /// `~/.codex/packages/native/releases/0.111.0-x86_64-unknown-linux-musl`.
        release_dir: PathBuf,
        /// The installed native Codex version, for example `0.111.0`.
        version: String,
        /// The target triple recorded in native metadata, for example
        /// `x86_64-unknown-linux-musl` or `aarch64-apple-darwin`.
        target: String,
        /// The bundled ripgrep binary for this native release, for example
        /// `~/.codex/packages/native/releases/.../rg`.
        rg_command: PathBuf,
        /// The platform of the native release, either `Unix` or `Windows`.
        platform: NativePlatform,
    },
    /// A Codex binary launched through the npm-managed `codex.js` shim.
    Npm,
    /// A Codex binary launched through the bun-managed `codex.js` shim.
    Bun,
    /// A Codex binary that appears to come from a Homebrew install prefix.
    Brew,
    /// Any other execution environment.
    ///
    /// This commonly covers `cargo run`, app-bundled Codex binaries, custom
    /// internal launchers, and tests that execute Codex from an arbitrary path.
    Other,
}

impl InstallContext {
    pub fn from_exe(
        is_macos: bool,
        current_exe: Option<&Path>,
        managed_by_npm: bool,
        managed_by_bun: bool,
    ) -> Self {
        if managed_by_npm {
            return Self::Npm;
        }

        if managed_by_bun {
            return Self::Bun;
        }

        if let Some(exe_path) = current_exe
            && let Some(native_context) = native_install_context(exe_path)
        {
            return native_context;
        }

        if is_macos
            && let Some(exe_path) = current_exe
            && (exe_path.starts_with("/opt/homebrew") || exe_path.starts_with("/usr/local"))
        {
            return Self::Brew;
        }

        Self::Other
    }

    pub fn current() -> &'static Self {
        INSTALL_CONTEXT.get_or_init(|| {
            let current_exe = std::env::current_exe().ok();
            let managed_by_npm = std::env::var_os("CODEX_MANAGED_BY_NPM").is_some();
            let managed_by_bun = std::env::var_os("CODEX_MANAGED_BY_BUN").is_some();
            Self::from_exe(
                cfg!(target_os = "macos"),
                current_exe.as_deref(),
                managed_by_npm,
                managed_by_bun,
            )
        })
    }

    pub fn rg_command(&self) -> PathBuf {
        match self {
            Self::Native { rg_command, .. } => rg_command.clone(),
            Self::Npm | Self::Bun | Self::Brew | Self::Other => default_rg_command(),
        }
    }
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
struct NativeInstallMetadata {
    install_method: String,
    version: String,
    target: String,
}

fn native_install_context(exe_path: &Path) -> Option<InstallContext> {
    let canonical_exe = std::fs::canonicalize(exe_path).ok()?;
    let release_dir = canonical_exe.parent()?.to_path_buf();
    let metadata = parse_native_install_metadata(&release_dir.join(METADATA_FILENAME))?;
    let platform = native_platform_from_target(&metadata.target);
    let rg_name = match platform {
        NativePlatform::Unix => "rg",
        NativePlatform::Windows => "rg.exe",
    };
    let rg_command = release_dir.join(rg_name);

    Some(InstallContext::Native {
        platform,
        release_dir,
        version: metadata.version,
        target: metadata.target,
        rg_command,
    })
}

fn parse_native_install_metadata(path: &Path) -> Option<NativeInstallMetadata> {
    let contents = std::fs::read_to_string(path).ok()?;
    let metadata: NativeInstallMetadata = toml::from_str(&contents).ok()?;
    if metadata.install_method != "native" {
        return None;
    }
    Some(metadata)
}

fn native_platform_from_target(target: &str) -> NativePlatform {
    if target.contains("-windows-") {
        NativePlatform::Windows
    } else {
        NativePlatform::Unix
    }
}

fn default_rg_command() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from("rg.exe")
    } else {
        PathBuf::from("rg")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::fs;

    #[test]
    fn detects_native_install_from_adjacent_metadata() -> std::io::Result<()> {
        let root = tempfile::tempdir()?;
        let release_dir = root.path().join("1.2.3-x86_64-unknown-linux-musl");
        fs::create_dir(&release_dir)?;
        fs::write(
            release_dir.join("metadata.toml"),
            "install_method = \"native\"\nversion = \"1.2.3\"\ntarget = \"x86_64-unknown-linux-musl\"\n",
        )?;
        let exe_name = if cfg!(windows) { "codex.exe" } else { "codex" };
        let rg_name = "rg";
        let exe_path = release_dir.join(exe_name);
        fs::write(&exe_path, "")?;
        fs::write(release_dir.join(rg_name), "")?;
        let canonical_release_dir = release_dir.canonicalize()?;

        let context = InstallContext::from_exe(false, Some(&exe_path), false, false);
        assert_eq!(
            context,
            InstallContext::Native {
                platform: NativePlatform::Unix,
                release_dir: canonical_release_dir.clone(),
                version: "1.2.3".to_string(),
                target: "x86_64-unknown-linux-musl".to_string(),
                rg_command: canonical_release_dir.join(rg_name),
            }
        );
        Ok(())
    }

    #[test]
    fn detects_windows_native_platform_from_target() -> std::io::Result<()> {
        let root = tempfile::tempdir()?;
        let release_dir = root.path().join("1.2.3-x86_64-pc-windows-msvc");
        fs::create_dir(&release_dir)?;
        fs::write(
            release_dir.join("metadata.toml"),
            "install_method = \"native\"\nversion = \"1.2.3\"\ntarget = \"x86_64-pc-windows-msvc\"\n",
        )?;
        let exe_path = release_dir.join("codex");
        fs::write(&exe_path, "")?;
        fs::write(release_dir.join("rg.exe"), "")?;
        let canonical_release_dir = release_dir.canonicalize()?;

        let context = InstallContext::from_exe(false, Some(&exe_path), false, false);
        assert_eq!(
            context,
            InstallContext::Native {
                platform: NativePlatform::Windows,
                release_dir: canonical_release_dir.clone(),
                version: "1.2.3".to_string(),
                target: "x86_64-pc-windows-msvc".to_string(),
                rg_command: canonical_release_dir.join("rg.exe"),
            }
        );
        Ok(())
    }

    #[test]
    fn native_metadata_rejects_non_native_install_method() -> std::io::Result<()> {
        let root = tempfile::tempdir()?;
        let release_dir = root.path().join("bad-release");
        fs::create_dir(&release_dir)?;
        fs::write(
            release_dir.join("metadata.toml"),
            "install_method = \"npm\"\nversion = \"1.2.3\"\ntarget = \"x86_64-unknown-linux-musl\"\n",
        )?;
        let exe_path = release_dir.join(if cfg!(windows) { "codex.exe" } else { "codex" });
        fs::write(&exe_path, "")?;

        let context = InstallContext::from_exe(false, Some(&exe_path), false, false);
        assert_eq!(context, InstallContext::Other);
        Ok(())
    }

    #[test]
    fn npm_and_bun_take_precedence() {
        let npm_context =
            InstallContext::from_exe(false, Some(Path::new("/tmp/codex")), true, false);
        assert_eq!(npm_context, InstallContext::Npm);

        let bun_context =
            InstallContext::from_exe(false, Some(Path::new("/tmp/codex")), false, true);
        assert_eq!(bun_context, InstallContext::Bun);
    }

    #[test]
    fn brew_is_detected_on_macos_prefixes() {
        let context = InstallContext::from_exe(
            true,
            Some(Path::new("/opt/homebrew/bin/codex")),
            false,
            false,
        );
        assert_eq!(context, InstallContext::Brew);
    }
}
