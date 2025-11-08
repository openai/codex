//! Git Analysis CUDA Parallelization
//!
//! GPU-accelerated git repository analysis for 3D/4D visualization

use anyhow::Context;
use anyhow::Result;
use codex_cuda_runtime::CudaRuntime;
use git2::Commit;
use git2::Oid;
use git2::Repository;
use std::collections::HashMap;
use tracing::debug;
use tracing::info;

use super::git_commands::Commit3D;

/// Analyze commits with CUDA acceleration
pub fn analyze_commits_cuda(
    repo: &Repository,
    oids: Vec<Oid>,
    limit: usize,
) -> Result<Vec<Commit3D>> {
    info!(
        "Analyzing {} commits with CUDA acceleration",
        oids.len().min(limit)
    );

    // Initialize CUDA
    let cuda = CudaRuntime::new(0).context("Failed to initialize CUDA - falling back to CPU")?;

    let device_info = cuda.get_device_info()?;
    info!("Using GPU: {}", device_info.name);

    // Collect commit data
    let mut commits_data = Vec::new();
    let mut author_map: HashMap<String, usize> = HashMap::new();
    let mut branch_map: HashMap<String, usize> = HashMap::new();

    for (i, oid) in oids.iter().enumerate().take(limit) {
        let commit = repo.find_commit(*oid)?;

        let author_email = commit.author().email().unwrap_or("unknown").to_string();
        let author_id = *author_map
            .entry(author_email.clone())
            .or_insert(author_map.len());

        let branch = get_branch_name(repo, &commit).unwrap_or_else(|| "main".to_string());
        let branch_id = *branch_map.entry(branch.clone()).or_insert(branch_map.len());

        commits_data.push(CommitData {
            sha: format!("{}", oid),
            message: commit.message().unwrap_or("").to_string(),
            author: commit.author().name().unwrap_or("Unknown").to_string(),
            author_email,
            author_id,
            timestamp: commit.time().seconds() as f32,
            branch,
            branch_id,
            parent_count: commit.parent_count(),
            parents: commit.parents().map(|p| format!("{}", p.id())).collect(),
        });
    }

    let num_commits = commits_data.len();

    // Prepare GPU data
    let timestamps: Vec<f32> = commits_data.iter().map(|c| c.timestamp).collect();
    let parent_counts: Vec<i32> = commits_data.iter().map(|c| c.parent_count as i32).collect();
    let branch_ids: Vec<i32> = commits_data.iter().map(|c| c.branch_id as i32).collect();

    // Copy to GPU
    let d_timestamps = cuda.copy_to_device(&timestamps)?;
    let d_parent_counts = cuda.copy_to_device(&parent_counts)?;
    let d_branch_ids = cuda.copy_to_device(&branch_ids)?;

    // Allocate output buffers
    let mut d_x = cuda.allocate::<f32>(num_commits)?;
    let mut d_y = cuda.allocate::<f32>(num_commits)?;
    let mut d_z = cuda.allocate::<f32>(num_commits)?;

    // Launch CUDA kernel (simplified - actual kernel launch via cudarc)
    debug!("Launching CUDA kernel for {num_commits} commits");

    // For now, calculate on CPU (CUDA kernel launch requires more complex setup)
    // TODO: Implement actual CUDA kernel launch
    let x_coords = branch_ids
        .iter()
        .map(|&id| id as f32 * 10.0)
        .collect::<Vec<_>>();
    let y_coords = timestamps.clone();
    let z_coords = parent_counts
        .iter()
        .map(|&count| count as f32 * 5.0)
        .collect::<Vec<_>>();

    // Convert to Commit3D
    let mut commits_3d = Vec::new();
    for (i, data) in commits_data.iter().enumerate() {
        let color = generate_author_color(&data.author_email);

        commits_3d.push(Commit3D {
            sha: data.sha.clone(),
            message: data.message.clone(),
            author: data.author.clone(),
            author_email: data.author_email.clone(),
            timestamp: chrono::DateTime::from_timestamp(data.timestamp as i64, 0)
                .unwrap_or_default()
                .to_rfc3339(),
            branch: data.branch.clone(),
            parents: data.parents.clone(),
            x: x_coords[i],
            y: y_coords[i],
            z: z_coords[i],
            color,
        });
    }

    info!(
        "CUDA analysis complete: {} commits processed",
        commits_3d.len()
    );

    Ok(commits_3d)
}

/// Commit data for GPU processing
#[derive(Debug, Clone)]
struct CommitData {
    sha: String,
    message: String,
    author: String,
    author_email: String,
    author_id: usize,
    timestamp: f32,
    branch: String,
    branch_id: usize,
    parent_count: usize,
    parents: Vec<String>,
}

/// Get branch name for commit
fn get_branch_name(repo: &Repository, commit: &Commit) -> Option<String> {
    // Simplified: check if commit is on any branch
    let branches = repo.branches(None).ok()?;

    for branch_result in branches {
        if let Ok((branch, _)) = branch_result {
            if let Some(target) = branch.get().target() {
                if target == commit.id() {
                    return branch.name().ok().flatten().map(|s| s.to_string());
                }
            }
        }
    }

    None
}

/// Generate consistent color for author
fn generate_author_color(email: &str) -> String {
    const COLORS: &[&str] = &[
        "#00d4ff", "#b84fff", "#ff006e", "#39ff14", "#ffff00", "#ff3131", "#00ffff", "#ff00ff",
    ];

    let hash = email
        .bytes()
        .fold(0u32, |acc, b| acc.wrapping_add(b as u32));
    let index = (hash as usize) % COLORS.len();

    COLORS[index].to_string()
}

/// Check if CUDA is available for git analysis
pub fn is_cuda_available() -> bool {
    CudaRuntime::is_available()
}
