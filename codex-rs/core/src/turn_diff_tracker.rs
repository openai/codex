use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use sha1::digest::Output;
use uuid::Uuid;

use codex_protocol::protocol::FileChange;

const ZERO_OID: &str = "0000000000000000000000000000000000000000";
const DEV_NULL: &str = "/dev/null";

struct BaselineFileInfo {
    path: PathBuf,
    content: Vec<u8>,
    mode: FileMode,
    oid: String,
}

/// Tracks sets of changes to files and exposes the overall unified diff.
/// Internally, the way this works is now:
/// 1. Maintain an in-memory baseline snapshot of files when they are first seen.
///    For new additions, do not create a baseline so that diffs are shown as proper additions (using /dev/null).
/// 2. Keep a stable internal filename (uuid) per external path for rename tracking.
/// 3. To compute the aggregated unified diff, compare each baseline snapshot to the current file on disk entirely in-memory
///    using the `similar` crate and emit unified diffs with rewritten external paths.
#[derive(Default)]
pub(crate) struct TurnDiffTracker {
    /// Map external path -> internal filename (uuid).
    external_to_temp_name: HashMap<PathBuf, String>,
    /// Internal filename -> baseline file info.
    baseline_file_info: HashMap<String, BaselineFileInfo>,
    /// Internal filename -> external path as of current accumulated state (after applying all changes).
    /// This is where renames are tracked.
    temp_name_to_current_path: HashMap<String, PathBuf>,
    /// Cache of known git worktree roots to avoid repeated filesystem walks.
    git_root_cache: Vec<PathBuf>,
}

impl TurnDiffTracker {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Front-run apply patch calls to track the starting contents of any modified files.
    /// - Creates an in-memory baseline snapshot for files that already exist on disk when first seen.
    /// - For additions, we intentionally do not create a baseline snapshot so that diffs are proper additions.
    /// - Also updates internal mappings for move/rename events.
    pub(crate) fn on_patch_begin(&mut self, changes: &HashMap<PathBuf, FileChange>) {
        for (path, change) in changes.iter() {
            // Ensure a stable internal filename exists for this external path.
            if !self.external_to_temp_name.contains_key(path) {
                let internal = Uuid::new_v4().to_string();
                self.external_to_temp_name
                    .insert(path.clone(), internal.clone());
                self.temp_name_to_current_path
                    .insert(internal.clone(), path.clone());

                // If the file exists on disk now, snapshot as baseline; else leave missing to represent /dev/null.
                let baseline_file_info = if path.exists() {
                    let mode = file_mode_for_path(path);
                    let mode_val = mode.unwrap_or(FileMode::Regular);
                    let content = blob_bytes(path, mode_val).unwrap_or_default();
                    let oid = if mode == Some(FileMode::Symlink) {
                        format!("{:x}", git_blob_sha1_hex_bytes(&content))
                    } else {
                        self.git_blob_oid_for_path(path)
                            .unwrap_or_else(|| format!("{:x}", git_blob_sha1_hex_bytes(&content)))
                    };
                    Some(BaselineFileInfo {
                        path: path.clone(),
                        content,
                        mode: mode_val,
                        oid,
                    })
                } else {
                    Some(BaselineFileInfo {
                        path: path.clone(),
                        content: vec![],
                        mode: FileMode::Regular,
                        oid: ZERO_OID.to_string(),
                    })
                };

                if let Some(baseline_file_info) = baseline_file_info {
                    self.baseline_file_info
                        .insert(internal.clone(), baseline_file_info);
                }
            }

            // Track rename/move in current mapping if provided in an Update.
            if let FileChange::Update {
                move_path: Some(dest),
                ..
            } = change
            {
                let uuid_filename = match self.external_to_temp_name.get(path) {
                    Some(i) => i.clone(),
                    None => {
                        // This should be rare, but if we haven't mapped the source, create it with no baseline.
                        let i = Uuid::new_v4().to_string();
                        self.baseline_file_info.insert(
                            i.clone(),
                            BaselineFileInfo {
                                path: path.clone(),
                                content: vec![],
                                mode: FileMode::Regular,
                                oid: ZERO_OID.to_string(),
                            },
                        );
                        i
                    }
                };
                // Update current external mapping for temp file name.
                self.temp_name_to_current_path
                    .insert(uuid_filename.clone(), dest.clone());
                // Update forward file_mapping: external current -> internal name.
                self.external_to_temp_name.remove(path);
                self.external_to_temp_name
                    .insert(dest.clone(), uuid_filename);
            };
        }
    }

    fn get_path_for_internal(&self, internal: &str) -> Option<PathBuf> {
        self.temp_name_to_current_path
            .get(internal)
            .cloned()
            .or_else(|| {
                self.baseline_file_info
                    .get(internal)
                    .map(|info| info.path.clone())
            })
    }

    /// Find the git worktree root for a file/directory by walking up to the first ancestor containing a `.git` entry.
    /// Uses a simple cache of known roots and avoids negative-result caching for simplicity.
    fn find_git_root_cached(&mut self, start: &Path) -> Option<PathBuf> {
        let dir = if start.is_dir() {
            start
        } else {
            start.parent()?
        };

        // Fast path: if any cached root is an ancestor of this path, use it.
        if let Some(root) = self
            .git_root_cache
            .iter()
            .find(|r| dir.starts_with(r))
            .cloned()
        {
            return Some(root);
        }

        // Walk up to find a `.git` marker.
        let mut cur = dir.to_path_buf();
        loop {
            let git_marker = cur.join(".git");
            if git_marker.is_dir() || git_marker.is_file() {
                if !self.git_root_cache.iter().any(|r| r == &cur) {
                    self.git_root_cache.push(cur.clone());
                }
                return Some(cur);
            }

            // On Windows, avoid walking above the drive or UNC share root.
            #[cfg(windows)]
            {
                if is_windows_drive_or_unc_root(&cur) {
                    return None;
                }
            }

            if let Some(parent) = cur.parent() {
                cur = parent.to_path_buf();
            } else {
                return None;
            }
        }
    }

    /// Return a display string for `path` relative to its git root if found, else absolute.
    fn relative_to_git_root_str(&mut self, path: &Path) -> String {
        let s = if let Some(root) = self.find_git_root_cached(path) {
            if let Ok(rel) = path.strip_prefix(&root) {
                rel.display().to_string()
            } else {
                path.display().to_string()
            }
        } else {
            path.display().to_string()
        };
        s.replace('\\', "/")
    }

    /// Ask git to compute the blob SHA-1 for the file at `path` within its repository.
    /// Returns None if no repository is found or git invocation fails.
    fn git_blob_oid_for_path(&mut self, path: &Path) -> Option<String> {
        let root = self.find_git_root_cached(path)?;
        // Compute a path relative to the repo root for better portability across platforms.
        let rel = path.strip_prefix(&root).unwrap_or(path);
        let output = Command::new("git")
            .arg("-C")
            .arg(&root)
            .arg("hash-object")
            .arg("--")
            .arg(rel)
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if s.len() == 40 { Some(s) } else { None }
    }

    /// Recompute the aggregated unified diff by comparing all of the in-memory snapshots that were
    /// collected before the first time they were touched by apply_patch during this turn with
    /// the current repo state.
    pub(crate) fn get_unified_diff(&mut self) -> Result<Option<String>> {
        let mut aggregated = String::new();

        // Compute diffs per tracked internal file in a stable order by external path.
        let mut baseline_file_names: Vec<String> =
            self.baseline_file_info.keys().cloned().collect();
        // Sort lexicographically by full repo-relative path to match git behavior.
        baseline_file_names.sort_by_key(|internal| {
            self.get_path_for_internal(internal)
                .map(|p| self.relative_to_git_root_str(&p))
                .unwrap_or_default()
        });

        for internal in baseline_file_names {
            aggregated.push_str(self.get_file_diff(&internal).as_str());
            if !aggregated.ends_with('\n') {
                aggregated.push('\n');
            }
        }

        if aggregated.trim().is_empty() {
            Ok(None)
        } else {
            Ok(Some(aggregated))
        }
    }

    fn get_file_diff(&mut self, internal_file_name: &str) -> String {
        let mut aggregated = String::new();

        // Snapshot lightweight fields only.
        let (baseline_external_path, baseline_mode, left_oid) = {
            if let Some(info) = self.baseline_file_info.get(internal_file_name) {
                (info.path.clone(), info.mode, info.oid.clone())
            } else {
                (PathBuf::new(), FileMode::Regular, ZERO_OID.to_string())
            }
        };
        let current_external_path = match self.get_path_for_internal(internal_file_name) {
            Some(p) => p,
            None => return aggregated,
        };

        let current_mode = file_mode_for_path(&current_external_path).unwrap_or(FileMode::Regular);
        let right_bytes = blob_bytes(&current_external_path, current_mode);

        // Compute displays with &mut self before borrowing any baseline content.
        let left_display = self.relative_to_git_root_str(&baseline_external_path);
        let right_display = self.relative_to_git_root_str(&current_external_path);

        // Compute right oid before borrowing baseline content.
        let right_oid = if let Some(b) = right_bytes.as_ref() {
            if current_mode == FileMode::Symlink {
                format!("{:x}", git_blob_sha1_hex_bytes(b))
            } else {
                self.git_blob_oid_for_path(&current_external_path)
                    .unwrap_or_else(|| format!("{:x}", git_blob_sha1_hex_bytes(b)))
            }
        } else {
            ZERO_OID.to_string()
        };

        // Borrow baseline content only after all &mut self uses are done.
        let left_present = left_oid.as_str() != ZERO_OID;
        let left_bytes: Option<&[u8]> = if left_present {
            self.baseline_file_info
                .get(internal_file_name)
                .map(|i| i.content.as_slice())
        } else {
            None
        };

        // Fast path: identical bytes or both missing.
        if left_bytes == right_bytes.as_deref() {
            return aggregated;
        }

        aggregated.push_str(&format!("diff --git a/{left_display} b/{right_display}\n"));

        let is_add = !left_present && right_bytes.is_some();
        let is_delete = left_present && right_bytes.is_none();

        if is_add {
            aggregated.push_str(&format!("new file mode {current_mode}\n"));
        } else if is_delete {
            aggregated.push_str(&format!("deleted file mode {baseline_mode}\n"));
        } else if baseline_mode != current_mode {
            aggregated.push_str(&format!("old mode {baseline_mode}\n"));
            aggregated.push_str(&format!("new mode {current_mode}\n"));
        }

        let left_text = left_bytes.and_then(|b| std::str::from_utf8(b).ok());
        let right_text = right_bytes
            .as_deref()
            .and_then(|b| std::str::from_utf8(b).ok());

        let can_text_diff = matches!(
            (left_text, right_text, is_add, is_delete),
            (Some(_), Some(_), _, _) | (_, Some(_), true, _) | (Some(_), _, _, true)
        );

        if can_text_diff {
            let l = left_text.unwrap_or("");
            let r = right_text.unwrap_or("");

            aggregated.push_str(&format!("index {left_oid}..{right_oid}\n"));

            let old_header = if left_present {
                format!("a/{left_display}")
            } else {
                DEV_NULL.to_string()
            };
            let new_header = if right_bytes.is_some() {
                format!("b/{right_display}")
            } else {
                DEV_NULL.to_string()
            };

            let diff = similar::TextDiff::from_lines(l, r);
            let unified = diff
                .unified_diff()
                .context_radius(3)
                .header(&old_header, &new_header)
                .to_string();

            aggregated.push_str(&unified);
        } else {
            aggregated.push_str(&format!("index {left_oid}..{right_oid}\n"));
            let old_header = if left_present {
                format!("a/{left_display}")
            } else {
                DEV_NULL.to_string()
            };
            let new_header = if right_bytes.is_some() {
                format!("b/{right_display}")
            } else {
                DEV_NULL.to_string()
            };
            aggregated.push_str(&format!("--- {old_header}\n"));
            aggregated.push_str(&format!("+++ {new_header}\n"));
            aggregated.push_str("Binary files differ\n");
        }
        aggregated
    }
}

/// Compute the Git SHA-1 blob object ID for the given content (bytes).
fn git_blob_sha1_hex_bytes(data: &[u8]) -> Output<sha1::Sha1> {
    // Git blob hash is sha1 of: "blob <len>\0<data>"
    let header = format!("blob {}\0", data.len());
    use sha1::Digest;
    let mut hasher = sha1::Sha1::new();
    hasher.update(header.as_bytes());
    hasher.update(data);
    hasher.finalize()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FileMode {
    Regular,
    #[cfg(unix)]
    Executable,
    Symlink,
}

impl FileMode {
    fn as_str(self) -> &'static str {
        match self {
            FileMode::Regular => "100644",
            #[cfg(unix)]
            FileMode::Executable => "100755",
            FileMode::Symlink => "120000",
        }
    }
}

impl std::fmt::Display for FileMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(unix)]
fn file_mode_for_path(path: &Path) -> Option<FileMode> {
    use std::os::unix::fs::PermissionsExt;
    let meta = fs::symlink_metadata(path).ok()?;
    let ft = meta.file_type();
    if ft.is_symlink() {
        return Some(FileMode::Symlink);
    }
    let mode = meta.permissions().mode();
    let is_exec = (mode & 0o111) != 0;
    Some(if is_exec {
        FileMode::Executable
    } else {
        FileMode::Regular
    })
}

#[cfg(not(unix))]
fn file_mode_for_path(_path: &Path) -> Option<FileMode> {
    // Default to non-executable on non-unix.
    Some(FileMode::Regular)
}

fn blob_bytes(path: &Path, mode: FileMode) -> Option<Vec<u8>> {
    if path.exists() {
        let contents = if mode == FileMode::Symlink {
            symlink_blob_bytes(path)
                .ok_or_else(|| anyhow!("failed to read symlink target for {}", path.display()))
        } else {
            fs::read(path)
                .with_context(|| format!("failed to read current file for diff {}", path.display()))
        };
        contents.ok()
    } else {
        None
    }
}

#[cfg(unix)]
fn symlink_blob_bytes(path: &Path) -> Option<Vec<u8>> {
    use std::os::unix::ffi::OsStrExt;
    let target = std::fs::read_link(path).ok()?;
    Some(target.as_os_str().as_bytes().to_vec())
}

#[cfg(not(unix))]
fn symlink_blob_bytes(_path: &Path) -> Option<Vec<u8>> {
    None
}

#[cfg(windows)]
fn is_windows_drive_or_unc_root(p: &std::path::Path) -> bool {
    use std::path::Component;
    let mut comps = p.components();
    matches!(
        (comps.next(), comps.next(), comps.next()),
        (Some(Component::Prefix(_)), Some(Component::RootDir), None)
    )
}

#[cfg(test)]
#[path = "turn_diff_tracker_tests.rs"]
mod tests;
