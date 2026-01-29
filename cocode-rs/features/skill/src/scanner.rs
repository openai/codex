//! Directory scanner for skill discovery.
//!
//! Walks a directory tree to find skill directories (those containing a
//! `SKILL.toml` file). Supports configurable depth limits and detects
//! symlink cycles via canonical path tracking.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// The expected metadata file name in each skill directory.
const SKILL_TOML: &str = "SKILL.toml";

/// Scans directory trees for skill directories.
///
/// Walks each root directory looking for directories that contain a
/// `SKILL.toml` file. Symlink cycles are detected by tracking canonical
/// paths. Errors during scanning (e.g., permission denied) are logged
/// and skipped.
pub struct SkillScanner {
    /// Maximum depth to walk into the directory tree.
    pub max_scan_depth: i32,

    /// Maximum number of skill directories to discover per root.
    pub max_skills_dirs_per_root: i32,
}

impl Default for SkillScanner {
    fn default() -> Self {
        Self {
            max_scan_depth: 6,
            max_skills_dirs_per_root: 2000,
        }
    }
}

impl SkillScanner {
    /// Creates a new scanner with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Scans a single root directory for skill directories.
    ///
    /// Returns a list of absolute paths to directories containing `SKILL.toml`.
    /// Symlink cycles are detected and skipped. Errors are logged but do not
    /// cause the scan to abort.
    pub fn scan(&self, root: &Path) -> Vec<PathBuf> {
        let mut results = Vec::new();
        let mut seen_canonical = HashSet::<PathBuf>::new();

        // Convert max_scan_depth to usize for walkdir; clamp negative values to 0
        let depth = self.max_scan_depth.max(0) as usize;
        let max_results = self.max_skills_dirs_per_root.max(0) as usize;

        let walker = WalkDir::new(root)
            .max_depth(depth)
            .follow_links(true)
            .into_iter();

        for entry in walker {
            if results.len() >= max_results {
                tracing::warn!(
                    root = %root.display(),
                    limit = self.max_skills_dirs_per_root,
                    "reached skill directory scan limit, stopping"
                );
                break;
            }

            let entry = match entry {
                Ok(e) => e,
                Err(err) => {
                    tracing::debug!(
                        error = %err,
                        "skipping inaccessible entry during skill scan"
                    );
                    continue;
                }
            };

            // Only process directories
            if !entry.file_type().is_dir() {
                continue;
            }

            let dir_path = entry.path();

            // Check for symlink cycles by tracking canonical paths
            match dir_path.canonicalize() {
                Ok(canonical) => {
                    if !seen_canonical.insert(canonical) {
                        tracing::debug!(
                            path = %dir_path.display(),
                            "skipping symlink cycle"
                        );
                        continue;
                    }
                }
                Err(err) => {
                    tracing::debug!(
                        path = %dir_path.display(),
                        error = %err,
                        "failed to canonicalize path, skipping"
                    );
                    continue;
                }
            }

            // Check if this directory contains SKILL.toml
            let skill_toml = dir_path.join(SKILL_TOML);
            if skill_toml.is_file() {
                results.push(dir_path.to_path_buf());
            }
        }

        results
    }

    /// Scans multiple root directories for skill directories.
    ///
    /// Results from all roots are concatenated. Duplicates across roots
    /// are not removed here; use [`crate::dedup`] for that.
    pub fn scan_roots(&self, roots: &[PathBuf]) -> Vec<PathBuf> {
        let mut all = Vec::new();
        for root in roots {
            if root.is_dir() {
                let found = self.scan(root);
                tracing::debug!(
                    root = %root.display(),
                    count = found.len(),
                    "scanned root for skills"
                );
                all.extend(found);
            } else {
                tracing::debug!(
                    root = %root.display(),
                    "skill scan root does not exist or is not a directory"
                );
            }
        }
        all
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_scanner_default() {
        let scanner = SkillScanner::default();
        assert_eq!(scanner.max_scan_depth, 6);
        assert_eq!(scanner.max_skills_dirs_per_root, 2000);
    }

    #[test]
    fn test_scan_finds_skill_directories() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let root = tmp.path();

        // Create two skill directories
        let skill1 = root.join("skill1");
        fs::create_dir_all(&skill1).expect("mkdir skill1");
        fs::write(
            skill1.join("SKILL.toml"),
            "name = \"s1\"\ndescription = \"d\"\nprompt_inline = \"p\"",
        )
        .expect("write SKILL.toml");

        let skill2 = root.join("nested").join("skill2");
        fs::create_dir_all(&skill2).expect("mkdir skill2");
        fs::write(
            skill2.join("SKILL.toml"),
            "name = \"s2\"\ndescription = \"d\"\nprompt_inline = \"p\"",
        )
        .expect("write SKILL.toml");

        // Create a directory without SKILL.toml
        let no_skill = root.join("no-skill");
        fs::create_dir_all(&no_skill).expect("mkdir no-skill");
        fs::write(no_skill.join("README.md"), "not a skill").expect("write README");

        let scanner = SkillScanner::new();
        let found = scanner.scan(root);

        assert_eq!(found.len(), 2);
        assert!(found.contains(&skill1));
        assert!(found.contains(&skill2));
    }

    #[test]
    fn test_scan_respects_depth_limit() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let root = tmp.path();

        // Create skill at depth 3 (root/a/b/c/SKILL.toml)
        let deep = root.join("a").join("b").join("c");
        fs::create_dir_all(&deep).expect("mkdir deep");
        fs::write(
            deep.join("SKILL.toml"),
            "name = \"deep\"\ndescription = \"d\"\nprompt_inline = \"p\"",
        )
        .expect("write SKILL.toml");

        // Scanner with depth 2 should not find it
        let scanner = SkillScanner {
            max_scan_depth: 2,
            max_skills_dirs_per_root: 2000,
        };
        let found = scanner.scan(root);
        assert!(found.is_empty());

        // Scanner with depth 4 should find it
        let scanner = SkillScanner {
            max_scan_depth: 4,
            max_skills_dirs_per_root: 2000,
        };
        let found = scanner.scan(root);
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn test_scan_respects_max_dirs_limit() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let root = tmp.path();

        // Create 5 skill directories
        for i in 0..5 {
            let skill = root.join(format!("skill{i}"));
            fs::create_dir_all(&skill).expect("mkdir");
            fs::write(
                skill.join("SKILL.toml"),
                format!("name = \"s{i}\"\ndescription = \"d\"\nprompt_inline = \"p\""),
            )
            .expect("write");
        }

        let scanner = SkillScanner {
            max_scan_depth: 6,
            max_skills_dirs_per_root: 3,
        };
        let found = scanner.scan(root);
        assert!(found.len() <= 3);
    }

    #[test]
    fn test_scan_nonexistent_root() {
        let scanner = SkillScanner::new();
        let found = scanner.scan(Path::new("/nonexistent/path/xyz"));
        assert!(found.is_empty());
    }

    #[test]
    fn test_scan_roots_skips_missing() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let root = tmp.path();

        let skill = root.join("skill");
        fs::create_dir_all(&skill).expect("mkdir");
        fs::write(
            skill.join("SKILL.toml"),
            "name = \"s\"\ndescription = \"d\"\nprompt_inline = \"p\"",
        )
        .expect("write");

        let scanner = SkillScanner::new();
        let roots = vec![root.to_path_buf(), PathBuf::from("/nonexistent/root")];
        let found = scanner.scan_roots(&roots);
        assert_eq!(found.len(), 1);
    }
}
