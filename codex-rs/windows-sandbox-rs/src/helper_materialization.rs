use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::OnceLock;
use tempfile::NamedTempFile;

use crate::logging::log_note;
use crate::sandbox_bin_dir;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum HelperExecutable {
    CommandRunner,
    Setup,
}

impl HelperExecutable {
    fn file_name(self) -> &'static str {
        match self {
            Self::CommandRunner => "codex-command-runner.exe",
            Self::Setup => "codex-windows-sandbox-setup.exe",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::CommandRunner => "command-runner",
            Self::Setup => "setup-helper",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CopyOutcome {
    Reused,
    ReCopied,
}

static HELPER_PATH_CACHE: OnceLock<Mutex<HashMap<String, PathBuf>>> = OnceLock::new();

pub(crate) fn helper_bin_dir(codex_home: &Path) -> PathBuf {
    sandbox_bin_dir(codex_home)
}

pub(crate) fn legacy_lookup(kind: HelperExecutable) -> PathBuf {
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        let candidate = dir.join(kind.file_name());
        if candidate.exists() && !is_windowsapps_path(&candidate) {
            return candidate;
        }
    }
    PathBuf::from(kind.file_name())
}

pub(crate) fn resolve_helper_for_launch(
    kind: HelperExecutable,
    codex_home: &Path,
    log_dir: Option<&Path>,
) -> PathBuf {
    match copy_helper_if_needed(kind, codex_home, log_dir) {
        Ok(path) => {
            log_note(
                &format!(
                    "helper launch resolution: using copied {} path {}",
                    kind.label(),
                    path.display()
                ),
                log_dir,
            );
            path
        }
        Err(err) => {
            let fallback = legacy_lookup(kind);
            log_note(
                &format!(
                    "helper copy failed for {}: {err:#}; falling back to legacy path {}",
                    kind.label(),
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
    let Some(file_name) = source.file_name() else {
        return source;
    };
    let destination = helper_bin_dir(codex_home).join(file_name);
    match copy_from_source_if_needed(&source, &destination) {
        Ok(_) => destination,
        Err(err) => {
            let sandbox_log_dir = crate::sandbox_dir(codex_home);
            log_note(
                &format!(
                    "helper copy failed for current executable: {err:#}; falling back to legacy path {}",
                    source.display()
                ),
                Some(&sandbox_log_dir),
            );
            source
        }
    }
}

pub(crate) fn copy_helper_if_needed(
    kind: HelperExecutable,
    codex_home: &Path,
    log_dir: Option<&Path>,
) -> Result<PathBuf> {
    let cache_key = format!("{}|{}", kind.file_name(), codex_home.display());
    if let Some(path) = cached_helper_path(&cache_key) {
        log_note(
            &format!(
                "helper copy: using in-memory cache for {} -> {}",
                kind.label(),
                path.display()
            ),
            log_dir,
        );
        return Ok(path);
    }

    let source = sibling_source_path(kind)?;
    let destination = helper_bin_dir(codex_home).join(kind.file_name());
    log_note(
        &format!(
            "helper copy: validating {} source={} destination={}",
            kind.label(),
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
            "helper copy: {} {} source={} destination={}",
            action,
            kind.label(),
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

fn sibling_source_path(kind: HelperExecutable) -> Result<PathBuf> {
    let exe = std::env::current_exe().context("resolve current executable for helper lookup")?;
    let dir = exe
        .parent()
        .ok_or_else(|| anyhow!("current executable has no parent directory"))?;
    if is_windowsapps_path(dir) {
        return Err(anyhow!(
            "helper source lookup refused packaged executable parent {}",
            dir.display()
        ));
    }
    let candidate = dir.join(kind.file_name());
    if candidate.exists() {
        Ok(candidate)
    } else {
        Err(anyhow!(
            "helper not found next to current executable: {}",
            candidate.display()
        ))
    }
}

fn copy_from_source_if_needed(source: &Path, destination: &Path) -> Result<CopyOutcome> {
    if destination_is_fresh(source, destination)? {
        return Ok(CopyOutcome::Reused);
    }

    let destination_dir = destination.parent().ok_or_else(|| {
        anyhow!(
            "helper destination has no parent: {}",
            destination.display()
        )
    })?;
    fs::create_dir_all(destination_dir).with_context(|| {
        format!(
            "create helper destination directory {}",
            destination_dir.display()
        )
    })?;

    let temp_path = NamedTempFile::new_in(destination_dir)
        .with_context(|| {
            format!(
                "create temporary helper file in {}",
                destination_dir.display()
            )
        })?
        .into_temp_path();
    let temp_path_buf = temp_path.to_path_buf();

    let mut source_file = fs::File::open(source)
        .with_context(|| format!("open helper source for read {}", source.display()))?;
    let mut temp_file = fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(&temp_path_buf)
        .with_context(|| format!("open temporary helper file {}", temp_path_buf.display()))?;

    // Write into a temp file created inside `.sandbox-bin` so the copied helper keeps the
    // destination directory's inherited ACLs instead of reusing the source file's descriptor.
    std::io::copy(&mut source_file, &mut temp_file).with_context(|| {
        format!(
            "copy helper from {} to {}",
            source.display(),
            temp_path_buf.display()
        )
    })?;
    temp_file
        .flush()
        .with_context(|| format!("flush temporary helper file {}", temp_path_buf.display()))?;
    drop(temp_file);

    if destination.exists() {
        fs::remove_file(destination).with_context(|| {
            format!("remove stale helper destination {}", destination.display())
        })?;
    }

    match fs::rename(&temp_path_buf, destination) {
        Ok(()) => Ok(CopyOutcome::ReCopied),
        Err(rename_err) => {
            if destination_is_fresh(source, destination)? {
                Ok(CopyOutcome::Reused)
            } else {
                Err(rename_err).with_context(|| {
                    format!(
                        "rename helper temp file {} to {}",
                        temp_path_buf.display(),
                        destination.display()
                    )
                })
            }
        }
    }
}

fn destination_is_fresh(source: &Path, destination: &Path) -> Result<bool> {
    let source_meta = fs::metadata(source)
        .with_context(|| format!("read helper source metadata {}", source.display()))?;
    let destination_meta = match fs::metadata(destination) {
        Ok(meta) => meta,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(err) => {
            return Err(err).with_context(|| {
                format!("read helper destination metadata {}", destination.display())
            });
        }
    };

    if source_meta.len() != destination_meta.len() {
        return Ok(false);
    }

    let source_modified = source_meta
        .modified()
        .with_context(|| format!("read helper source mtime {}", source.display()))?;
    let destination_modified = destination_meta
        .modified()
        .with_context(|| format!("read helper destination mtime {}", destination.display()))?;

    if destination_modified < source_modified {
        return Ok(false);
    }

    file_contents_match(source, destination)
}

fn file_contents_match(source: &Path, destination: &Path) -> Result<bool> {
    let source_bytes = fs::read(source)
        .with_context(|| format!("read helper source bytes {}", source.display()))?;
    let destination_bytes = fs::read(destination)
        .with_context(|| format!("read helper destination bytes {}", destination.display()))?;
    Ok(source_bytes == destination_bytes)
}

fn is_windowsapps_path(path: &Path) -> bool {
    path.to_string_lossy()
        .to_ascii_lowercase()
        .contains("\\windowsapps\\")
}

#[cfg(test)]
mod tests {
    use super::CopyOutcome;
    use super::copy_from_source_if_needed;
    use super::destination_is_fresh;
    use super::file_contents_match;
    use super::helper_bin_dir;
    use super::is_windowsapps_path;
    use pretty_assertions::assert_eq;
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn copy_from_source_if_needed_copies_missing_destination() {
        let tmp = TempDir::new().expect("tempdir");
        let source = tmp.path().join("source.exe");
        let destination = tmp.path().join("bin").join("helper.exe");

        fs::write(&source, b"runner-v1").expect("write source");

        let outcome = copy_from_source_if_needed(&source, &destination).expect("copy helper");

        assert_eq!(CopyOutcome::ReCopied, outcome);
        assert_eq!(
            b"runner-v1".as_slice(),
            fs::read(&destination).expect("read destination")
        );
    }

    #[test]
    fn destination_is_fresh_uses_size_and_mtime() {
        let tmp = TempDir::new().expect("tempdir");
        let source = tmp.path().join("source.exe");
        let destination = tmp.path().join("destination.exe");

        fs::write(&destination, b"same-size").expect("write destination");
        std::thread::sleep(std::time::Duration::from_secs(1));
        fs::write(&source, b"same-size").expect("write source");
        assert!(!destination_is_fresh(&source, &destination).expect("stale metadata"));

        fs::write(&destination, b"same-size").expect("rewrite destination");
        assert!(destination_is_fresh(&source, &destination).expect("fresh metadata"));
    }

    #[test]
    fn destination_is_fresh_rejects_same_size_content_drift() {
        let tmp = TempDir::new().expect("tempdir");
        let source = tmp.path().join("source.exe");
        let destination = tmp.path().join("destination.exe");

        fs::write(&source, b"runner-v1").expect("write source");
        std::thread::sleep(std::time::Duration::from_secs(1));
        fs::write(&destination, b"runner-v2").expect("write destination");

        assert!(!destination_is_fresh(&source, &destination).expect("detect content drift"));
    }

    #[test]
    fn copy_from_source_if_needed_reuses_fresh_destination() {
        let tmp = TempDir::new().expect("tempdir");
        let source = tmp.path().join("source.exe");
        let destination = tmp.path().join("bin").join("helper.exe");

        fs::write(&source, b"runner-v1").expect("write source");
        copy_from_source_if_needed(&source, &destination).expect("initial copy");

        let outcome = copy_from_source_if_needed(&source, &destination).expect("revalidate helper");

        assert_eq!(CopyOutcome::Reused, outcome);
        assert_eq!(
            b"runner-v1".as_slice(),
            fs::read(&destination).expect("read destination")
        );
    }

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
        let runner_destination = helper_bin_dir(&codex_home).join("codex-command-runner.exe");
        fs::write(&runner_source, b"runner").expect("runner");

        let runner_outcome =
            copy_from_source_if_needed(&runner_source, &runner_destination).expect("runner copy");

        assert_eq!(CopyOutcome::ReCopied, runner_outcome);
        assert_eq!(
            b"runner".as_slice(),
            fs::read(&runner_destination).expect("read runner")
        );
    }

    #[test]
    fn file_contents_match_detects_equal_files() {
        let tmp = TempDir::new().expect("tempdir");
        let left = tmp.path().join("left.exe");
        let right = tmp.path().join("right.exe");
        fs::write(&left, b"runner").expect("write left");
        fs::write(&right, b"runner").expect("write right");

        assert!(file_contents_match(&left, &right).expect("compare equal files"));
    }

    #[test]
    fn windowsapps_detection_matches_packaged_path() {
        assert!(is_windowsapps_path(Path::new(
            r"C:\Program Files\WindowsApps\OpenAI.Codex\codex.exe"
        )));
        assert!(!is_windowsapps_path(Path::new(
            r"C:\Program Files\OpenAI\codex.exe"
        )));
    }
}
