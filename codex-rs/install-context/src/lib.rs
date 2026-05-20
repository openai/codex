use std::ffi::OsStr;
use std::path::Path;
use std::path::PathBuf;
use std::sync::OnceLock;

const BIN_DIRNAME: &str = "bin";
const PACKAGE_METADATA_FILENAME: &str = "codex-package.json";
const PATH_DIRNAME: &str = "codex-path";
const RELEASES_DIRNAME: &str = "releases";
const RESOURCES_DIRNAME: &str = "codex-resources";
const STANDALONE_PACKAGES_DIRNAME: &str = "standalone";
static INSTALL_CONTEXT: OnceLock<InstallContext> = OnceLock::new();

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StandalonePlatform {
    Unix,
    Windows,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodexPackageLayout {
    /// The package root that contains the metadata file and layout directories.
    pub package_dir: PathBuf,
    /// Directory containing the Codex entrypoint executable.
    pub bin_dir: PathBuf,
    /// Directory containing managed helper binaries and data files, when present.
    pub resources_dir: Option<PathBuf>,
    /// Directory containing executables that should be preferred over PATH, when present.
    pub path_dir: Option<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InstallContext {
    Standalone {
        /// The managed standalone release directory. Legacy installs use paths
        /// such as
        /// `~/.codex/packages/standalone/releases/0.111.0-x86_64-unknown-linux-musl`.
        /// Package-layout installs use the package root that contains `bin/`,
        /// `codex-resources/`, and `codex-path/`.
        release_dir: PathBuf,
        /// The bundled resource directory for managed dependencies.
        resources_dir: Option<PathBuf>,
        /// The canonical package layout when the executable is running from a
        /// package directory.
        package_layout: Option<CodexPackageLayout>,
        /// The platform of the standalone release, either `Unix` or `Windows`.
        platform: StandalonePlatform,
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
        let codex_home = codex_utils_home_dir::find_codex_home().ok();
        Self::from_exe_with_codex_home(
            is_macos,
            current_exe,
            managed_by_npm,
            managed_by_bun,
            codex_home.as_deref(),
        )
    }

    fn from_exe_with_codex_home(
        is_macos: bool,
        current_exe: Option<&Path>,
        managed_by_npm: bool,
        managed_by_bun: bool,
        codex_home: Option<&Path>,
    ) -> Self {
        if managed_by_npm {
            return Self::Npm;
        }

        if managed_by_bun {
            return Self::Bun;
        }

        if let Some(exe_path) = current_exe
            && let Some(standalone_context) = standalone_install_context(exe_path, codex_home)
        {
            return standalone_context;
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
            Self::Standalone {
                package_layout: Some(package_layout),
                ..
            } => package_layout
                .path_dir
                .as_ref()
                .and_then(|path_dir| {
                    let bundled_rg = path_dir.join(default_rg_command());
                    bundled_rg.exists().then_some(bundled_rg)
                })
                .unwrap_or_else(default_rg_command),
            Self::Standalone {
                resources_dir: Some(resources_dir),
                ..
            } => {
                let bundled_rg = resources_dir.join(default_rg_command());
                if bundled_rg.exists() {
                    bundled_rg
                } else {
                    default_rg_command()
                }
            }
            Self::Standalone {
                resources_dir: None,
                ..
            }
            | Self::Npm
            | Self::Bun
            | Self::Brew
            | Self::Other => default_rg_command(),
        }
    }
}

fn standalone_install_context(
    exe_path: &Path,
    codex_home: Option<&Path>,
) -> Option<InstallContext> {
    if let Some(package_context) = standalone_package_install_context(exe_path) {
        return Some(package_context);
    }

    let canonical_exe = std::fs::canonicalize(exe_path).ok()?;
    let canonical_codex_home = std::fs::canonicalize(codex_home?).ok()?;
    let release_dir = canonical_exe.parent()?.to_path_buf();
    let releases_root = canonical_codex_home
        .join("packages")
        .join(STANDALONE_PACKAGES_DIRNAME)
        .join(RELEASES_DIRNAME);
    if !release_dir.starts_with(releases_root) {
        return None;
    }

    let resources_dir = release_dir.join(RESOURCES_DIRNAME);
    Some(InstallContext::Standalone {
        release_dir,
        resources_dir: resources_dir.is_dir().then_some(resources_dir),
        package_layout: None,
        platform: standalone_platform(),
    })
}

fn standalone_package_install_context(exe_path: &Path) -> Option<InstallContext> {
    let canonical_exe = std::fs::canonicalize(exe_path).ok()?;
    let bin_dir = canonical_exe.parent()?;
    if bin_dir.file_name() != Some(OsStr::new(BIN_DIRNAME)) {
        return None;
    }

    let package_dir = bin_dir.parent()?.to_path_buf();
    if !package_dir.join(PACKAGE_METADATA_FILENAME).is_file() {
        return None;
    }

    let resources_dir = existing_dir(package_dir.join(RESOURCES_DIRNAME));
    let path_dir = existing_dir(package_dir.join(PATH_DIRNAME));
    let package_layout = CodexPackageLayout {
        package_dir: package_dir.clone(),
        bin_dir: bin_dir.to_path_buf(),
        resources_dir: resources_dir.clone(),
        path_dir,
    };
    Some(InstallContext::Standalone {
        release_dir: package_dir,
        resources_dir,
        package_layout: Some(package_layout),
        platform: standalone_platform(),
    })
}

fn standalone_platform() -> StandalonePlatform {
    if cfg!(windows) {
        StandalonePlatform::Windows
    } else {
        StandalonePlatform::Unix
    }
}

fn existing_dir(path: PathBuf) -> Option<PathBuf> {
    path.is_dir().then_some(path)
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
    fn detects_standalone_install_from_release_layout() -> std::io::Result<()> {
        let codex_home = tempfile::tempdir()?;
        let release_dir = codex_home
            .path()
            .join("packages/standalone/releases/1.2.3-x86_64-unknown-linux-musl");
        let resources_dir = release_dir.join(RESOURCES_DIRNAME);
        fs::create_dir_all(&resources_dir)?;
        let exe_path = release_dir.join(if cfg!(windows) { "codex.exe" } else { "codex" });
        fs::write(&exe_path, "")?;
        fs::write(resources_dir.join(default_rg_command()), "")?;
        let canonical_release_dir = release_dir.canonicalize()?;
        let canonical_resources_dir = resources_dir.canonicalize()?;

        let context = InstallContext::from_exe_with_codex_home(
            /*is_macos*/ false,
            /*current_exe*/ Some(&exe_path),
            /*managed_by_npm*/ false,
            /*managed_by_bun*/ false,
            /*codex_home*/ Some(codex_home.path()),
        );
        assert_eq!(
            context,
            InstallContext::Standalone {
                release_dir: canonical_release_dir,
                resources_dir: Some(canonical_resources_dir),
                package_layout: None,
                platform: standalone_platform(),
            }
        );
        Ok(())
    }

    #[test]
    fn standalone_rg_falls_back_when_resources_are_missing() -> std::io::Result<()> {
        let codex_home = tempfile::tempdir()?;
        let release_dir = codex_home
            .path()
            .join("packages/standalone/releases/1.2.3-x86_64-unknown-linux-musl");
        fs::create_dir_all(&release_dir)?;
        let exe_path = release_dir.join(if cfg!(windows) { "codex.exe" } else { "codex" });
        fs::write(&exe_path, "")?;

        let context = InstallContext::from_exe_with_codex_home(
            /*is_macos*/ false,
            /*current_exe*/ Some(&exe_path),
            /*managed_by_npm*/ false,
            /*managed_by_bun*/ false,
            /*codex_home*/ Some(codex_home.path()),
        );
        assert_eq!(context.rg_command(), default_rg_command());
        Ok(())
    }

    #[test]
    fn detects_standalone_package_layout() -> std::io::Result<()> {
        let package_dir = tempfile::tempdir()?;
        let bin_dir = package_dir.path().join(BIN_DIRNAME);
        let resources_dir = package_dir.path().join(RESOURCES_DIRNAME);
        let path_dir = package_dir.path().join(PATH_DIRNAME);
        fs::create_dir_all(&bin_dir)?;
        fs::create_dir_all(&resources_dir)?;
        fs::create_dir_all(&path_dir)?;
        fs::write(package_dir.path().join(PACKAGE_METADATA_FILENAME), "{}")?;
        let exe_path = bin_dir.join(if cfg!(windows) { "codex.exe" } else { "codex" });
        fs::write(&exe_path, "")?;
        fs::write(path_dir.join(default_rg_command()), "")?;
        let canonical_package_dir = package_dir.path().canonicalize()?;
        let canonical_bin_dir = bin_dir.canonicalize()?;
        let canonical_resources_dir = resources_dir.canonicalize()?;
        let canonical_path_dir = path_dir.canonicalize()?;
        let package_layout = CodexPackageLayout {
            package_dir: canonical_package_dir.clone(),
            bin_dir: canonical_bin_dir,
            resources_dir: Some(canonical_resources_dir.clone()),
            path_dir: Some(canonical_path_dir.clone()),
        };

        let context = InstallContext::from_exe_with_codex_home(
            /*is_macos*/ false,
            /*current_exe*/ Some(&exe_path),
            /*managed_by_npm*/ false,
            /*managed_by_bun*/ false,
            /*codex_home*/ None,
        );
        assert_eq!(
            context,
            InstallContext::Standalone {
                release_dir: canonical_package_dir,
                resources_dir: Some(canonical_resources_dir),
                package_layout: Some(package_layout),
                platform: standalone_platform(),
            }
        );
        assert_eq!(
            context.rg_command(),
            canonical_path_dir.join(default_rg_command())
        );
        Ok(())
    }

    #[test]
    fn standalone_package_rg_falls_back_when_codex_path_is_missing() -> std::io::Result<()> {
        let package_dir = tempfile::tempdir()?;
        let bin_dir = package_dir.path().join(BIN_DIRNAME);
        fs::create_dir_all(&bin_dir)?;
        fs::write(package_dir.path().join(PACKAGE_METADATA_FILENAME), "{}")?;
        let exe_path = bin_dir.join(if cfg!(windows) { "codex.exe" } else { "codex" });
        fs::write(&exe_path, "")?;

        let context = InstallContext::from_exe_with_codex_home(
            /*is_macos*/ false,
            /*current_exe*/ Some(&exe_path),
            /*managed_by_npm*/ false,
            /*managed_by_bun*/ false,
            /*codex_home*/ None,
        );
        assert_eq!(context.rg_command(), default_rg_command());
        Ok(())
    }

    #[test]
    fn npm_and_bun_take_precedence() {
        let npm_context = InstallContext::from_exe_with_codex_home(
            /*is_macos*/ false,
            /*current_exe*/ Some(Path::new("/tmp/codex")),
            /*managed_by_npm*/ true,
            /*managed_by_bun*/ false,
            /*codex_home*/ None,
        );
        assert_eq!(npm_context, InstallContext::Npm);

        let bun_context = InstallContext::from_exe_with_codex_home(
            /*is_macos*/ false,
            /*current_exe*/ Some(Path::new("/tmp/codex")),
            /*managed_by_npm*/ false,
            /*managed_by_bun*/ true,
            /*codex_home*/ None,
        );
        assert_eq!(bun_context, InstallContext::Bun);
    }

    #[test]
    fn brew_is_detected_on_macos_prefixes() {
        let context = InstallContext::from_exe_with_codex_home(
            /*is_macos*/ true,
            /*current_exe*/ Some(Path::new("/opt/homebrew/bin/codex")),
            /*managed_by_npm*/ false,
            /*managed_by_bun*/ false,
            /*codex_home*/ None,
        );
        assert_eq!(context, InstallContext::Brew);
    }
}
