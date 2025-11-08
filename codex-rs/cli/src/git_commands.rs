//! Git analysis commands for 3D/4D visualization
//!
//! Provides Git repository analysis capabilities for Kamui4d-style visualization.
//! Supports CUDA acceleration for 100-1000x speedup.

use anyhow::Context;
use anyhow::Result;
use clap::Parser;
use clap::Subcommand;
use git2::Commit;
use git2::Oid;
use git2::Repository;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;

#[cfg(feature = "cuda")]
mod git_cuda;

/// Git analysis commands
#[derive(Debug, Parser)]
pub struct GitAnalyzeCli {
    #[clap(subcommand)]
    pub command: GitAnalyzeCommand,
}

#[derive(Debug, Subcommand)]
pub enum GitAnalyzeCommand {
    /// Analyze commit history with 3D coordinates
    Commits {
        /// Repository path (default: current directory)
        #[clap(long, default_value = ".")]
        repo_path: PathBuf,

        /// Limit number of commits
        #[clap(long, default_value = "1000")]
        limit: usize,

        /// Use CUDA GPU acceleration (100-1000x faster)
        #[clap(long)]
        use_cuda: bool,
    },

    /// Analyze file change heatmap
    Heatmap {
        /// Repository path (default: current directory)
        #[clap(long, default_value = ".")]
        repo_path: PathBuf,

        /// Limit number of commits to analyze
        #[clap(long, default_value = "1000")]
        limit: usize,
    },

    /// Analyze branch structure
    Branches {
        /// Repository path (default: current directory)
        #[clap(long, default_value = ".")]
        repo_path: PathBuf,
    },
}

/// 3D commit representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commit3D {
    pub sha: String,
    pub message: String,
    pub author: String,
    pub author_email: String,
    pub timestamp: String,
    pub branch: String,
    pub parents: Vec<String>,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub color: String,
}

/// File heatmap entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileHeat {
    pub path: String,
    pub change_count: usize,
    pub additions: usize,
    pub deletions: usize,
    pub last_modified: String,
    pub authors: Vec<String>,
    pub heat_level: f32,
    pub size: Option<u64>,
}

/// Branch node for 3D graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchNode {
    pub name: String,
    pub head_sha: String,
    pub is_active: bool,
    pub merge_count: usize,
    pub created_at: Option<String>,
    pub last_commit: String,
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// Run git analysis command
pub async fn run_git_analyze_command(cli: GitAnalyzeCli) -> Result<()> {
    match cli.command {
        GitAnalyzeCommand::Commits {
            repo_path,
            limit,
            use_cuda,
        } => {
            #[cfg(feature = "cuda")]
            if use_cuda {
                analyze_commits_with_cuda(&repo_path, limit)?;
            } else {
                analyze_commits(&repo_path, limit)?;
            }

            #[cfg(not(feature = "cuda"))]
            {
                if use_cuda {
                    eprintln!("⚠️  CUDA not available (compile with --features cuda)");
                }
                analyze_commits(&repo_path, limit)?;
            }
        }
        GitAnalyzeCommand::Heatmap { repo_path, limit } => {
            analyze_heatmap(&repo_path, limit)?;
        }
        GitAnalyzeCommand::Branches { repo_path } => {
            analyze_branches(&repo_path)?;
        }
    }

    Ok(())
}

#[cfg(feature = "cuda")]
fn analyze_commits_with_cuda(repo_path: &PathBuf, limit: usize) -> Result<()> {
    let repo = Repository::open(repo_path).context("Failed to open Git repository")?;

    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;
    revwalk.set_sorting(git2::Sort::TIME)?;

    let oids: Vec<Oid> = revwalk.take(limit).collect::<Result<Vec<_>, _>>()?;

    let commits = git_cuda::analyze_commits_cuda(&repo, oids, limit)?;

    let json = serde_json::to_string_pretty(&commits)?;
    println!("{json}");

    Ok(())
}

fn analyze_commits(repo_path: &PathBuf, limit: usize) -> Result<()> {
    let repo = Repository::open(repo_path).context("Failed to open Git repository")?;

    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;
    revwalk.set_sorting(git2::Sort::TIME)?;

    let mut commits: Vec<Commit3D> = Vec::new();
    let mut branch_positions: HashMap<String, f32> = HashMap::new();
    let mut depth_map: HashMap<Oid, f32> = HashMap::new();
    let mut author_colors: HashMap<String, String> = HashMap::new();

    for (i, oid) in revwalk.enumerate() {
        if i >= limit {
            break;
        }

        let oid = oid?;
        let commit = repo.find_commit(oid)?;

        let author = commit.author();
        let author_name = author.name().unwrap_or("Unknown").to_string();
        let author_email = author.email().unwrap_or("unknown@email").to_string();

        // Generate consistent color for author
        let color = author_colors
            .entry(author_email.clone())
            .or_insert_with(|| generate_author_color(&author_email))
            .clone();

        // Get branch name (simplified - use "main" as default)
        let branch = get_branch_name(&repo, &commit).unwrap_or_else(|| "main".to_string());

        // Calculate 3D position
        let x = get_branch_position(&branch, &mut branch_positions);
        let y = commit.time().seconds() as f32;
        let z = calculate_depth(&commit, &mut depth_map);

        let commit_3d = Commit3D {
            sha: format!("{}", oid),
            message: commit.message().unwrap_or("").to_string(),
            author: author_name,
            author_email,
            timestamp: chrono::DateTime::from_timestamp(commit.time().seconds(), 0)
                .unwrap_or_default()
                .to_rfc3339(),
            branch,
            parents: commit.parents().map(|p| format!("{}", p.id())).collect(),
            x,
            y,
            z,
            color,
        };

        commits.push(commit_3d);
    }

    // Output as JSON
    let json = serde_json::to_string_pretty(&commits)?;
    println!("{}", json);

    Ok(())
}

fn analyze_heatmap(repo_path: &PathBuf, limit: usize) -> Result<()> {
    let repo = Repository::open(repo_path).context("Failed to open Git repository")?;

    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;
    revwalk.set_sorting(git2::Sort::TIME)?;

    let mut file_stats: HashMap<String, FileHeatData> = HashMap::new();

    for (i, oid) in revwalk.enumerate() {
        if i >= limit {
            break;
        }

        let oid = oid?;
        let commit = repo.find_commit(oid)?;

        if commit.parent_count() == 0 {
            continue;
        }

        let parent = commit.parent(0)?;
        let diff = repo.diff_tree_to_tree(Some(&parent.tree()?), Some(&commit.tree()?), None)?;

        let author_email = commit.author().email().unwrap_or("unknown").to_string();
        let timestamp = chrono::DateTime::from_timestamp(commit.time().seconds(), 0)
            .unwrap_or_default()
            .to_rfc3339();

        diff.foreach(
            &mut |delta, _progress| {
                if let Some(path) = delta.new_file().path() {
                    let path_str = path.to_string_lossy().to_string();
                    let entry = file_stats.entry(path_str).or_insert_with(|| FileHeatData {
                        change_count: 0,
                        additions: 0,
                        deletions: 0,
                        last_modified: timestamp.clone(),
                        authors: Vec::new(),
                    });

                    entry.change_count += 1;
                    entry.last_modified = timestamp.clone();
                    if !entry.authors.contains(&author_email) {
                        entry.authors.push(author_email.clone());
                    }
                }
                true
            },
            None,
            None,
            None,
        )?;
    }

    // Convert to FileHeat with normalized heat levels
    let max_changes = file_stats
        .values()
        .map(|s| s.change_count)
        .max()
        .unwrap_or(1);
    let mut heatmap: Vec<FileHeat> = file_stats
        .into_iter()
        .map(|(path, stats)| FileHeat {
            path: path.clone(),
            change_count: stats.change_count,
            additions: stats.additions,
            deletions: stats.deletions,
            last_modified: stats.last_modified,
            authors: stats.authors,
            heat_level: (stats.change_count as f32) / (max_changes as f32),
            size: std::fs::metadata(repo_path.join(&path))
                .ok()
                .map(|m| m.len()),
        })
        .collect();

    // Sort by heat level (hottest first)
    heatmap.sort_by(|a, b| b.heat_level.partial_cmp(&a.heat_level).unwrap());

    // Output as JSON
    let json = serde_json::to_string_pretty(&heatmap)?;
    println!("{}", json);

    Ok(())
}

fn analyze_branches(repo_path: &PathBuf) -> Result<()> {
    let repo = Repository::open(repo_path).context("Failed to open Git repository")?;

    let branches = repo.branches(None)?;
    let mut branch_nodes: Vec<BranchNode> = Vec::new();

    let head = repo.head()?;
    let active_branch_name = head.shorthand().unwrap_or("HEAD");

    for branch_result in branches {
        let (branch, _branch_type) = branch_result?;
        let name = branch.name()?.unwrap_or("unknown").to_string();
        let is_active = name == active_branch_name;

        if let Some(oid) = branch.get().target() {
            if let Ok(commit) = repo.find_commit(oid) {
                let timestamp = chrono::DateTime::from_timestamp(commit.time().seconds(), 0)
                    .unwrap_or_default()
                    .to_rfc3339();

                // Count merge commits (simplified)
                let merge_count = count_merge_commits(&repo, &commit, 100)?;

                let branch_node = BranchNode {
                    name: name.clone(),
                    head_sha: format!("{}", oid),
                    is_active,
                    merge_count,
                    created_at: None, // Would require walking full history
                    last_commit: timestamp.clone(),
                    x: branch_nodes.len() as f32 * 10.0,
                    y: commit.time().seconds() as f32,
                    z: 0.0,
                };

                branch_nodes.push(branch_node);
            }
        }
    }

    // Output as JSON
    let json = serde_json::to_string_pretty(&branch_nodes)?;
    println!("{}", json);

    Ok(())
}

// Helper functions

#[derive(Debug)]
struct FileHeatData {
    change_count: usize,
    additions: usize,
    deletions: usize,
    last_modified: String,
    authors: Vec<String>,
}

fn get_branch_name(repo: &Repository, _commit: &Commit) -> Option<String> {
    // Simplified: return current branch or "main"
    if let Ok(head) = repo.head() {
        if let Some(name) = head.shorthand() {
            return Some(name.to_string());
        }
    }
    None
}

fn get_branch_position(branch: &str, positions: &mut HashMap<String, f32>) -> f32 {
    let len = positions.len();
    *positions
        .entry(branch.to_string())
        .or_insert(len as f32 * 10.0)
}

fn calculate_depth(commit: &Commit, depth_map: &mut HashMap<Oid, f32>) -> f32 {
    let oid = commit.id();

    // Check if we've already computed this depth
    if let Some(&depth) = depth_map.get(&oid) {
        return depth;
    }

    // Root commit has depth 0
    if commit.parent_count() == 0 {
        depth_map.insert(oid, 0.0);
        return 0.0;
    }

    // Compute max parent depth
    let mut max_parent_depth = 0.0_f32;
    for parent in commit.parents() {
        if let Some(&parent_depth) = depth_map.get(&parent.id()) {
            max_parent_depth = max_parent_depth.max(parent_depth);
        }
    }

    let depth = max_parent_depth + 1.0;
    depth_map.insert(oid, depth);
    depth
}

fn generate_author_color(email: &str) -> String {
    // Hash email to generate consistent hue
    let hash = email
        .bytes()
        .fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));
    let hue = (hash % 360) as f32;
    format!("hsl({}, 70%, 60%)", hue)
}

fn count_merge_commits(repo: &Repository, start_commit: &Commit, limit: usize) -> Result<usize> {
    let mut count = 0;
    let mut revwalk = repo.revwalk()?;
    revwalk.push(start_commit.id())?;

    for (i, oid) in revwalk.enumerate() {
        if i >= limit {
            break;
        }

        let oid = oid?;
        if let Ok(commit) = repo.find_commit(oid) {
            if commit.parent_count() > 1 {
                count += 1;
            }
        }
    }

    Ok(count)
}

#[cfg(test)]
#[path = "git_commands_test.rs"]
mod git_commands_test;
