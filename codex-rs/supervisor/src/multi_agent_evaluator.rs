use crate::Supervisor;
use crate::types::AggregatedResult;
use crate::types::Assignment;
use crate::types::Plan;
use crate::types::SupervisorConfig;
use crate::types::TaskResult;
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

/// Configuration for running multi-agent evaluation loops.
#[derive(Debug, Clone)]
pub struct MultiAgentEvaluationConfig {
    /// Maximum rounds to execute before stopping.
    pub max_rounds: usize,
    /// Number of top scoring agents to carry forward each round.
    pub top_k: usize,
    /// Minimum average improvement required to keep iterating (None disables the check).
    pub improvement_threshold: Option<f64>,
    /// Optional risk ceiling; agents above this risk are excluded from further rounds.
    pub max_risk_score: Option<f64>,
}

impl Default for MultiAgentEvaluationConfig {
    fn default() -> Self {
        Self {
            max_rounds: 3,
            top_k: 2,
            improvement_threshold: Some(0.05),
            max_risk_score: Some(0.6),
        }
    }
}

/// Evaluation score for a single agent within a round.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationScore {
    pub agent_name: String,
    pub round_index: usize,
    pub score: f64,
    pub risk: Option<f64>,
    pub feedback: Option<String>,
}

/// Summary of a single round in the evaluation loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiAgentRoundReport {
    pub round_index: usize,
    pub active_agents: Vec<String>,
    pub plan: Plan,
    pub assignments: Vec<Assignment>,
    pub results: Vec<TaskResult>,
    pub aggregated: AggregatedResult,
    pub scores: Vec<EvaluationScore>,
    pub next_active_agents: Vec<String>,
}

/// Final report returned after the evaluation loop finishes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiAgentEvaluationReport {
    pub goal: String,
    pub rounds: Vec<MultiAgentRoundReport>,
    pub terminated_early: bool,
    pub final_agents: Vec<String>,
    pub score_history: HashMap<String, Vec<EvaluationScore>>,
}

/// Strategy used to evaluate agent results after each round.
pub trait EvaluationStrategy: Send + Sync {
    fn evaluate(
        &self,
        round_index: usize,
        plan: &Plan,
        assignments: &[Assignment],
        results: &[TaskResult],
        aggregated: &AggregatedResult,
    ) -> Vec<EvaluationScore>;

    fn should_terminate(
        &self,
        _round_index: usize,
        _history: &[MultiAgentRoundReport],
        _config: &MultiAgentEvaluationConfig,
    ) -> bool {
        false
    }
}

/// Basic evaluation strategy that averages task scores per agent and flags failed work.
#[derive(Debug, Default, Clone)]
pub struct SimpleEvaluationStrategy;

impl EvaluationStrategy for SimpleEvaluationStrategy {
    fn evaluate(
        &self,
        round_index: usize,
        _plan: &Plan,
        _assignments: &[Assignment],
        results: &[TaskResult],
        _aggregated: &AggregatedResult,
    ) -> Vec<EvaluationScore> {
        let mut score_accumulator: HashMap<String, (f64, usize, f64)> = HashMap::new();
        let mut order: Vec<String> = Vec::new();

        for result in results {
            let base_score = result
                .score
                .unwrap_or_else(|| if result.success { 1.0 } else { 0.0 });
            let risk = if result.success { 0.0 } else { 1.0 };
            let entry = score_accumulator
                .entry(result.agent_name.clone())
                .or_insert((0.0_f64, 0, 0.0_f64));
            entry.0 += base_score;
            entry.1 += 1;
            entry.2 = entry.2.max(risk);

            if !order.contains(&result.agent_name) {
                order.push(result.agent_name.clone());
            }
        }

        order
            .into_iter()
            .filter_map(|agent| {
                let (total, count, risk) = score_accumulator.remove(&agent)?;
                let average = if count == 0 {
                    0.0
                } else {
                    total / count as f64
                };
                Some(EvaluationScore {
                    agent_name: agent.clone(),
                    round_index,
                    score: average,
                    risk: Some(risk),
                    feedback: None,
                })
            })
            .collect()
    }
}

/// Executes the multi-round evaluation loop while reusing the existing supervisor pipeline.
pub struct MultiAgentEvaluator<S: EvaluationStrategy> {
    supervisor: Supervisor,
    evaluation_strategy: S,
    config: MultiAgentEvaluationConfig,
}

impl<S: EvaluationStrategy> MultiAgentEvaluator<S> {
    /// Construct an evaluator from a supervisor configuration.
    pub fn new(
        supervisor_config: SupervisorConfig,
        evaluation_strategy: S,
        config: MultiAgentEvaluationConfig,
    ) -> Self {
        Self {
            supervisor: Supervisor::new(supervisor_config),
            evaluation_strategy,
            config,
        }
    }

    /// Construct an evaluator with an existing supervisor instance.
    pub fn with_supervisor(
        supervisor: Supervisor,
        evaluation_strategy: S,
        config: MultiAgentEvaluationConfig,
    ) -> Self {
        Self {
            supervisor,
            evaluation_strategy,
            config,
        }
    }

    /// Run the evaluation loop for the provided goal and initial agent set.
    pub async fn run(
        &self,
        goal: &str,
        initial_agents: Vec<String>,
    ) -> Result<MultiAgentEvaluationReport> {
        if initial_agents.is_empty() {
            bail!("initial agent set cannot be empty");
        }

        let mut active_agents = dedupe_preserve_order(initial_agents);
        let mut rounds = Vec::new();
        let mut score_history: HashMap<String, Vec<EvaluationScore>> = HashMap::new();
        let mut previous_top_average: Option<f64> = None;
        let mut terminated_early = false;
        let mut final_agents = active_agents.clone();

        for round_index in 0..self.config.max_rounds {
            if active_agents.is_empty() {
                break;
            }

            let plan = self.supervisor.analyze_goal(goal)?;
            let assignments = self
                .supervisor
                .assign_tasks(&plan, Some(active_agents.clone()))?;
            let results = self.supervisor.execute_plan(assignments.clone()).await?;
            let aggregated = self.supervisor.aggregate_results(results.clone());

            let scores = self.evaluation_strategy.evaluate(
                round_index,
                &plan,
                &assignments,
                &results,
                &aggregated,
            );

            if scores.is_empty() {
                bail!("evaluation strategy returned no scores for round {round_index}");
            }

            for score in &scores {
                score_history
                    .entry(score.agent_name.clone())
                    .or_default()
                    .push(score.clone());
            }

            let next_agents = self.select_next_agents(&scores);
            let top_average = compute_average(&scores, &next_agents);

            let strategy_stop =
                self.evaluation_strategy
                    .should_terminate(round_index, &rounds, &self.config);

            let threshold_stop = match (previous_top_average, self.config.improvement_threshold) {
                (Some(previous), Some(threshold)) => match top_average {
                    Some(current) => (current - previous) <= threshold,
                    None => true,
                },
                _ => false,
            };

            final_agents = if next_agents.is_empty() {
                active_agents.clone()
            } else {
                next_agents.clone()
            };

            rounds.push(MultiAgentRoundReport {
                round_index,
                active_agents: active_agents.clone(),
                plan: plan.clone(),
                assignments,
                results,
                aggregated,
                scores: scores.clone(),
                next_active_agents: next_agents.clone(),
            });

            if strategy_stop || threshold_stop || next_agents.is_empty() {
                terminated_early = strategy_stop || threshold_stop;
                break;
            }

            previous_top_average = top_average;
            active_agents = next_agents;
        }

        Ok(MultiAgentEvaluationReport {
            goal: goal.to_string(),
            rounds,
            terminated_early,
            final_agents,
            score_history,
        })
    }

    fn select_next_agents(&self, scores: &[EvaluationScore]) -> Vec<String> {
        let mut filtered: Vec<&EvaluationScore> = scores
            .iter()
            .filter(|score| {
                self.config
                    .max_risk_score
                    .map_or(true, |limit| score.risk.map_or(true, |risk| risk <= limit))
            })
            .collect();

        filtered.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(Ordering::Equal)
                .then_with(|| a.agent_name.cmp(&b.agent_name))
        });

        let mut seen = HashSet::new();
        let mut next_agents = Vec::new();
        let take = self.config.top_k.max(1);

        for score in filtered {
            if seen.insert(score.agent_name.clone()) {
                next_agents.push(score.agent_name.clone());
                if next_agents.len() >= take {
                    break;
                }
            }
        }

        next_agents
    }
}

fn dedupe_preserve_order(mut agents: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    agents.retain(|agent| seen.insert(agent.clone()));
    agents
}

fn compute_average(scores: &[EvaluationScore], agents: &[String]) -> Option<f64> {
    if agents.is_empty() {
        return None;
    }

    let mut total = 0.0;
    let mut count = 0usize;

    for agent in agents {
        if let Some(score) = scores.iter().find(|entry| &entry.agent_name == agent) {
            total += score.score;
            count += 1;
        }
    }

    if count == 0 {
        None
    } else {
        Some(total / count as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::CoordinationStrategy;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn evaluator_runs_and_filters_agents() {
        let supervisor_config = SupervisorConfig {
            strategy: CoordinationStrategy::Parallel,
            ..Default::default()
        };

        let evaluator = MultiAgentEvaluator::new(
            supervisor_config,
            SimpleEvaluationStrategy::default(),
            MultiAgentEvaluationConfig {
                max_rounds: 2,
                top_k: 2,
                improvement_threshold: None,
                max_risk_score: Some(0.5),
            },
        );

        let report = evaluator
            .run(
                "Implement secure auth",
                vec![
                    "Security".to_string(),
                    "Backend".to_string(),
                    "Frontend".to_string(),
                ],
            )
            .await
            .expect("evaluation should succeed");

        assert!(!report.rounds.is_empty());
        // Risk filter excludes none because synthetic tasks never fail.
        assert_eq!(report.final_agents.len(), 2);
        assert!(
            report
                .score_history
                .get("Security")
                .map(|entries| !entries.is_empty())
                .unwrap_or(false)
        );
    }

    #[tokio::test]
    async fn evaluator_respects_threshold_stop() {
        let supervisor_config = SupervisorConfig {
            strategy: CoordinationStrategy::Sequential,
            ..Default::default()
        };

        let evaluator = MultiAgentEvaluator::new(
            supervisor_config,
            SimpleEvaluationStrategy::default(),
            MultiAgentEvaluationConfig {
                max_rounds: 5,
                top_k: 1,
                improvement_threshold: Some(0.0),
                max_risk_score: None,
            },
        );

        let report = evaluator
            .run(
                "Build a new feature",
                vec!["Generalist".to_string(), "Explorer".to_string()],
            )
            .await
            .expect("evaluation should succeed");

        assert_eq!(report.rounds.len(), 1);
        assert!(report.terminated_early);
    }
}
