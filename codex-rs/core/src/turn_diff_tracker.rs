use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

use sha1::digest::Output;

use codex_apply_patch::AppliedPatchChange;
use codex_apply_patch::AppliedPatchDelta;
use codex_apply_patch::AppliedPatchFileChange;

const ZERO_OID: &str = "0000000000000000000000000000000000000000";
const DEV_NULL: &str = "/dev/null";
const REGULAR_FILE_MODE: &str = "100644";

/// Tracks the net text diff for the current turn from successful apply_patch
/// operations, without rereading the workspace filesystem.
pub struct TurnDiffTracker {
    valid: bool,
    display_root: Option<PathBuf>,
    baseline_by_path: HashMap<PathBuf, String>,
    current_by_path: HashMap<PathBuf, String>,
    origin_by_current_path: HashMap<PathBuf, PathBuf>,
}

impl Default for TurnDiffTracker {
    fn default() -> Self {
        Self {
            valid: true,
            display_root: None,
            baseline_by_path: HashMap::new(),
            current_by_path: HashMap::new(),
            origin_by_current_path: HashMap::new(),
        }
    }
}

impl TurnDiffTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_display_root(display_root: PathBuf) -> Self {
        let mut tracker = Self::new();
        tracker.display_root = Some(display_root);
        tracker
    }

    pub fn track_successful_patch(&mut self, delta: &AppliedPatchDelta) {
        if !delta.is_exact() {
            self.invalidate();
            return;
        }

        for change in delta.changes() {
            self.apply_change(change);
        }
    }

    pub fn invalidate(&mut self) {
        self.valid = false;
    }

    pub fn get_unified_diff(&self) -> Option<String> {
        if !self.valid {
            return None;
        }

        let rename_pairs = self.rename_pairs();
        let paired_destinations = rename_pairs.values().cloned().collect::<HashSet<_>>();
        let mut handled = HashSet::new();
        let mut paths = self
            .baseline_by_path
            .keys()
            .chain(self.current_by_path.keys())
            .cloned()
            .collect::<Vec<_>>();
        paths.sort_by_key(|path| self.display_path(path));
        paths.dedup();

        let mut aggregated = String::new();
        for path in paths {
            if !handled.insert(path.clone()) {
                continue;
            }

            if paired_destinations.contains(&path) {
                continue;
            }

            let diff = if let Some(dest) = rename_pairs.get(&path) {
                handled.insert(dest.clone());
                self.render_rename_diff(&path, dest)
            } else {
                self.render_path_diff(&path)
            };

            if let Some(diff) = diff {
                aggregated.push_str(&diff);
                if !aggregated.ends_with('\n') {
                    aggregated.push('\n');
                }
            }
        }

        (!aggregated.is_empty()).then_some(aggregated)
    }

    fn apply_change(&mut self, change: &AppliedPatchChange) {
        let source_path = change.path.as_path();
        match &change.change {
            AppliedPatchFileChange::Add {
                content,
                overwritten_content,
            } => self.apply_add(source_path, content, overwritten_content.as_deref()),
            AppliedPatchFileChange::Delete { content } => self.apply_delete(source_path, content),
            AppliedPatchFileChange::Update {
                move_path,
                old_content,
                overwritten_move_content,
                new_content,
            } => self.apply_update(
                source_path,
                move_path.as_deref(),
                old_content,
                overwritten_move_content.as_deref(),
                new_content,
            ),
        }
    }

    fn apply_add(&mut self, path: &Path, content: &str, overwritten_content: Option<&str>) {
        self.origin_by_current_path.remove(path);
        if !self.current_by_path.contains_key(path)
            && !self.baseline_by_path.contains_key(path)
            && let Some(overwritten_content) = overwritten_content
        {
            self.baseline_by_path
                .insert(path.to_path_buf(), overwritten_content.to_string());
        }
        self.current_by_path
            .insert(path.to_path_buf(), content.to_string());
    }

    fn apply_delete(&mut self, path: &Path, content: &str) {
        if self.current_by_path.remove(path).is_none() && !self.baseline_by_path.contains_key(path)
        {
            self.baseline_by_path
                .insert(path.to_path_buf(), content.to_string());
        }
        self.origin_by_current_path.remove(path);
    }

    fn apply_update(
        &mut self,
        source_path: &Path,
        move_path: Option<&Path>,
        old_content: &str,
        overwritten_move_content: Option<&str>,
        new_content: &str,
    ) {
        if !self.current_by_path.contains_key(source_path)
            && !self.baseline_by_path.contains_key(source_path)
        {
            self.baseline_by_path
                .insert(source_path.to_path_buf(), old_content.to_string());
        }

        match move_path {
            Some(dest_path) => {
                if !self.current_by_path.contains_key(dest_path)
                    && !self.baseline_by_path.contains_key(dest_path)
                    && let Some(overwritten_move_content) = overwritten_move_content
                {
                    self.baseline_by_path.insert(
                        dest_path.to_path_buf(),
                        overwritten_move_content.to_string(),
                    );
                }
                let origin = self
                    .origin_by_current_path
                    .remove(source_path)
                    .unwrap_or_else(|| source_path.to_path_buf());
                self.current_by_path.remove(source_path);
                self.current_by_path
                    .insert(dest_path.to_path_buf(), new_content.to_string());
                self.origin_by_current_path.remove(dest_path);
                if dest_path != origin.as_path() {
                    self.origin_by_current_path
                        .insert(dest_path.to_path_buf(), origin);
                }
            }
            None => {
                self.current_by_path
                    .insert(source_path.to_path_buf(), new_content.to_string());
            }
        }
    }

    fn rename_pairs(&self) -> HashMap<PathBuf, PathBuf> {
        self.origin_by_current_path
            .iter()
            .filter_map(|(dest_path, origin_path)| {
                if dest_path == origin_path
                    || self.current_by_path.contains_key(origin_path)
                    || !self.current_by_path.contains_key(dest_path)
                    || !self.baseline_by_path.contains_key(origin_path)
                    || self.baseline_by_path.contains_key(dest_path)
                {
                    return None;
                }

                Some((origin_path.clone(), dest_path.clone()))
            })
            .collect()
    }

    fn render_path_diff(&self, path: &Path) -> Option<String> {
        self.render_diff(
            path,
            self.baseline_by_path.get(path).map(String::as_str),
            path,
            self.current_by_path.get(path).map(String::as_str),
        )
    }

    fn render_rename_diff(&self, source_path: &Path, dest_path: &Path) -> Option<String> {
        self.render_diff(
            source_path,
            self.baseline_by_path.get(source_path).map(String::as_str),
            dest_path,
            self.current_by_path.get(dest_path).map(String::as_str),
        )
    }

    fn render_diff(
        &self,
        left_path: &Path,
        left_content: Option<&str>,
        right_path: &Path,
        right_content: Option<&str>,
    ) -> Option<String> {
        if left_content == right_content {
            return None;
        }

        let left_display = self.display_path(left_path);
        let right_display = self.display_path(right_path);
        let left_oid = left_content.map_or_else(
            || ZERO_OID.to_string(),
            |content| git_blob_oid(content.as_bytes()),
        );
        let right_oid = right_content.map_or_else(
            || ZERO_OID.to_string(),
            |content| git_blob_oid(content.as_bytes()),
        );

        let mut diff = format!("diff --git a/{left_display} b/{right_display}\n");
        match (left_content, right_content) {
            (None, Some(_)) => diff.push_str(&format!("new file mode {REGULAR_FILE_MODE}\n")),
            (Some(_), None) => diff.push_str(&format!("deleted file mode {REGULAR_FILE_MODE}\n")),
            (Some(_), Some(_)) => {}
            (None, None) => return None,
        }

        diff.push_str(&format!("index {left_oid}..{right_oid}\n"));

        let old_header = if left_content.is_some() {
            format!("a/{left_display}")
        } else {
            DEV_NULL.to_string()
        };
        let new_header = if right_content.is_some() {
            format!("b/{right_display}")
        } else {
            DEV_NULL.to_string()
        };

        let unified =
            similar::TextDiff::from_lines(left_content.unwrap_or(""), right_content.unwrap_or(""))
                .unified_diff()
                .context_radius(3)
                .header(&old_header, &new_header)
                .to_string();
        diff.push_str(&unified);
        Some(diff)
    }

    fn display_path(&self, path: &Path) -> String {
        let display = self
            .display_root
            .as_deref()
            .and_then(|root| path.strip_prefix(root).ok())
            .unwrap_or(path);
        display.display().to_string().replace('\\', "/")
    }
}

fn git_blob_oid(data: &[u8]) -> String {
    format!("{:x}", git_blob_sha1_hex_bytes(data))
}

/// Compute the Git SHA-1 blob object ID for the given content (bytes).
fn git_blob_sha1_hex_bytes(data: &[u8]) -> Output<sha1::Sha1> {
    let header = format!("blob {}\0", data.len());
    use sha1::Digest;
    let mut hasher = sha1::Sha1::new();
    hasher.update(header.as_bytes());
    hasher.update(data);
    hasher.finalize()
}

#[cfg(test)]
#[path = "turn_diff_tracker_tests.rs"]
mod tests;
