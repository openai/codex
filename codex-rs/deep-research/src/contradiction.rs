use crate::types::Finding;
use crate::types::Source;
use serde::Deserialize;
use serde::Serialize;

/// 矛盾検出結果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContradictionReport {
    /// 矛盾が見つかった件数
    pub contradiction_count: usize,
    /// 矛盾の詳細
    pub contradictions: Vec<Contradiction>,
    /// ドメイン多様性スコア（0.0〜1.0）
    pub diversity_score: f64,
}

/// 個別の矛盾
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contradiction {
    /// 矛盾するファインディングのインデックス
    pub finding_indices: Vec<usize>,
    /// 矛盾の説明
    pub description: String,
    /// 信頼度の差
    pub confidence_delta: f64,
}

/// 反証チェッカー
pub struct ContradictionChecker;

impl ContradictionChecker {
    /// ファインディング間の矛盾をチェック
    pub fn check_contradictions(findings: &[Finding]) -> ContradictionReport {
        let mut contradictions = Vec::new();

        // 簡易的な矛盾検出（実際はLLMや意味解析を使用）
        for (i, finding_a) in findings.iter().enumerate() {
            for (j, finding_b) in findings.iter().enumerate().skip(i + 1) {
                if Self::detect_contradiction(finding_a, finding_b) {
                    let confidence_delta = (finding_a.confidence - finding_b.confidence).abs();

                    contradictions.push(Contradiction {
                        finding_indices: vec![i, j],
                        description: format!(
                            "矛盾: ファインディング {} と {} の間で矛盾を検出",
                            i, j
                        ),
                        confidence_delta,
                    });
                }
            }
        }

        ContradictionReport {
            contradiction_count: contradictions.len(),
            contradictions,
            diversity_score: 0.0, // 後で計算
        }
    }

    /// 2つのファインディング間で矛盾を検出（簡易版）
    fn detect_contradiction(finding_a: &Finding, finding_b: &Finding) -> bool {
        // TODO: より高度な矛盾検出（意味解析、LLM使用）
        // 現在は信頼度の差が大きく、内容が異なる場合を矛盾とみなす

        if finding_a.content == finding_b.content {
            return false;
        }

        let confidence_delta = (finding_a.confidence - finding_b.confidence).abs();

        // 信頼度の差が0.3以上の場合、矛盾の可能性
        confidence_delta > 0.3
    }

    /// ソースのドメイン多様性を計算
    pub fn calculate_diversity_score(sources: &[Source]) -> f64 {
        if sources.is_empty() {
            return 0.0;
        }

        let unique_domains: std::collections::HashSet<String> = sources
            .iter()
            .filter_map(|s| Self::extract_domain(&s.url))
            .collect();

        unique_domains.len() as f64 / sources.len() as f64
    }

    /// URLからドメインを抽出
    fn extract_domain(url: &str) -> Option<String> {
        url.split("://")
            .nth(1)?
            .split('/')
            .next()
            .map(|s| s.to_string())
    }

    /// 複数ドメインでの相互裏付けをチェック
    pub fn verify_cross_domain(finding: &Finding, sources: &[Source]) -> bool {
        let domains: std::collections::HashSet<String> = finding
            .sources
            .iter()
            .filter_map(|url| {
                sources
                    .iter()
                    .find(|s| &s.url == url)
                    .and_then(|s| Self::extract_domain(&s.url))
            })
            .collect();

        // 最低2つの異なるドメインからの裏付けが必要
        domains.len() >= 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_contradictions() {
        let findings = vec![
            Finding {
                content: "Rust は安全性を重視する".to_string(),
                sources: vec!["https://example.com/1".to_string()],
                confidence: 0.9,
            },
            Finding {
                content: "Rust はパフォーマンスを重視する".to_string(),
                sources: vec!["https://example.com/2".to_string()],
                confidence: 0.5,
            },
        ];

        let report = ContradictionChecker::check_contradictions(&findings);

        // 信頼度の差が0.4なので矛盾として検出される
        assert!(report.contradiction_count > 0);
    }

    #[test]
    fn test_calculate_diversity_score() {
        let sources = vec![
            Source {
                url: "https://example.com/1".to_string(),
                title: "Test 1".to_string(),
                snippet: "Snippet 1".to_string(),
                relevance_score: 0.9,
            },
            Source {
                url: "https://another.com/2".to_string(),
                title: "Test 2".to_string(),
                snippet: "Snippet 2".to_string(),
                relevance_score: 0.8,
            },
            Source {
                url: "https://example.com/3".to_string(),
                title: "Test 3".to_string(),
                snippet: "Snippet 3".to_string(),
                relevance_score: 0.7,
            },
        ];

        let score = ContradictionChecker::calculate_diversity_score(&sources);

        // 3つのソース、2つのユニークドメイン → 2/3 ≈ 0.666
        assert!((score - 0.666).abs() < 0.01);
    }

    #[test]
    fn test_verify_cross_domain() {
        let sources = vec![
            Source {
                url: "https://example.com/1".to_string(),
                title: "Test 1".to_string(),
                snippet: "Snippet 1".to_string(),
                relevance_score: 0.9,
            },
            Source {
                url: "https://another.com/2".to_string(),
                title: "Test 2".to_string(),
                snippet: "Snippet 2".to_string(),
                relevance_score: 0.8,
            },
        ];

        let finding = Finding {
            content: "Test content".to_string(),
            sources: vec![
                "https://example.com/1".to_string(),
                "https://another.com/2".to_string(),
            ],
            confidence: 0.9,
        };

        assert!(ContradictionChecker::verify_cross_domain(
            &finding, &sources
        ));
    }
}
