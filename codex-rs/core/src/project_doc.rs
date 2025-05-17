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
pub(crate) async fn create_full_instructions(config: &Config) -> Option<String> {
    match find_project_doc(config).await {
        Ok(Some(project_doc)) => match &config.instructions {
            Some(original_instructions) => Some(format!(
                "{original_instructions}{PROJECT_DOC_SEPARATOR}{project_doc}"
            )),
            None => Some(project_doc),
        },
        Ok(None) => config.instructions.clone(),
        Err(e) => {
            error!("error trying to find project doc: {e:#}");
            config.instructions.clone()
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
    let max_bytes = config.project_doc_max_bytes;

    // Attempt to load from the working directory first.
    if let Some(doc) = load_first_candidate(&config.cwd, CANDIDATE_FILENAMES, max_bytes).await? {
        return Ok(Some(doc));
    }

    // Walk up towards the filesystem root, stopping once we encounter the Git
    // repository root. The presence of **either** a `.git` *file* or
    // *directory* counts.
    let mut dir = config.cwd.clone();

    // Canonicalize the path so that we do not end up in an infinite loop when
    // `cwd` contains `..` components.
    if let Ok(canon) = dir.canonicalize() {
        dir = canon;
    }

    while let Some(parent) = dir.parent() {
        // `.git` can be a *file* (for worktrees or submodules) or a *dir*.
        let git_marker = dir.join(".git");
        let git_exists = match tokio::fs::metadata(&git_marker).await {
            Ok(_) => true,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
            Err(e) => return Err(e),
        };

        if git_exists {
            // We are at the repo root – attempt one final load.
            if let Some(doc) = load_first_candidate(&dir, CANDIDATE_FILENAMES, max_bytes).await? {
                return Ok(Some(doc));
            }
            break;
        }

        dir = parent.to_path_buf();
    }

    Ok(None)
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

        config.instructions = instructions.map(ToOwned::to_owned);
        config
    }

    /// AGENTS.md missing – should yield `None`.
    #[tokio::test]
    async fn no_doc_file_returns_none() {
        let tmp = tempfile::tempdir().expect("tempdir");

        let res = create_full_instructions(&make_config(&tmp, 4096, None)).await;
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

        let res = create_full_instructions(&make_config(&tmp, 4096, None))
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

        let res = create_full_instructions(&make_config(&tmp, LIMIT, None))
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

        let res = create_full_instructions(&cfg).await.expect("doc expected");
        assert_eq!(res, "root level doc");
    }

    /// Explicitly setting the byte-limit to zero disables project docs.
    #[tokio::test]
    async fn zero_byte_limit_disables_docs() {
        let tmp = tempfile::tempdir().expect("tempdir");
        fs::write(tmp.path().join("AGENTS.md"), "something").unwrap();

        let res = create_full_instructions(&make_config(&tmp, 0, None)).await;
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

        let res = create_full_instructions(&make_config(&tmp, 4096, Some(INSTRUCTIONS)))
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

        let res = create_full_instructions(&make_config(&tmp, 4096, Some(INSTRUCTIONS))).await;

        assert_eq!(res, Some(INSTRUCTIONS.to_string()));
    }
}
