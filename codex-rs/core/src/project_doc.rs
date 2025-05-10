//! Project-level documentation discovery.
//!
//! Project-level documentation can be stored in a file named `AGENTS.md`.
//! Currently, we include only the contents of the first file found as follows:
//!
//! 1.  Look for the doc file in the current working directory.
//! 2.  If not found, walk *upwards* until the Git repository root is reached
//!     (detected by the presence of a `.git` directory/file).
//! 3.  If/when the Git root is encountered, look for the doc file there. If it
//!     exists, the search stops – we do **not** walk past the Git root.

use crate::config::Config;

use std::path::Path;

/// Maximum number of bytes of the documentation that will be embedded. Larger
/// files are *silently truncated* to this size so we never blow the context
/// window.
pub(crate) const PROJECT_DOC_MAX_BYTES: usize = 32 * 1024; // 32 KiB

/// Currently, we only match `AGENTS.md` exactly.
const CANDIDATE_FILENAMES: &[&str] = &["AGENTS.md"];

/// Attempt to locate and load the project documentation.
///
/// On success returns `Ok(Some(contents))`. If no documentation file is found
/// the function returns `Ok(None)`. Unexpected I/O failures bubble up as
/// `Err` so callers can decide how to handle them.
pub(crate) async fn find_project_doc(config: &Config) -> std::io::Result<Option<String>> {
    // Attempt to load from the working directory first.
    if let Some(doc) = load_first_candidate(&config.cwd, CANDIDATE_FILENAMES).await? {
        return Ok(Some(doc));
    }

    // Walk up towards the filesystem root, stopping once we encounter the Git
    // repository root.  The presence of **either** a `.git` *file* or
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
            if let Some(doc) = load_first_candidate(&dir, CANDIDATE_FILENAMES).await? {
                return Ok(Some(doc));
            }
            break;
        }

        dir = parent.to_path_buf();
    }

    Ok(None)
}

/// Attempt to load the first candidate file found in `dir`. Returns the file
/// contents (truncated) when successful.
async fn load_first_candidate(dir: &Path, names: &[&str]) -> std::io::Result<Option<String>> {
    use tokio::io::AsyncReadExt;

    for name in names {
        let candidate = dir.join(name);

        let file = match tokio::fs::File::open(&candidate).await {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e),
            Ok(f) => f,
        };

        let size = file.metadata().await?.len();

        let reader = tokio::io::BufReader::new(file);
        let mut data = Vec::with_capacity(std::cmp::min(size as usize, PROJECT_DOC_MAX_BYTES));
        let mut limited = reader.take(PROJECT_DOC_MAX_BYTES as u64);
        limited.read_to_end(&mut data).await?;

        if size as usize > PROJECT_DOC_MAX_BYTES {
            tracing::warn!(
                "Project doc `{}` exceeds {PROJECT_DOC_MAX_BYTES} bytes - truncating.",
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
