//! Regression coverage for rollout snippet matching and per-file error handling.

use super::*;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

thread_local! {
    pub(super) static REGEX_COMPILATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[tokio::test]
async fn plain_snippets_compile_once_and_match_decoded_text_across_files() -> io::Result<()> {
    let root = TempDir::new()?;
    REGEX_COMPILATIONS.set(/*value*/ 0);
    let mut matcher = RolloutSnippetMatcher::new("é\"\n[");
    let mut other = RolloutSnippetMatcher::new("unrelated");
    assert_eq!(REGEX_COMPILATIONS.get(), 0);

    for (index, message) in ["unrelated", "first É\"\n[hit]", "next é\"\n[hit]"]
        .into_iter()
        .enumerate()
    {
        let path = root.path().join(format!("{index}.jsonl"));
        let line = serde_json::json!({
            "timestamp": "2025-01-01T00:00:00Z",
            "type": "event_msg",
            "payload": { "type": "agent_message", "message": message }
        });
        std::fs::write(&path, line.to_string())?;
        let expected = (index != 0).then(|| message.replace('\n', " "));
        assert_eq!(matcher.first_match(&path).await?, expected);
        assert_eq!(REGEX_COMPILATIONS.get(), 2);
    }

    assert_eq!(
        other.first_match(&root.path().join("0.jsonl")).await?,
        Some("unrelated".to_string())
    );
    assert_eq!(REGEX_COMPILATIONS.get(), 4);
    Ok(())
}

#[tokio::test]
async fn compressed_search_preserves_error_handling() -> io::Result<()> {
    let root = TempDir::new()?;
    let line = br#"{"timestamp":"2025-01-01T00:00:00Z","type":"event_msg","payload":{"type":"agent_message","message":"needle"}}"#;
    let compressed = zstd::stream::encode_all(&line[..], 3)?;
    let mut expected = RolloutSearchMatches::new();
    for index in 0..7 {
        let path = root.path().join(format!("rollout-{index:04}.jsonl"));
        std::fs::write(path.with_extension("jsonl.zst"), &compressed)?;
        expected.insert(path, Some("needle".to_string()));
    }
    std::fs::write(root.path().join("rollout-broken.jsonl.zst"), b"not zstd")?;
    assert_eq!(
        scan_compressed_rollout_matches(root.path(), "needle").await?,
        expected
    );
    let query = "a".repeat(500_000);

    assert!(case_insensitive_literal_regex(&query).is_err());
    assert_eq!(
        scan_compressed_rollout_matches(root.path(), &query).await?,
        RolloutSearchMatches::new()
    );
    assert_eq!(
        RolloutSnippetMatcher::new(&query)
            .first_match(&root.path().join("missing.jsonl"))
            .await
            .unwrap_err()
            .kind(),
        io::ErrorKind::NotFound
    );
    Ok(())
}
