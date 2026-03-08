use std::path::Path;
use std::path::PathBuf;
use std::sync::OnceLock;

use serde::Deserialize;

const METADATA_FILENAME: &str = "metadata.toml";
static INSTALL_CONTEXT: OnceLock<InstallContext> = OnceLock::new();

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstallManager {
    Native,
    Npm,
    Bun,
    Brew,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstallContext {
    pub manager: InstallManager,
    pub current_exe: Option<PathBuf>,
    pub release_dir: Option<PathBuf>,
    pub version: Option<String>,
    pub target: Option<String>,
    pub rg_command: String,
}

impl InstallContext {
    pub fn from_exe(
        is_macos: bool,
        current_exe: Option<&Path>,
        managed_by_npm: bool,
        managed_by_bun: bool,
    ) -> Self {
        if managed_by_npm {
            return Self::unknown_with_manager(InstallManager::Npm, current_exe);
        }

        if managed_by_bun {
            return Self::unknown_with_manager(InstallManager::Bun, current_exe);
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
            return Self::unknown_with_manager(InstallManager::Brew, Some(exe_path));
        }

        Self::unknown_with_manager(InstallManager::Unknown, current_exe)
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

    fn unknown_with_manager(manager: InstallManager, current_exe: Option<&Path>) -> Self {
        Self {
            manager,
            current_exe: current_exe.map(Path::to_path_buf),
            release_dir: None,
            version: None,
            target: None,
            rg_command: default_rg_command(),
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

    let rg_name = if cfg!(windows) { "rg.exe" } else { "rg" };
    let rg_command = release_dir.join(rg_name).display().to_string();

    Some(InstallContext {
        manager: InstallManager::Native,
        current_exe: Some(canonical_exe),
        release_dir: Some(release_dir),
        version: Some(metadata.version),
        target: Some(metadata.target),
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

fn default_rg_command() -> String {
    if cfg!(windows) {
        "rg.exe".to_string()
    } else {
        "rg".to_string()
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
        let rg_name = if cfg!(windows) { "rg.exe" } else { "rg" };
        let exe_path = release_dir.join(exe_name);
        fs::write(&exe_path, "")?;
        fs::write(release_dir.join(rg_name), "")?;

        let context = InstallContext::from_exe(false, Some(&exe_path), false, false);
        assert_eq!(context.manager, InstallManager::Native);
        assert_eq!(context.release_dir, Some(release_dir.canonicalize()?));
        assert_eq!(context.version.as_deref(), Some("1.2.3"));
        assert_eq!(context.target.as_deref(), Some("x86_64-unknown-linux-musl"));
        assert!(context.rg_command.ends_with(rg_name));
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
        assert_eq!(context.manager, InstallManager::Unknown);
        assert_eq!(context.version, None);
        Ok(())
    }

    #[test]
    fn npm_and_bun_take_precedence() {
        let npm_context =
            InstallContext::from_exe(false, Some(Path::new("/tmp/codex")), true, false);
        assert_eq!(npm_context.manager, InstallManager::Npm);

        let bun_context =
            InstallContext::from_exe(false, Some(Path::new("/tmp/codex")), false, true);
        assert_eq!(bun_context.manager, InstallManager::Bun);
    }

    #[test]
    fn brew_is_detected_on_macos_prefixes() {
        let context = InstallContext::from_exe(
            true,
            Some(Path::new("/opt/homebrew/bin/codex")),
            false,
            false,
        );
        assert_eq!(context.manager, InstallManager::Brew);
    }
}
