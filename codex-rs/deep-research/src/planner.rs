use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;

/// 研究計画
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchPlan {
    /// メイントピック
    pub main_topic: String,
    /// サブクエリのリスト
    pub sub_queries: Vec<String>,
    /// 評価基準
    pub evaluation_criteria: Vec<String>,
    /// 停止条件
    pub stop_conditions: StopConditions,
    /// 必要なエビデンス深度
    pub evidence_depth: u8,
}

/// 停止条件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopConditions {
    /// 最大探索深度
    pub max_depth: u8,
    /// 最大ソース数
    pub max_sources: usize,
    /// 最大時間（秒）
    pub max_duration_secs: Option<u64>,
    /// 収束判定用の最小改善率
    pub min_improvement_threshold: f64,
}

impl Default for StopConditions {
    fn default() -> Self {
        Self {
            max_depth: 3,
            max_sources: 10,
            max_duration_secs: None,
            min_improvement_threshold: 0.1,
        }
    }
}

/// 研究計画生成器
pub struct ResearchPlanner;

impl ResearchPlanner {
    /// トピックから研究計画を生成
    pub fn generate_plan(topic: &str, depth: u8, breadth: usize) -> Result<ResearchPlan> {
        // TODO: LLMを使用して実際に計画を生成
        // 現在はモック実装

        let sub_queries = Self::generate_sub_queries(topic, breadth);
        let evaluation_criteria = Self::generate_evaluation_criteria(topic);

        Ok(ResearchPlan {
            main_topic: topic.to_string(),
            sub_queries,
            evaluation_criteria,
            stop_conditions: StopConditions {
                max_depth: depth,
                max_sources: breadth,
                ..Default::default()
            },
            evidence_depth: depth,
        })
    }

    /// サブクエリを生成
    fn generate_sub_queries(topic: &str, count: usize) -> Vec<String> {
        let mut queries = Vec::new();

        // 基本的なサブクエリパターンを生成
        queries.push(format!("{} 概要", topic));
        queries.push(format!("{} 最新動向", topic));
        queries.push(format!("{} ベストプラクティス", topic));
        queries.push(format!("{} 課題と問題点", topic));
        queries.push(format!("{} 事例研究", topic));
        queries.push(format!("{} 比較分析", topic));

        // 指定された数に調整
        queries.truncate(count);

        // 不足分は基本クエリで埋める
        while queries.len() < count {
            queries.push(format!("{} 詳細情報 {}", topic, queries.len() + 1));
        }

        queries
    }

    /// 評価基準を生成
    fn generate_evaluation_criteria(_topic: &str) -> Vec<String> {
        vec![
            "正確性：事実に基づく情報か".to_string(),
            "一次情報優先：オリジナルソースか".to_string(),
            "偏り多様性：複数の視点を含むか".to_string(),
            "新しさ：最新の情報か".to_string(),
            "信頼性：信頼できるソースか".to_string(),
        ]
    }

    /// 計画を軽量版に縮退
    pub fn downgrade_to_lightweight(plan: &ResearchPlan) -> ResearchPlan {
        let mut lightweight = plan.clone();

        // 深度と幅を縮小
        lightweight.stop_conditions.max_depth = (plan.stop_conditions.max_depth / 2).max(1);
        lightweight.stop_conditions.max_sources = (plan.stop_conditions.max_sources / 2).max(3);
        lightweight.evidence_depth = (plan.evidence_depth / 2).max(1);

        // サブクエリを減らす
        lightweight
            .sub_queries
            .truncate((plan.sub_queries.len() / 2).max(2));

        lightweight
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_generate_plan() {
        let plan = ResearchPlanner::generate_plan("Rustのプロセス分離", 3, 6).unwrap();

        assert_eq!(plan.main_topic, "Rustのプロセス分離");
        assert_eq!(plan.sub_queries.len(), 6);
        assert_eq!(plan.stop_conditions.max_depth, 3);
        assert_eq!(plan.stop_conditions.max_sources, 6);
        assert!(!plan.evaluation_criteria.is_empty());
    }

    #[test]
    fn test_downgrade_to_lightweight() {
        let plan = ResearchPlanner::generate_plan("テストトピック", 4, 10).unwrap();
        let lightweight = ResearchPlanner::downgrade_to_lightweight(&plan);

        assert!(lightweight.stop_conditions.max_depth < plan.stop_conditions.max_depth);
        assert!(lightweight.stop_conditions.max_sources < plan.stop_conditions.max_sources);
        assert!(lightweight.sub_queries.len() < plan.sub_queries.len());
    }

    #[test]
    fn test_sub_queries_generation() {
        let queries = ResearchPlanner::generate_sub_queries("AI技術", 4);
        assert_eq!(queries.len(), 4);
        assert!(queries[0].contains("AI技術"));
    }
}
