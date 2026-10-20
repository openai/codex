//! Regression coverage for rollout search error handling.

use super::*;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

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
        first_rollout_content_match_snippet(&root.path().join("missing.jsonl"), &query)
            .await
            .unwrap_err()
            .kind(),
        io::ErrorKind::NotFound
    );
    Ok(())
}
