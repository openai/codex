use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use uuid::Uuid;

use crate::protocol::FileChange;

/// Tracks sets of changes to files and exposes the overall unified diff.
/// Internally, the way this works is now:
/// 1. Maintain an in-memory baseline snapshot of files when they are first seen.
///    For new additions, do not create a baseline so that diffs are shown as proper additions (using /dev/null).
/// 2. Keep a stable internal filename (uuid + same extension) per external path for rename tracking.
/// 3. To compute the aggregated unified diff, compare each baseline snapshot to the current file on disk entirely in-memory
///    using the `similar` crate and emit unified diffs with rewritten external paths.
#[derive(Default)]
pub struct TurnDiffTracker {
    /// Map external path -> internal filename (uuid + same extension).
    external_to_temp_name: HashMap<PathBuf, String>,
    /// Internal filename -> external path as of baseline snapshot.
    temp_name_to_baseline_external: HashMap<String, PathBuf>,
    /// Internal filename -> external path as of current accumulated state (after applying all changes).
    /// This is where renames are tracked.
    temp_name_to_current_external: HashMap<String, PathBuf>,
    /// Internal filename -> baseline file contents (None means the file did not exist, i.e. /dev/null).
    baseline_contents: HashMap<String, Option<String>>,
    /// Internal filename -> baseline file mode (100644 or 100755). Only set when baseline file existed.
    baseline_mode: HashMap<String, String>,
    /// Aggregated unified diff for all accumulated changes across files.
    pub unified_diff: Option<String>,
    /// Cache of known git worktree roots to avoid repeated filesystem walks.
    git_root_cache: Vec<PathBuf>,
}

impl TurnDiffTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Front-run apply patch calls to track the starting contents of any modified files.
    /// - Creates an in-memory baseline snapshot for files that already exist on disk when first seen.
    /// - For additions, we intentionally do not create a baseline snapshot so that diffs are proper additions.
    /// - Also updates internal mappings for move/rename events.
    pub fn on_patch_begin(&mut self, changes: &HashMap<PathBuf, FileChange>) -> Result<()> {
        for (path, change) in changes.iter() {
            // Ensure a stable internal filename exists for this external path.
            if !self.external_to_temp_name.contains_key(path) {
                let internal = uuid_filename_for(path);
                self.external_to_temp_name
                    .insert(path.clone(), internal.clone());
                self.temp_name_to_baseline_external
                    .insert(internal.clone(), path.clone());
                self.temp_name_to_current_external
                    .insert(internal.clone(), path.clone());

                // If the file exists on disk now, snapshot as baseline; else leave missing to represent /dev/null.
                let baseline = if path.exists() {
                    let contents = fs::read(path)
                        .with_context(|| format!("failed to read original {}", path.display()))?;
                    // Capture baseline mode for later file mode lines.
                    if let Some(mode) = file_mode_for_path(path) {
                        self.baseline_mode.insert(internal.clone(), mode);
                    }
                    Some(String::from_utf8_lossy(&contents).into_owned())
                } else {
                    None
                };
                self.baseline_contents.insert(internal.clone(), baseline);
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
                        let i = uuid_filename_for(path);
                        self.external_to_temp_name.insert(path.clone(), i.clone());
                        self.temp_name_to_baseline_external
                            .insert(i.clone(), path.clone());
                        // No on-disk file read here; treat as addition.
                        self.baseline_contents.insert(i.clone(), None);
                        i
                    }
                };
                // Update current external mapping for temp file name.
                self.temp_name_to_current_external
                    .insert(uuid_filename.clone(), dest.clone());
                // Update forward file_mapping: external current -> internal name.
                self.external_to_temp_name.remove(path);
                self.external_to_temp_name
                    .insert(dest.clone(), uuid_filename);
            };
        }

        Ok(())
    }

    fn get_path_for_internal(&self, internal: &str) -> Option<PathBuf> {
        self.temp_name_to_current_external
            .get(internal)
            .or_else(|| self.temp_name_to_baseline_external.get(internal))
            .cloned()
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
            if let Some(parent) = cur.parent() {
                cur = parent.to_path_buf();
            } else {
                return None;
            }
        }
    }

    /// Return a display string for `path` relative to its git root if found, else absolute.
    fn relative_to_git_root_str(&mut self, path: &Path) -> String {
        if let Some(root) = self.find_git_root_cached(path) {
            if let Ok(rel) = path.strip_prefix(&root) {
                return rel.display().to_string();
            }
        }
        path.display().to_string()
    }

    /// Recompute the aggregated unified diff by comparing all of the in-memory snapshots that were
    /// collected before the first time they were touched by apply_patch during this turn with
    /// the current repo state.
    pub fn get_unified_diff(&mut self) -> Result<Option<String>> {
        let mut aggregated = String::new();

        // Compute diffs per tracked internal file in a stable order by external path.
        let mut internals: Vec<String> = self
            .temp_name_to_baseline_external
            .keys()
            .cloned()
            .collect();
        // Sort lexicographically by external path to match git behavior.
        internals.sort_by_key(|a| {
            let path = self.get_path_for_internal(a);
            match path {
                Some(p) => p
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_owned())
                    .unwrap_or_default(),
                None => String::new(),
            }
        });

        for internal in internals {
            // Baseline external must exist for any tracked internal.
            let baseline_external = match self.temp_name_to_baseline_external.get(&internal) {
                Some(p) => p.clone(),
                None => continue,
            };
            let current_external = match self.get_path_for_internal(&internal) {
                Some(p) => p,
                None => continue,
            };

            let left_content = self
                .baseline_contents
                .get(&internal)
                .cloned()
                .unwrap_or(None);

            let right_content = if current_external.exists() {
                let contents = fs::read(&current_external).with_context(|| {
                    format!(
                        "failed to read current file for diff {}",
                        current_external.display()
                    )
                })?;
                Some(String::from_utf8_lossy(&contents).into_owned())
            } else {
                None
            };

            let left_text = left_content.as_deref().unwrap_or("");
            let right_text = right_content.as_deref().unwrap_or("");

            if left_text == right_text {
                continue;
            }

            let left_display = self.relative_to_git_root_str(&baseline_external);
            let right_display = self.relative_to_git_root_str(&current_external);

            // Diff the contents.
            let diff = similar::TextDiff::from_lines(left_text, right_text);

            // Emit a git-style header for better readability and parity with previous behavior.
            aggregated.push_str(&format!("diff --git a/{left_display} b/{right_display}\n"));

            // Emit file mode lines and index line similar to git.
            let is_add = left_content.is_none() && right_content.is_some();
            let is_delete = left_content.is_some() && right_content.is_none();

            // Determine modes.
            let baseline_mode = self
                .baseline_mode
                .get(&internal)
                .cloned()
                .unwrap_or_else(|| "100644".to_string());
            let current_mode =
                file_mode_for_path(&current_external).unwrap_or_else(|| "100644".to_string());

            if is_add {
                aggregated.push_str(&format!("new file mode {current_mode}\n"));
            } else if is_delete {
                aggregated.push_str(&format!("deleted file mode {baseline_mode}\n"));
            } else if baseline_mode != current_mode {
                aggregated.push_str(&format!("old mode {baseline_mode}\n"));
                aggregated.push_str(&format!("new mode {current_mode}\n"));
            }

            aggregated.push_str(&format!("index {ZERO_OID}..{ZERO_OID}\n"));

            let old_header = if left_content.is_some() {
                format!("a/{left_display}")
            } else {
                "/dev/null".to_string()
            };
            let new_header = if right_content.is_some() {
                format!("b/{right_display}")
            } else {
                "/dev/null".to_string()
            };

            let unified = diff
                .unified_diff()
                .context_radius(3)
                .header(&old_header, &new_header)
                .to_string();

            aggregated.push_str(&unified);
            if !aggregated.ends_with('\n') {
                aggregated.push('\n');
            }
        }

        self.unified_diff = if aggregated.trim().is_empty() {
            None
        } else {
            Some(aggregated)
        };

        Ok(self.unified_diff.clone())
    }
}

fn uuid_filename_for(path: &Path) -> String {
    let id = Uuid::new_v4().to_string();
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext) if !ext.is_empty() => format!("{id}.{ext}"),
        _ => id,
    }
}

const ZERO_OID: &str = "0000000000000000000000000000000000000000";

fn file_mode_for_path(path: &Path) -> Option<String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let meta = fs::metadata(path).ok()?;
        let mode = meta.permissions().mode();
        let is_exec = (mode & 0o111) != 0;
        Some(if is_exec {
            "100755".to_string()
        } else {
            "100644".to_string()
        })
    }
    #[cfg(not(unix))]
    {
        // Default to non-executable on non-unix.
        Some("100644".to_string())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;

    fn normalize_diff_for_test(input: &str, root: &Path) -> String {
        let root_str = root.display().to_string();
        let replaced = input.replace(&root_str, "<TMP>");
        // Split into blocks on lines starting with "diff --git ", sort blocks for determinism, and rejoin
        let mut blocks: Vec<String> = Vec::new();
        let mut current = String::new();
        for line in replaced.lines() {
            if line.starts_with("diff --git ") && !current.is_empty() {
                blocks.push(current);
                current = String::new();
            }
            if !current.is_empty() {
                current.push('\n');
            }
            current.push_str(line);
        }
        if !current.is_empty() {
            blocks.push(current);
        }
        blocks.sort();
        let mut out = blocks.join("\n");
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out
    }

    #[test]
    fn accumulates_add_and_update() {
        let mut acc = TurnDiffTracker::new();

        let dir = tempdir().unwrap();
        let file = dir.path().join("a.txt");

        // First patch: add file (baseline should be /dev/null).
        let add_changes = HashMap::from([(
            file.clone(),
            FileChange::Add {
                content: "foo\n".to_string(),
            },
        )]);
        acc.on_patch_begin(&add_changes).unwrap();

        // Simulate apply: create the file on disk.
        fs::write(&file, "foo\n").unwrap();
        acc.get_unified_diff().unwrap();
        let first = acc.unified_diff.clone().unwrap();
        let first = normalize_diff_for_test(&first, dir.path());
        let expected_first = {
            let mode = file_mode_for_path(&file).unwrap_or_else(|| "100644".to_string());
            format!(
                "diff --git a/<TMP>/a.txt b/<TMP>/a.txt\nnew file mode {mode}\nindex {ZERO_OID}..{ZERO_OID}\n--- /dev/null\n+++ b/<TMP>/a.txt\n@@ -0,0 +1 @@\n+foo\n",
            )
        };
        assert_eq!(first, expected_first);

        // Second patch: update the file on disk.
        let update_changes = HashMap::from([(
            file.clone(),
            FileChange::Update {
                unified_diff: "".to_owned(),
                move_path: None,
            },
        )]);
        acc.on_patch_begin(&update_changes).unwrap();

        // Simulate apply: append a new line.
        fs::write(&file, "foo\nbar\n").unwrap();
        acc.get_unified_diff().unwrap();
        let combined = acc.unified_diff.clone().unwrap();
        let combined = normalize_diff_for_test(&combined, dir.path());
        let expected_combined = {
            let mode = file_mode_for_path(&file).unwrap_or_else(|| "100644".to_string());
            format!(
                "diff --git a/<TMP>/a.txt b/<TMP>/a.txt\nnew file mode {mode}\nindex {ZERO_OID}..{ZERO_OID}\n--- /dev/null\n+++ b/<TMP>/a.txt\n@@ -0,0 +1,2 @@\n+foo\n+bar\n",
            )
        };
        assert_eq!(combined, expected_combined);
    }

    #[test]
    fn accumulates_delete() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("b.txt");
        fs::write(&file, "x\n").unwrap();

        let mut acc = TurnDiffTracker::new();
        let del_changes = HashMap::from([(file.clone(), FileChange::Delete)]);
        acc.on_patch_begin(&del_changes).unwrap();

        // Simulate apply: delete the file from disk.
        let baseline_mode = file_mode_for_path(&file).unwrap_or_else(|| "100644".to_string());
        fs::remove_file(&file).unwrap();
        acc.get_unified_diff().unwrap();
        let diff = acc.unified_diff.clone().unwrap();
        let diff = normalize_diff_for_test(&diff, dir.path());
        let expected = format!(
            "diff --git a/<TMP>/b.txt b/<TMP>/b.txt\ndeleted file mode {baseline_mode}\nindex {ZERO_OID}..{ZERO_OID}\n--- a/<TMP>/b.txt\n+++ /dev/null\n@@ -1 +0,0 @@\n-x\n",
        );
        assert_eq!(diff, expected);
    }

    #[test]
    fn accumulates_move_and_update() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src.txt");
        let dest = dir.path().join("dst.txt");
        fs::write(&src, "line\n").unwrap();

        let mut acc = TurnDiffTracker::new();
        let mv_changes = HashMap::from([(
            src.clone(),
            FileChange::Update {
                unified_diff: "".to_owned(),
                move_path: Some(dest.clone()),
            },
        )]);
        acc.on_patch_begin(&mv_changes).unwrap();

        // Simulate apply: move and update content.
        fs::rename(&src, &dest).unwrap();
        fs::write(&dest, "line2\n").unwrap();

        acc.get_unified_diff().unwrap();
        let out = acc.unified_diff.clone().unwrap();
        let out = normalize_diff_for_test(&out, dir.path());
        let expected = {
            format!(
                "diff --git a/<TMP>/src.txt b/<TMP>/dst.txt\nindex {ZERO_OID}..{ZERO_OID}\n--- a/<TMP>/src.txt\n+++ b/<TMP>/dst.txt\n@@ -1 +1 @@\n-line\n+line2\n"
            )
        };
        assert_eq!(out, expected);
    }

    #[test]
    fn update_persists_across_new_baseline_for_new_file() {
        let dir = tempdir().unwrap();
        let a = dir.path().join("a.txt");
        let b = dir.path().join("b.txt");
        fs::write(&a, "foo\n").unwrap();
        fs::write(&b, "z\n").unwrap();

        let mut acc = TurnDiffTracker::new();

        // First: update existing a.txt (baseline snapshot is created for a).
        let update_a = HashMap::from([(
            a.clone(),
            FileChange::Update {
                unified_diff: "".to_owned(),
                move_path: None,
            },
        )]);
        acc.on_patch_begin(&update_a).unwrap();
        // Simulate apply: modify a.txt on disk.
        fs::write(&a, "foo\nbar\n").unwrap();
        acc.get_unified_diff().unwrap();
        let first = acc.unified_diff.clone().unwrap();
        let first = normalize_diff_for_test(&first, dir.path());
        let expected_first = {
            format!(
                "diff --git a/<TMP>/a.txt b/<TMP>/a.txt\nindex {ZERO_OID}..{ZERO_OID}\n--- a/<TMP>/a.txt\n+++ b/<TMP>/a.txt\n@@ -1 +1,2 @@\n foo\n+bar\n"
            )
        };
        assert_eq!(first, expected_first);

        // Next: introduce a brand-new path b.txt into baseline snapshots via a delete change.
        let del_b = HashMap::from([(b.clone(), FileChange::Delete)]);
        acc.on_patch_begin(&del_b).unwrap();
        // Simulate apply: delete b.txt.
        let baseline_mode = file_mode_for_path(&b).unwrap_or_else(|| "100644".to_string());
        fs::remove_file(&b).unwrap();
        acc.get_unified_diff().unwrap();

        let combined = acc.unified_diff.clone().unwrap();
        let combined = normalize_diff_for_test(&combined, dir.path());
        let expected = {
            format!(
                "diff --git a/<TMP>/a.txt b/<TMP>/a.txt\nindex {ZERO_OID}..{ZERO_OID}\n--- a/<TMP>/a.txt\n+++ b/<TMP>/a.txt\n@@ -1 +1,2 @@\n foo\n+bar\n\
                diff --git a/<TMP>/b.txt b/<TMP>/b.txt\ndeleted file mode {baseline_mode}\nindex {ZERO_OID}..{ZERO_OID}\n--- a/<TMP>/b.txt\n+++ /dev/null\n@@ -1 +0,0 @@\n-z\n",
            )
        };
        assert_eq!(combined, expected);
    }
}
