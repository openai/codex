use super::HistoryCell;
use crate::diff_render::create_diff_summary;
use codex_core::protocol::FileChange;
use ratatui::text::Line;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

/// File change summary showing patch metadata in diff card form.
///
/// Used after `/review` or patch application to list added, modified, and deleted files relative to
/// a working directory so users can skim the affected files and line counts before diving into the
/// full diff rendering.
///
/// # Output
///
/// ```plain
/// • Added src/lib.rs (+2 -0)
///     1 +fn main() {
///     2 +    println!("hi");
/// ```
#[derive(Debug)]
pub(crate) struct PatchHistoryCell {
    changes: HashMap<PathBuf, FileChange>,
    cwd: PathBuf,
}

impl PatchHistoryCell {
    pub(crate) fn new(changes: HashMap<PathBuf, FileChange>, cwd: &Path) -> Self {
        Self {
            changes,
            cwd: cwd.to_path_buf(),
        }
    }
}

pub(crate) fn new_patch_event(
    changes: HashMap<PathBuf, FileChange>,
    cwd: &Path,
) -> PatchHistoryCell {
    PatchHistoryCell::new(changes, cwd)
}

impl HistoryCell for PatchHistoryCell {
    /// Render the diff summary for each file with counts and inline hunks.
    ///
    /// Delegates to `create_diff_summary`, which emits a header summarizing total +/- lines, per-
    /// file headers (or a single-line header when only one file changed), and an indented block
    /// showing the hunks for each file with colored +/- gutters and wrapping at `width`.
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        create_diff_summary(&self.changes, &self.cwd, width as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_core::protocol::FileChange;
    use diffy::create_patch;
    use insta::assert_snapshot;

    #[test]
    fn single_added_file_shows_header_and_hunks() {
        let mut changes = HashMap::new();
        changes.insert(
            PathBuf::from("src/lib.rs"),
            FileChange::Add {
                content: "fn main() {}\n".into(),
            },
        );

        let cell = PatchHistoryCell::new(changes, Path::new("/repo"));
        let rendered = cell.display_string(80);

        assert!(
            rendered.starts_with("• Added src/lib.rs (+1 -0)"),
            "expected single-file header with path and counts:\n{rendered}"
        );
        assert!(rendered.contains("+fn main() {}"));
    }

    #[test]
    fn multiple_files_render_summary_and_move_path() {
        let mut changes = HashMap::new();
        let patch = create_patch("old\n", "new\n").to_string();
        changes.insert(
            PathBuf::from("/repo/old.txt"),
            FileChange::Update {
                unified_diff: patch,
                move_path: Some(PathBuf::from("/repo/new.txt")),
            },
        );
        changes.insert(
            PathBuf::from("/repo/added.txt"),
            FileChange::Add {
                content: "extra\n".into(),
            },
        );

        let cell = PatchHistoryCell::new(changes, Path::new("/repo"));
        let rendered = cell.display_string(80);

        assert!(
            rendered.starts_with("• Edited 2 files (+2 -1)"),
            "expected multi-file summary header:\n{rendered}"
        );
        assert!(
            rendered.contains("/repo/old.txt → /repo/new.txt (+1 -1)"),
            "rendered output did not include move summary:\n{rendered}"
        );
        assert!(
            rendered.contains("+new"),
            "rendered output missing applied patch content:\n{rendered}"
        );
        assert!(
            rendered.contains("added.txt (+1 -0)"),
            "rendered output missing added file header:\n{rendered}"
        );
    }

    #[test]
    fn single_file_patch_wraps_hunks() {
        let mut changes = HashMap::new();
        changes.insert(
            PathBuf::from("src/lib.rs"),
            FileChange::Add {
                content: indoc::indoc! {"
                    fn main() {
                        println!(\"hello world from a very chatty function that will wrap\");
                    }
                "}
                .into(),
            },
        );

        let cell = PatchHistoryCell::new(changes, Path::new("/repo"));
        assert_snapshot!(cell.display_string(56));
    }

    #[test]
    fn multiple_files_summary_and_moves() {
        let mut changes = HashMap::new();
        let patch = create_patch(
            indoc::indoc! {"
                pub fn old() {
                    println!(\"old\");
                }
            "},
            indoc::indoc! {"
                pub fn renamed() {
                    println!(\"renamed with longer output line to wrap cleanly\");
                }
            "},
        )
        .to_string();
        changes.insert(
            PathBuf::from("/repo/src/old.rs"),
            FileChange::Update {
                unified_diff: patch,
                move_path: Some(PathBuf::from("/repo/src/renamed.rs")),
            },
        );
        changes.insert(
            PathBuf::from("/repo/docs/notes.md"),
            FileChange::Add {
                content: "Added runbook steps for deploy.\n".into(),
            },
        );

        let cell = PatchHistoryCell::new(changes, Path::new("/repo"));
        assert_snapshot!(cell.display_string(64));
    }
}
