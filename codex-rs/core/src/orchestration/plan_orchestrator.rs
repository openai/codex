//! Plan-aware orchestrator
//!
//! Enhances AutoOrchestrator to accept PlanBlock, emit telemetry, and trigger webhooks.

use crate::agents::AgentRuntime;
use crate::orchestration::{
    AutoOrchestrator, CollaborationStore, OrchestratedResult, TaskAnalyzer,
};
use crate::plan::PlanBlock;
use crate::telemetry::{EventType, TelemetryEvent};
use crate::webhooks::{WebhookConfig, WebhookPayload};
use anyhow::{Context, Result};
use std::sync::Arc;
use tracing::debug;
use tracing::info;

/// Plan-aware orchestrator
pub struct PlanOrchestrator {
    /// Underlying auto-orchestrator
    auto_orchestrator: AutoOrchestrator,

    /// Webhook configurations (optional)
    webhook_configs: Vec<WebhookConfig>,
}

impl PlanOrchestrator {
    /// Create a new Plan orchestrator
    pub fn new(
        runtime: Arc<AgentRuntime>,
        collaboration_store: Arc<CollaborationStore>,
        workspace_dir: std::path::PathBuf,
        webhook_configs: Vec<WebhookConfig>,
    ) -> Self {
        let auto_orchestrator = AutoOrchestrator::new(runtime, collaboration_store, workspace_dir);

        Self {
            auto_orchestrator,
            webhook_configs,
        }
    }

    /// Execute a plan with telemetry and webhooks
    pub async fn execute_plan(&self, plan: &PlanBlock) -> Result<OrchestratedResult> {
        // Verify plan is approved
        if !plan.can_execute() {
            anyhow::bail!("Plan {} is not approved (state: {})", plan.id, plan.state);
        }

        info!("Executing plan {} with orchestrator", plan.id);

        // Emit telemetry: execution started
        self.emit_telemetry_event(EventType::ExecStart, plan, None)
            .await;

        // Trigger webhook: execution started
        self.trigger_webhook("exec.start", plan, None).await;

        // Convert plan goal to task description
        let task = format!(
            "{}\n\nApproach: {}\n\nWork Items: {}",
            plan.goal,
            plan.approach,
            plan.work_items
                .iter()
                .map(|w| format!("- {}: {:?}", w.name, w.files_touched))
                .collect::<Vec<_>>()
                .join("\n")
        );

        // Analyze task complexity
        let analyzer = TaskAnalyzer::new(0.7);
        let analysis = analyzer.analyze(&task);

        info!("Task complexity: {:.2}", analysis.complexity_score);

        // Execute with auto-orchestrator if beneficial
        let result = if analysis.complexity_score > 0.7 {
            self.auto_orchestrator
                .orchestrate(analysis, task)
                .await
                .context("Orchestrated execution failed")?
        } else {
            // Simple task, no orchestration needed
            info!("Task complexity below threshold, skipping orchestration");
            OrchestratedResult {
                was_orchestrated: false,
                agents_used: vec![],
                execution_summary: "Task executed without orchestration".to_string(),
                agent_results: vec![],
                total_execution_time_secs: 0.0,
                task_analysis: analysis,
            }
        };

        // Emit telemetry: execution completed
        self.emit_telemetry_event(EventType::ExecResult, plan, Some(&result))
            .await;

        // Trigger webhook: execution completed
        self.trigger_webhook("exec.result", plan, Some(&result))
            .await;

        Ok(result)
    }

    /// Emit telemetry event
    async fn emit_telemetry_event(
        &self,
        event_type: EventType,
        Plan: &PlanBlock,
        result: Option<&OrchestratedResult>,
    ) {
        if let Some(collector) = crate::telemetry::instance() {
            let mut event = TelemetryEvent::new(event_type).with_plan_id(&plan.id);

            if let Some(user) = &plan.created_by {
                event = event.with_user_id(user);
            }

            // Add metadata
            event = event.with_metadata("mode", Plan.mode.to_string());

            if let Some(r) = result {
                event = event.with_metadata("was_orchestrated", r.was_orchestrated);
                event = event.with_metadata("agents_used", r.agents_used.len());
                event = event.with_metadata("execution_time_secs", r.total_execution_time_secs);
            }

            if let Err(e) = collector.record(event).await {
                debug!("Failed to record telemetry: {}", e);
            }
        }
    }

    /// Trigger webhook
    async fn trigger_webhook(
        &self,
        _event_name: &str,
        Plan: &PlanBlock,
        result: Option<&OrchestratedResult>,
    ) {
        if self.webhook_configs.is_empty() {
            return;
        }

        let summary = if let Some(r) = result {
            format!(
                "Execution completed. Orchestrated: {}. Agents: {}. Time: {:.1}s",
                r.was_orchestrated,
                r.agents_used.join(", "),
                r.total_execution_time_secs
            )
        } else {
            format!("Execution started for Plan: {}", Plan.title)
        };

        let mut payload = WebhookPayload::new(
            Plan.id.clone(),
            Plan.title.clone(),
            Plan.state.clone(),
            summary,
        );

        payload = payload.with_mode(Plan.mode.to_string());
        payload = payload.with_artifacts(Plan.artifacts.clone());

        // Send webhooks asynchronously
        for config in &self.webhook_configs {
            let config = config.clone();
            let payload = payload.clone();

            tokio::spawn(async move {
                if let Err(e) = crate::webhooks::send(&config, &payload).await {
                    debug!("Failed to send webhook: {}", e);
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::state::PlanState;

    fn create_test_runtime() -> Arc<AgentRuntime> {
        Arc::new(AgentRuntime::default())
    }

    fn create_approved_Plan() -> PlanBlock {
        let mut bp = PlanBlock::new("Test Plan".to_string(), "test-bp".to_string());
        bp.state = PlanState::Approved {
            approved_by: "test-user".to_string(),
            approved_at: chrono::Utc::now(),
        };
        bp.approach = "Test approach".to_string();
        bp
    }

    #[test]
    fn test_Plan_orchestrator_creation() {
        let runtime = create_test_runtime();
        let workspace = std::path::PathBuf::from("/tmp");
        let collaboration_store = Arc::new(CollaborationStore::new());
        let webhooks = vec![];

        let _orchestrator =
            PlanOrchestrator::new(runtime, collaboration_store, workspace, webhooks);
    }

    #[tokio::test]
    async fn test_execute_unapproved_fails() {
        let runtime = create_test_runtime();
        let workspace = std::path::PathBuf::from("/tmp");
        let collaboration_store = Arc::new(CollaborationStore::new());
        let orchestrator = PlanOrchestrator::new(runtime, collaboration_store, workspace, vec![]);

        let bp = PlanBlock::new("Test".to_string(), "test".to_string());

        let result = orchestrator.execute_Plan(&bp).await;
        assert!(result.is_err());
    }
}
