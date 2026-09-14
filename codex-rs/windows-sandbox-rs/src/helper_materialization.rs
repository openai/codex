//! Selects sandbox helper paths; legacy file copying lives in the copy module.

mod copy;
use copy::CopyOutcome;
use copy::copy_from_source_if_needed;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;

use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::time::UNIX_EPOCH;

use crate::logging::log_note;
use crate::sandbox_bin_dir;

const DEV_BUILD_VERSION_SENTINEL: &str = "0.0.0";
const COMMAND_RUNNER_EXE: &str = "codex-command-runner.exe";
pub(crate) const BIN_DIRNAME: &str = "bin";
pub(crate) const RESOURCES_DIRNAME: &str = "codex-resources";

static HELPER_PATH_CACHE: OnceLock<Mutex<HashMap<String, PathBuf>>> = OnceLock::new();

pub(crate) fn helper_bin_dir(codex_home: &Path) -> PathBuf {
    sandbox_bin_dir(codex_home)
}

pub(crate) fn legacy_lookup() -> PathBuf {
    if let Ok(exe) = std::env::current_exe()
        && let Some(candidate) = bundled_executable_path_for_exe(&exe, COMMAND_RUNNER_EXE)
    {
        return candidate;
    }
    PathBuf::from(COMMAND_RUNNER_EXE)
}

pub(crate) fn resolve_command_runner(codex_home: &Path, log_dir: Option<&Path>) -> PathBuf {
    match copy_runner_if_needed(codex_home, log_dir) {
        Ok(path) => {
            log_note(
                &format!(
                    "helper launch resolution: using copied command-runner path {}",
                    path.display()
                ),
                log_dir,
            );
            path
        }
        Err(err) => {
            let fallback = legacy_lookup();
            log_note(
                &format!(
                    "helper copy failed for command-runner: {err:#}; falling back to legacy path {}",
                    fallback.display()
                ),
                log_dir,
            );
            fallback
        }
    }
}

pub fn resolve_current_exe_for_launch(codex_home: &Path, fallback_executable: &str) -> PathBuf {
    let source = match std::env::current_exe() {
        Ok(path) => path,
        Err(_) => return PathBuf::from(fallback_executable),
    };
    resolve_exe_for_launch(&source, codex_home)
}

pub fn resolve_exe_for_launch(source: &Path, codex_home: &Path) -> PathBuf {
    let Some(file_name) = source.file_name() else {
        return source.to_path_buf();
    };
    let destination = helper_bin_dir(codex_home).join(file_name);
    match copy_from_source_if_needed(source, &destination) {
        Ok(_) => destination,
        Err(err) => {
            let sandbox_log_dir = crate::sandbox_dir(codex_home);
            log_note(
                &format!(
                    "helper copy failed for executable: {err:#}; falling back to legacy path {}",
                    source.display()
                ),
                Some(&sandbox_log_dir),
            );
            source.to_path_buf()
        }
    }
}

fn copy_runner_if_needed(codex_home: &Path, log_dir: Option<&Path>) -> Result<PathBuf> {
    let cache_key = format!("{}|{}", COMMAND_RUNNER_EXE, codex_home.display());
    if let Some(path) = cached_helper_path(&cache_key) {
        log_note(
            &format!(
                "helper copy: using in-memory cache for command-runner -> {}",
                path.display()
            ),
            log_dir,
        );
        return Ok(path);
    }

    let source = sibling_source_path()?;
    let destination = helper_destination_for_source(codex_home, &source)?;
    log_note(
        &format!(
            "helper copy: validating command-runner source={} destination={}",
            source.display(),
            destination.display()
        ),
        log_dir,
    );
    let outcome = copy_from_source_if_needed(&source, &destination)?;
    let action = match outcome {
        CopyOutcome::Reused => "reused",
        CopyOutcome::ReCopied => "recopied",
    };
    log_note(
        &format!(
            "helper copy: {} command-runner source={} destination={}",
            action,
            source.display(),
            destination.display()
        ),
        log_dir,
    );
    store_helper_path(cache_key, destination.clone());
    Ok(destination)
}

fn cached_helper_path(cache_key: &str) -> Option<PathBuf> {
    let cache = HELPER_PATH_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let guard = cache.lock().ok()?;
    guard.get(cache_key).cloned()
}

fn store_helper_path(cache_key: String, path: PathBuf) {
    let cache = HELPER_PATH_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut guard) = cache.lock() {
        guard.insert(cache_key, path);
    }
}

fn sibling_source_path() -> Result<PathBuf> {
    let exe = std::env::current_exe().context("resolve current executable for helper lookup")?;
    bundled_executable_path_for_exe(&exe, COMMAND_RUNNER_EXE).ok_or_else(|| {
        anyhow!(
            "helper not found next to current executable or under {RESOURCES_DIRNAME}: {}",
            exe.display()
        )
    })
}

pub(crate) fn bundled_executable_path_for_exe(exe: &Path, file_name: &str) -> Option<PathBuf> {
    let find = |exe: &Path| {
        let dir = exe.parent()?;
        let direct_candidate = dir.join(file_name);
        if direct_candidate.is_file() {
            return Some(direct_candidate);
        }

        if dir.file_name() == Some(OsStr::new(BIN_DIRNAME))
            && let Some(package_dir) = dir.parent()
        {
            let package_resource_candidate = package_dir.join(RESOURCES_DIRNAME).join(file_name);
            if package_resource_candidate.is_file() {
                return Some(package_resource_candidate);
            }
        }

        let resource_candidate = dir.join(RESOURCES_DIRNAME).join(file_name);
        resource_candidate.is_file().then_some(resource_candidate)
    };

    // Installer bin directories can be junctions, so retry beside the real executable once.
    find(exe).or_else(|| find(&dunce::canonicalize(exe).ok()?))
}

fn helper_destination_for_source(codex_home: &Path, source: &Path) -> Result<PathBuf> {
    let suffix = helper_version_suffix(source)?;
    Ok(helper_bin_dir(codex_home).join(materialized_file_name(&suffix)))
}

fn materialized_file_name(suffix: &str) -> String {
    format!("codex-command-runner-{suffix}.exe")
}

fn helper_version_suffix(source: &Path) -> Result<String> {
    let version = env!("CARGO_PKG_VERSION");
    if version == DEV_BUILD_VERSION_SENTINEL {
        dev_build_suffix(source)
    } else {
        Ok(version.to_string())
    }
}

fn dev_build_suffix(source: &Path) -> Result<String> {
    let metadata = fs::metadata(source)
        .with_context(|| format!("read helper source metadata {}", source.display()))?;
    let modified = metadata
        .modified()
        .with_context(|| format!("read helper source mtime {}", source.display()))?;
    let duration = modified
        .duration_since(UNIX_EPOCH)
        .with_context(|| format!("convert helper source mtime {}", source.display()))?;
    Ok(format!("{}-{:x}", metadata.len(), duration.as_secs(),))
}

#[cfg(test)]
mod tests {
    use super::BIN_DIRNAME;
    use super::CopyOutcome;
    use super::DEV_BUILD_VERSION_SENTINEL;

    use super::RESOURCES_DIRNAME;
    use super::bundled_executable_path_for_exe;
    use super::copy_from_source_if_needed;

    use super::dev_build_suffix;
    use super::helper_bin_dir;
    use super::helper_version_suffix;
    use super::materialized_file_name;
    use pretty_assertions::assert_eq;
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn helper_bin_dir_is_under_sandbox_bin() {
        let codex_home = Path::new(r"C:\Users\example\.codex");

        assert_eq!(
            PathBuf::from(r"C:\Users\example\.codex\.sandbox-bin"),
            helper_bin_dir(codex_home)
        );
    }

    #[test]
    fn copy_runner_into_shared_bin_dir() {
        let tmp = TempDir::new().expect("tempdir");
        let codex_home = tmp.path().join("codex-home");
        let source_dir = tmp.path().join("sibling-source");
        fs::create_dir_all(&source_dir).expect("create source dir");
        let runner_source = source_dir.join("codex-command-runner.exe");
        fs::write(&runner_source, b"runner").expect("runner");
        let runner_suffix = helper_version_suffix(&runner_source).expect("runner suffix");
        let runner_destination =
            helper_bin_dir(&codex_home).join(materialized_file_name(&runner_suffix));

        let runner_outcome =
            copy_from_source_if_needed(&runner_source, &runner_destination).expect("runner copy");

        assert_eq!(CopyOutcome::ReCopied, runner_outcome);
        assert_eq!(
            b"runner".as_slice(),
            fs::read(&runner_destination).expect("read runner")
        );
    }

    #[test]
    fn helper_source_lookup_checks_resource_dir() {
        let tmp = TempDir::new().expect("tempdir");
        let release_dir = tmp.path().join("release");
        let resources_dir = release_dir.join(RESOURCES_DIRNAME);
        fs::create_dir_all(&resources_dir).expect("create resources dir");
        let exe = release_dir.join("codex.exe");
        let helper = resources_dir.join("codex-command-runner.exe");
        fs::write(&exe, b"codex").expect("write exe");
        fs::write(&helper, b"runner").expect("write helper");

        let resolved =
            bundled_executable_path_for_exe(&exe, /*file_name*/ "codex-command-runner.exe")
                .expect("helper path");

        assert_eq!(resolved, helper);
    }

    #[test]
    fn helper_source_lookup_checks_package_resource_dir_for_bin_exe() {
        let tmp = TempDir::new().expect("tempdir");
        let package_dir = tmp.path().join("package");
        let bin_dir = package_dir.join(BIN_DIRNAME);
        let resources_dir = package_dir.join(RESOURCES_DIRNAME);
        fs::create_dir_all(&bin_dir).expect("create bin dir");
        fs::create_dir_all(&resources_dir).expect("create resources dir");
        let exe = bin_dir.join("codex.exe");
        let helper = resources_dir.join("codex-command-runner.exe");
        fs::write(&exe, b"codex").expect("write exe");
        fs::write(&helper, b"runner").expect("write helper");

        let resolved =
            bundled_executable_path_for_exe(&exe, /*file_name*/ "codex-command-runner.exe")
                .expect("helper path");

        assert_eq!(resolved, helper);
    }

    #[test]
    fn helper_source_lookup_resolves_bin_junctions() {
        let tmp = TempDir::new().expect("tempdir");
        let package_dir = tmp.path().join("package");
        let bin_dir = package_dir.join(BIN_DIRNAME);
        let resources_dir = package_dir.join(RESOURCES_DIRNAME);
        let install_dir = tmp.path().join("install");
        fs::create_dir_all(&bin_dir).expect("create bin dir");
        fs::create_dir_all(&resources_dir).expect("create resources dir");
        fs::create_dir_all(&install_dir).expect("create install dir");
        fs::write(bin_dir.join("codex.exe"), b"codex").expect("write exe");
        let helper = resources_dir.join("codex-windows-sandbox-setup.exe");
        fs::write(&helper, b"setup").expect("write helper");

        let junction = install_dir.join(BIN_DIRNAME);
        let output = std::process::Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(&junction)
            .arg(&bin_dir)
            .output()
            .expect("create bin junction");
        assert!(output.status.success());

        assert_eq!(
            bundled_executable_path_for_exe(
                &junction.join("codex.exe"),
                /*file_name*/ "codex-windows-sandbox-setup.exe"
            ),
            Some(dunce::canonicalize(&helper).expect("canonical helper"))
        );
    }

    #[test]
    fn helper_source_lookup_prefers_package_resource_dir_over_bin_resource_dir() {
        let tmp = TempDir::new().expect("tempdir");
        let package_dir = tmp.path().join("package");
        let bin_dir = package_dir.join(BIN_DIRNAME);
        let package_resources_dir = package_dir.join(RESOURCES_DIRNAME);
        let bin_resources_dir = bin_dir.join(RESOURCES_DIRNAME);
        fs::create_dir_all(&package_resources_dir).expect("create package resources dir");
        fs::create_dir_all(&bin_resources_dir).expect("create bin resources dir");
        let exe = bin_dir.join("codex.exe");
        let package_helper = package_resources_dir.join("codex-command-runner.exe");
        let bin_helper = bin_resources_dir.join("codex-command-runner.exe");
        fs::write(&exe, b"codex").expect("write exe");
        fs::write(&package_helper, b"package runner").expect("write package helper");
        fs::write(&bin_helper, b"bin runner").expect("write bin helper");

        let resolved =
            bundled_executable_path_for_exe(&exe, /*file_name*/ "codex-command-runner.exe")
                .expect("helper path");

        assert_eq!(resolved, package_helper);
    }

    #[test]
    fn helper_source_lookup_prefers_direct_sibling_over_resource_dir() {
        let tmp = TempDir::new().expect("tempdir");
        let release_dir = tmp.path().join("release");
        let resources_dir = release_dir.join(RESOURCES_DIRNAME);
        fs::create_dir_all(&resources_dir).expect("create resources dir");
        let exe = release_dir.join("codex.exe");
        let sibling_helper = release_dir.join("codex-command-runner.exe");
        let resource_helper = resources_dir.join("codex-command-runner.exe");
        fs::write(&exe, b"codex").expect("write exe");
        fs::write(&sibling_helper, b"sibling runner").expect("write sibling helper");
        fs::write(&resource_helper, b"resource runner").expect("write resource helper");

        let resolved =
            bundled_executable_path_for_exe(&exe, /*file_name*/ "codex-command-runner.exe")
                .expect("helper path");

        assert_eq!(resolved, sibling_helper);
    }

    #[test]
    fn helper_version_suffix_uses_cli_version_or_dev_build_metadata() {
        let tmp = TempDir::new().expect("tempdir");
        let source = tmp.path().join("source.exe");
        fs::write(&source, b"runner-v1").expect("write source");
        let suffix = helper_version_suffix(&source).expect("suffix");

        if env!("CARGO_PKG_VERSION") == DEV_BUILD_VERSION_SENTINEL {
            assert_eq!(suffix, dev_build_suffix(&source).expect("dev build suffix"));
        } else {
            assert_eq!(suffix, env!("CARGO_PKG_VERSION"));
        }
    }

    #[test]
    fn materialized_file_name_adds_suffix_before_extension() {
        let file_name = materialized_file_name("test-suffix");

        assert_eq!(file_name, "codex-command-runner-test-suffix.exe");
    }
}
