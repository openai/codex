/// Advanced scoring metrics for agent result evaluation
///
/// Provides comprehensive quality assessment including tests, linting,
/// performance, change risk, and readability metrics.

use anyhow::Result;
use serde::{Deserialize, Serialize};
// use std::collections::HashMap;

/// Comprehensive scoring metrics for code quality evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringMetrics {
    /// Test success rate (0.0-1.0)
    pub test_pass_rate: f64,
    /// Coverage delta compared to baseline (-1.0 to 1.0)
    pub coverage_delta: f64,
    /// Lint/type/security score (0.0-1.0, higher is better)
    pub lint_score: f64,
    /// Performance delta (negative = improvement, positive = regression)
    pub performance_delta: f64,
    /// Change risk score (0.0-1.0, higher = more risky)
    pub change_risk: f64,
    /// Readability score (0.0-1.0, higher is better)
    pub readability: f64,
}

impl Default for ScoringMetrics {
    fn default() -> Self {
        Self {
            test_pass_rate: 1.0,
            coverage_delta: 0.0,
            lint_score: 1.0,
            performance_delta: 0.0,
            change_risk: 0.0,
            readability: 1.0,
        }
    }
}

/// Weights for each scoring metric
#[derive(Debug, Clone)]
pub struct ScoringWeights {
    pub test_weight: f64,
    pub coverage_weight: f64,
    pub lint_weight: f64,
    pub performance_weight: f64,
    pub risk_weight: f64,
    pub readability_weight: f64,
}

impl Default for ScoringWeights {
    fn default() -> Self {
        Self {
            test_weight: 0.30,      // 30% - tests are critical
            coverage_weight: 0.15,   // 15% - coverage improvement
            lint_weight: 0.20,       // 20% - code quality
            performance_weight: 0.15, // 15% - performance
            risk_weight: 0.10,       // 10% - risk assessment
            readability_weight: 0.10, // 10% - code readability
        }
    }
}

/// Calculate weighted score from metrics
///
/// Returns a score between 0.0 and 1.0 (higher is better)
pub fn calculate_score(metrics: &ScoringMetrics, weights: &ScoringWeights) -> f64 {
    let mut total_score = 0.0;

    // Test pass rate (0.0-1.0, higher is better)
    total_score += metrics.test_pass_rate * weights.test_weight;

    // Coverage delta (-1.0 to 1.0, normalize to 0.0-1.0)
    let coverage_score = (metrics.coverage_delta + 1.0) / 2.0;
    total_score += coverage_score * weights.coverage_weight;

    // Lint score (0.0-1.0, higher is better)
    total_score += metrics.lint_score * weights.lint_weight;

    // Performance delta (negative is better, normalize to 0.0-1.0)
    // Assume -50% to +50% range
    let perf_score = ((-metrics.performance_delta).clamp(-0.5, 0.5) + 0.5) / 1.0;
    total_score += perf_score * weights.performance_weight;

    // Change risk (0.0-1.0, lower is better, so invert)
    let risk_score = 1.0 - metrics.change_risk;
    total_score += risk_score * weights.risk_weight;

    // Readability (0.0-1.0, higher is better)
    total_score += metrics.readability * weights.readability_weight;

    total_score.clamp(0.0, 1.0)
}

/// Rank multiple solutions by their scores
///
/// Returns vec of (solution_index, score) sorted by score descending
pub fn rank_solutions(
    solutions: &[(String, ScoringMetrics)],
    weights: &ScoringWeights,
) -> Vec<(usize, String, f64)> {
    let mut ranked: Vec<(usize, String, f64)> = solutions
        .iter()
        .enumerate()
        .map(|(idx, (name, metrics))| {
            let score = calculate_score(metrics, weights);
            (idx, name.clone(), score)
        })
        .collect();

    // Sort by score descending (higher scores first)
    ranked.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(Ordering::Equal));

    ranked
}

/// Select the best solution based on scoring
pub fn select_best_solution(
    solutions: &[(String, ScoringMetrics)],
    weights: &ScoringWeights,
) -> Option<(usize, String, f64)> {
    rank_solutions(solutions, weights).into_iter().next()
}

/// Decision log entry for recording why a solution was chosen
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionLog {
    pub timestamp: u64,
    pub goal: String,
    pub solutions_evaluated: usize,
    pub winner: String,
    pub winner_score: f64,
    pub metrics: ScoringMetrics,
    pub reasoning: String,
    pub evidence_urls: Vec<String>,
}

impl DecisionLog {
    /// Create a new decision log
    pub fn new(
        goal: String,
        solutions_evaluated: usize,
        winner: String,
        winner_score: f64,
        metrics: ScoringMetrics,
        reasoning: String,
        evidence_urls: Vec<String>,
    ) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            timestamp,
            goal,
            solutions_evaluated,
            winner,
            winner_score,
            metrics,
            reasoning,
            evidence_urls,
        }
    }

    /// Save decision log to file
    pub fn save_to_file(&self, path: &std::path::Path) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }
}

use std::cmp::Ordering;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_score_perfect() {
        let metrics = ScoringMetrics {
            test_pass_rate: 1.0,
            coverage_delta: 0.2,
            lint_score: 1.0,
            performance_delta: -0.1, // 10% improvement
            change_risk: 0.1,
            readability: 1.0,
        };
        let weights = ScoringWeights::default();
        let score = calculate_score(&metrics, &weights);
        
        // Should be high score (close to 1.0)
        assert!(score > 0.85, "Score should be high: {}", score);
    }

    #[test]
    fn test_calculate_score_poor() {
        let metrics = ScoringMetrics {
            test_pass_rate: 0.5,
            coverage_delta: -0.3,
            lint_score: 0.4,
            performance_delta: 0.2, // 20% regression
            change_risk: 0.8,
            readability: 0.5,
        };
        let weights = ScoringWeights::default();
        let score = calculate_score(&metrics, &weights);
        
        // Should be low score
        assert!(score < 0.6, "Score should be low: {}", score);
    }

    #[test]
    fn test_rank_solutions() {
        let solutions = vec![
            ("agent1".to_string(), ScoringMetrics {
                test_pass_rate: 1.0,
                coverage_delta: 0.1,
                lint_score: 0.9,
                performance_delta: 0.0,
                change_risk: 0.2,
                readability: 0.9,
            }),
            ("agent2".to_string(), ScoringMetrics {
                test_pass_rate: 0.8,
                coverage_delta: 0.0,
                lint_score: 0.7,
                performance_delta: 0.1,
                change_risk: 0.5,
                readability: 0.7,
            }),
            ("agent3".to_string(), ScoringMetrics {
                test_pass_rate: 1.0,
                coverage_delta: 0.3,
                lint_score: 1.0,
                performance_delta: -0.2,
                change_risk: 0.1,
                readability: 1.0,
            }),
        ];

        let weights = ScoringWeights::default();
        let ranked = rank_solutions(&solutions, &weights);

        // agent3 should be first (highest score)
        assert_eq!(ranked[0].1, "agent3");
        // agent1 should be second
        assert_eq!(ranked[1].1, "agent1");
        // agent2 should be last
        assert_eq!(ranked[2].1, "agent2");
    }

    #[test]
    fn test_select_best_solution() {
        let solutions = vec![
            ("mediocre".to_string(), ScoringMetrics::default()),
            ("best".to_string(), ScoringMetrics {
                test_pass_rate: 1.0,
                coverage_delta: 0.5,
                lint_score: 1.0,
                performance_delta: -0.3,
                change_risk: 0.1,
                readability: 1.0,
            }),
        ];

        let weights = ScoringWeights::default();
        let best = select_best_solution(&solutions, &weights);

        assert!(best.is_some());
        let (idx, name, _score) = best.unwrap();
        assert_eq!(idx, 1);
        assert_eq!(name, "best");
    }
}

