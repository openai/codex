//! Project-level documentation discovery.
//!
//! Project-level documentation can be stored in a file named `AGENTS.md`.
//! Currently, we include only the contents of the first file found as follows:
//!
//! 1.  Look for the doc file in the current working directory (as determined
//!     by the `Config`).
//! 2.  If not found, walk *upwards* until the Git repository root is reached
//!     (detected by the presence of a `.git` directory/file), or failing that,
//!     the filesystem root.
//! 3.  If the Git root is encountered, look for the doc file there. If it
//!     exists, the search stops – we do **not** walk past the Git root.

use crate::config::Config;
use std::fs;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use tokio::io::AsyncReadExt;
use tokio::io::BufReader;
use tracing::error;

/// Currently, we only match the filename `AGENTS.md` exactly.
const CANDIDATE_FILENAMES: &[&str] = &["AGENTS.md"];

/// When both `Config::instructions` and the project doc are present, they will
/// be concatenated with the following separator.
const PROJECT_DOC_SEPARATOR: &str = "\n\n--- project-doc ---\n\n";

/// Public helper that returns the discovered AGENTS.md path.
/// Returns `Ok(None)` when no suitable file is found or
/// `project_doc_max_bytes == 0`.
pub fn discover_project_doc_path(config: &Config) -> std::io::Result<Option<std::path::PathBuf>> {
    if config.project_doc_max_bytes > 0 {
        discover_project_doc_path_from_dir(
            &config.cwd,
            CANDIDATE_FILENAMES,
            config.project_doc_max_bytes,
        )
    } else {
        Ok(None)
    }
}

fn discover_project_doc_path_from_dir(
    start_dir: &Path,
    names: &[&str],
    max_bytes: usize,
) -> std::io::Result<Option<std::path::PathBuf>> {
    // Canonicalize the path so that we do not end up in an infinite loop when
    // `cwd` contains `..` components.
    let mut dir = start_dir.to_path_buf();
    if let Ok(canon) = dir.canonicalize() {
        dir = canon;
    }

    // Attempt in the working directory first.
    if let Some(path) = find_non_empty_candidate(&dir, names, max_bytes)? {
        return Ok(Some(path));
    }

    // Walk up towards the filesystem root, stopping once we encounter the Git root.
    while let Some(parent) = dir.parent() {
        let git_marker = dir.join(".git");
        let git_exists = match fs::metadata(&git_marker) {
            Ok(_) => true,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
            Err(e) => return Err(e),
        };

        if git_exists {
            if let Some(path) = find_non_empty_candidate(&dir, names, max_bytes)? {
                return Ok(Some(path));
            }
            break; // do not walk past the Git root
        }

        dir = parent.to_path_buf();
    }

    Ok(None)
}

fn find_non_empty_candidate(
    dir: &Path,
    names: &[&str],
    max_bytes: usize,
) -> std::io::Result<Option<std::path::PathBuf>> {
    for name in names {
        let candidate = dir.join(name);
        let file = match File::open(&candidate) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e),
            Ok(f) => f,
        };

        let size = file.metadata()?.len() as usize;
        let to_read = std::cmp::min(size, max_bytes);
        let mut reader = std::io::BufReader::new(file);
        let mut data = vec![0u8; to_read];
        reader.read_exact(&mut data)?;
        let contents = String::from_utf8_lossy(&data).to_string();
        if contents.trim().is_empty() {
            continue;
        }

        return Ok(Some(candidate));
    }

    Ok(None)
}

/// Combines `Config::instructions` and `AGENTS.md` (if present) into a single
/// string of instructions.
pub(crate) async fn get_user_instructions(config: &Config) -> Option<String> {
    match find_project_doc(config).await {
        Ok(Some(project_doc)) => match &config.user_instructions {
            Some(original_instructions) => Some(format!(
                "{original_instructions}{PROJECT_DOC_SEPARATOR}{project_doc}"
            )),
            None => Some(project_doc),
        },
        Ok(None) => config.user_instructions.clone(),
        Err(e) => {
            error!("error trying to find project doc: {e:#}");
            config.user_instructions.clone()
        }
    }
}

/// Attempt to locate and load the project documentation. Currently, the search
/// starts from `Config::cwd`, but if we may want to consider other directories
/// in the future, e.g., additional writable directories in the `SandboxPolicy`.
///
/// On success returns `Ok(Some(contents))`. If no documentation file is found
/// the function returns `Ok(None)`. Unexpected I/O failures bubble up as
/// `Err` so callers can decide how to handle them.
async fn find_project_doc(config: &Config) -> std::io::Result<Option<String>> {
    let Some(path) = discover_project_doc_path(config)? else {
        return Ok(None);
    };

    let max_bytes = config.project_doc_max_bytes;

    let file = tokio::fs::File::open(&path).await?;
    let size = file.metadata().await?.len() as usize;

    let reader = BufReader::new(file);
    let mut data = Vec::with_capacity(std::cmp::min(size, max_bytes));
    let mut limited = reader.take(max_bytes as u64);
    limited.read_to_end(&mut data).await?;

    if size > max_bytes {
        tracing::warn!(
            "Project doc `{}` exceeds {max_bytes} bytes - truncating.",
            path.display(),
        );
    }

    let contents = String::from_utf8_lossy(&data).to_string();
    if contents.trim().is_empty() {
        return Ok(None);
    }

    Ok(Some(contents))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use super::*;
    use crate::config::ConfigOverrides;
    use crate::config::ConfigToml;
    use std::fs;
    use tempfile::TempDir;

    /// Helper that returns a `Config` pointing at `root` and using `limit` as
    /// the maximum number of bytes to embed from AGENTS.md. The caller can
    /// optionally specify a custom `instructions` string – when `None` the
    /// value is cleared to mimic a scenario where no system instructions have
    /// been configured.
    fn make_config(root: &TempDir, limit: usize, instructions: Option<&str>) -> Config {
        let codex_home = TempDir::new().unwrap();
        let mut config = Config::load_from_base_config_with_overrides(
            ConfigToml::default(),
            ConfigOverrides::default(),
            codex_home.path().to_path_buf(),
        )
        .expect("defaults for test should always succeed");

        config.cwd = root.path().to_path_buf();
        config.project_doc_max_bytes = limit;

        config.user_instructions = instructions.map(ToOwned::to_owned);
        config
    }

    /// AGENTS.md missing – should yield `None`.
    #[tokio::test]
    async fn no_doc_file_returns_none() {
        let tmp = tempfile::tempdir().expect("tempdir");

        let res = get_user_instructions(&make_config(&tmp, 4096, None)).await;
        assert!(
            res.is_none(),
            "Expected None when AGENTS.md is absent and no system instructions provided"
        );
        assert!(res.is_none(), "Expected None when AGENTS.md is absent");
    }

    /// Small file within the byte-limit is returned unmodified.
    #[tokio::test]
    async fn doc_smaller_than_limit_is_returned() {
        let tmp = tempfile::tempdir().expect("tempdir");
        fs::write(tmp.path().join("AGENTS.md"), "hello world").unwrap();

        let res = get_user_instructions(&make_config(&tmp, 4096, None))
            .await
            .expect("doc expected");

        assert_eq!(
            res, "hello world",
            "The document should be returned verbatim when it is smaller than the limit and there are no existing instructions"
        );
    }

    /// Oversize file is truncated to `project_doc_max_bytes`.
    #[tokio::test]
    async fn doc_larger_than_limit_is_truncated() {
        const LIMIT: usize = 1024;
        let tmp = tempfile::tempdir().expect("tempdir");

        let huge = "A".repeat(LIMIT * 2); // 2 KiB
        fs::write(tmp.path().join("AGENTS.md"), &huge).unwrap();

        let res = get_user_instructions(&make_config(&tmp, LIMIT, None))
            .await
            .expect("doc expected");

        assert_eq!(res.len(), LIMIT, "doc should be truncated to LIMIT bytes");
        assert_eq!(res, huge[..LIMIT]);
    }

    /// When `cwd` is nested inside a repo, the search should locate AGENTS.md
    /// placed at the repository root (identified by `.git`).
    #[tokio::test]
    async fn finds_doc_in_repo_root() {
        let repo = tempfile::tempdir().expect("tempdir");

        // Simulate a git repository. Note .git can be a file or a directory.
        std::fs::write(
            repo.path().join(".git"),
            "gitdir: /path/to/actual/git/dir\n",
        )
        .unwrap();

        // Put the doc at the repo root.
        fs::write(repo.path().join("AGENTS.md"), "root level doc").unwrap();

        // Now create a nested working directory: repo/workspace/crate_a
        let nested = repo.path().join("workspace/crate_a");
        std::fs::create_dir_all(&nested).unwrap();

        // Build config pointing at the nested dir.
        let mut cfg = make_config(&repo, 4096, None);
        cfg.cwd = nested;

        let res = get_user_instructions(&cfg).await.expect("doc expected");
        assert_eq!(res, "root level doc");
    }

    /// Test if AGENTS.md located in the current working directory is preferred over the repo root.
    #[tokio::test]
    async fn prefers_cwd_doc_over_repo_root() {
        let repo = tempfile::tempdir().expect("tempdir");

        // Simulate a git repository at repo root.
        std::fs::write(repo.path().join(".git"), "gitdir: /dev/null\n").unwrap();

        // Create AGENTS.md at repo root and in a nested cwd.
        fs::write(repo.path().join("AGENTS.md"), "root level doc").unwrap();
        let nested = repo.path().join("workspace/crate_b");
        std::fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("AGENTS.md"), "nested cwd doc").unwrap();

        // Build config pointing at the nested dir.
        let mut cfg = make_config(&repo, 4096, None);
        cfg.cwd = nested.clone();

        // Path discovery should prefer the nested cwd doc.
        let discovered = super::discover_project_doc_path(&cfg)
            .expect("discovery should succeed")
            .expect("path should be found");
        let discovered_canon = fs::canonicalize(&discovered).expect("canonicalize discovered");
        let expected_canon =
            fs::canonicalize(nested.join("AGENTS.md")).expect("canonicalize expected");
        assert_eq!(discovered_canon, expected_canon);

        // get_user_instructions should load the nested document contents.
        let res = get_user_instructions(&cfg).await.expect("doc expected");
        assert_eq!(res, "nested cwd doc");
    }

    /// Test if AGENTS.md at the repo root is used when none exists in cwd.
    #[tokio::test]
    async fn falls_back_to_repo_root_when_cwd_missing_doc() {
        let repo = tempfile::tempdir().expect("tempdir");

        // Simulate a git repository at repo root.
        std::fs::write(repo.path().join(".git"), "gitdir: /dev/null\n").unwrap();

        // Create AGENTS.md only at repo root.
        fs::write(repo.path().join("AGENTS.md"), "root level doc").unwrap();

        // Nested cwd without its own AGENTS.md.
        let nested = repo.path().join("nested/dir");
        std::fs::create_dir_all(&nested).unwrap();

        let mut cfg = make_config(&repo, 4096, None);
        cfg.cwd = nested;

        let discovered = super::discover_project_doc_path(&cfg)
            .expect("discovery should succeed")
            .expect("path should be found");
        let discovered_canon = fs::canonicalize(&discovered).expect("canonicalize discovered");
        let expected_canon =
            fs::canonicalize(repo.path().join("AGENTS.md")).expect("canonicalize expected");
        assert_eq!(discovered_canon, expected_canon);

        let res = get_user_instructions(&cfg).await.expect("doc expected");
        assert_eq!(res, "root level doc");
    }

    /// Explicitly setting the byte-limit to zero disables project docs.
    #[tokio::test]
    async fn zero_byte_limit_disables_docs() {
        let tmp = tempfile::tempdir().expect("tempdir");
        fs::write(tmp.path().join("AGENTS.md"), "something").unwrap();

        let res = get_user_instructions(&make_config(&tmp, 0, None)).await;
        assert!(
            res.is_none(),
            "With limit 0 the function should return None"
        );
    }

    /// When both system instructions *and* a project doc are present the two
    /// should be concatenated with the separator.
    #[tokio::test]
    async fn merges_existing_instructions_with_project_doc() {
        let tmp = tempfile::tempdir().expect("tempdir");
        fs::write(tmp.path().join("AGENTS.md"), "proj doc").unwrap();

        const INSTRUCTIONS: &str = "base instructions";

        let res = get_user_instructions(&make_config(&tmp, 4096, Some(INSTRUCTIONS)))
            .await
            .expect("should produce a combined instruction string");

        let expected = format!("{INSTRUCTIONS}{PROJECT_DOC_SEPARATOR}{}", "proj doc");

        assert_eq!(res, expected);
    }

    /// If there are existing system instructions but the project doc is
    /// missing we expect the original instructions to be returned unchanged.
    #[tokio::test]
    async fn keeps_existing_instructions_when_doc_missing() {
        let tmp = tempfile::tempdir().expect("tempdir");

        const INSTRUCTIONS: &str = "some instructions";

        let res = get_user_instructions(&make_config(&tmp, 4096, Some(INSTRUCTIONS))).await;

        assert_eq!(res, Some(INSTRUCTIONS.to_string()));
    }
}
