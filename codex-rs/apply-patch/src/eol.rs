//! EOL selection and normalization for apply-patch writes.
//!
//! Precedence used when choosing EOLs for writes:
//! - CLI/env override wins (lf|crlf|git|detect)
//! - .gitattributes (path-specific) → lf/crlf/native; binary or -text => Unknown (skip)
//! - For new files only: if no attribute matches, default to LF (not OS/native)
//! - Detect from content (existing files only; callers sniff original bytes)
//!
//! Notes:
//! - Existing files: when no CLI/env override, callers should infer from the bytes
//!   already in memory (original buffer or nearby hunk context). Do not re-read the file.
//! - Normalization only happens on final disk writes; previews/summaries may remain LF.
//! - Trailing newline presence is preserved exactly; we do not add or remove it.

#[cfg(feature = "eol-cache")]
use std::collections::HashMap;
use std::path::Path;
#[cfg(feature = "eol-cache")]
use std::path::PathBuf;
use std::sync::LazyLock;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Eol {
    Lf,
    Crlf,
    Unknown,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum AssumeEol {
    Unspecified,
    Git,
    Detect,
    Lf,
    Crlf,
}

static ASSUME_EOL: LazyLock<std::sync::Mutex<AssumeEol>> =
    LazyLock::new(|| std::sync::Mutex::new(assume_eol_from_env()));

fn assume_eol_from_env() -> AssumeEol {
    match std::env::var("APPLY_PATCH_ASSUME_EOL") {
        Ok(v) => parse_assume_eol(&v).unwrap_or(AssumeEol::Unspecified),
        Err(_) => AssumeEol::Unspecified,
    }
}

pub fn set_assume_eol(a: AssumeEol) {
    if let Ok(mut guard) = ASSUME_EOL.lock() {
        *guard = a;
    }
}

pub fn get_assume_eol() -> AssumeEol {
    ASSUME_EOL
        .lock()
        .map(|g| *g)
        .unwrap_or(AssumeEol::Unspecified)
}

pub fn parse_assume_eol(s: &str) -> Option<AssumeEol> {
    let val = s.trim().to_ascii_lowercase();
    match val.as_str() {
        "lf" => Some(AssumeEol::Lf),
        "crlf" => Some(AssumeEol::Crlf),
        "git" => Some(AssumeEol::Git),
        "detect" => Some(AssumeEol::Detect),
        _ => None,
    }
}

pub fn os_native_eol() -> Eol {
    if cfg!(windows) { Eol::Crlf } else { Eol::Lf }
}

// Byte-based detection that counts CRLF vs lone LF to handle mixed files.
pub fn detect_eol_from_bytes(buf: &[u8]) -> Eol {
    let mut crlf = 0i32;
    let mut lf = 0i32;
    let mut i = 0usize;
    while i < buf.len() {
        if buf[i] == b'\n' {
            if i > 0 && buf[i - 1] == b'\r' {
                crlf += 1;
            } else {
                lf += 1;
            }
        }
        i += 1;
    }
    if crlf == 0 && lf == 0 {
        return Eol::Unknown;
    }
    if crlf >= lf { Eol::Crlf } else { Eol::Lf }
}

// Preserve whether the original had a trailing newline. Do NOT add or remove it.
pub fn normalize_to_eol_preserve_eof(mut s: String, target: Eol) -> String {
    let had_trailing_nl = s.as_bytes().last().map(|b| *b == b'\n').unwrap_or(false);
    let eol_str = match target {
        Eol::Crlf => "\r\n",
        Eol::Lf | Eol::Unknown => "\n",
    };
    s = s.replace("\r\n", "\n");
    if matches!(target, Eol::Crlf) {
        s = s.replace('\n', "\r\n");
    }
    let ends_with_target = s.ends_with(eol_str);
    match (had_trailing_nl, ends_with_target) {
        (true, false) => s.push_str(eol_str),
        (false, true) => {
            let new_len = s.len().saturating_sub(eol_str.len());
            s.truncate(new_len);
        }
        _ => {}
    }
    s
}

#[cfg(test)]
pub fn git_core_eol(repo_root: &Path) -> Option<Eol> {
    #[cfg(all(test, feature = "eol-cache"))]
    RAW_CORE_EOL.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .arg("config")
        .arg("--get")
        .arg("core.eol")
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let val = String::from_utf8_lossy(&out.stdout)
        .trim()
        .to_ascii_lowercase();
    match val.as_str() {
        "lf" => Some(Eol::Lf),
        "crlf" => Some(Eol::Crlf),
        "native" => Some(os_native_eol()),
        _ => None,
    }
}

// Helper for core.autocrlf was used in production previously; tests rely on
// core.eol coverage now, so this is intentionally omitted.

pub fn git_check_attr_eol(repo_root: &Path, rel_path: &Path) -> Option<Eol> {
    #[cfg(all(test, feature = "eol-cache"))]
    RAW_ATTR.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let rel = rel_path.to_string_lossy().replace('\\', "/");
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .arg("check-attr")
        .arg("eol")
        .arg("text")
        .arg("binary")
        .arg("--")
        .arg(rel)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&out.stdout);

    let mut eol_val: Option<String> = None;
    let mut text_val: Option<String> = None;
    let mut binary_set = false;
    for line in stdout.lines() {
        // Format: "path: attr: value"
        let mut parts = line.splitn(3, ": ");
        let _ = parts.next();
        if let (Some(attr), Some(value)) = (parts.next(), parts.next()) {
            let attr = attr.trim();
            let value = value.trim();
            match attr {
                "eol" => eol_val = Some(value.to_ascii_lowercase()),
                "text" => text_val = Some(value.to_ascii_lowercase()),
                "binary" => binary_set = value.eq_ignore_ascii_case("set"),
                _ => {}
            }
        }
    }

    if binary_set {
        return Some(Eol::Unknown);
    }
    if matches!(text_val.as_deref(), Some("unset")) {
        return Some(Eol::Unknown);
    }

    match eol_val.as_deref() {
        Some("lf") => Some(Eol::Lf),
        Some("crlf") => Some(Eol::Crlf),
        Some("native") => Some(os_native_eol()),
        _ => None,
    }
}

/// Decide EOL based on repo policy and CLI/env.
/// - For existing files (is_new_file=false):
///   - If CLI override is Lf/Crlf => return it
///   - If CLI override is Git => consult Git and return if specified; Unknown for binary/-text
///   - Otherwise return Unknown so caller can detect from original bytes they already hold
/// - For new files (is_new_file=true):
///   - CLI override Lf/Crlf wins
///   - CLI override Git => consult Git
///   - Otherwise consult .gitattributes → core.eol → core.autocrlf
///   - Fall back to OS native; detection from patch bytes should be handled by caller
//
// Caching layer
#[cfg(feature = "eol-cache")]
type AttrKey = (PathBuf, String);
#[cfg(all(test, feature = "eol-cache"))]
static CORE_EOL_CACHE: LazyLock<std::sync::Mutex<HashMap<PathBuf, Option<Eol>>>> =
    LazyLock::new(|| std::sync::Mutex::new(HashMap::new()));
#[cfg(feature = "eol-cache")]
static ATTRS_CACHE: LazyLock<std::sync::Mutex<HashMap<AttrKey, Option<Eol>>>> =
    LazyLock::new(|| std::sync::Mutex::new(HashMap::new()));

#[cfg(feature = "eol-cache")]
fn canonical_repo_root(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

pub(crate) fn norm_rel_key(rel: &Path) -> String {
    let s = rel.to_string_lossy().replace('\\', "/");
    if cfg!(windows) {
        s.to_ascii_lowercase()
    } else {
        s
    }
}

#[cfg(all(test, feature = "eol-cache"))]
fn git_core_eol_cached(repo_root: &Path) -> Option<Eol> {
    let key = canonical_repo_root(repo_root);
    if let Ok(mut m) = CORE_EOL_CACHE.lock() {
        if let Some(v) = m.get(&key) {
            return *v;
        }
        let v = git_core_eol(&key);
        m.insert(key, v);
        return v;
    }
    git_core_eol(repo_root)
}
// Only used by tests; no non-test variant needed.

// Note: git_core_autocrlf_* no longer used in production code; omitted outside tests.

#[cfg(feature = "eol-cache")]
pub(crate) fn git_check_attr_eol_cached(repo_root: &Path, rel_path: &Path) -> Option<Eol> {
    let rkey = canonical_repo_root(repo_root);
    let pkey = norm_rel_key(rel_path);
    if let Ok(mut m) = ATTRS_CACHE.lock() {
        if let Some(v) = m.get(&(rkey.clone(), pkey.clone())) {
            return *v;
        }
        let v = git_check_attr_eol(&rkey, Path::new(&pkey));
        m.insert((rkey, pkey), v);
        return v;
    }
    git_check_attr_eol(repo_root, rel_path)
}
#[cfg(not(feature = "eol-cache"))]
pub(crate) fn git_check_attr_eol_cached(repo_root: &Path, rel_path: &Path) -> Option<Eol> {
    git_check_attr_eol(repo_root, rel_path)
}

#[cfg(feature = "eol-cache")]
pub fn notify_gitattributes_touched(repo_root: &Path) {
    let key = canonical_repo_root(repo_root);
    if let Ok(mut m) = ATTRS_CACHE.lock() {
        m.retain(|(root, _), _| root != &key);
    }
}
#[cfg(not(feature = "eol-cache"))]
pub fn notify_gitattributes_touched(_repo_root: &Path) {}

pub fn decide_eol(repo_root: Option<&Path>, rel_path: Option<&Path>, is_new_file: bool) -> Eol {
    match get_assume_eol() {
        AssumeEol::Lf => return Eol::Lf,
        AssumeEol::Crlf => return Eol::Crlf,
        AssumeEol::Git => {
            // Respect only path-specific attributes. Avoid global git core.*
            // settings to keep behavior deterministic across runners.
            if let (Some(root), Some(rel)) = (repo_root, rel_path)
                && let Some(e) = git_check_attr_eol_cached(root, rel)
            {
                return e;
            }
            // No attribute match: default to LF for new files, Unknown for existing.
            return if is_new_file { Eol::Lf } else { Eol::Unknown };
        }
        AssumeEol::Detect | AssumeEol::Unspecified => {}
    }

    if !is_new_file {
        // Existing: let caller decide from original bytes
        return Eol::Unknown;
    }

    // New file without explicit CLI override: consult .gitattributes only,
    // otherwise default to LF. Ignore git core.* to avoid host variability.
    if let (Some(root), Some(rel)) = (repo_root, rel_path)
        && let Some(e) = git_check_attr_eol_cached(root, rel)
    {
        return e;
    }
    Eol::Lf
}

// Test instrumentation and unit tests for caching
#[cfg(all(test, feature = "eol-cache"))]
static RAW_CORE_EOL: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
#[cfg(all(test, feature = "eol-cache"))]
static RAW_AUTOCRLF: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
#[cfg(all(test, feature = "eol-cache"))]
static RAW_ATTR: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

#[cfg(all(test, feature = "eol-cache"))]
pub fn reset_git_counters() {
    RAW_CORE_EOL.store(0, std::sync::atomic::Ordering::Relaxed);
    RAW_AUTOCRLF.store(0, std::sync::atomic::Ordering::Relaxed);
    RAW_ATTR.store(0, std::sync::atomic::Ordering::Relaxed);
}

#[cfg(all(test, feature = "eol-cache"))]
pub fn raw_counts() -> (usize, usize, usize) {
    (
        RAW_CORE_EOL.load(std::sync::atomic::Ordering::Relaxed),
        RAW_AUTOCRLF.load(std::sync::atomic::Ordering::Relaxed),
        RAW_ATTR.load(std::sync::atomic::Ordering::Relaxed),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "eol-cache")]
    use tempfile::tempdir;

    #[cfg(feature = "eol-cache")]
    #[test]
    fn test_core_eol_cached_only_runs_git_once() {
        reset_git_counters();
        let dir = tempdir().unwrap();
        std::process::Command::new("git")
            .arg("init")
            .arg("-q")
            .current_dir(dir.path())
            .status()
            .unwrap();
        std::process::Command::new("git")
            .args([
                "-C",
                dir.path().to_str().unwrap(),
                "config",
                "core.eol",
                "lf",
            ])
            .status()
            .unwrap();
        assert_eq!(git_core_eol_cached(dir.path()), Some(Eol::Lf));
        assert_eq!(git_core_eol_cached(dir.path()), Some(Eol::Lf));
        let (core, _, _) = raw_counts();
        assert_eq!(core, 1);
    }

    #[cfg(feature = "eol-cache")]
    #[test]
    fn test_attrs_cache_and_invalidate() {
        reset_git_counters();
        let dir = tempdir().unwrap();
        std::process::Command::new("git")
            .arg("init")
            .arg("-q")
            .current_dir(dir.path())
            .status()
            .unwrap();
        std::fs::write(dir.path().join(".gitattributes"), "*.txt text eol=crlf\n").unwrap();
        // First call populates cache
        let rel = Path::new("foo.txt");
        assert_eq!(git_check_attr_eol_cached(dir.path(), rel), Some(Eol::Crlf));
        // Second call hits cache
        assert_eq!(git_check_attr_eol_cached(dir.path(), rel), Some(Eol::Crlf));
        let (_, _, attr1) = raw_counts();
        assert_eq!(attr1, 1);

        // Change gitattributes and notify
        std::fs::write(dir.path().join(".gitattributes"), "*.txt text eol=lf\n").unwrap();
        notify_gitattributes_touched(dir.path());

        // Next call re-runs git and reflects new mapping
        assert_eq!(git_check_attr_eol_cached(dir.path(), rel), Some(Eol::Lf));
        let (_, _, attr2) = raw_counts();
        assert_eq!(attr2, 2);
    }

    #[test]
    fn test_windows_rel_key_normalization() {
        let a = norm_rel_key(Path::new("A\\B.txt"));
        let b = norm_rel_key(Path::new("a/b.txt"));
        if cfg!(windows) {
            assert_eq!(a, b);
        } else {
            assert_ne!(a, b);
        }
    }
}

// Note: detection-from-buffer fallback for new files is implemented at the
// call site so it can incorporate local context (e.g., repo presence).
