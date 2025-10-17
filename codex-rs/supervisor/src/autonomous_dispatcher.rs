// メインエージェントによる自律的サブエージェント呼び出しシステム
use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;

use crate::subagent::AgentType;

/// タスク分類結果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskClassification {
    pub recommended_agent: AgentType,
    pub confidence: f32, // 0.0-1.0
    pub reasoning: String,
    pub alternative_agents: Vec<AgentType>,
}

/// 自動呼び出しトリガー
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoCallTrigger {
    pub keywords: Vec<String>,
    pub agent_type: AgentType,
    pub priority: u8, // 0-255
    pub description: String,
}

/// 自動ディスパッチャー
#[derive(Debug)]
pub struct AutonomousDispatcher {
    triggers: Vec<AutoCallTrigger>,
    classification_cache: HashMap<String, TaskClassification>,
}

impl AutonomousDispatcher {
    pub fn new() -> Self {
        let mut triggers = Vec::new();

        // デフォルトトリガーを登録
        triggers.extend(vec![
            AutoCallTrigger {
                keywords: vec![
                    "analyze code".to_string(),
                    "review code".to_string(),
                    "refactor".to_string(),
                    "implement".to_string(),
                ],
                agent_type: AgentType::CodeExpert,
                priority: 10,
                description: "Code analysis and implementation tasks".to_string(),
            },
            AutoCallTrigger {
                keywords: vec![
                    "security".to_string(),
                    "vulnerability".to_string(),
                    "exploit".to_string(),
                    "CVE".to_string(),
                ],
                agent_type: AgentType::SecurityExpert,
                priority: 20, // 高優先度
                description: "Security review and vulnerability assessment".to_string(),
            },
            AutoCallTrigger {
                keywords: vec![
                    "test".to_string(),
                    "unit test".to_string(),
                    "integration test".to_string(),
                    "coverage".to_string(),
                ],
                agent_type: AgentType::TestingExpert,
                priority: 8,
                description: "Test creation and execution".to_string(),
            },
            AutoCallTrigger {
                keywords: vec![
                    "document".to_string(),
                    "documentation".to_string(),
                    "README".to_string(),
                    "API doc".to_string(),
                ],
                agent_type: AgentType::DocsExpert,
                priority: 5,
                description: "Documentation generation".to_string(),
            },
            AutoCallTrigger {
                keywords: vec![
                    "research".to_string(),
                    "investigate".to_string(),
                    "deep dive".to_string(),
                    "analyze in depth".to_string(),
                ],
                agent_type: AgentType::DeepResearcher,
                priority: 12,
                description: "Deep research and investigation".to_string(),
            },
            AutoCallTrigger {
                keywords: vec![
                    "debug".to_string(),
                    "fix bug".to_string(),
                    "troubleshoot".to_string(),
                    "error".to_string(),
                ],
                agent_type: AgentType::DebugExpert,
                priority: 15,
                description: "Debugging and error resolution".to_string(),
            },
            AutoCallTrigger {
                keywords: vec![
                    "optimize".to_string(),
                    "performance".to_string(),
                    "speed up".to_string(),
                    "efficiency".to_string(),
                ],
                agent_type: AgentType::PerformanceExpert,
                priority: 7,
                description: "Performance optimization".to_string(),
            },
        ]);

        Self {
            triggers,
            classification_cache: HashMap::new(),
        }
    }

    /// タスクを分類してエージェントを推奨
    pub fn classify_task(&mut self, task: &str) -> TaskClassification {
        // キャッシュチェック
        if let Some(cached) = self.classification_cache.get(task) {
            return cached.clone();
        }

        let task_lower = task.to_lowercase();
        let mut matches: Vec<(AgentType, u8, Vec<String>)> = Vec::new();

        // トリガーとマッチング
        for trigger in &self.triggers {
            let mut matched_keywords = Vec::new();
            for keyword in &trigger.keywords {
                if task_lower.contains(&keyword.to_lowercase()) {
                    matched_keywords.push(keyword.clone());
                }
            }

            if !matched_keywords.is_empty() {
                matches.push((
                    trigger.agent_type.clone(),
                    trigger.priority,
                    matched_keywords,
                ));
            }
        }

        // マッチがない場合はGeneralエージェント
        if matches.is_empty() {
            let classification = TaskClassification {
                recommended_agent: AgentType::General,
                confidence: 0.5,
                reasoning: "No specific triggers matched, using General agent".to_string(),
                alternative_agents: vec![],
            };
            self.classification_cache
                .insert(task.to_string(), classification.clone());
            return classification;
        }

        // 優先度順にソート
        matches.sort_by(|a, b| b.1.cmp(&a.1));

        let best_match = &matches[0];
        let confidence = (best_match.2.len() as f32 / 3.0).min(1.0); // 最大3キーワードで100%

        let alternative_agents: Vec<AgentType> = matches
            .iter()
            .skip(1)
            .take(2)
            .map(|(agent, _, _)| agent.clone())
            .collect();

        let reasoning = format!(
            "Matched keywords: {}. Priority: {}",
            best_match.2.join(", "),
            best_match.1
        );

        let classification = TaskClassification {
            recommended_agent: best_match.0.clone(),
            confidence,
            reasoning,
            alternative_agents,
        };

        // キャッシュに保存
        self.classification_cache
            .insert(task.to_string(), classification.clone());

        classification
    }

    /// カスタムトリガーを追加
    pub fn add_trigger(&mut self, trigger: AutoCallTrigger) {
        self.triggers.push(trigger);
        // 優先度順にソート
        self.triggers.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    /// トリガーを削除
    pub fn remove_trigger(&mut self, agent_type: &AgentType) {
        self.triggers.retain(|t| &t.agent_type != agent_type);
    }

    /// 自動呼び出しが必要か判断
    pub fn should_auto_call(&self, task: &str, threshold: f32) -> Option<AgentType> {
        let task_lower = task.to_lowercase();

        for trigger in &self.triggers {
            let match_count = trigger
                .keywords
                .iter()
                .filter(|k| task_lower.contains(&k.to_lowercase()))
                .count();

            let confidence = (match_count as f32 / trigger.keywords.len() as f32).min(1.0);

            if confidence >= threshold {
                return Some(trigger.agent_type.clone());
            }
        }

        None
    }

    /// キャッシュをクリア
    pub fn clear_cache(&mut self) {
        self.classification_cache.clear();
    }

    /// 統計情報を取得
    pub fn get_stats(&self) -> DispatcherStats {
        DispatcherStats {
            total_triggers: self.triggers.len(),
            cache_size: self.classification_cache.len(),
            agent_types: self
                .triggers
                .iter()
                .map(|t| t.agent_type.clone())
                .collect::<std::collections::HashSet<_>>()
                .len(),
        }
    }
}

impl Default for AutonomousDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

/// ディスパッチャー統計
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatcherStats {
    pub total_triggers: usize,
    pub cache_size: usize,
    pub agent_types: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_classify_task() {
        let mut dispatcher = AutonomousDispatcher::new();

        let task = "Please analyze this code for security vulnerabilities";
        let classification = dispatcher.classify_task(task);

        assert_eq!(classification.recommended_agent, AgentType::SecurityExpert);
        assert!(classification.confidence > 0.0);
    }

    #[test]
    fn test_classify_task_multiple_keywords() {
        let mut dispatcher = AutonomousDispatcher::new();

        let task = "Implement unit tests for the authentication system";
        let classification = dispatcher.classify_task(task);

        // "test"と"unit test"がマッチするはず
        assert_eq!(classification.recommended_agent, AgentType::TestingExpert);
    }

    #[test]
    fn test_classify_task_no_match() {
        let mut dispatcher = AutonomousDispatcher::new();

        let task = "Hello world";
        let classification = dispatcher.classify_task(task);

        assert_eq!(classification.recommended_agent, AgentType::General);
        assert_eq!(classification.confidence, 0.5);
    }

    #[test]
    fn test_should_auto_call() {
        let dispatcher = AutonomousDispatcher::new();

        let task1 = "Please review the security of this code";
        let result1 = dispatcher.should_auto_call(task1, 0.3);
        assert_eq!(result1, Some(AgentType::SecurityExpert));

        let task2 = "Hello";
        let result2 = dispatcher.should_auto_call(task2, 0.5);
        assert_eq!(result2, None);
    }

    #[test]
    fn test_add_custom_trigger() {
        let mut dispatcher = AutonomousDispatcher::new();
        let initial_count = dispatcher.triggers.len();

        let custom_trigger = AutoCallTrigger {
            keywords: vec!["custom".to_string()],
            agent_type: AgentType::General,
            priority: 100,
            description: "Custom trigger".to_string(),
        };

        dispatcher.add_trigger(custom_trigger);
        assert_eq!(dispatcher.triggers.len(), initial_count + 1);

        // 優先度順にソートされているか確認
        assert_eq!(dispatcher.triggers[0].priority, 100);
    }

    #[test]
    fn test_cache() {
        let mut dispatcher = AutonomousDispatcher::new();

        let task = "Optimize performance";
        dispatcher.classify_task(task);
        assert_eq!(dispatcher.classification_cache.len(), 1);

        // 2回目は同じ結果が返る
        let classification = dispatcher.classify_task(task);
        assert_eq!(
            classification.recommended_agent,
            AgentType::PerformanceExpert
        );

        dispatcher.clear_cache();
        assert_eq!(dispatcher.classification_cache.len(), 0);
    }
}
