use crate::types::BranchConnection;
use crate::types::BranchNode;
use crate::types::Commit3D;
use crate::types::FileStats;
use anyhow::Context;
use anyhow::Result;
use chrono::DateTime;
use chrono::Utc;
use git2::BranchType;
use git2::Commit;
use git2::Oid;
use git2::Repository;
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;

/// Git repository analyzer for 3D visualization
pub struct GitAnalyzer {
    repo: Repository,
    color_map: RefCell<HashMap<String, String>>,
}

impl GitAnalyzer {
    /// Open a git repository at the given path
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let repo = Repository::open(path).context("Failed to open git repository")?;

        Ok(Self {
            repo,
            color_map: RefCell::new(HashMap::new()),
        })
    }

    /// Analyze commits and generate 3D coordinates
    pub fn analyze_commits(&mut self, max_commits: Option<usize>) -> Result<Vec<Commit3D>> {
        let mut revwalk = self.repo.revwalk()?;
        revwalk.push_head()?;
        revwalk.set_sorting(git2::Sort::TIME)?;

        let mut commits = Vec::new();
        let mut branch_positions: HashMap<String, f32> = HashMap::new();
        let mut depth_map: HashMap<Oid, f32> = HashMap::new();

        let limit = max_commits.unwrap_or(1000);
        let mut count = 0;

        for oid_result in revwalk {
            if count >= limit {
                break;
            }

            let oid = oid_result?;
            let commit = self.repo.find_commit(oid)?;

            // Calculate 3D coordinates
            let branch_name = self.get_branch_for_commit(&commit)?;
            let x = self.get_branch_position(&branch_name, &mut branch_positions);
            let y = commit.time().seconds() as f32;
            let z = self.calculate_depth(&commit, &mut depth_map)?;

            // Get or generate author color
            let author_email = commit.author().email().unwrap_or("unknown").to_string();
            let color = self.get_author_color(&author_email);

            let commit_3d = Commit3D {
                sha: format!("{}", oid),
                message: commit.message().unwrap_or("").to_string(),
                author: commit.author().name().unwrap_or("Unknown").to_string(),
                author_email: author_email.clone(),
                timestamp: DateTime::from_timestamp(commit.time().seconds(), 0)
                    .unwrap_or_else(|| Utc::now()),
                branch: branch_name,
                parents: commit.parent_ids().map(|p| format!("{}", p)).collect(),
                x,
                y,
                z,
                color,
            };

            commits.push(commit_3d);
            count += 1;
        }

        Ok(commits)
    }

    /// Analyze file change statistics for heatmap
    pub fn analyze_file_stats(&self, max_commits: Option<usize>) -> Result<Vec<FileStats>> {
        let mut file_map: HashMap<String, FileStatsBuilder> = HashMap::new();

        let mut revwalk = self.repo.revwalk()?;
        revwalk.push_head()?;

        let limit = max_commits.unwrap_or(1000);
        let mut count = 0;

        for oid_result in revwalk {
            if count >= limit {
                break;
            }

            let oid = oid_result?;
            let commit = self.repo.find_commit(oid)?;

            // Get diff for this commit
            let tree = commit.tree()?;
            let parent_tree = if commit.parent_count() > 0 {
                Some(commit.parent(0)?.tree()?)
            } else {
                None
            };

            let diff = self
                .repo
                .diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)?;

            // Process each file in the diff
            diff.foreach(
                &mut |delta, _| {
                    if let Some(path) = delta.new_file().path() {
                        let path_str = path.to_string_lossy().to_string();
                        let author = commit.author().email().unwrap_or("unknown").to_string();

                        file_map
                            .entry(path_str)
                            .or_insert_with(FileStatsBuilder::default)
                            .increment(author);
                    }
                    true
                },
                None,
                None,
                None,
            )?;

            count += 1;
        }

        // Convert to FileStats
        let max_changes = file_map.values().map(|s| s.change_count).max().unwrap_or(1) as f32;

        let stats: Vec<FileStats> = file_map
            .into_iter()
            .map(|(path, builder)| {
                let heat_level = (builder.change_count as f32 / max_changes).min(1.0);
                FileStats {
                    path: path.clone(),
                    change_count: builder.change_count,
                    additions: builder.additions,
                    deletions: builder.deletions,
                    last_modified: builder.last_modified,
                    authors: builder.authors.into_iter().collect(),
                    heat_level,
                    size: self.get_file_size(&path).unwrap_or(0),
                }
            })
            .collect();

        Ok(stats)
    }

    /// Analyze branch structure for graph visualization
    pub fn analyze_branches(&mut self) -> Result<Vec<BranchNode>> {
        let mut branches = Vec::new();
        let mut branch_positions: HashMap<String, f32> = HashMap::new();

        // Get all branches
        let branch_iter = self.repo.branches(Some(BranchType::Local))?;

        for branch_result in branch_iter {
            let (branch, _) = branch_result?;
            let name = branch.name()?.unwrap_or("unknown").to_string();

            if let Some(oid) = branch.get().target() {
                let commit = self.repo.find_commit(oid)?;

                // Calculate position
                let x = self.get_branch_position(&name, &mut branch_positions);

                // Find merge information
                let connections = self.find_branch_connections(&name)?;

                let is_active = self.repo.head()?.shorthand() == Some(&name);

                branches.push(BranchNode {
                    name: name.clone(),
                    head_sha: format!("{}", oid),
                    is_active,
                    merge_count: connections.len() as u32,
                    created_at: DateTime::from_timestamp(commit.time().seconds(), 0)
                        .unwrap_or_else(|| Utc::now()),
                    last_commit: DateTime::from_timestamp(commit.time().seconds(), 0)
                        .unwrap_or_else(|| Utc::now()),
                    x,
                    y: commit.time().seconds() as f32,
                    z: 0.0,
                    connections,
                });
            }
        }

        Ok(branches)
    }

    // Helper methods

    fn get_branch_for_commit(&self, commit: &Commit) -> Result<String> {
        // Try to find which branch this commit belongs to
        let oid = commit.id();

        let branches = self.repo.branches(Some(BranchType::Local))?;
        for branch_result in branches {
            let (branch, _) = branch_result?;
            if let Some(branch_oid) = branch.get().target() {
                if branch_oid == oid {
                    return Ok(branch.name()?.unwrap_or("unknown").to_string());
                }
            }
        }

        Ok("main".to_string())
    }

    fn get_branch_position(&self, branch: &str, positions: &mut HashMap<String, f32>) -> f32 {
        let len = positions.len();
        *positions
            .entry(branch.to_string())
            .or_insert(len as f32 * 10.0)
    }

    fn calculate_depth(&self, commit: &Commit, depth_map: &mut HashMap<Oid, f32>) -> Result<f32> {
        let oid = commit.id();

        if let Some(&depth) = depth_map.get(&oid) {
            return Ok(depth);
        }

        let depth = if commit.parent_count() == 0 {
            0.0
        } else {
            let parent_depths: Vec<f32> = commit
                .parents()
                .filter_map(|p| depth_map.get(&p.id()).copied())
                .collect();

            if parent_depths.is_empty() {
                1.0
            } else {
                parent_depths.iter().copied().fold(0.0, f32::max) + 1.0
            }
        };

        depth_map.insert(oid, depth);
        Ok(depth)
    }

    fn get_author_color(&self, email: &str) -> String {
        let mut color_map = self.color_map.borrow_mut();

        if let Some(color) = color_map.get(email) {
            return color.clone();
        }

        // Generate a deterministic color based on email hash
        let hash = email
            .bytes()
            .fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));
        let hue = (hash % 360) as f32;
        let color = format!("hsl({}, 70%, 60%)", hue);

        color_map.insert(email.to_string(), color.clone());
        color
    }

    fn find_branch_connections(&self, _branch_name: &str) -> Result<Vec<BranchConnection>> {
        // Simplified: would need more complex logic to detect actual merges
        Ok(Vec::new())
    }

    fn get_file_size(&self, path: &str) -> Result<u64> {
        let workdir = self.repo.workdir().context("No working directory")?;
        let file_path = workdir.join(path);

        if file_path.exists() {
            Ok(std::fs::metadata(file_path)?.len())
        } else {
            Ok(0)
        }
    }
}

#[derive(Default)]
struct FileStatsBuilder {
    change_count: u32,
    additions: u32,
    deletions: u32,
    last_modified: DateTime<Utc>,
    authors: HashSet<String>,
}

impl FileStatsBuilder {
    fn increment(&mut self, author: String) {
        self.change_count += 1;
        self.authors.insert(author);
        self.last_modified = Utc::now();
    }
}
