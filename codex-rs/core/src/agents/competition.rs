//! Worktree competition for multi-variant execution
//!
//! Creates multiple git worktrees, executes identical tasks in parallel,
//! scores results, and auto-merges the winner.

use crate::blueprint::BlueprintBlock;
use crate::blueprint::EvalCriteria;
use anyhow::Context;
use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;
use std::path::PathBuf;
use std::process::Command;
use tracing::debug;
use tracing::info;
use tracing::warn;

/// Competition configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompetitionConfig {
    /// Number of variants to run (2-3 recommended)
    pub num_variants: usize,

    /// Scoring weights
    pub weights: ScoreWeights,

    /// Time budget in minutes
    pub time_budget_min: u64,

    /// Base directory for worktrees
    pub worktree_base: PathBuf,
}

impl Default for CompetitionConfig {
    fn default() -> Self {
        Self {
            num_variants: 2,
            weights: ScoreWeights::default(),
            time_budget_min: 30,
            worktree_base: PathBuf::from(".codex/worktrees"),
        }
    }
}

/// Scoring weights for competition variants
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreWeights {
    /// Weight for test results (0.0-1.0)
    pub tests: f64,

    /// Weight for performance (0.0-1.0)
    pub performance: f64,

    /// Weight for code simplicity (0.0-1.0)
    pub simplicity: f64,
}

impl Default for ScoreWeights {
    fn default() -> Self {
        Self {
            tests: 0.5,
            performance: 0.3,
            simplicity: 0.2,
        }
    }
}

/// A single competition variant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompetitionVariant {
    /// Variant name (A, B, C, etc.)
    pub name: String,

    /// Branch name
    pub branch: String,

    /// Worktree path
    pub worktree_path: PathBuf,

    /// Score breakdown
    pub score: CompetitionScore,
}

/// Competition score breakdown
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompetitionScore {
    /// Test score (0.0-100.0)
    pub tests: f64,

    /// Performance score (0.0-100.0)
    pub performance: f64,

    /// Simplicity score (0.0-100.0)
    pub simplicity: f64,

    /// Weighted total score
    pub total: f64,

    /// Whether this is the winning variant
    pub is_winner: bool,
}

impl CompetitionScore {
    /// Create a new score
    pub fn new(tests: f64, performance: f64, simplicity: f64, weights: &ScoreWeights) -> Self {
        let total = tests * weights.tests
            + performance * weights.performance
            + simplicity * weights.simplicity;

        Self {
            tests,
            performance,
            simplicity,
            total,
            is_winner: false,
        }
    }

    /// Mark as winner
    pub fn mark_winner(&mut self) {
        self.is_winner = true;
    }
}

/// Competition result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompetitionResult {
    /// All variants
    pub variants: Vec<CompetitionVariant>,

    /// Winner variant name
    pub winner: String,

    /// Comparison table (human-readable)
    pub comparison_table: String,

    /// Total execution time in seconds
    pub execution_time_secs: f64,
}

/// Worktree manager
pub struct WorktreeManager {
    /// Repository root
    repo_root: PathBuf,

    /// Base directory for worktrees
    worktree_base: PathBuf,
}

impl WorktreeManager {
    /// Create a new worktree manager
    pub fn new(repo_root: PathBuf, worktree_base: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&worktree_base)
            .context("Failed to create worktree base directory")?;

        Ok(Self {
            repo_root,
            worktree_base,
        })
    }

    /// Create a worktree for a variant
    pub fn create_worktree(&self, variant_name: &str) -> Result<PathBuf> {
        let branch_name = format!("blueprint-competition-{}", variant_name);
        let worktree_path = self.worktree_base.join(variant_name);

        // Remove existing worktree if present
        self.remove_worktree(&worktree_path)?;

        // Create new branch
        let output = Command::new("git")
            .current_dir(&self.repo_root)
            .args(["branch", &branch_name])
            .output()
            .context("Failed to create branch")?;

        if !output.status.success() {
            // Branch might already exist, that's ok
            debug!("Branch {} might already exist", branch_name);
        }

        // Create worktree
        let output = Command::new("git")
            .current_dir(&self.repo_root)
            .args([
                "worktree",
                "add",
                worktree_path.to_str().unwrap(),
                &branch_name,
            ])
            .output()
            .context("Failed to create worktree")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to create worktree: {}", stderr);
        }

        info!(
            "Created worktree for variant {}: {:?}",
            variant_name, worktree_path
        );
        Ok(worktree_path)
    }

    /// Remove a worktree
    pub fn remove_worktree(&self, worktree_path: &PathBuf) -> Result<()> {
        if !worktree_path.exists() {
            return Ok(());
        }

        let output = Command::new("git")
            .current_dir(&self.repo_root)
            .args([
                "worktree",
                "remove",
                worktree_path.to_str().unwrap(),
                "--force",
            ])
            .output()
            .context("Failed to remove worktree")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!(
                "Failed to remove worktree {}: {}",
                worktree_path.display(),
                stderr
            );
        }

        Ok(())
    }

    /// Archive a variant (rename branch to archived-)
    pub fn archive_variant(&self, variant_name: &str) -> Result<()> {
        let branch_name = format!("blueprint-competition-{}", variant_name);
        let archived_name = format!("archived-{}", branch_name);

        let output = Command::new("git")
            .current_dir(&self.repo_root)
            .args(["branch", "-m", &branch_name, &archived_name])
            .output()
            .context("Failed to archive branch")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("Failed to archive branch {}: {}", branch_name, stderr);
        }

        Ok(())
    }

    /// Cleanup all competition worktrees
    pub fn cleanup_all(&self) -> Result<()> {
        if self.worktree_base.exists() {
            std::fs::remove_dir_all(&self.worktree_base)
                .context("Failed to remove worktree base directory")?;
        }
        Ok(())
    }
}

/// Competition scorer
pub struct CompetitionScorer {
    weights: ScoreWeights,
}

impl CompetitionScorer {
    /// Create a new scorer
    pub fn new(weights: ScoreWeights) -> Self {
        Self { weights }
    }

    /// Score a variant
    pub async fn score_variant(
        &self,
        worktree_path: &PathBuf,
        eval: &EvalCriteria,
    ) -> Result<CompetitionScore> {
        debug!("Scoring variant at {:?}", worktree_path);

        // Run tests
        let test_score = self.run_tests(worktree_path, eval).await?;

        // Measure performance
        let perf_score = self.measure_performance(worktree_path).await?;

        // Measure simplicity
        let simplicity_score = self.measure_simplicity(worktree_path).await?;

        Ok(CompetitionScore::new(
            test_score,
            perf_score,
            simplicity_score,
            &self.weights,
        ))
    }

    /// Run tests and calculate score
    async fn run_tests(&self, worktree_path: &PathBuf, eval: &EvalCriteria) -> Result<f64> {
        if eval.tests.is_empty() {
            return Ok(100.0);
        }

        let mut passed = 0;
        let total = eval.tests.len();

        for test in &eval.tests {
            debug!("Running test: {}", test);

            // Parse test command
            let parts: Vec<&str> = test.split_whitespace().collect();
            if parts.is_empty() {
                continue;
            }

            let output = Command::new(parts[0])
                .current_dir(worktree_path)
                .args(&parts[1..])
                .output()
                .context("Failed to run test")?;

            if output.status.success() {
                passed += 1;
            } else {
                debug!("Test failed: {}", test);
            }
        }

        let score = (passed as f64 / total as f64) * 100.0;
        info!("Test score: {:.1}% ({}/{})", score, passed, total);

        Ok(score)
    }

    /// Measure performance (stub - could run benchmarks)
    async fn measure_performance(&self, _worktree_path: &PathBuf) -> Result<f64> {
        // TODO: Run actual benchmarks
        // For now, return baseline score
        Ok(85.0)
    }

    /// Measure code simplicity (LOC, complexity metrics)
    async fn measure_simplicity(&self, worktree_path: &PathBuf) -> Result<f64> {
        // Count lines of code (simple metric)
        let output = Command::new("git")
            .current_dir(worktree_path)
            .args(["diff", "HEAD~1", "--shortstat"])
            .output()
            .context("Failed to run git diff")?;

        if output.status.success() {
            let diff_stat = String::from_utf8_lossy(&output.stdout);
            // Parse lines changed (lower is better for simplicity)
            // This is a simple heuristic
            let lines = diff_stat
                .split_whitespace()
                .filter_map(|s| s.parse::<u32>().ok())
                .sum::<u32>();

            // Score: fewer lines = higher score
            let score = 100.0 - (lines as f64 * 0.1).min(50.0);
            Ok(score.max(50.0))
        } else {
            Ok(75.0) // Default simplicity score
        }
    }
}

/// Competition runner
pub struct CompetitionRunner {
    config: CompetitionConfig,
    worktree_manager: WorktreeManager,
    scorer: CompetitionScorer,
}

impl CompetitionRunner {
    /// Create a new competition runner
    pub fn new(config: CompetitionConfig, repo_root: PathBuf) -> Result<Self> {
        let worktree_manager = WorktreeManager::new(repo_root, config.worktree_base.clone())?;

        let scorer = CompetitionScorer::new(config.weights.clone());

        Ok(Self {
            config,
            worktree_manager,
            scorer,
        })
    }

    /// Run competition
    pub async fn run_competition(&self, blueprint: &BlueprintBlock) -> Result<CompetitionResult> {
        let start = std::time::Instant::now();

        info!(
            "Starting competition with {} variants for blueprint {}",
            self.config.num_variants, blueprint.id
        );

        // Create variants
        let variant_names: Vec<String> = (0..self.config.num_variants)
            .map(|i| format!("{}", (b'A' + i as u8) as char))
            .collect();

        let mut variants = Vec::new();

        for name in &variant_names {
            let worktree_path = self.worktree_manager.create_worktree(name)?;

            // Execute task in worktree (stub - actual execution would happen here)
            // TODO: Integrate with actual task execution

            // Score variant
            let score = self
                .scorer
                .score_variant(&worktree_path, &blueprint.eval)
                .await?;

            variants.push(CompetitionVariant {
                name: name.clone(),
                branch: format!("blueprint-competition-{}", name),
                worktree_path,
                score,
            });
        }

        // Find winner
        let winner_idx = variants
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.score.total.partial_cmp(&b.score.total).unwrap())
            .map(|(idx, _)| idx)
            .unwrap_or(0);

        variants[winner_idx].score.mark_winner();
        let winner_name = variants[winner_idx].name.clone();

        // Generate comparison table
        let comparison_table = self.format_comparison_table(&variants);

        info!("Competition winner: Variant {}", winner_name);

        let execution_time_secs = start.elapsed().as_secs_f64();

        Ok(CompetitionResult {
            variants,
            winner: winner_name,
            comparison_table,
            execution_time_secs,
        })
    }

    /// Merge winner to main branch
    pub fn merge_winner(&self, result: &CompetitionResult) -> Result<()> {
        let winner = result
            .variants
            .iter()
            .find(|v| v.name == result.winner)
            .context("Winner variant not found")?;

        info!("Merging winner variant {} to main", winner.name);

        // Checkout main
        Command::new("git")
            .args(["checkout", "main"])
            .output()
            .context("Failed to checkout main")?;

        // Merge winner branch
        let output = Command::new("git")
            .args([
                "merge",
                &winner.branch,
                "--no-ff",
                "-m",
                &format!("Merge competition winner: variant {}", winner.name),
            ])
            .output()
            .context("Failed to merge winner")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to merge winner: {}", stderr);
        }

        Ok(())
    }

    /// Archive losers
    pub fn archive_losers(&self, result: &CompetitionResult) -> Result<()> {
        for variant in &result.variants {
            if variant.name != result.winner {
                info!("Archiving loser variant {}", variant.name);
                self.worktree_manager.archive_variant(&variant.name)?;
                self.worktree_manager
                    .remove_worktree(&variant.worktree_path)?;
            }
        }
        Ok(())
    }

    /// Format comparison table
    fn format_comparison_table(&self, variants: &[CompetitionVariant]) -> String {
        let mut table = String::new();

        table.push_str("| Variant | Tests | Performance | Simplicity | Total | Winner |\n");
        table.push_str("|---------|-------|-------------|------------|-------|--------|\n");

        for variant in variants {
            let winner = if variant.score.is_winner { "✅" } else { "" };
            table.push_str(&format!(
                "| {} | {:.1} | {:.1} | {:.1} | {:.1} | {} |\n",
                variant.name,
                variant.score.tests,
                variant.score.performance,
                variant.score.simplicity,
                variant.score.total,
                winner
            ));
        }

        table
    }

    /// Cleanup all worktrees
    pub fn cleanup(&self) -> Result<()> {
        self.worktree_manager.cleanup_all()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_score_calculation() {
        let weights = ScoreWeights {
            tests: 0.5,
            performance: 0.3,
            simplicity: 0.2,
        };

        let score = CompetitionScore::new(100.0, 90.0, 80.0, &weights);

        // 100*0.5 + 90*0.3 + 80*0.2 = 50 + 27 + 16 = 93
        assert!((score.total - 93.0).abs() < 0.01);
    }

    #[test]
    fn test_default_config() {
        let config = CompetitionConfig::default();

        assert_eq!(config.num_variants, 2);
        assert_eq!(config.weights.tests, 0.5);
        assert_eq!(config.weights.performance, 0.3);
        assert_eq!(config.weights.simplicity, 0.2);
    }

    #[test]
    fn test_comparison_table_format() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config = CompetitionConfig::default();
        let runner = CompetitionRunner::new(config.clone(), temp_dir.path().to_path_buf()).unwrap();

        let mut variants = vec![
            CompetitionVariant {
                name: "A".to_string(),
                branch: "branch-a".to_string(),
                worktree_path: PathBuf::from("/tmp/a"),
                score: CompetitionScore::new(100.0, 90.0, 80.0, &config.weights),
            },
            CompetitionVariant {
                name: "B".to_string(),
                branch: "branch-b".to_string(),
                worktree_path: PathBuf::from("/tmp/b"),
                score: CompetitionScore::new(95.0, 95.0, 85.0, &config.weights),
            },
        ];

        variants[0].score.mark_winner();

        let table = runner.format_comparison_table(&variants);

        assert!(table.contains("| Variant |"));
        assert!(table.contains("| A |"));
        assert!(table.contains("| B |"));
        assert!(table.contains("✅"));
    }
}
