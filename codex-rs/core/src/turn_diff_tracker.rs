use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use anyhow::Context;
use anyhow::Result;
use uuid::Uuid;

use crate::protocol::FileChange;

struct BaselineFileInfo {
    path: Option<PathBuf>,
    contents_bytes: Option<Vec<u8>>,
    mode: Option<String>,
    oid: Option<String>,
}

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
    /// Internal filename -> external path as of current accumulated state (after applying all changes).
    /// This is where renames are tracked.
    temp_name_to_current_external: HashMap<String, PathBuf>,
    /// Internal filename -> baseline file info.
    baseline_file_info: HashMap<String, BaselineFileInfo>,
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
                self.temp_name_to_current_external
                    .insert(internal.clone(), path.clone());

                // If the file exists on disk now, snapshot as baseline; else leave missing to represent /dev/null.
                let (contents_bytes, mode, oid) = if path.exists() {
                    let contents_bytes = fs::read(path)
                        .with_context(|| format!("failed to read original {}", path.display()))?;
                    let mode = file_mode_for_path(path);
                    let oid = self
                        .git_blob_oid_for_path(path)
                        .unwrap_or_else(|| git_blob_sha1_hex_bytes(&contents_bytes));
                    (Some(contents_bytes), mode, Some(oid))
                } else {
                    (None, None, Some(ZERO_OID.to_string()))
                };

                self.baseline_file_info.insert(
                    internal.clone(),
                    BaselineFileInfo {
                        path: Some(path.clone()),
                        contents_bytes,
                        mode,
                        oid,
                    },
                );
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
                        // No on-disk file read here; treat as addition.
                        self.baseline_file_info.insert(
                            i.clone(),
                            BaselineFileInfo {
                                path: Some(path.clone()),
                                contents_bytes: None,
                                mode: None,
                                oid: Some(ZERO_OID.to_string()),
                            },
                        );
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
            .cloned()
            .or_else(|| {
                self.baseline_file_info
                    .get(internal)
                    .and_then(|info| info.path.clone())
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
    pub fn get_unified_diff(&mut self) -> Result<Option<String>> {
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
            // Baseline external must exist for any tracked internal.
            let baseline_external = match self
                .baseline_file_info
                .get(&internal)
                .and_then(|i| i.path.clone())
            {
                Some(p) => p,
                None => continue,
            };
            let current_external = match self.get_path_for_internal(&internal) {
                Some(p) => p,
                None => continue,
            };

            let left_bytes = self
                .baseline_file_info
                .get(&internal)
                .and_then(|i| i.contents_bytes.clone());

            let right_bytes = if current_external.exists() {
                let contents = fs::read(&current_external).with_context(|| {
                    format!(
                        "failed to read current file for diff {}",
                        current_external.display()
                    )
                })?;
                Some(contents)
            } else {
                None
            };

            // Fast path: identical bytes or both missing.
            if left_bytes.as_deref() == right_bytes.as_deref() {
                continue;
            }

            let left_display = self.relative_to_git_root_str(&baseline_external);
            let right_display = self.relative_to_git_root_str(&current_external);

            // Emit a git-style header for better readability and parity with previous behavior.
            aggregated.push_str(&format!("diff --git a/{left_display} b/{right_display}\n"));

            let is_add = left_bytes.is_none() && right_bytes.is_some();
            let is_delete = left_bytes.is_some() && right_bytes.is_none();

            // Determine modes.
            let baseline_mode = self
                .baseline_file_info
                .get(&internal)
                .and_then(|i| i.mode.clone())
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

            // Determine blob object IDs for left and right contents. Prefer stored OIDs
            // captured from the original repo state when the change was first seen.
            let left_oid = self
                .baseline_file_info
                .get(&internal)
                .and_then(|i| i.oid.clone())
                .or_else(|| {
                    left_bytes
                        .as_ref()
                        .map(|b| git_blob_sha1_hex_bytes(b))
                        .or(Some(ZERO_OID.to_string()))
                })
                .unwrap_or_else(|| ZERO_OID.to_string());
            let right_oid = if let Some(b) = right_bytes.as_ref() {
                self.git_blob_oid_for_path(&current_external)
                    .unwrap_or_else(|| git_blob_sha1_hex_bytes(b))
            } else {
                ZERO_OID.to_string()
            };

            // If either side isn't valid UTF-8, emit a binary diff header and continue.
            let left_text = left_bytes
                .as_deref()
                .and_then(|b| std::str::from_utf8(b).ok());
            let right_text = right_bytes
                .as_deref()
                .and_then(|b| std::str::from_utf8(b).ok());

            // Prefer text diffs when possible:
            // - both sides are valid UTF-8
            // - OR one side is missing (add/delete) and the present side is valid UTF-8
            let can_text_diff = match (left_text, right_text, is_add, is_delete) {
                (Some(_), Some(_), _, _) => true,
                (_, Some(_), true, _) => true, // add: left missing, right text
                (Some(_), _, _, true) => true, // delete: left text, right missing
                _ => false,
            };

            if can_text_diff {
                // Diff the contents as text, treating missing side as empty string.
                let l = left_text.unwrap_or("");
                let r = right_text.unwrap_or("");

                // Emit index line without mode suffix to preserve current test expectations.
                aggregated.push_str(&format!("index {left_oid}..{right_oid}\n"));

                let old_header = if left_bytes.is_some() {
                    format!("a/{left_display}")
                } else {
                    "/dev/null".to_string()
                };
                let new_header = if right_bytes.is_some() {
                    format!("b/{right_display}")
                } else {
                    "/dev/null".to_string()
                };

                let diff = similar::TextDiff::from_lines(l, r);
                let unified = diff
                    .unified_diff()
                    .context_radius(3)
                    .header(&old_header, &new_header)
                    .to_string();

                aggregated.push_str(&unified);
                if !aggregated.ends_with('\n') {
                    aggregated.push('\n');
                }
            } else {
                // Binary or invalid UTF-8: emit header only.
                aggregated.push_str(&format!("index {left_oid}..{right_oid}\n"));
                let old_header = if left_bytes.is_some() {
                    format!("a/{left_display}")
                } else {
                    "/dev/null".to_string()
                };
                let new_header = if right_bytes.is_some() {
                    format!("b/{right_display}")
                } else {
                    "/dev/null".to_string()
                };
                aggregated.push_str(&format!("--- {old_header}\n"));
                aggregated.push_str(&format!("+++ {new_header}\n"));
                aggregated.push_str("Binary files differ\n");
                if !aggregated.ends_with('\n') {
                    aggregated.push('\n');
                }
            }
        }

        if aggregated.trim().is_empty() {
            Ok(None)
        } else {
            Ok(Some(aggregated))
        }
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

/// Compute the Git SHA-1 blob object ID for the given content (bytes).
fn git_blob_sha1_hex_bytes(data: &[u8]) -> String {
    // Git blob hash is sha1 of: "blob <len>\0<data>"
    let header = format!("blob {}\0", data.len());
    use sha1::Digest;
    let mut hasher = sha1::Sha1::new();
    hasher.update(header.as_bytes());
    hasher.update(data);
    let digest = hasher.finalize();
    let mut out = String::with_capacity(40);
    for b in digest {
        use std::fmt::Write;
        let _ = write!(&mut out, "{b:02x}");
    }
    out
}

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
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;

    /// Compute the Git SHA-1 blob object ID for the given content (string).
    /// This delegates to the bytes version to avoid UTF-8 lossy conversions here.
    fn git_blob_sha1_hex(data: &str) -> String {
        git_blob_sha1_hex_bytes(data.as_bytes())
    }

    fn normalize_diff_for_test(input: &str, root: &Path) -> String {
        let root_str = root.display().to_string().replace('\\', "/");
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
        let first = acc.get_unified_diff().unwrap().unwrap();
        let first = normalize_diff_for_test(&first, dir.path());
        let expected_first = {
            let mode = file_mode_for_path(&file).unwrap_or_else(|| "100644".to_string());
            let right_oid = git_blob_sha1_hex("foo\n");
            format!(
                "diff --git a/<TMP>/a.txt b/<TMP>/a.txt\nnew file mode {mode}\nindex {ZERO_OID}..{right_oid}\n--- /dev/null\n+++ b/<TMP>/a.txt\n@@ -0,0 +1 @@\n+foo\n",
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
        let combined = acc.get_unified_diff().unwrap().unwrap();
        let combined = normalize_diff_for_test(&combined, dir.path());
        let expected_combined = {
            let mode = file_mode_for_path(&file).unwrap_or_else(|| "100644".to_string());
            let right_oid = git_blob_sha1_hex("foo\nbar\n");
            format!(
                "diff --git a/<TMP>/a.txt b/<TMP>/a.txt\nnew file mode {mode}\nindex {ZERO_OID}..{right_oid}\n--- /dev/null\n+++ b/<TMP>/a.txt\n@@ -0,0 +1,2 @@\n+foo\n+bar\n",
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
        let diff = acc.get_unified_diff().unwrap().unwrap();
        let diff = normalize_diff_for_test(&diff, dir.path());
        let expected = {
            let left_oid = git_blob_sha1_hex("x\n");
            format!(
                "diff --git a/<TMP>/b.txt b/<TMP>/b.txt\ndeleted file mode {baseline_mode}\nindex {left_oid}..{ZERO_OID}\n--- a/<TMP>/b.txt\n+++ /dev/null\n@@ -1 +0,0 @@\n-x\n",
            )
        };
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

        let out = acc.get_unified_diff().unwrap().unwrap();
        let out = normalize_diff_for_test(&out, dir.path());
        let expected = {
            let left_oid = git_blob_sha1_hex("line\n");
            let right_oid = git_blob_sha1_hex("line2\n");
            format!(
                "diff --git a/<TMP>/src.txt b/<TMP>/dst.txt\nindex {left_oid}..{right_oid}\n--- a/<TMP>/src.txt\n+++ b/<TMP>/dst.txt\n@@ -1 +1 @@\n-line\n+line2\n"
            )
        };
        assert_eq!(out, expected);
    }

    #[test]
    fn move_without_content_change_yields_no_diff() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("moved.txt");
        let dest = dir.path().join("renamed.txt");
        fs::write(&src, "same\n").unwrap();

        let mut acc = TurnDiffTracker::new();
        let mv_changes = HashMap::from([(
            src.clone(),
            FileChange::Update {
                unified_diff: "".to_owned(),
                move_path: Some(dest.clone()),
            },
        )]);
        acc.on_patch_begin(&mv_changes).unwrap();

        // Simulate apply: move only, no content change.
        fs::rename(&src, &dest).unwrap();

        let diff = acc.get_unified_diff().unwrap();
        assert_eq!(diff, None);
    }

    #[test]
    fn move_declared_but_file_only_appears_at_dest_is_add() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src.txt");
        let dest = dir.path().join("dest.txt");
        let mut acc = TurnDiffTracker::new();
        let mv = HashMap::from([(
            src.clone(),
            FileChange::Update {
                unified_diff: "".into(),
                move_path: Some(dest.clone()),
            },
        )]);
        acc.on_patch_begin(&mv).unwrap();
        // No file existed initially; create only dest
        fs::write(&dest, "hello\n").unwrap();
        let diff = acc.get_unified_diff().unwrap().unwrap();
        assert!(diff.contains("new file mode"));
        assert!(diff.contains("--- /dev/null"));
        assert!(diff.contains("+++ b/"));
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
        let first = acc.get_unified_diff().unwrap().unwrap();
        let first = normalize_diff_for_test(&first, dir.path());
        let expected_first = {
            let left_oid = git_blob_sha1_hex("foo\n");
            let right_oid = git_blob_sha1_hex("foo\nbar\n");
            format!(
                "diff --git a/<TMP>/a.txt b/<TMP>/a.txt\nindex {left_oid}..{right_oid}\n--- a/<TMP>/a.txt\n+++ b/<TMP>/a.txt\n@@ -1 +1,2 @@\n foo\n+bar\n"
            )
        };
        assert_eq!(first, expected_first);

        // Next: introduce a brand-new path b.txt into baseline snapshots via a delete change.
        let del_b = HashMap::from([(b.clone(), FileChange::Delete)]);
        acc.on_patch_begin(&del_b).unwrap();
        // Simulate apply: delete b.txt.
        let baseline_mode = file_mode_for_path(&b).unwrap_or_else(|| "100644".to_string());
        fs::remove_file(&b).unwrap();

        let combined = acc.get_unified_diff().unwrap().unwrap();
        let combined = normalize_diff_for_test(&combined, dir.path());
        let expected = {
            let left_oid_a = git_blob_sha1_hex("foo\n");
            let right_oid_a = git_blob_sha1_hex("foo\nbar\n");
            let left_oid_b = git_blob_sha1_hex("z\n");
            format!(
                "diff --git a/<TMP>/a.txt b/<TMP>/a.txt\nindex {left_oid_a}..{right_oid_a}\n--- a/<TMP>/a.txt\n+++ b/<TMP>/a.txt\n@@ -1 +1,2 @@\n foo\n+bar\n\
                diff --git a/<TMP>/b.txt b/<TMP>/b.txt\ndeleted file mode {baseline_mode}\nindex {left_oid_b}..{ZERO_OID}\n--- a/<TMP>/b.txt\n+++ /dev/null\n@@ -1 +0,0 @@\n-z\n",
            )
        };
        assert_eq!(combined, expected);
    }

    #[test]
    fn binary_files_differ_update() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("bin.dat");

        // Initial non-UTF8 bytes
        let left_bytes: Vec<u8> = vec![0xff, 0xfe, 0xfd, 0x00];
        // Updated non-UTF8 bytes
        let right_bytes: Vec<u8> = vec![0x01, 0x02, 0x03, 0x00];

        fs::write(&file, &left_bytes).unwrap();

        let mut acc = TurnDiffTracker::new();
        let update_changes = HashMap::from([(
            file.clone(),
            FileChange::Update {
                unified_diff: "".to_owned(),
                move_path: None,
            },
        )]);
        acc.on_patch_begin(&update_changes).unwrap();

        // Apply update on disk
        fs::write(&file, &right_bytes).unwrap();

        let diff = acc.get_unified_diff().unwrap().unwrap();
        let diff = normalize_diff_for_test(&diff, dir.path());
        let expected = {
            let left_oid = git_blob_sha1_hex_bytes(&left_bytes);
            let right_oid = git_blob_sha1_hex_bytes(&right_bytes);
            format!(
                "diff --git a/<TMP>/bin.dat b/<TMP>/bin.dat\nindex {left_oid}..{right_oid}\n--- a/<TMP>/bin.dat\n+++ b/<TMP>/bin.dat\nBinary files differ\n"
            )
        };
        assert_eq!(diff, expected);
    }
}
