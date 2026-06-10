use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use sha1::digest::Output;

use codex_apply_patch::AppliedPatchChange;
use codex_apply_patch::AppliedPatchDelta;
use codex_apply_patch::AppliedPatchFileChange;

const ZERO_OID: &str = "0000000000000000000000000000000000000000";
const DEV_NULL: &str = "/dev/null";
const REGULAR_FILE_MODE: &str = "100644";
// Normal edits finish well within 100 ms; pathological inputs fall back to a coarse,
// content-exact diff without stalling tool completion.
const DIFF_TIMEOUT: Duration = Duration::from_millis(100);

struct TrackedContent {
    content: String,
    revision: u64,
}

#[derive(Eq, Hash, PartialEq)]
struct DiffCacheKey {
    left_path: PathBuf,
    left_revision: Option<u64>,
    right_path: PathBuf,
    right_revision: Option<u64>,
}

/// Tracks the net text diff for the current turn from committed apply_patch
/// mutations, without rereading the workspace filesystem.
pub struct TurnDiffTracker {
    valid: bool,
    display_root: Option<PathBuf>,
    baseline_by_path: HashMap<PathBuf, TrackedContent>,
    current_by_path: HashMap<PathBuf, TrackedContent>,
    origin_by_current_path: HashMap<PathBuf, PathBuf>,
    next_revision: u64,
    rendered_diffs: HashMap<DiffCacheKey, Option<String>>,
    unified_diff: Option<String>,
    #[cfg(test)]
    rendered_diff_count: std::cell::Cell<usize>,
}

impl Default for TurnDiffTracker {
    fn default() -> Self {
        Self {
            valid: true,
            display_root: None,
            baseline_by_path: HashMap::new(),
            current_by_path: HashMap::new(),
            origin_by_current_path: HashMap::new(),
            next_revision: 0,
            rendered_diffs: HashMap::new(),
            unified_diff: None,
            #[cfg(test)]
            rendered_diff_count: std::cell::Cell::new(0),
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

    pub fn track_delta(&mut self, delta: &AppliedPatchDelta) {
        if !self.valid {
            return;
        }

        if !delta.is_exact() {
            self.invalidate();
            return;
        }

        for change in delta.changes() {
            self.apply_change(change);
        }
        self.refresh_unified_diff();
    }

    pub fn invalidate(&mut self) {
        self.valid = false;
        self.rendered_diffs.clear();
        self.unified_diff = None;
    }

    pub fn get_unified_diff(&self) -> Option<String> {
        self.unified_diff.clone()
    }

    pub(crate) fn has_unified_diff(&self) -> bool {
        self.unified_diff.is_some()
    }

    fn refresh_unified_diff(&mut self) {
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

        let mut previous_diffs = std::mem::take(&mut self.rendered_diffs);
        let mut rendered_diffs = HashMap::new();
        let mut aggregated = String::new();
        for path in paths {
            if !handled.insert(path.clone()) {
                continue;
            }

            if paired_destinations.contains(&path) {
                continue;
            }

            let (left_path, right_path) = if let Some(dest) = rename_pairs.get(&path) {
                handled.insert(dest.clone());
                (path.as_path(), dest.as_path())
            } else {
                (path.as_path(), path.as_path())
            };

            let left_content = self.baseline_by_path.get(left_path);
            let right_content = self.current_by_path.get(right_path);
            let key = DiffCacheKey {
                left_path: left_path.to_path_buf(),
                left_revision: left_content.map(|content| content.revision),
                right_path: right_path.to_path_buf(),
                right_revision: right_content.map(|content| content.revision),
            };
            let rendered = previous_diffs.remove(&key).unwrap_or_else(|| {
                self.render_diff(
                    left_path,
                    left_content.map(|content| content.content.as_str()),
                    right_path,
                    right_content.map(|content| content.content.as_str()),
                )
            });

            if let Some(diff) = rendered.as_deref() {
                aggregated.push_str(diff);
                if !aggregated.ends_with('\n') {
                    aggregated.push('\n');
                }
            }
            rendered_diffs.insert(key, rendered);
        }

        self.rendered_diffs = rendered_diffs;
        self.unified_diff = (!aggregated.is_empty()).then_some(aggregated);
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
            let overwritten_content = self.tracked_content(overwritten_content);
            self.baseline_by_path
                .insert(path.to_path_buf(), overwritten_content);
        }
        let content = self.tracked_content(content);
        self.current_by_path.insert(path.to_path_buf(), content);
    }

    fn apply_delete(&mut self, path: &Path, content: &str) {
        if self.current_by_path.remove(path).is_none() && !self.baseline_by_path.contains_key(path)
        {
            let content = self.tracked_content(content);
            self.baseline_by_path.insert(path.to_path_buf(), content);
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
            let old_content = self.tracked_content(old_content);
            self.baseline_by_path
                .insert(source_path.to_path_buf(), old_content);
        }

        match move_path {
            Some(dest_path) => {
                if !self.current_by_path.contains_key(dest_path)
                    && !self.baseline_by_path.contains_key(dest_path)
                    && let Some(overwritten_move_content) = overwritten_move_content
                {
                    let overwritten_move_content = self.tracked_content(overwritten_move_content);
                    self.baseline_by_path
                        .insert(dest_path.to_path_buf(), overwritten_move_content);
                }
                let origin = self
                    .origin_by_current_path
                    .remove(source_path)
                    .unwrap_or_else(|| source_path.to_path_buf());
                self.current_by_path.remove(source_path);
                let new_content = self.tracked_content(new_content);
                self.current_by_path
                    .insert(dest_path.to_path_buf(), new_content);
                self.origin_by_current_path.remove(dest_path);
                if dest_path != origin.as_path() {
                    self.origin_by_current_path
                        .insert(dest_path.to_path_buf(), origin);
                }
            }
            None => {
                let new_content = self.tracked_content(new_content);
                self.current_by_path
                    .insert(source_path.to_path_buf(), new_content);
            }
        }
    }

    fn tracked_content(&mut self, content: &str) -> TrackedContent {
        let revision = self.next_revision;
        self.next_revision += 1;
        TrackedContent {
            content: content.to_string(),
            revision,
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

        #[cfg(test)]
        self.rendered_diff_count
            .set(self.rendered_diff_count.get() + 1);

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

        let mut config = similar::TextDiff::configure();
        config.timeout(DIFF_TIMEOUT);
        let unified = config
            .diff_lines(left_content.unwrap_or(""), right_content.unwrap_or(""))
            .unified_diff()
            .context_radius(3)
            .header(&old_header, &new_header)
            .to_string();
        diff.push_str(&unified);
        Some(diff)
    }

    #[cfg(test)]
    fn rendered_diff_count(&self) -> usize {
        self.rendered_diff_count.get()
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
