use crate::Installer;
use crate::errors::Error;
use async_trait::async_trait;
use semver::Version;
use serde::Deserialize;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

const CODENAME: &str = "codex";

#[derive(Clone, Debug)]
pub(crate) struct BrewInstaller {
    path: PathBuf,
}

impl BrewInstaller {
    pub(crate) fn detect() -> Result<Option<Self>, Error> {
        let path = match which::which("brew") {
            Ok(path) => path,
            Err(which::Error::CannotFindBinaryPath) => return Err(Error::BrewMissing),
            Err(err) => return Err(Error::Io(err.to_string())),
        };

        let installer = Self { path };
        match installer.status() {
            Ok(_) => Ok(Some(installer)),
            Err(Error::Unsupported) => Ok(None),
            Err(err) => Err(err),
        }
    }

    fn status(&self) -> Result<InstallStatus, Error> {
        if let Some(info) = self.formula_info()? {
            let current_version = self.formula_current_version()?;
            return Ok(InstallStatus {
                method: InstallMethod::Formula,
                current_version,
                latest_version: info.latest_version,
            });
        }
        if let Some(info) = self.cask_info()? {
            let current_version = self.cask_current_version()?;
            return Ok(InstallStatus {
                method: InstallMethod::Cask,
                current_version,
                latest_version: info.latest_version,
            });
        }
        Err(Error::Unsupported)
    }

    async fn upgrade(&self, method: InstallMethod) -> Result<(), Error> {
        let args: &[&str] = match method {
            InstallMethod::Formula => &["upgrade", CODENAME],
            InstallMethod::Cask => &["upgrade", "--cask", CODENAME],
        };
        run_command_async(&self.path, args, Some(("HOMEBREW_NO_AUTO_UPDATE", "1"))).await?;
        Ok(())
    }

    fn current_version(&self, method: InstallMethod) -> Result<String, Error> {
        match method {
            InstallMethod::Formula => self.formula_current_version(),
            InstallMethod::Cask => self.cask_current_version(),
        }
    }

    fn formula_info(&self) -> Result<Option<BrewFormulaInfo>, Error> {
        let output = match run_command_sync(&self.path, &["info", "--json=v2", CODENAME]) {
            Ok(output) => output,
            Err(Error::Command { .. }) => return Ok(None),
            Err(err) => return Err(err),
        };
        let parsed: BrewFormulaInfoResponse =
            serde_json::from_str(&output.stdout).map_err(|err| Error::Json(err.to_string()))?;
        let formula = match parsed.formulae.into_iter().next() {
            Some(value) => value,
            None => return Ok(None),
        };
        if formula.installed.is_empty() {
            return Ok(None);
        }
        let latest_version = formula
            .versions
            .stable
            .ok_or_else(|| Error::Version("missing stable formula version".into()))?;
        Ok(Some(BrewFormulaInfo { latest_version }))
    }

    fn cask_info(&self) -> Result<Option<BrewCaskInfo>, Error> {
        let output = match run_command_sync(&self.path, &["info", "--cask", "--json=v2", CODENAME])
        {
            Ok(output) => output,
            Err(Error::Command { .. }) => return Ok(None),
            Err(err) => return Err(err),
        };
        let parsed: BrewCaskInfoResponse =
            serde_json::from_str(&output.stdout).map_err(|err| Error::Json(err.to_string()))?;
        let cask = match parsed.casks.into_iter().next() {
            Some(value) => value,
            None => return Ok(None),
        };
        if cask.installed.is_empty() {
            return Ok(None);
        }
        let latest_version = cask
            .version
            .ok_or_else(|| Error::Version("missing cask version".into()))?;
        Ok(Some(BrewCaskInfo { latest_version }))
    }

    fn formula_current_version(&self) -> Result<String, Error> {
        self.parse_current_version(&["list", "--formula", "--versions", CODENAME])
    }

    fn cask_current_version(&self) -> Result<String, Error> {
        self.parse_current_version(&["list", "--cask", "--versions", CODENAME])
    }

    fn parse_current_version(&self, args: &[&str]) -> Result<String, Error> {
        let output = run_command_sync(&self.path, args)?;
        parse_brew_list_version(&output.stdout)
    }
}

#[async_trait]
impl Installer for BrewInstaller {
    fn update_available(&self) -> Result<bool, Error> {
        let status = self.status()?;
        status.needs_update()
    }

    async fn update(&self) -> Result<String, Error> {
        let initial_status = run_blocking({
            let brew = self.clone();
            move || brew.status()
        })
        .await?;

        if !initial_status.needs_update()? {
            return Ok(initial_status.current_version);
        }

        self.upgrade(initial_status.method).await?;

        run_blocking({
            let brew = self.clone();
            move || brew.current_version(initial_status.method)
        })
        .await
    }
}

#[derive(Clone, Copy, Debug)]
enum InstallMethod {
    Formula,
    Cask,
}

#[derive(Debug)]
struct InstallStatus {
    method: InstallMethod,
    current_version: String,
    latest_version: String,
}

impl InstallStatus {
    fn needs_update(&self) -> Result<bool, Error> {
        compare_versions(&self.current_version, &self.latest_version)
    }
}

#[derive(Debug, Deserialize)]
struct BrewFormulaInfoResponse {
    formulae: Vec<BrewFormulaEntry>,
}

#[derive(Debug, Deserialize)]
struct BrewFormulaEntry {
    installed: Vec<serde::de::IgnoredAny>,
    versions: BrewFormulaVersions,
}

#[derive(Debug, Deserialize)]
struct BrewFormulaVersions {
    stable: Option<String>,
}

#[derive(Debug)]
struct BrewFormulaInfo {
    latest_version: String,
}

#[derive(Debug, Deserialize)]
struct BrewCaskInfoResponse {
    casks: Vec<BrewCaskEntry>,
}

#[derive(Debug, Deserialize)]
struct BrewCaskEntry {
    installed: Vec<serde::de::IgnoredAny>,
    version: Option<String>,
}

#[derive(Debug)]
struct BrewCaskInfo {
    latest_version: String,
}

struct CommandOutput {
    stdout: String,
}

fn run_command_sync(path: &Path, args: &[&str]) -> Result<CommandOutput, Error> {
    let output = Command::new(path)
        .args(args)
        .output()
        .map_err(|err| Error::Io(err.to_string()))?;
    handle_command_output(path, args, output)
}

async fn run_command_async(
    path: &Path,
    args: &[&str],
    env: Option<(&str, &str)>,
) -> Result<CommandOutput, Error> {
    let mut command = tokio::process::Command::new(path);
    command.args(args);
    if let Some((key, value)) = env {
        command.env(key, value);
    }
    let output = command
        .output()
        .await
        .map_err(|err| Error::Io(err.to_string()))?;
    handle_command_output(path, args, output)
}

fn handle_command_output(
    path: &Path,
    args: &[&str],
    output: std::process::Output,
) -> Result<CommandOutput, Error> {
    if output.status.success() {
        Ok(CommandOutput {
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        })
    } else {
        Err(Error::Command {
            command: format_command(path, args),
            status: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

fn format_command(path: &Path, args: &[&str]) -> String {
    let mut display = path.display().to_string();
    for arg in args {
        display.push(' ');
        display.push_str(arg);
    }
    display
}

fn parse_brew_list_version(stdout: &str) -> Result<String, Error> {
    let line = stdout
        .lines()
        .find(|candidate| !candidate.trim().is_empty())
        .ok_or_else(|| Error::Version("missing version output".into()))?;
    let mut parts = line.split_whitespace();
    let name = parts
        .next()
        .ok_or_else(|| Error::Version("unexpected brew list output".into()))?;
    if name != CODENAME {
        return Err(Error::Version(
            "brew list returned unexpected formula".into(),
        ));
    }
    let version = parts
        .last()
        .ok_or_else(|| Error::Version("brew list did not include version".into()))?;
    Ok(version.to_string())
}

fn compare_versions(current: &str, latest: &str) -> Result<bool, Error> {
    match (Version::parse(current), Version::parse(latest)) {
        (Ok(current_semver), Ok(latest_semver)) => Ok(latest_semver > current_semver),
        (Err(_), Err(_)) | (Ok(_), Err(_)) | (Err(_), Ok(_)) => {
            if let Some(result) = compare_brew_versions(current, latest) {
                return Ok(result);
            }
            Ok(latest > current)
        }
    }
}

fn compare_brew_versions(current: &str, latest: &str) -> Option<bool> {
    let (current_semver, current_revision) = parse_brew_version(current)?;
    let (latest_semver, latest_revision) = parse_brew_version(latest)?;
    if current_semver != latest_semver {
        return Some(latest_semver > current_semver);
    }
    Some(latest_revision > current_revision)
}

fn parse_brew_version(version: &str) -> Option<(Version, i64)> {
    let (core, revision) = version
        .split_once('_')
        .map_or((version, "0"), |(base, revision)| (base, revision));
    let semver = normalize_semver(core)?;
    let revision_value = if revision.is_empty() {
        0
    } else {
        revision.parse::<i64>().ok()?
    };
    Some((semver, revision_value))
}

fn normalize_semver(version: &str) -> Option<Version> {
    if let Ok(parsed) = Version::parse(version) {
        return Some(parsed);
    }

    let (without_build, build) = version
        .split_once('+')
        .map_or((version, None), |(core, build)| (core, Some(build)));
    let (numeric, suffix) = without_build
        .split_once('-')
        .map_or((without_build, None), |(core, suffix)| (core, Some(suffix)));

    if numeric.is_empty() {
        return None;
    }

    let mut components: Vec<&str> = numeric.split('.').collect();
    if components.is_empty() || components.iter().any(|component| component.is_empty()) {
        return None;
    }
    if components.len() > 3 {
        return None;
    }
    while components.len() < 3 {
        components.push("0");
    }

    let mut normalized = components.join(".");
    if let Some(suffix) = suffix {
        if suffix.is_empty() {
            return None;
        }
        normalized.push('-');
        normalized.push_str(suffix);
    }
    if let Some(build) = build {
        if build.is_empty() {
            return None;
        }
        normalized.push('+');
        normalized.push_str(build);
    }

    Version::parse(&normalized).ok()
}

async fn run_blocking<F, T>(func: F) -> Result<T, Error>
where
    F: FnOnce() -> Result<T, Error> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(func)
        .await
        .map_err(|err| Error::Io(err.to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compare_versions_prefers_semver() -> Result<(), Error> {
        pretty_assertions::assert_eq!(compare_versions("0.8.0", "0.9.0")?, true);
        pretty_assertions::assert_eq!(compare_versions("0.9.0", "0.9.0")?, false);
        pretty_assertions::assert_eq!(compare_versions("0.10.0", "0.9.0")?, false);
        Ok(())
    }

    #[test]
    fn compare_versions_falls_back_to_string_compare() -> Result<(), Error> {
        pretty_assertions::assert_eq!(compare_versions("0.9.0_1", "0.9.1")?, true);
        pretty_assertions::assert_eq!(compare_versions("1.0-nightly", "1.0-nightly")?, false);
        Ok(())
    }

    #[test]
    fn compare_versions_handles_brew_revision_suffix() -> Result<(), Error> {
        pretty_assertions::assert_eq!(compare_versions("0.9.0_1", "0.10.0_1")?, true);
        pretty_assertions::assert_eq!(compare_versions("0.10.0_1", "0.10.0_2")?, true);
        pretty_assertions::assert_eq!(compare_versions("0.10.0_2", "0.10.0_1")?, false);
        pretty_assertions::assert_eq!(compare_versions("0.10.0", "0.10.0_1")?, true);
        Ok(())
    }

    #[cfg(unix)]
    mod unix {
        use super::*;
        use crate::update;
        use crate::update_available;
        use serde_json::json;
        use std::env;
        use std::error::Error as StdError;
        use std::ffi::OsStr;
        use std::ffi::OsString;
        use std::fs;
        use std::os::unix::fs::PermissionsExt;
        use std::path::Path;
        use std::path::PathBuf;
        use std::sync::Mutex;
        use std::sync::OnceLock;
        use tempfile::TempDir;

        const BREW_SCRIPT: &str = r#"#!/bin/sh
set -eu

command="$1"
case "$command" in
  info)
    if [ "${2:-}" = "--cask" ]; then
      cat "$BREW_CASK_INFO"
    else
      cat "$BREW_FORMULA_INFO"
    fi
    ;;
  list)
    if [ "${2:-}" = "--cask" ]; then
      cat "$BREW_CASK_LIST"
    else
      cat "$BREW_FORMULA_LIST"
    fi
    ;;
  upgrade)
    if [ "${HOMEBREW_NO_AUTO_UPDATE:-}" != "1" ]; then
      echo "missing HOMEBREW_NO_AUTO_UPDATE" >&2
      exit 7
    fi
    if [ "${2:-}" = "--cask" ]; then
      printf '%s\n' 'upgrade --cask codex' >> "$BREW_UPGRADE_LOG"
      cp "$BREW_CASK_UPDATED_LIST" "$BREW_CASK_LIST"
    else
      printf '%s\n' 'upgrade codex' >> "$BREW_UPGRADE_LOG"
      cp "$BREW_FORMULA_UPDATED_LIST" "$BREW_FORMULA_LIST"
    fi
    ;;
  *)
    echo "unsupported command: $command" >&2
    exit 8
    ;;
esac
"#;

        #[tokio::test]
        async fn update_available_reports_formula_upgrade() -> Result<(), Box<dyn StdError>> {
            let fake_brew = FakeBrew::formula("0.8.0", "0.9.0", "0.9.0")?;

            let available = update_available()?;

            pretty_assertions::assert_eq!(available, true);
            drop(fake_brew);
            Ok(())
        }

        #[tokio::test]
        async fn update_available_reports_formula_up_to_date() -> Result<(), Box<dyn StdError>> {
            let fake_brew = FakeBrew::formula("0.9.0", "0.9.0", "0.9.0")?;

            let available = update_available()?;

            pretty_assertions::assert_eq!(available, false);
            drop(fake_brew);
            Ok(())
        }

        #[tokio::test]
        async fn update_executes_formula_upgrade() -> Result<(), Box<dyn StdError>> {
            let fake_brew = FakeBrew::formula("0.8.0", "0.9.0", "0.9.0")?;

            let version = update().await?;

            pretty_assertions::assert_eq!(version, "0.9.0".to_string());
            pretty_assertions::assert_eq!(
                fake_brew.upgrade_log_contents()?,
                "upgrade codex\n".to_string()
            );
            pretty_assertions::assert_eq!(
                fake_brew.current_list_contents(InstallMethod::Formula)?,
                "codex 0.9.0\n".to_string()
            );
            drop(fake_brew);
            Ok(())
        }

        #[tokio::test]
        async fn update_skips_formula_when_up_to_date() -> Result<(), Box<dyn StdError>> {
            let fake_brew = FakeBrew::formula("0.9.0", "0.9.0", "0.9.0")?;

            let version = update().await?;

            pretty_assertions::assert_eq!(version, "0.9.0".to_string());
            pretty_assertions::assert_eq!(fake_brew.upgrade_log_contents()?, String::new());
            drop(fake_brew);
            Ok(())
        }

        #[tokio::test]
        async fn update_available_reports_cask_upgrade() -> Result<(), Box<dyn StdError>> {
            let fake_brew = FakeBrew::cask("0.8.0", "0.9.0", "0.9.0")?;

            let available = update_available()?;

            pretty_assertions::assert_eq!(available, true);
            drop(fake_brew);
            Ok(())
        }

        #[tokio::test]
        async fn update_executes_cask_upgrade() -> Result<(), Box<dyn StdError>> {
            let fake_brew = FakeBrew::cask("0.8.0", "0.9.0", "0.9.0")?;

            let version = update().await?;

            pretty_assertions::assert_eq!(version, "0.9.0".to_string());
            pretty_assertions::assert_eq!(
                fake_brew.upgrade_log_contents()?,
                "upgrade --cask codex\n".to_string()
            );
            pretty_assertions::assert_eq!(
                fake_brew.current_list_contents(InstallMethod::Cask)?,
                "codex 0.9.0\n".to_string()
            );
            drop(fake_brew);
            Ok(())
        }

        #[tokio::test]
        async fn detects_cask_when_formula_entry_missing() -> Result<(), Box<dyn StdError>> {
            let fake_brew = FakeBrew::cask_without_formula_entry("0.8.0", "0.9.0", "0.9.0")?;

            let available = update_available()?;
            pretty_assertions::assert_eq!(available, true);

            let version = update().await?;
            pretty_assertions::assert_eq!(version, "0.9.0".to_string());

            drop(fake_brew);
            Ok(())
        }

        struct FakeBrew {
            _tempdir: TempDir,
            _env: EnvironmentGuard,
            _vars: Vec<VarGuard>,
            upgrade_log: PathBuf,
            formula_list: PathBuf,
            cask_list: PathBuf,
        }

        impl FakeBrew {
            fn formula(
                current: &str,
                latest: &str,
                updated: &str,
            ) -> Result<Self, Box<dyn StdError>> {
                Self::new(
                    build_formula_info(&[current], latest),
                    build_cask_info(&[], latest),
                    format!("codex {current}\n"),
                    "codex 0.0.0\n".to_string(),
                    format!("codex {updated}\n"),
                    "codex 0.0.0\n".to_string(),
                )
            }

            fn cask(current: &str, latest: &str, updated: &str) -> Result<Self, Box<dyn StdError>> {
                Self::new(
                    build_formula_info(&[], latest),
                    build_cask_info(&[current], latest),
                    "codex 0.0.0\n".to_string(),
                    format!("codex {current}\n"),
                    "codex 0.0.0\n".to_string(),
                    format!("codex {updated}\n"),
                )
            }

            fn cask_without_formula_entry(
                current: &str,
                latest: &str,
                updated: &str,
            ) -> Result<Self, Box<dyn StdError>> {
                Self::new(
                    build_empty_formula_info(),
                    build_cask_info(&[current], latest),
                    "codex 0.0.0\n".to_string(),
                    format!("codex {current}\n"),
                    "codex 0.0.0\n".to_string(),
                    format!("codex {updated}\n"),
                )
            }

            fn new(
                formula_info: serde_json::Value,
                cask_info: serde_json::Value,
                formula_list: String,
                cask_list: String,
                formula_updated_list: String,
                cask_updated_list: String,
            ) -> Result<Self, Box<dyn StdError>> {
                let tempdir = TempDir::new()?;
                let brew_path = tempdir.path().join("brew");
                fs::write(&brew_path, BREW_SCRIPT)?;
                let mut permissions = fs::metadata(&brew_path)?.permissions();
                permissions.set_mode(0o755);
                fs::set_permissions(&brew_path, permissions)?;

                let formula_info_string = serde_json::to_string(&formula_info)?;
                let formula_info_path = tempdir.path().join("formula_info.json");
                fs::write(&formula_info_path, formula_info_string.as_bytes())?;
                let cask_info_string = serde_json::to_string(&cask_info)?;
                let cask_info_path = tempdir.path().join("cask_info.json");
                fs::write(&cask_info_path, cask_info_string.as_bytes())?;

                let formula_list_path = tempdir.path().join("formula_list.txt");
                fs::write(&formula_list_path, formula_list.as_bytes())?;
                let cask_list_path = tempdir.path().join("cask_list.txt");
                fs::write(&cask_list_path, cask_list.as_bytes())?;

                let formula_updated_path = tempdir.path().join("formula_list_updated.txt");
                fs::write(&formula_updated_path, formula_updated_list.as_bytes())?;
                let cask_updated_path = tempdir.path().join("cask_list_updated.txt");
                fs::write(&cask_updated_path, cask_updated_list.as_bytes())?;

                let upgrade_log = tempdir.path().join("upgrade.log");
                fs::write(&upgrade_log, Vec::new())?;

                let env = EnvironmentGuard::new(tempdir.path());
                let mut vars = Vec::new();
                vars.push(VarGuard::new("BREW_FORMULA_INFO", &formula_info_path));
                vars.push(VarGuard::new("BREW_CASK_INFO", &cask_info_path));
                vars.push(VarGuard::new("BREW_FORMULA_LIST", &formula_list_path));
                vars.push(VarGuard::new("BREW_CASK_LIST", &cask_list_path));
                vars.push(VarGuard::new(
                    "BREW_FORMULA_UPDATED_LIST",
                    &formula_updated_path,
                ));
                vars.push(VarGuard::new("BREW_CASK_UPDATED_LIST", &cask_updated_path));
                vars.push(VarGuard::new("BREW_UPGRADE_LOG", &upgrade_log));

                Ok(Self {
                    _tempdir: tempdir,
                    _env: env,
                    _vars: vars,
                    upgrade_log,
                    formula_list: formula_list_path,
                    cask_list: cask_list_path,
                })
            }

            fn upgrade_log_contents(&self) -> Result<String, Box<dyn StdError>> {
                Ok(fs::read_to_string(&self.upgrade_log)?)
            }

            fn current_list_contents(
                &self,
                method: InstallMethod,
            ) -> Result<String, Box<dyn StdError>> {
                let path = match method {
                    InstallMethod::Formula => &self.formula_list,
                    InstallMethod::Cask => &self.cask_list,
                };
                Ok(fs::read_to_string(path)?)
            }
        }

        fn build_formula_info(installed: &[&str], latest: &str) -> serde_json::Value {
            json!({
                "formulae": [{
                    "installed": installed
                        .iter()
                        .map(|version| json!({"version": version}))
                        .collect::<Vec<_>>(),
                    "versions": {"stable": latest}
                }]
            })
        }

        fn build_cask_info(installed: &[&str], latest: &str) -> serde_json::Value {
            json!({
                "casks": [{
                    "installed": installed
                        .iter()
                        .map(|version| json!({"version": version}))
                        .collect::<Vec<_>>(),
                    "version": latest
                }]
            })
        }

        fn build_empty_formula_info() -> serde_json::Value {
            json!({
                "formulae": []
            })
        }

        struct EnvironmentGuard {
            _lock: std::sync::MutexGuard<'static, ()>,
            _path_guard: PathGuard,
        }

        impl EnvironmentGuard {
            fn new(new_dir: &Path) -> Self {
                let lock = acquire_environment_lock();
                let path_guard = PathGuard::set(new_dir);
                Self {
                    _lock: lock,
                    _path_guard: path_guard,
                }
            }
        }

        struct PathGuard {
            original: Option<OsString>,
        }

        impl PathGuard {
            fn set(new_dir: &Path) -> Self {
                let original = env::var_os("PATH");
                let mut joined = OsString::new();
                joined.push(new_dir.as_os_str());
                if let Some(current) = original.as_ref() {
                    joined.push(OsStr::new(":"));
                    joined.push(current);
                }
                // SAFETY: environment access is guarded by acquire_environment_lock().
                unsafe { env::set_var("PATH", &joined) };
                Self { original }
            }
        }

        impl Drop for PathGuard {
            fn drop(&mut self) {
                match &self.original {
                    Some(value) => unsafe { env::set_var("PATH", value) },
                    None => unsafe { env::remove_var("PATH") },
                }
            }
        }

        struct VarGuard {
            key: &'static str,
            original: Option<OsString>,
        }

        impl VarGuard {
            fn new(key: &'static str, value: &Path) -> Self {
                let original = env::var_os(key);
                // SAFETY: environment access is guarded by acquire_environment_lock().
                unsafe { env::set_var(key, value) };
                Self { key, original }
            }
        }

        impl Drop for VarGuard {
            fn drop(&mut self) {
                match &self.original {
                    Some(value) => unsafe { env::set_var(self.key, value) },
                    None => unsafe { env::remove_var(self.key) },
                }
            }
        }

        fn acquire_environment_lock() -> std::sync::MutexGuard<'static, ()> {
            static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
            match LOCK.get_or_init(|| Mutex::new(())).lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            }
        }
    }
}
