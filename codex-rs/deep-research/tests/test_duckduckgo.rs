// DuckDuckGo実装の統合テスト
use codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR;
use codex_deep_research::WebSearchProvider;
use std::env;

#[tokio::test]
async fn test_duckduckgo_search_real() {
    // Skip test if network is disabled in sandbox
    if env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return;
    }

    // APIキーを削除してDuckDuckGoを強制使用
    env::remove_var("BRAVE_API_KEY");
    env::remove_var("GOOGLE_API_KEY");
    env::remove_var("GOOGLE_CSE_ID");
    env::remove_var("BING_API_KEY");

    let provider = WebSearchProvider::default();

    // 実際のDuckDuckGo検索を実行
    let results = provider
        .duckduckgo_search_real("Rust async programming", 5)
        .await
        .expect("DuckDuckGo検索が失敗しました");

    println!("🔍 DuckDuckGo検索結果:");
    println!("検索クエリ: Rust async programming");
    println!("取得件数: {}", results.len());
    println!();

    for (i, result) in results.iter().enumerate() {
        println!("結果 #{}", i + 1);
        println!("  タイトル: {}", result.title);
        println!("  URL: {}", result.url);
        println!("  スニペット: {}", result.snippet);
        println!("  関連性スコア: {}", result.relevance_score);
        println!();
    }

    // 基本的なアサーション
    assert!(!results.is_empty(), "検索結果が0件です");
    assert!(results.len() <= 5, "結果が5件を超えています");

    // 各結果の妥当性チェック
    for result in &results {
        assert!(!result.title.is_empty(), "タイトルが空です");
        assert!(!result.url.is_empty(), "URLが空です");
        assert!(result.relevance_score > 0.0, "関連性スコアが0以下です");
    }

    println!("✅ DuckDuckGo検索テスト成功！");
}

#[tokio::test]
async fn test_web_search_fallback_chain() {
    // Skip test if network is disabled in sandbox
    if env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return;
    }

    // APIキーを削除
    env::remove_var("BRAVE_API_KEY");
    env::remove_var("GOOGLE_API_KEY");
    env::remove_var("GOOGLE_CSE_ID");
    env::remove_var("BING_API_KEY");

    let provider = WebSearchProvider::default();

    // call_search_api経由でフォールバックチェーンをテスト
    // （内部でduckduckgo_search_realが呼ばれる）
    let sources = provider
        .search("Rust ownership", 3)
        .await
        .expect("検索が失敗しました");

    println!("🔗 フォールバックチェーンテスト:");
    println!("検索クエリ: Rust ownership");
    println!("取得件数: {}", sources.len());
    println!();

    for (i, source) in sources.iter().enumerate() {
        println!("ソース #{}", i + 1);
        println!("  タイトル: {}", source.title);
        println!("  URL: {}", source.url);
        println!("  スニペット: {}", source.snippet);
        println!();
    }

    assert!(sources.len() <= 3, "結果が3件を超えています");
    println!("✅ フォールバックチェーンテスト成功！");
}

#[tokio::test]
async fn test_multiple_queries() {
    // Skip test if network is disabled in sandbox
    if env::var(CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
        println!(
            "Skipping test because it cannot execute when network is disabled in a Codex sandbox."
        );
        return;
    }

    env::remove_var("BRAVE_API_KEY");
    env::remove_var("GOOGLE_API_KEY");
    env::remove_var("GOOGLE_CSE_ID");
    env::remove_var("BING_API_KEY");

    let provider = WebSearchProvider::default();

    let queries = vec![
        "Rust tokio tutorial",
        "async await Rust",
        "Rust web framework",
    ];

    println!("🔍 複数クエリテスト:");
    println!();

    for query in queries {
        let results = provider
            .duckduckgo_search_real(query, 3)
            .await
            .expect(&format!("「{}」の検索が失敗しました", query));

        println!("クエリ: {}", query);
        println!("結果数: {}", results.len());

        if !results.is_empty() {
            println!("  最初の結果: {}", results[0].title);
        }
        println!();

        assert!(!results.is_empty(), "検索結果が0件です: {}", query);
    }

    println!("✅ 複数クエリテスト成功！");
}
