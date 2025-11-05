/// Consensus mechanism for multi-agent decision making
///
/// Provides voting, scoring aggregation, and automatic best solution selection
/// with detailed decision logging.
use crate::scoring::{
    DecisionLog, ScoringMetrics, ScoringWeights, calculate_score, rank_solutions,
};
// use crate::types::TaskResult;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Consensus strategy for selecting the best solution
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsensusStrategy {
    /// Select solution with highest score
    HighestScore,
    /// Majority voting (requires >50% agreement)
    MajorityVote,
    /// Weighted voting based on agent reliability
    WeightedVote,
    /// Unanimous (all agents must agree)
    Unanimous,
}

/// Vote from a single agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentVote {
    pub agent_name: String,
    pub preferred_solution: String,
    pub confidence: f64,
    pub reasoning: String,
}

/// Consensus result with selected solution and justification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusResult {
    pub selected_solution: String,
    pub selection_strategy: String,
    pub final_score: f64,
    pub votes: Vec<AgentVote>,
    pub decision_log: DecisionLog,
}

/// Consensus builder for multi-agent decision making
pub struct ConsensusBuilder {
    strategy: ConsensusStrategy,
    weights: ScoringWeights,
    agent_reliability: HashMap<String, f64>,
}

impl ConsensusBuilder {
    /// Create a new consensus builder
    pub fn new(strategy: ConsensusStrategy) -> Self {
        Self {
            strategy,
            weights: ScoringWeights::default(),
            agent_reliability: HashMap::new(),
        }
    }

    /// Set scoring weights
    pub fn with_weights(mut self, weights: ScoringWeights) -> Self {
        self.weights = weights;
        self
    }

    /// Set agent reliability scores (for weighted voting)
    pub fn with_agent_reliability(mut self, reliability: HashMap<String, f64>) -> Self {
        self.agent_reliability = reliability;
        self
    }

    /// Build consensus from multiple agent results
    pub fn build_consensus(
        &self,
        goal: &str,
        solutions: &[(String, ScoringMetrics)],
        votes: Vec<AgentVote>,
    ) -> Result<ConsensusResult> {
        let selection = match self.strategy {
            ConsensusStrategy::HighestScore => self.select_by_highest_score(solutions)?,
            ConsensusStrategy::MajorityVote => self.select_by_majority_vote(&votes, solutions)?,
            ConsensusStrategy::WeightedVote => self.select_by_weighted_vote(&votes, solutions)?,
            ConsensusStrategy::Unanimous => self.select_by_unanimous(&votes, solutions)?,
        };

        let (idx, winner, score) = selection;
        let winner_metrics = solutions[idx].1.clone();

        // Generate reasoning
        let reasoning = format!(
            "Selected '{}' using {:?} strategy. Score: {:.3}. {} solutions evaluated, {} votes collected.",
            winner,
            self.strategy,
            score,
            solutions.len(),
            votes.len()
        );

        // Collect evidence URLs from votes
        let evidence_urls: Vec<String> = votes
            .iter()
            .flat_map(|v| {
                // Extract URLs from reasoning if present
                // Simple URL extraction (could be enhanced)
                v.reasoning
                    .split_whitespace()
                    .filter(|s| s.starts_with("http"))
                    .map(|s| s.trim_end_matches(&['.', ',', ';', ':'][..]).to_string())
                    .collect::<Vec<_>>()
            })
            .collect();

        let decision_log = DecisionLog::new(
            goal.to_string(),
            solutions.len(),
            winner.clone(),
            score,
            winner_metrics,
            reasoning.clone(),
            evidence_urls,
        );

        Ok(ConsensusResult {
            selected_solution: winner,
            selection_strategy: format!("{:?}", self.strategy),
            final_score: score,
            votes,
            decision_log,
        })
    }

    fn select_by_highest_score(
        &self,
        solutions: &[(String, ScoringMetrics)],
    ) -> Result<(usize, String, f64)> {
        let ranked = rank_solutions(solutions, &self.weights);
        ranked
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("No solutions to evaluate"))
    }

    fn select_by_majority_vote(
        &self,
        votes: &[AgentVote],
        solutions: &[(String, ScoringMetrics)],
    ) -> Result<(usize, String, f64)> {
        if votes.is_empty() {
            return self.select_by_highest_score(solutions);
        }

        // Count votes for each solution
        let mut vote_counts: HashMap<String, usize> = HashMap::new();
        for vote in votes {
            *vote_counts
                .entry(vote.preferred_solution.clone())
                .or_insert(0) += 1;
        }

        // Find solution with most votes
        let winner = vote_counts
            .into_iter()
            .max_by_key(|(_, count)| *count)
            .map(|(name, _)| name)
            .ok_or_else(|| anyhow::anyhow!("No votes recorded"))?;

        // Get score for winner
        let (idx, score) = solutions
            .iter()
            .enumerate()
            .find(|(_, (name, _))| name == &winner)
            .map(|(idx, (_, metrics))| (idx, calculate_score(metrics, &self.weights)))
            .ok_or_else(|| anyhow::anyhow!("Winner not found in solutions"))?;

        Ok((idx, winner, score))
    }

    fn select_by_weighted_vote(
        &self,
        votes: &[AgentVote],
        solutions: &[(String, ScoringMetrics)],
    ) -> Result<(usize, String, f64)> {
        if votes.is_empty() {
            return self.select_by_highest_score(solutions);
        }

        // Weight votes by agent reliability and confidence
        let mut weighted_scores: HashMap<String, f64> = HashMap::new();
        for vote in votes {
            let agent_reliability = self
                .agent_reliability
                .get(&vote.agent_name)
                .copied()
                .unwrap_or(1.0);
            let weight = agent_reliability * vote.confidence;
            *weighted_scores
                .entry(vote.preferred_solution.clone())
                .or_insert(0.0) += weight;
        }

        // Find solution with highest weighted score
        let winner = weighted_scores
            .into_iter()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(name, _)| name)
            .ok_or_else(|| anyhow::anyhow!("No votes recorded"))?;

        // Get score for winner
        let (idx, score) = solutions
            .iter()
            .enumerate()
            .find(|(_, (name, _))| name == &winner)
            .map(|(idx, (_, metrics))| (idx, calculate_score(metrics, &self.weights)))
            .ok_or_else(|| anyhow::anyhow!("Winner not found in solutions"))?;

        Ok((idx, winner, score))
    }

    fn select_by_unanimous(
        &self,
        votes: &[AgentVote],
        solutions: &[(String, ScoringMetrics)],
    ) -> Result<(usize, String, f64)> {
        if votes.is_empty() {
            return Err(anyhow::anyhow!("Unanimous strategy requires votes"));
        }

        // Check if all votes agree
        let first_choice = &votes[0].preferred_solution;
        let all_agree = votes.iter().all(|v| &v.preferred_solution == first_choice);

        if !all_agree {
            return Err(anyhow::anyhow!("No unanimous consensus reached"));
        }

        // Get score for unanimous choice
        let (idx, score) = solutions
            .iter()
            .enumerate()
            .find(|(_, (name, _))| name == first_choice)
            .map(|(idx, (_, metrics))| (idx, calculate_score(metrics, &self.weights)))
            .ok_or_else(|| anyhow::anyhow!("Unanimous choice not found in solutions"))?;

        Ok((idx, first_choice.clone(), score))
    }
}

impl Default for ConsensusBuilder {
    fn default() -> Self {
        Self::new(ConsensusStrategy::HighestScore)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_highest_score_consensus() {
        let solutions = vec![
            (
                "agent1".to_string(),
                ScoringMetrics {
                    test_pass_rate: 0.8,
                    coverage_delta: 0.0,
                    lint_score: 0.7,
                    performance_delta: 0.0,
                    change_risk: 0.3,
                    readability: 0.8,
                },
            ),
            (
                "agent2".to_string(),
                ScoringMetrics {
                    test_pass_rate: 1.0,
                    coverage_delta: 0.2,
                    lint_score: 1.0,
                    performance_delta: -0.1,
                    change_risk: 0.1,
                    readability: 1.0,
                },
            ),
        ];

        let builder = ConsensusBuilder::new(ConsensusStrategy::HighestScore);
        let result = builder
            .build_consensus("test goal", &solutions, vec![])
            .unwrap();

        assert_eq!(result.selected_solution, "agent2");
        assert!(result.final_score > 0.8);
    }

    #[test]
    fn test_majority_vote_consensus() {
        let solutions = vec![
            ("agent1".to_string(), ScoringMetrics::default()),
            ("agent2".to_string(), ScoringMetrics::default()),
        ];

        let votes = vec![
            AgentVote {
                agent_name: "reviewer1".to_string(),
                preferred_solution: "agent1".to_string(),
                confidence: 0.9,
                reasoning: "Better implementation".to_string(),
            },
            AgentVote {
                agent_name: "reviewer2".to_string(),
                preferred_solution: "agent1".to_string(),
                confidence: 0.8,
                reasoning: "Cleaner code".to_string(),
            },
            AgentVote {
                agent_name: "reviewer3".to_string(),
                preferred_solution: "agent2".to_string(),
                confidence: 0.7,
                reasoning: "More features".to_string(),
            },
        ];

        let builder = ConsensusBuilder::new(ConsensusStrategy::MajorityVote);
        let result = builder
            .build_consensus("test goal", &solutions, votes)
            .unwrap();

        assert_eq!(result.selected_solution, "agent1");
    }

    #[test]
    fn test_weighted_vote_consensus() {
        let solutions = vec![
            ("agent1".to_string(), ScoringMetrics::default()),
            ("agent2".to_string(), ScoringMetrics::default()),
        ];

        let votes = vec![
            AgentVote {
                agent_name: "expert".to_string(),
                preferred_solution: "agent2".to_string(),
                confidence: 1.0,
                reasoning: "Expert opinion".to_string(),
            },
            AgentVote {
                agent_name: "junior".to_string(),
                preferred_solution: "agent1".to_string(),
                confidence: 0.5,
                reasoning: "Junior opinion".to_string(),
            },
        ];

        let mut reliability = HashMap::new();
        reliability.insert("expert".to_string(), 2.0);
        reliability.insert("junior".to_string(), 0.5);

        let builder = ConsensusBuilder::new(ConsensusStrategy::WeightedVote)
            .with_agent_reliability(reliability);
        let result = builder
            .build_consensus("test goal", &solutions, votes)
            .unwrap();

        // Expert vote should win due to higher reliability
        assert_eq!(result.selected_solution, "agent2");
    }
}
