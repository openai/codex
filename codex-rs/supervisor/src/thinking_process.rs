// サブエージェントの思考プロセス明示化システム
use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;
use std::collections::VecDeque;

use crate::subagent::AgentType;

/// 思考ステップ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingStep {
    pub step_id: String,
    pub timestamp: String,
    pub step_type: ThinkingStepType,
    pub content: String,
    pub confidence: f32, // 0.0-1.0
    pub reasoning: String,
}

/// 思考ステップのタイプ
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThinkingStepType {
    /// 問題分析
    ProblemAnalysis,
    /// 仮説生成
    HypothesisGeneration,
    /// 情報収集
    InformationGathering,
    /// 推論
    Reasoning,
    /// 判断
    Decision,
    /// アクション計画
    ActionPlanning,
    /// 実行
    Execution,
    /// 検証
    Verification,
    /// 結論
    Conclusion,
}

/// 思考プロセス
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingProcess {
    pub agent_type: AgentType,
    pub task_id: String,
    pub steps: VecDeque<ThinkingStep>,
    pub max_steps: usize,
    pub current_phase: ThinkingStepType,
    pub overall_confidence: f32,
}

impl ThinkingProcess {
    pub fn new(agent_type: AgentType, task_id: String, max_steps: usize) -> Self {
        Self {
            agent_type,
            task_id,
            steps: VecDeque::new(),
            max_steps,
            current_phase: ThinkingStepType::ProblemAnalysis,
            overall_confidence: 0.0,
        }
    }

    /// 思考ステップを追加
    pub fn add_step(&mut self, step: ThinkingStep) {
        // サイズ制限チェック
        if self.steps.len() >= self.max_steps {
            self.steps.pop_front();
        }

        self.current_phase = step.step_type.clone();
        self.steps.push_back(step);

        // 全体の信頼度を更新
        self.update_overall_confidence();
    }

    /// 全体の信頼度を更新
    fn update_overall_confidence(&mut self) {
        if self.steps.is_empty() {
            self.overall_confidence = 0.0;
            return;
        }

        let sum: f32 = self.steps.iter().map(|s| s.confidence).sum();
        self.overall_confidence = sum / self.steps.len() as f32;
    }

    /// 思考プロセスのサマリーを取得
    pub fn get_summary(&self) -> String {
        let mut summary = format!(
            "[{}] Task: {} (Confidence: {:.1}%)\n",
            self.agent_type,
            self.task_id,
            self.overall_confidence * 100.0
        );

        summary.push_str(&format!("Current Phase: {:?}\n", self.current_phase));
        summary.push_str(&format!("Total Steps: {}\n\n", self.steps.len()));

        for (i, step) in self.steps.iter().enumerate() {
            summary.push_str(&format!(
                "Step {}: {:?} (Confidence: {:.1}%)\n",
                i + 1,
                step.step_type,
                step.confidence * 100.0
            ));
            summary.push_str(&format!("  Content: {}\n", step.content));
            summary.push_str(&format!("  Reasoning: {}\n\n", step.reasoning));
        }

        summary
    }

    /// 特定のステップタイプの思考を取得
    pub fn get_steps_by_type(&self, step_type: ThinkingStepType) -> Vec<&ThinkingStep> {
        self.steps
            .iter()
            .filter(|s| s.step_type == step_type)
            .collect()
    }

    /// 最新の思考ステップを取得
    pub fn get_latest_step(&self) -> Option<&ThinkingStep> {
        self.steps.back()
    }

    /// 思考プロセスをクリア
    pub fn clear(&mut self) {
        self.steps.clear();
        self.current_phase = ThinkingStepType::ProblemAnalysis;
        self.overall_confidence = 0.0;
    }
}

/// 思考プロセスビルダー
pub struct ThinkingStepBuilder {
    step_id: String,
    timestamp: String,
    step_type: ThinkingStepType,
    content: String,
    confidence: f32,
    reasoning: String,
}

impl ThinkingStepBuilder {
    pub fn new(step_type: ThinkingStepType) -> Self {
        Self {
            step_id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            step_type,
            content: String::new(),
            confidence: 0.5, // デフォルト50%
            reasoning: String::new(),
        }
    }

    pub fn content(mut self, content: String) -> Self {
        self.content = content;
        self
    }

    pub fn confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    pub fn reasoning(mut self, reasoning: String) -> Self {
        self.reasoning = reasoning;
        self
    }

    pub fn build(self) -> ThinkingStep {
        ThinkingStep {
            step_id: self.step_id,
            timestamp: self.timestamp,
            step_type: self.step_type,
            content: self.content,
            confidence: self.confidence,
            reasoning: self.reasoning,
        }
    }
}

/// 思考プロセス管理システム
#[derive(Debug)]
pub struct ThinkingProcessManager {
    processes: std::collections::HashMap<String, ThinkingProcess>,
}

impl ThinkingProcessManager {
    pub fn new() -> Self {
        Self {
            processes: std::collections::HashMap::new(),
        }
    }

    /// 新しい思考プロセスを開始
    pub fn start_process(
        &mut self,
        agent_type: AgentType,
        task_id: String,
        max_steps: usize,
    ) -> &mut ThinkingProcess {
        let process = ThinkingProcess::new(agent_type, task_id.clone(), max_steps);
        self.processes.insert(task_id.clone(), process);
        self.processes.get_mut(&task_id).unwrap()
    }

    /// 思考プロセスを取得
    pub fn get_process(&self, task_id: &str) -> Option<&ThinkingProcess> {
        self.processes.get(task_id)
    }

    /// 思考プロセスを取得（可変）
    pub fn get_process_mut(&mut self, task_id: &str) -> Option<&mut ThinkingProcess> {
        self.processes.get_mut(task_id)
    }

    /// 思考プロセスを削除
    pub fn remove_process(&mut self, task_id: &str) -> Option<ThinkingProcess> {
        self.processes.remove(task_id)
    }

    /// 全ての思考プロセスのサマリーを取得
    pub fn get_all_summaries(&self) -> String {
        let mut summary = String::new();
        summary.push_str("=== All Thinking Processes ===\n\n");

        for (task_id, process) in &self.processes {
            summary.push_str(&format!("Task ID: {}\n", task_id));
            summary.push_str(&process.get_summary());
            summary.push_str("\n---\n\n");
        }

        summary
    }

    /// 思考プロセス数を取得
    pub fn count(&self) -> usize {
        self.processes.len()
    }

    /// 全ての思考プロセスをクリア
    pub fn clear_all(&mut self) {
        self.processes.clear();
    }
}

impl Default for ThinkingProcessManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_thinking_step_builder() {
        let step = ThinkingStepBuilder::new(ThinkingStepType::ProblemAnalysis)
            .content("Analyzing the problem...".to_string())
            .confidence(0.8)
            .reasoning("Based on initial assessment".to_string())
            .build();

        assert_eq!(step.step_type, ThinkingStepType::ProblemAnalysis);
        assert_eq!(step.content, "Analyzing the problem...");
        assert_eq!(step.confidence, 0.8);
    }

    #[test]
    fn test_thinking_process() {
        let mut process = ThinkingProcess::new(AgentType::CodeExpert, "task-1".to_string(), 10);

        let step1 = ThinkingStepBuilder::new(ThinkingStepType::ProblemAnalysis)
            .content("Step 1".to_string())
            .confidence(0.7)
            .reasoning("Reason 1".to_string())
            .build();

        let step2 = ThinkingStepBuilder::new(ThinkingStepType::Reasoning)
            .content("Step 2".to_string())
            .confidence(0.9)
            .reasoning("Reason 2".to_string())
            .build();

        process.add_step(step1);
        process.add_step(step2);

        assert_eq!(process.steps.len(), 2);
        assert_eq!(process.current_phase, ThinkingStepType::Reasoning);
        assert!((process.overall_confidence - 0.8).abs() < 0.01); // (0.7 + 0.9) / 2 = 0.8
    }

    #[test]
    fn test_thinking_process_manager() {
        let mut manager = ThinkingProcessManager::new();

        manager.start_process(AgentType::CodeExpert, "task-1".to_string(), 10);
        manager.start_process(AgentType::SecurityExpert, "task-2".to_string(), 10);

        assert_eq!(manager.count(), 2);

        let process = manager.get_process("task-1");
        assert!(process.is_some());
        assert_eq!(process.unwrap().agent_type, AgentType::CodeExpert);

        manager.remove_process("task-1");
        assert_eq!(manager.count(), 1);
    }

    #[test]
    fn test_get_steps_by_type() {
        let mut process = ThinkingProcess::new(AgentType::CodeExpert, "task-1".to_string(), 10);

        let step1 = ThinkingStepBuilder::new(ThinkingStepType::ProblemAnalysis)
            .content("Analysis 1".to_string())
            .build();

        let step2 = ThinkingStepBuilder::new(ThinkingStepType::Reasoning)
            .content("Reasoning 1".to_string())
            .build();

        let step3 = ThinkingStepBuilder::new(ThinkingStepType::ProblemAnalysis)
            .content("Analysis 2".to_string())
            .build();

        process.add_step(step1);
        process.add_step(step2);
        process.add_step(step3);

        let analysis_steps = process.get_steps_by_type(ThinkingStepType::ProblemAnalysis);
        assert_eq!(analysis_steps.len(), 2);
    }
}
