//! Project-level documentation discovery.
//!
//! Project-level documentation can be stored in files named `AGENTS.md`.
//! We include the concatenation of all files found along the path from the
//! repository root to the current working directory as follows:
//!
//! 1.  Determine the Git repository root by walking upwards from the current
//!     working directory until a `.git` directory or file is found. If no Git
//!     root is found, only the current working directory is considered.
//! 2.  Collect every `AGENTS.md` found from the repository root down to the
//!     current working directory (inclusive) and concatenate their contents in
//!     that order.
//! 3.  We do **not** walk past the Git root.

use crate::config::Config;
use std::path::Path;
use tokio::io::AsyncReadExt;
use tracing::error;

/// Currently, we only match the filename `AGENTS.md` exactly.
const CANDIDATE_FILENAMES: &[&str] = &["AGENTS.md"];

/// When both `Config::instructions` and the project doc are present, they will
/// be concatenated with the following separator.
const PROJECT_DOC_SEPARATOR: &str = "\n\n--- project-doc ---\n\n";

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

/// Attempt to locate and load the project documentation.
///
/// On success returns `Ok(Some(contents))` where `contents` is the
/// concatenation of all discovered docs. If no documentation file is found the
/// function returns `Ok(None)`. Unexpected I/O failures bubble up as `Err` so
/// callers can decide how to handle them.
async fn find_project_doc(config: &Config) -> std::io::Result<Option<String>> {
    let max_bytes = config.project_doc_max_bytes;

    // If limit is zero, docs are disabled.
    if max_bytes == 0 {
        return Ok(None);
    }

    // Build a list of directories from repo root to the current working
    // directory (inclusive). If no repo root is found, only consider the cwd.
    let mut dir = config.cwd.clone();

    // Canonicalize to avoid infinite loops when `cwd` contains `..` components.
    if let Ok(canon) = dir.canonicalize() {
        dir = canon;
    }

    // First, discover the repo root (if any) while collecting the path chain.
    let mut chain: Vec<std::path::PathBuf> = vec![dir.clone()];
    let mut git_root: Option<std::path::PathBuf> = None;
    let mut cursor = dir.clone();
    while let Some(parent) = cursor.parent() {
        // `.git` can be a *file* (for worktrees or submodules) or a *dir*.
        let git_marker = cursor.join(".git");
        let git_exists = match tokio::fs::metadata(&git_marker).await {
            Ok(_) => true,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
            Err(e) => return Err(e),
        };

        if git_exists {
            git_root = Some(cursor.clone());
            break;
        }

        chain.push(parent.to_path_buf());
        cursor = parent.to_path_buf();
    }

    // If we found a git root, trim the chain so that it starts at the git
    // root and ends at the original cwd; otherwise, the chain should only
    // include the cwd.
    let search_dirs: Vec<std::path::PathBuf> = if let Some(root) = git_root {
        // `chain` currently contains: [cwd, parent, ..., maybe root?, ...]
        // Keep entries from `root` down to the original `cwd` and reverse to
        // get [root, ..., cwd].
        let mut dirs: Vec<std::path::PathBuf> = Vec::new();
        let mut saw_root = false;
        for p in chain.iter().rev() {
            if !saw_root {
                if p == &root {
                    saw_root = true;
                } else {
                    continue;
                }
            }
            dirs.push(p.clone());
        }
        // Now `dirs` is [root, ..., cwd]
        dirs
    } else {
        vec![config.cwd.clone()]
    };

    // Load and concatenate docs from all search directories.
    let mut parts: Vec<String> = Vec::new();
    for d in search_dirs {
        if let Some(doc) = load_first_candidate(&d, CANDIDATE_FILENAMES, max_bytes).await?
            && !doc.trim().is_empty() {
                parts.push(doc);
            }
    }

    if parts.is_empty() {
        Ok(None)
    } else {
        Ok(Some(parts.join("\n\n")))
    }
}

/// Attempt to load the first candidate file found in `dir`. Returns the file
/// contents (truncated if it exceeds `max_bytes`) when successful.
async fn load_first_candidate(
    dir: &Path,
    names: &[&str],
    max_bytes: usize,
) -> std::io::Result<Option<String>> {
    for name in names {
        let candidate = dir.join(name);

        let file = match tokio::fs::File::open(&candidate).await {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e),
            Ok(f) => f,
        };

        let size = file.metadata().await?.len();

        let reader = tokio::io::BufReader::new(file);
        let mut data = Vec::with_capacity(std::cmp::min(size as usize, max_bytes));
        let mut limited = reader.take(max_bytes as u64);
        limited.read_to_end(&mut data).await?;

        if size as usize > max_bytes {
            tracing::warn!(
                "Project doc `{}` exceeds {max_bytes} bytes - truncating.",
                candidate.display(),
            );
        }

        let contents = String::from_utf8_lossy(&data).to_string();
        if contents.trim().is_empty() {
            // Empty file – treat as not found.
            continue;
        }

        return Ok(Some(contents));
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
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

    /// When both the repository root and the working directory contain
    /// AGENTS.md files, their contents are concatenated from root to cwd.
    #[tokio::test]
    async fn concatenates_root_and_cwd_docs() {
        let repo = tempfile::tempdir().expect("tempdir");

        // Simulate a git repository.
        std::fs::write(
            repo.path().join(".git"),
            "gitdir: /path/to/actual/git/dir\n",
        )
        .unwrap();

        // Repo root doc.
        fs::write(repo.path().join("AGENTS.md"), "root doc").unwrap();

        // Nested working directory with its own doc.
        let nested = repo.path().join("workspace/crate_a");
        std::fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("AGENTS.md"), "crate doc").unwrap();

        let mut cfg = make_config(&repo, 4096, None);
        cfg.cwd = nested;

        let res = get_user_instructions(&cfg).await.expect("doc expected");
        assert_eq!(res, "root doc\n\ncrate doc");
    }
}
