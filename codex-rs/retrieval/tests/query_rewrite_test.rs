//! Bilingual query rewrite integration tests.
//!
//! Tests for Chinese query detection and translation.

use codex_retrieval::Result;
use codex_retrieval::RetrievalConfig;
use codex_retrieval::RetrievalFeatures;
use codex_retrieval::RetrievalService;
use codex_retrieval::query::LlmRewriter;
use codex_retrieval::query::QueryRewriter;
use codex_retrieval::query::RewrittenQuery;
use codex_retrieval::query::SimpleRewriter;
use codex_retrieval::query::Translator;
use codex_retrieval::query::preprocessor::contains_chinese;

use async_trait::async_trait;
use tempfile::TempDir;

// ==== Chinese Detection Tests ====

#[test]
fn test_chinese_detection_pure_chinese() {
    // Pure Chinese queries
    assert!(contains_chinese("用户认证"));
    assert!(contains_chinese("查找函数"));
    assert!(contains_chinese("错误处理"));
    assert!(contains_chinese("配置文件"));
}

#[test]
fn test_chinese_detection_mixed() {
    // Mixed Chinese + English
    assert!(contains_chinese("用户 authentication"));
    assert!(contains_chinese("find 函数"));
    assert!(contains_chinese("error 处理"));
    assert!(contains_chinese("config 文件解析"));
}

#[test]
fn test_chinese_detection_pure_english() {
    // Pure English queries - should not detect Chinese
    assert!(!contains_chinese("user authentication"));
    assert!(!contains_chinese("find function"));
    assert!(!contains_chinese("error handling"));
    assert!(!contains_chinese("config file"));
}

#[test]
fn test_chinese_detection_code_identifiers() {
    // Code identifiers with Chinese comments
    assert!(contains_chinese("getUserName // 获取用户名"));
    assert!(contains_chinese("handleError /* 处理错误 */"));

    // Pure code identifiers
    assert!(!contains_chinese("getUserName"));
    assert!(!contains_chinese("handle_error"));
}

#[test]
fn test_chinese_detection_unicode_ranges() {
    // CJK Unified Ideographs (4E00-9FFF) - the main range we detect
    assert!(contains_chinese("\u{4E00}")); // First CJK char (一)
    assert!(contains_chinese("\u{9FFF}")); // Last CJK char

    // Common Chinese characters
    assert!(contains_chinese("中")); // 4E2D
    assert!(contains_chinese("国")); // 56FD
    assert!(contains_chinese("人")); // 4EBA

    // CJK Extension A (3400-4DBF) - rare characters, not detected
    assert!(!contains_chinese("\u{3400}"));

    // Hiragana/Katakana (Japanese) - not in CJK Unified, not detected
    assert!(!contains_chinese("\u{3041}")); // Hiragana
    assert!(!contains_chinese("\u{30A1}")); // Katakana
}

// ==== Query Rewriting Tests ====

#[tokio::test]
async fn test_simple_rewriter_english_query() {
    let rewriter = SimpleRewriter::new();

    let result = rewriter.rewrite("find user authentication").await.unwrap();
    assert_eq!(result.original, "find user authentication");
    assert_eq!(result.rewritten, "find user authentication");
    assert!(!result.was_translated);
}

#[tokio::test]
async fn test_simple_rewriter_chinese_query() {
    let rewriter = SimpleRewriter::new();

    // SimpleRewriter doesn't actually translate, but marks for translation
    let result = rewriter.rewrite("查找用户认证").await.unwrap();
    assert_eq!(result.original, "查找用户认证");
    // Without LLM, rewritten is same as original
    assert_eq!(result.rewritten, "查找用户认证");
    // was_translated is false because SimpleRewriter can't translate
    assert!(!result.was_translated);
}

#[tokio::test]
async fn test_simple_rewriter_with_expansion() {
    let rewriter = SimpleRewriter::new().with_expansion(true);

    let result = rewriter.rewrite("test function").await.unwrap();
    assert!(!result.expansions.is_empty());

    // Should expand "function" to include synonyms
    assert!(result.has_expansion("fn"));
    assert!(result.has_expansion("method"));
}

#[tokio::test]
async fn test_rewritten_query_effective() {
    let query = RewrittenQuery::translated("用户认证", "user authentication")
        .with_expansions(vec!["login".to_string(), "authorize".to_string()]);

    assert_eq!(
        query.effective_query(),
        "user authentication login authorize"
    );
}

// ==== Mock Translator for LLM Rewriter Tests ====

struct MockTranslator {
    translations: std::collections::HashMap<String, String>,
}

impl MockTranslator {
    fn new() -> Self {
        let mut translations = std::collections::HashMap::new();
        translations.insert("用户认证".to_string(), "user authentication".to_string());
        translations.insert("查找函数".to_string(), "find function".to_string());
        translations.insert("错误处理".to_string(), "error handling".to_string());
        translations.insert("配置文件".to_string(), "config file".to_string());
        translations.insert(
            "如何处理异常".to_string(),
            "how to handle exceptions".to_string(),
        );
        Self { translations }
    }
}

#[async_trait]
impl Translator for MockTranslator {
    async fn translate_to_english(&self, text: &str) -> Result<String> {
        Ok(self
            .translations
            .get(text)
            .cloned()
            .unwrap_or_else(|| format!("[translated] {text}")))
    }

    fn detect_language(&self, text: &str) -> Option<String> {
        if contains_chinese(text) {
            Some("zh".to_string())
        } else {
            Some("en".to_string())
        }
    }
}

#[tokio::test]
async fn test_llm_rewriter_translates_chinese() {
    let translator = MockTranslator::new();
    let rewriter = LlmRewriter::new(translator);

    let result = rewriter.rewrite("用户认证").await.unwrap();
    assert_eq!(result.original, "用户认证");
    assert_eq!(result.rewritten, "user authentication");
    assert!(result.was_translated);
}

#[tokio::test]
async fn test_llm_rewriter_keeps_english() {
    let translator = MockTranslator::new();
    let rewriter = LlmRewriter::new(translator);

    let result = rewriter.rewrite("user authentication").await.unwrap();
    assert_eq!(result.original, "user authentication");
    assert_eq!(result.rewritten, "user authentication");
    assert!(!result.was_translated);
}

#[tokio::test]
async fn test_llm_rewriter_with_expansion() {
    let translator = MockTranslator::new();
    let rewriter = LlmRewriter::new(translator);

    let result = rewriter.rewrite("查找函数").await.unwrap();
    assert_eq!(result.rewritten, "find function");
    assert!(result.was_translated);
    // Should have expansions for "function"
    assert!(result.has_expansion("fn"));
}

// ==== Service Integration Tests ====

#[tokio::test]
async fn test_service_with_query_rewrite() {
    let dir = TempDir::new().unwrap();
    let mut config = RetrievalConfig::default();
    config.data_dir = dir.path().to_path_buf();

    let features = RetrievalFeatures {
        code_search: true,
        query_rewrite: true,
        ..Default::default()
    };

    let service = RetrievalService::new(config, features).await.unwrap();

    // Verify rewrite is available
    let result = service.rewrite_query("test function").await;
    assert!(result.is_some());
}

#[tokio::test]
async fn test_service_without_query_rewrite() {
    let dir = TempDir::new().unwrap();
    let mut config = RetrievalConfig::default();
    config.data_dir = dir.path().to_path_buf();

    let features = RetrievalFeatures {
        code_search: true,
        query_rewrite: false,
        ..Default::default()
    };

    let service = RetrievalService::new(config, features).await.unwrap();

    // Rewrite should be disabled
    let result = service.rewrite_query("test function").await;
    assert!(result.is_none());
}

// ==== Common Programming Terms Tests ====

#[test]
fn test_common_programming_terms_chinese() {
    // Common programming terms that should be detected as Chinese
    let terms = [
        ("函数", true),
        ("变量", true),
        ("类", true),
        ("方法", true),
        ("接口", true),
        ("模块", true),
        ("错误", true),
        ("异常", true),
        ("配置", true),
        ("测试", true),
    ];

    for (term, expected) in terms {
        assert_eq!(
            contains_chinese(term),
            expected,
            "Term '{}' should {} Chinese",
            term,
            if expected { "be" } else { "not be" }
        );
    }
}

#[tokio::test]
async fn test_expansion_programming_terms() {
    let rewriter = SimpleRewriter::new().with_expansion(true);

    // Test common programming term expansions
    let test_cases = [
        ("function handler", vec!["fn", "method"]),
        ("error handling", vec!["err", "exception", "panic"]),
        ("user authentication", vec!["login", "authorize"]),
        ("database query", vec!["db", "storage"]),
        ("test cases", vec!["spec", "unittest"]),
    ];

    for (query, expected_expansions) in test_cases {
        let result = rewriter.rewrite(query).await.unwrap();
        for exp in expected_expansions {
            assert!(
                result.has_expansion(exp),
                "Query '{}' should expand to include '{}'",
                query,
                exp
            );
        }
    }
}
